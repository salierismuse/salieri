use chrono::Local;
use serde_json::json;
use tauri_plugin_store::StoreExt;
use tauri::AppHandle;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::Duration;
use serde::{Serialize, Deserialize};

use crate::user::increment_tasks_done;

pub const TODO_FILE: &str = "tasks.json";
pub const DONE_FILE: &str = "donetasks.json";

static ACTIVE_TASK_ID: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
pub static ACTIVE_TASK: Lazy<Mutex<Option<Task>>> = Lazy::new(|| Mutex::new(None));


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub time_spent: u64,
}

pub fn clear_active_startup(app_handle: AppHandle) -> Result<(), String> {
    let today = chrono::Local::now().date_naive().to_string();
    let mut tasks = fetch_tasks(&app_handle, &today, false)?; // false -> todo file

    for task in tasks.iter_mut() {
        if task.status == "doing" {
            task.status = "todo".into();
        }
    }

    let store = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    store.set("tasks", serde_json::json!(tasks));
    store.save().map_err(|e| e.to_string())
}

fn clear_active_task() {
    let mut guard = ACTIVE_TASK.lock().unwrap();
    if let Some(task) = guard.as_mut() {
        task.status = "todo".into();
    }
    *guard = None;

    let mut id_guard = ACTIVE_TASK_ID.lock().unwrap();
    id_guard.clear();
}

fn set_active_task(task: Task) {
    {
        let mut id_guard = ACTIVE_TASK_ID.lock().unwrap();
        *id_guard = task.id.clone();
    }

    let mut guard = ACTIVE_TASK.lock().unwrap();
    *guard = Some(task);
}

pub fn start_task_timer_loop(app_handle: AppHandle) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;

            let active_id = {
                let guard = ACTIVE_TASK_ID.lock().unwrap();
                guard.clone()
            };

            if active_id.is_empty() {
                continue;
            }

            let store = match app_handle.store(TODO_FILE) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut tasks: Vec<Task> = store
                .get("tasks")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            let mut changed = false;

            for task in tasks.iter_mut() {
                if task.id == active_id && task.status == "doing" {
                    task.time_spent += 1;
                    changed = true;
                    break;
                }
            }

            if changed {
                let _ = store.set("tasks", json!(tasks));
                let _ = store.save();
            }
        }
    });
}

fn fetch_tasks(app: &AppHandle, day: &str, done: bool) -> Result<Vec<Task>, String> {
    let file = if done { DONE_FILE } else { TODO_FILE };
    let store = app.store(file).map_err(|e| e.to_string())?;
    let list: Vec<Task> = store.get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(list.into_iter().filter(|t| t.created_at.starts_with(day)).collect())
}

#[tauri::command]
pub fn get_tasks(app: AppHandle, day: String, done: bool) -> Result<Vec<Task>, String> {
    fetch_tasks(&app, &day, done)
}

pub fn command_todo(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }

    let title = parts[1..].join(" ");
    let new_task = Task {
        id: Uuid::new_v4().to_string(),
        title: title.clone(),
        created_at: Local::now().date_naive().to_string(),
        status: "todo".into(),
        time_spent: 0,
    };

    let store = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    let mut tasks: Vec<Task> = store
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    tasks.push(new_task.clone());
    store.set("tasks", json!(tasks));
    store.save().map_err(|e| e.to_string())?;

    Ok(format!("added task: {}", new_task.title))
}

pub fn command_doing(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let active_task = parts[1..].join(" ");

    let store = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    let mut tasks: Vec<Task> = store
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let old_active_id = {
        let guard = ACTIVE_TASK_ID.lock().unwrap();
        guard.clone()
    };

    let mut old_index: Option<usize> = None;
    let mut activated = false;

    for (i, task) in tasks.iter_mut().enumerate() {
        if task.id == old_active_id && task.title != active_task {
            old_index = Some(i);
        }
        if task.title == active_task {
            if task.status == "doing" {
                return Err("already active task".into());
            }
            clear_active_task();
            task.status = "doing".into();
            set_active_task(task.clone());
            activated = true;
        }
    }

    if activated {
        if let Some(idx) = old_index {
            tasks[idx].status = "todo".into();
        }
        store.set("tasks", serde_json::to_value(&tasks).unwrap());
        store.save().map_err(|e| e.to_string())?;
        return Ok("task active".into());
    }
    Err("task not found".into())
}

pub fn command_done(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let target_title = parts[1..].join(" ");

    let store_todo  = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    let store_done  = app_handle.store(DONE_FILE).map_err(|e| e.to_string())?;

    let mut todo_tasks: Vec<Task> = store_todo
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let mut done_tasks: Vec<Task> = store_done
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if let Some(pos) = todo_tasks.iter().position(|t| t.title == target_title) {
        let mut finished = todo_tasks.remove(pos);
        finished.status = "done".into();
        done_tasks.push(finished);

        store_todo.set("tasks", json!(todo_tasks));
        store_todo.save().map_err(|e| e.to_string())?;

        store_done.set("tasks", json!(done_tasks));
        store_done.save().map_err(|e| e.to_string())?;

        let mut guard = ACTIVE_TASK_ID.lock().unwrap();
        if *guard == target_title {
            guard.clear();
        }
        increment_tasks_done(app_handle);
        Ok("task moved to done".into())
    } else {
        Err("task not found".into())
    }
}

pub fn command_break(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let active_task = parts[1..].join(" ");

    let store = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    let mut tasks: Vec<Task> = store
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    for task in tasks.iter_mut() {
        if task.title == active_task && task.status == "doing" {
            task.status = "todo".into();
            store.set("tasks", serde_json::to_value(&tasks).unwrap());
            store.save().map_err(|e| e.to_string())?;
            return Ok("task paused".into());
        }
    }
    Err("task not active".into())
}

pub fn command_deleteT(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let deleted_task = parts[1..].join(" ");
    let store = app_handle.store(TODO_FILE).map_err(|e| e.to_string())?;
    let mut tasks: Vec<Task> = store 
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
     tasks.retain(|t| t.title != deleted_task);
    store
        .set("tasks", serde_json::to_value(&tasks).unwrap());
    store.save().map_err(|e| e.to_string())?;
    Err("task not active".into())
}


pub fn command_completed() -> Result<String, String> {
    Ok("success".into())
}
