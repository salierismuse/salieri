use chrono::{Local, Duration as ChronoDuration, NaiveDate};
use tauri::AppHandle;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Mutex, time::Duration};
use directories::ProjectDirs;
use tokio::sync::RwLock as TokioRwLock;
use futures::executor;         
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::sync::Mutex as TokioMutex;
use lazy_static::lazy_static;
use indexmap::IndexMap;

use crate::states::{increment_total_time, persist_states};

use crate::user::increment_tasks_done;

fn load_store_for_static_init() -> Store { // Renamed for clarity of purpose
    match load_json(&store_path()) { // load_json reads from disk
        Ok(store) => store,
        Err(_) => {
            println!("tasks_store.json not found or failed to load, creating a new one.");
            let empty_store = Store::new();
            // Attempt to save this initial empty store.
            // We can ignore the result here as it's a best-effort for first run.
            let _ = save_json(&store_path(), &empty_store);
            empty_store
        }
    }
}

lazy_static! {
    static ref TASK_STORE: TokioMutex<Store> = TokioMutex::new(load_store_for_static_init());
    static ref ACTIVE_TASK_ID: TokioRwLock<Option<TaskId>> = TokioRwLock::new(None);
    static ref ACTIVE_TASK:    TokioMutex<Option<Task>>    = TokioMutex::new(None);
}

// helper for command_todo
fn try_parse_date(input: &str) -> Option<String> {
    let formats = [
        "%m/%d/%Y",    // 06/01/2025
        "%m/%d/%y",    // 06/01/25
        "%Y-%m-%d",    // 2025-06-01 
        "%m-%d-%Y",    // 06-01-2025
        "%m-%d-%y",    // 06-01-25
    ];
    
    for format in &formats {
        if let Ok(date) = NaiveDate::parse_from_str(input, format) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    
    None
}


#[tauri::command]
pub fn get_current_logical_day_key(days_offset: Option<i64>) -> String {
    let offset = days_offset.unwrap_or(0);
    today_key(offset)
}


async fn persist_global_store() -> Result<(), String> {

    let store_guard = TASK_STORE.lock().await;
    let store_data_to_save = store_guard.clone(); 
    drop(store_guard); 

    tauri::async_runtime::spawn_blocking(move || save_json(&store_path(), &store_data_to_save))
        .await
        .map_err(|e| format!("Failed to join save task: {}", e))? 
        .map_err(|e| format!("Failed to save store: {}", e)) 
}


// ─── type aliases ────────────────────────────────────────────────────────
type TaskId      = String;
type LogicalDay  = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub time_spent: u64,
    #[serde(default)]
    pub state_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct DayBucket {
    // perhaps make the hash store a vector of a tuple
    // containing taskid and task
    // perhaps consistent "number" store as well
    todo: IndexMap<TaskId, Task>,
    done: IndexMap<TaskId, Task>,
}

type Store = IndexMap<LogicalDay, DayBucket>;


fn today_key(days_offset: i64) -> LogicalDay {
    (Local::now() - ChronoDuration::hours(4) - ChronoDuration::days(days_offset))
        .format("%Y-%m-%d")
        .to_string()
}

fn data_dir() -> PathBuf {
    ProjectDirs::from("com", "salieri", "salieri")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn store_path() -> PathBuf { data_dir().join("tasks_store.json") }

fn ensure_data_dir() {
    let dir = data_dir();
    if !dir.exists() { let _ = fs::create_dir_all(&dir); }
}

fn load_json<T: DeserializeOwned>(p: &Path) -> Result<T, String> {
    if !p.exists() { return Err("missing".into()); }
    serde_json::from_str(&fs::read_to_string(p).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn save_json<T: Serialize>(p: &Path, d: &T) -> Result<(), String> {
    ensure_data_dir();
    fs::write(p, serde_json::to_string_pretty(d).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn load_store() -> Result<Store, String> {
    load_json(&store_path()).or_else(|_| Ok(Store::new()))
}

fn save_store(st: &Store) -> Result<(), String> {
    save_json(&store_path(), st)
}

fn bucket_mut<'a>(st: &'a mut Store, day: &LogicalDay) -> &'a mut DayBucket {
    st.entry(day.clone()).or_insert_with(DayBucket::default)
}
// ─── startup fix 
pub async fn clear_active_startup(_h: AppHandle) -> Result<(), String> { 
    let mut store_guard = TASK_STORE.lock().await;
    if let Some(bucket) = store_guard.get_mut(&today_key(0)) {
        for t in bucket.todo.values_mut() {
            if t.status == "doing" { t.status = "todo".into(); }
        }
    }
    drop(store_guard);
    persist_global_store().await?;
    Ok(())
}

async fn clear_active_task() {
    let mut active_task_guard = ACTIVE_TASK.lock().await; 
    *active_task_guard = None;

    let mut active_id_guard = ACTIVE_TASK_ID.write().await; 
    *active_id_guard = None;
}

async fn set_active_task(task: Task) {
    let id = task.id.clone();

    let mut active_task_guard = ACTIVE_TASK.lock().await; 
    *active_task_guard = Some(task);


    let mut active_id_guard = ACTIVE_TASK_ID.write().await; 
    *active_id_guard = Some(id);
}

// ─── timer loop 
pub fn start_task_timer_loop(_h: AppHandle) { 
    tokio::spawn(async move { 
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        let mut tick_count = 0u64; 
        loop {
            ticker.tick().await;
            tick_count = tick_count.wrapping_add(1);

            let active_id_opt = ACTIVE_TASK_ID.read().await.clone();
            let Some(id) = active_id_opt else { continue }; 

            let mut store_guard = TASK_STORE.lock().await;
            let today = today_key(0);
            let mut changed_in_loop = false;
            let mut state_for_tick: Option<String> = None;
            if let Some(bucket) = store_guard.get_mut(&today) {
                if let Some(task) = bucket.todo.get_mut(&id) {
                    if task.status == "doing" {
                        task.time_spent += 1;
                        changed_in_loop = true;
                        state_for_tick = task.state_id.clone();
                    }
                }
            }
            drop(store_guard);

            if let Some(sid) = state_for_tick {
                let _ = increment_total_time(&sid, Duration::from_secs(1)).await;
            }

            if changed_in_loop && tick_count % 60 == 0 {
                if let Err(e) = persist_global_store().await {
                    eprintln!("Timer loop failed to save store: {}", e);
                }
                if let Err(e) = persist_states().await {
                    eprintln!("Timer loop failed to save states: {}", e);
                }
            }
        }
    });
}


// ─── query API 
#[tauri::command]
pub async fn get_tasks(_h: AppHandle, day: String, done: bool) -> Result<Vec<Task>, String> { 
    let store_guard = TASK_STORE.lock().await; 
    let bucket = store_guard.get(&day).cloned().unwrap_or_default();
    Ok(if done { bucket.done } else { bucket.todo }
        .into_values()
        .collect())
}

// ─── macro 
macro_rules! ensure_title { ($p:expr) => { if $p.len() < 2 { return Err("need task title".into()); } }; }

// ─── /todo 
pub async fn command_todo(parts: &[&str], _app: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let mut title = parts[1..].join(" ");
    let mut day = today_key(0);
    if parts.len() > 1 {
        if let Some(parsed_date) = try_parse_date(parts[1]) {
            day = parsed_date;
            title = parts[2..].join(" ");
        }
    }
    let mut store_guard = TASK_STORE.lock().await; 
    let bucket = bucket_mut(&mut *store_guard, &day); 

    if bucket.todo.values().any(|t| t.title == title) || bucket.done.values().any(|t| t.title == title) {
        return Err("duplicate title".into());
    }

    let task = Task { id: Uuid::new_v4().to_string(), title: title.clone(), status: "todo".into(), created_at: day.clone(), time_spent: 0, state_id: None };
    bucket.todo.insert(task.id.clone(), task);

    let store_data_to_save = store_guard.clone(); 
    drop(store_guard); 

    tauri::async_runtime::spawn_blocking(move || save_json(&store_path(), &store_data_to_save))
        .await
        .map_err(|e| format!("Failed to join save task: {}", e))? 
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok("added".into())
}

#[tauri::command]
pub async fn create_task(title: String, state_id: Option<String>) -> Result<Task, String> {
    if title.trim().is_empty() { return Err("need task title".into()); }
    let day = today_key(0);
    let mut store_guard = TASK_STORE.lock().await;
    let bucket = bucket_mut(&mut *store_guard, &day);
    if bucket.todo.values().any(|t| t.title == title) || bucket.done.values().any(|t| t.title == title) {
        return Err("duplicate title".into());
    }
    let task = Task {
        id: Uuid::new_v4().to_string(),
        title: title.clone(),
        status: "todo".into(),
        created_at: day.clone(),
        time_spent: 0,
        state_id,
    };
    bucket.todo.insert(task.id.clone(), task.clone());
    let store_data_to_save = store_guard.clone();
    drop(store_guard);
    tauri::async_runtime::spawn_blocking(move || save_json(&store_path(), &store_data_to_save))
        .await
        .map_err(|e| format!("Failed to join save task: {}", e))?
        .map_err(|e| format!("Failed to save store: {}", e))?;
    Ok(task)
}

// ─── /doing
pub async fn command_doing(parts: &[&str], _app: AppHandle, days_offset: Option<i64>) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store_guard = TASK_STORE.lock().await;
    let offset = days_offset.unwrap_or(0);
    let day = today_key(offset);
    let bucket = bucket_mut(&mut *store_guard, &day);

    let old_id_opt = ACTIVE_TASK_ID.read().await.clone();

    let task_to_activate_details = bucket.todo.iter_mut()
        .find(|(_, t)| t.title == title)
        .map(|(id, task_ref)| {
            if task_ref.status == "doing" {
                return Err("already active".to_string());
            }
            task_ref.status = "doing".into();
            Ok((id.clone(), task_ref.clone()))
        });

    let (task_id_to_activate, task_object_to_set_active) = match task_to_activate_details {
        Some(Ok(details)) => details,
        Some(Err(e)) => {
            drop(store_guard);
            return Err(e);
        }
        None => {
            drop(store_guard);
            return Err("task not found".into());
        }
    };

    clear_active_task().await;
    set_active_task(task_object_to_set_active).await;

    if let Some(old_id_val) = old_id_opt {
        if old_id_val != task_id_to_activate {
            if let Some(t) = bucket.todo.get_mut(&old_id_val) {
                t.status = "todo".into();
            }
        }
    }

    drop(store_guard);
    persist_global_store().await?;

    Ok("task active".into())
}


// ─── /done 
pub async fn command_done(parts: &[&str], h: AppHandle, days_offset: Option<i64>) -> Result<String, String> { 
    ensure_title!(parts);
    let title = parts[1..].join(" ");
    let offset = days_offset.unwrap_or(0);
    let mut store_guard = TASK_STORE.lock().await; 
    let day = today_key(offset);
    let bucket = bucket_mut(&mut *store_guard, &day); 

    let task_id_opt = bucket.todo.iter()
        .find(|(_, t)| t.title == title)
        .map(|(id, _)| id.clone());

    let task_id = match task_id_opt {
        Some(id) => id,
        None => return Err("task not found".into()),
    };
    if let Some(mut task) = bucket.todo.shift_remove(&task_id) {
        task.status = "done".into();
        bucket.done.insert(task_id.clone(), task);

        let current_active_id_opt = ACTIVE_TASK_ID.read().await.clone();
        if current_active_id_opt.as_deref() == Some(&task_id) {
            clear_active_task().await; 
        }

        increment_tasks_done(h); 

        drop(store_guard); 
        persist_global_store().await?; 

        Ok("task moved to done".into())
    } else {
        Err("task found by ID but could not be removed".into())
    }
}

// ─── /break 
// add offset logic
pub async fn command_break(parts: &[&str], _app: AppHandle) -> Result<String, String> { 
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store_guard = TASK_STORE.lock().await;
    let day = today_key(0);
    let bucket = bucket_mut(&mut *store_guard, &day); 

    if let Some(task) = bucket.todo.values_mut().find(|t| t.title == title) {
        if task.status == "doing" {
            task.status = "todo".into(); 

            clear_active_task().await;
            drop(store_guard); 
            persist_global_store().await?;

            Ok("task paused".into())
        } else {
            drop(store_guard); 
            Err(format!("Task '{}' found, but it's not currently 'doing'.", title))
        }
    } else {
        drop(store_guard); 
        Err(format!("Task '{}' not found in the to-do list.", title))
    }
}

// ─── /deleteT 
// add offset stuff
pub async fn command_deleteT(parts: &[&str], _app: AppHandle) -> Result<String, String> { 
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store_guard = TASK_STORE.lock().await; 
    let day = today_key(0);
    let bucket = bucket_mut(&mut *store_guard, &day);

    let task_id_opt = bucket.todo.iter()
        .find(|(_, t)| t.title == title)
        .map(|(id, _)| id.clone());

    let task_id = match task_id_opt {
        Some(id) => id,
        None => {
            drop(store_guard);
            return Err(format!("Task '{}' not found for deletion.", title));
        }
    };
    bucket.todo.remove(&task_id);
    let current_active_id_opt = ACTIVE_TASK_ID.read().await.clone();
    if current_active_id_opt.as_deref() == Some(&task_id) {
        clear_active_task().await;
    }

    drop(store_guard); 
    persist_global_store().await?; 

    Ok(format!("Task '{}' deleted.", title))
}

// ─── /completed (placeholder)
pub fn command_completed() -> Result<String, String> {
    Ok("success".into())
}
