// TODO
// expand /todo functionality to allow for days to be specified 
// example: "/todo your mom 5-24-2025";
// checking tasks will need to also check dates
//
// consider storing all tasks in a hash map
// using the id
// for an immediate look up
// instead of this nonsense for loop every single time
//
// need to move "done" tasks to a seperate tasksdone.json file

/*
TO DO CONT:
clean up handle_palette_command by turning all if statements
into their own functions
store these in seperate file ultimately


brainstorming:
hashmap<day, pair<id, task>> ? 
something like that. maybe not id exactly but..
would be nice to be able to quickly access days, cutting down from O(n) to O(5)/constant [5 tasks a day max assumption]
*/
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Local, NaiveDate};
use serde_json::json;
use tauri_plugin_store::{Builder as StorePlugin, StoreExt};
use tauri::{async_runtime, AppHandle, Emitter};  
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex;

const THEME_KEY: &str           = "current_theme";
const DEFAULT_THEME: &str       = "dark";
const SETTINGS_STORE_FILENAME: &str = "settings.json";

// keep track of "doing" task
// should only be doing one task at once for flow
static ACTIVE_TASK_ID: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

#[derive(Clone, serde::Serialize)]
struct ThemeChangedPayload {
    theme: String,
}

// ───────────────────── theme helpers ─────────────────────

#[tauri::command]
async fn set_theme(app_handle: tauri::AppHandle, theme_name: String) -> Result<(), String> {
    let store = app_handle.store(SETTINGS_STORE_FILENAME).map_err(|e| e.to_string())?;

    store.set(THEME_KEY, json!(theme_name.clone()));
    store.save().map_err(|e| e.to_string())?;

    app_handle
        .emit_to("main", "theme_changed", ThemeChangedPayload { theme: theme_name })
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_current_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    let store = app_handle.store(SETTINGS_STORE_FILENAME).map_err(|e| e.to_string())?;

    let theme = store
        .get(THEME_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| DEFAULT_THEME.to_owned());

    Ok(theme)
}

// day will be used when flicking through tasks for various days
#[tauri::command]
fn get_tasks(app_handle: tauri::AppHandle, day: String) -> Result<Vec<Task>, String> {
    let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
    let temp_tasks: Vec<Task> = store
    .get("tasks")
    .and_then(|v| serde_json::from_value(v.clone()).ok())
    .unwrap_or_default();

    let tasks: Vec<Task> = temp_tasks
        .into_iter()
        .filter(|t| t.created_at.starts_with(&day))
        .collect();

    Ok(tasks)
}

// -- structs

// tasks
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Task {
    id: String,
    title: String,
    status: String,
    created_at: String,
}

// ───────────────────── palette helpers ─────────────────────

fn command_ping() -> Result<String, String> {
    Ok("pong!".into())
}

fn command_date() -> Result<String, String> {
    let now = Local::now();
    Ok(now.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn command_theme(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    match parts.get(1) {
        Some(&"dark") => async_runtime::block_on(set_theme(app_handle, "dark".into()))
            .map(|_| "theme set to dark".into()),

        Some(&"light") => async_runtime::block_on(set_theme(app_handle, "light".into()))
            .map(|_| "theme set to light".into()),

        Some(&"toggle") => {
            match async_runtime::block_on(get_current_theme(app_handle.clone())) {
                Ok(current) => {
                    let new_theme = if current == "dark" { "light" } else { "dark" };
                    async_runtime::block_on(set_theme(app_handle, new_theme.into()))
                        .map(|_| format!("theme toggled to {new_theme}"))
                }
                Err(e) => Err(format!("couldn't read theme: {e}")),
            }
        }

        Some(arg) => Err(format!("unknown /theme argument '{arg}'. use dark, light, or toggle.")),
        None       => Err("usage: /theme [dark|light|toggle]".into()),
    }
}

fn command_todo(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }

    let title = parts[1..].join(" ");
    let new_task = Task {
        id: Uuid::new_v4().to_string(),
        title: title.clone(),
        created_at: Local::now().date_naive().to_string(),
        status: "todo".into(),
    };

    let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
    let mut tasks: Vec<Task> = store
        .get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    tasks.push(new_task.clone());
    store.set("tasks", json!(tasks));
    store.save().map_err(|e| e.to_string())?;

    Ok(format!("added task: {}", new_task.title))
}

fn command_doing(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let active_task = parts[1..].join(" ");

    let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
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
            task.status = "doing".into();
            let mut guard = ACTIVE_TASK_ID.lock().unwrap();
            *guard = task.id.clone();
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

fn command_done(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let target_title = parts[1..].join(" ");

    let store_todo  = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
    let store_done  = app_handle.store("donetasks.json").map_err(|e| e.to_string())?;

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

        Ok("task moved to done".into())
    } else {
        Err("task not found".into())
    }
}

fn command_break(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    if parts.len() < 2 {
        return Err("need task title".into());
    }
    let active_task = parts[1..].join(" ");

    let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
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

// ───────────────────── palette parser ─────────────────────

#[tauri::command]
fn handle_palette_command(command: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    let trimmed = command.trim();

    if trimmed == "ping" {
        return command_ping();
    }

    if trimmed == "date" {
        return command_date();
    }

    if trimmed.starts_with("/theme") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return command_theme(&parts, app_handle);
    }

    if trimmed.starts_with("/todo ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return command_todo(&parts, app_handle);
    }

    if trimmed.starts_with("/doing ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return command_doing(&parts, app_handle);
    }

    if trimmed.starts_with("/done ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return command_done(&parts, app_handle);
    }

    if trimmed.starts_with("/break ") {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        return command_break(&parts, app_handle);
    }

    Err(format!("unknown command: {trimmed}"))
}

// ───────────────────── tauri bootstrap ─────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let store = app.store(SETTINGS_STORE_FILENAME)?;

            let theme_value = match store.get(THEME_KEY) {
                Some(v) => v.as_str().map(|s| s.to_string()).unwrap_or_else(|| {
                    println!("invalid theme value in store, resetting to default");
                    store.set(THEME_KEY, json!(DEFAULT_THEME));
                    let _ = store.save();
                    DEFAULT_THEME.to_string()
                }),
                None => {
                    println!("no theme found in store, initializing with default");
                    store.set(THEME_KEY, json!(DEFAULT_THEME));
                    let _ = store.save();
                    DEFAULT_THEME.to_string()
                }
            };

            println!("initial theme value: {}", theme_value);

            store.save()?;

            app.emit("theme_changed", ThemeChangedPayload { theme: theme_value })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_theme,
            get_current_theme,
            handle_palette_command,
            get_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
