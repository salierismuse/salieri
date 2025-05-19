use chrono::{Local, Duration as ChronoDuration};
use tauri::AppHandle;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Mutex, time::Duration};
use directories::ProjectDirs;
use tokio::sync::RwLock;
use futures::executor;          // Cargo.toml: futures = "0.3"
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use crate::user::increment_tasks_done;

// ─── type aliases ────────────────────────────────────────────────────────
type TaskId      = String;
type LogicalDay  = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,     // "todo" | "doing" | "done"
    pub created_at: String, // logical-day key
    pub time_spent: u64,    // seconds
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct DayBucket {
    todo: HashMap<TaskId, Task>,
    done: HashMap<TaskId, Task>,
}

type Store = HashMap<LogicalDay, DayBucket>;

// ─── day helpers ─────────────────────────────────────────────────────────
fn today_key() -> LogicalDay {
    (Local::now() - ChronoDuration::hours(4)).format("%Y-%m-%d").to_string()
}

// ─── disk helpers ────────────────────────────────────────────────────────
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

// ─── globals ────────────────────────────────────────────────────────────
static ACTIVE_TASK_ID: Lazy<RwLock<Option<TaskId>>> = Lazy::new(|| RwLock::new(None));
static ACTIVE_TASK:    Lazy<Mutex<Option<Task>>>    = Lazy::new(|| Mutex::new(None));

// ─── startup fix ────────────────────────────────────────────────────────
pub fn clear_active_startup(_h: AppHandle) -> Result<(), String> {
    let mut store = load_store()?;
    if let Some(bucket) = store.get_mut(&today_key()) {
        for t in bucket.todo.values_mut() {
            if t.status == "doing" { t.status = "todo".into(); }
        }
    }
    save_store(&store)
}

// ─── active helpers ─────────────────────────────────────────────────────
fn clear_active_task() {
    *ACTIVE_TASK.lock().unwrap() = None;
    executor::block_on(async { *ACTIVE_TASK_ID.write().await = None; });
}

fn set_active_task(task: Task) {
    let id = task.id.clone();
    *ACTIVE_TASK.lock().unwrap() = Some(task);
    executor::block_on(async { *ACTIVE_TASK_ID.write().await = Some(id); });
}

// ─── timer loop ─────────────────────────────────────────────────────────
pub fn start_task_timer_loop(_h: AppHandle) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        let mut tick_count = 0u8;
        loop {
            ticker.tick().await;
            tick_count = tick_count.wrapping_add(1);
            let Some(id) = ACTIVE_TASK_ID.read().await.clone() else { continue };
            let mut store = load_store().unwrap_or_default();
            if let Some(bucket) = store.get_mut(&today_key()) {
                if let Some(task) = bucket.todo.get_mut(&id) {
                    task.time_spent += 1;
                    if tick_count % 60 == 0 { let _ = save_store(&store); }
                }
            }
        }
    });
}

// ─── query API ──────────────────────────────────────────────────────────
#[tauri::command]
pub fn get_tasks(_h: AppHandle, day: String, done: bool) -> Result<Vec<Task>, String> {
    let store = load_store()?;
    let bucket = store.get(&day).cloned().unwrap_or_default();
    Ok(if done { bucket.done } else { bucket.todo }
        .into_values()
        .collect())
}

// ─── macro ──────────────────────────────────────────────────────────────
macro_rules! ensure_title { ($p:expr) => { if $p.len() < 2 { return Err("need task title".into()); } }; }

// ─── /todo ──────────────────────────────────────────────────────────────
pub fn command_todo(parts: &[&str], _app: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store = load_store()?;
    let day = today_key();
    let bucket = bucket_mut(&mut store, &day);

    if bucket.todo.values().any(|t| t.title == title) || bucket.done.values().any(|t| t.title == title) {
        return Err("duplicate title".into());
    }

    let task = Task { id: Uuid::new_v4().to_string(), title: title.clone(), status: "todo".into(), created_at: day.clone(), time_spent: 0 };
    bucket.todo.insert(task.id.clone(), task);
    save_store(&store)?;
    Ok("added".into())
}

// ─── /doing ─────────────────────────────────────────────────────────────
pub fn command_doing(parts: &[&str], _app: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store = load_store()?;
    let day = today_key();
    let bucket = bucket_mut(&mut store, &day);

    let old_id = ACTIVE_TASK_ID.blocking_read().clone();

    let Some((id, task)) = bucket.todo.iter_mut().find(|(_, t)| t.title == title).map(|(i, t)| (i.clone(), t)) else {
        return Err("task not found".into());
    };

    if task.status == "doing" { return Err("already active".into()); }

    clear_active_task();
    task.status = "doing".into();
    set_active_task(task.clone());

    if let Some(old) = old_id {
        if let Some(t) = bucket.todo.get_mut(&old) { t.status = "todo".into(); }
    }

    save_store(&store)?;
    Ok("task active".into())
}

// ─── /done ──────────────────────────────────────────────────────────────
pub fn command_done(parts: &[&str], h: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store = load_store()?;
    let day = today_key();
    let bucket = bucket_mut(&mut store, &day);

    // find task by title in today's todo list
    let Some(task_id) = bucket.todo.iter().find(|(_, t)| t.title == title).map(|(id, _)| id.clone()) else {
        return Err("task not found".into());
    };

    let mut task = bucket.todo.remove(&task_id).unwrap();
    task.status = "done".into();
    bucket.done.insert(task_id.clone(), task);

    // if that task was active, clear globals
    if ACTIVE_TASK_ID.blocking_read().as_deref() == Some(&task_id) {
        clear_active_task();
    }

    increment_tasks_done(h);
    save_store(&store)?;
    Ok("task moved to done".into())
}

// ─── /break ─────────────────────────────────────────────────────────────
pub fn command_break(parts: &[&str], _app: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store = load_store()?;
    let day = today_key();
    let bucket = bucket_mut(&mut store, &day);

    for task in bucket.todo.values_mut() {
        if task.title == title && task.status == "doing" {
            task.status = "todo".into();
            clear_active_task();
            save_store(&store)?;
            return Ok("task paused".into());
        }
    }
    Err("task not active".into())
}

// ─── /deleteT ───────────────────────────────────────────────────────────
pub fn command_deleteT(parts: &[&str], _app: AppHandle) -> Result<String, String> {
    ensure_title!(parts);
    let title = parts[1..].join(" ");

    let mut store = load_store()?;
    let day = today_key();
    let bucket = bucket_mut(&mut store, &day);

    let Some(task_id) = bucket.todo.iter().find(|(_, t)| t.title == title).map(|(id, _)| id.clone()) else {
        return Err("task not found".into());
    };

    bucket.todo.remove(&task_id);
    if ACTIVE_TASK_ID.blocking_read().as_deref() == Some(&task_id) {
        clear_active_task();
    }
    save_store(&store)?;
    Ok("task deleted".into())
}

// ─── /completed (placeholder) ───────────────────────────────────────────
pub fn command_completed() -> Result<String, String> {
    Ok("success".into())
}