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
use lazy_static::lazy_static;
use std::sync::{Mutex, Arc};
use std::time::Duration;
use tokio::time::{interval, Interval};

// salieri's muse

const THEME_KEY: &str           = "current_theme";
const DEFAULT_THEME: &str       = "dark";
const SETTINGS_STORE_FILENAME: &str = "settings.json";
const TODO_FILE: &str             = "tasks.json";
const DONE_FILE: &str             = "donetasks.json";

// keep track of "doing" task
// should only be doing one task at once for flow
static ACTIVE_TASK_ID: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

#[derive(Clone, serde::Serialize)]
struct ThemeChangedPayload {
    theme: String,
}

// salieri's muse

#[derive(Clone, serde::Serialize)]
struct TimerUpdatePayload {
    state: String,
    remaining_time: u64, // in seconds
}

// timer stuff
#[derive(Debug, Clone, Copy, PartialEq)]
enum TimerState {
    Idle,
    Running,
    Paused,
    ShortBreak, 
    LongBreak,
}

struct PomodoroTimer {
    state: Arc<Mutex<TimerState>>,
    remaining_seconds: Arc<Mutex<u64>>,
    work_duration: Duration,
    short_break_duration: Duration,
    long_break_duration: Duration,
    sessions_before_long_break: u32,
    current_session: Arc<Mutex<u32>>,
    interval: Mutex<Option<Interval>>,
    app_handle: AppHandle,
}

impl PomodoroTimer {
    fn new(app_handle: AppHandle) -> Self {
        let initial_work_duration_secs = 25 * 60;
        PomodoroTimer {
            state: Arc::new(Mutex::new(TimerState::Idle)),
            remaining_seconds: Arc::new(Mutex::new(initial_work_duration_secs)),
            work_duration: Duration::from_secs(initial_work_duration_secs),
            short_break_duration: Duration::from_secs(60 * 5),
            long_break_duration: Duration::from_secs(15 * 5),
            sessions_before_long_break: 4,
            current_session: Arc::new(Mutex::new(1)),
            interval: Mutex::new(None),
            app_handle,
        }
    }

        // start will handle both beginning and resuming,
        // /start and /resume will both lead here
        async fn start(&self) {
            let mut state = self.state.lock().unwrap();
            if matches!(*state, TimerState::Running) {
                return;
            }
            let mut resume_from = None;
            if matches!(*state, TimerState::Paused){
                resume_from = Some(*self.remaining_seconds.lock().unwrap());
            }
            let initial_seconds = match resume_from {
                Some(secs) => secs,
                None => self.work_duration.as_secs(),
            };
            *state = TimerState::Running;
            let duration = initial_seconds;
            *self.remaining_seconds.lock().unwrap() = initial_seconds;

        // is this needed?
        let app_handle = self.app_handle.clone();
        let remaining_seconds = Arc::clone(&self.remaining_seconds);
        let state_clone = Arc::clone(&self.state);
        let next_session = Arc::clone(&self.current_session);
        let long_break_interval = self.sessions_before_long_break;
        let short_break_duration = self.short_break_duration;
        let long_break_duration = self.long_break_duration;
        let work_duration = self.work_duration;

        let mut interval = interval(Duration::from_secs(1));
        *self.interval.lock().unwrap() = Some(interval);

        tokio::spawn(async move {
        loop {
             { let mut remaining = remaining_seconds.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
            } else {
                let current_state = *state_clone.lock().unwrap();
                let mut session = next_session.lock().unwrap();
                match current_state {
                    TimerState::Running => {
                        if *session % long_break_interval == 0 {
                            *state_clone.lock().unwrap() = TimerState::LongBreak;
                            *remaining = long_break_duration.as_secs();
                        } else {
                            *state_clone.lock().unwrap() = TimerState::ShortBreak;
                            *remaining = short_break_duration.as_secs();
                        }
                        *session += 1;
                    }
                    TimerState::ShortBreak | TimerState::LongBreak => {
                        *state_clone.lock().unwrap() = TimerState::Running;
                        *remaining = work_duration.as_secs();
                    }
                    _ => break, // shouldnt happen
                }
                
            }
            let current_state = *state_clone.lock().unwrap();
            let _ = app_handle.emit(
                "timer_updated",
                TimerUpdatePayload {
                    state: format!("{:?}", current_state).to_lowercase(),
                    remaining_time: *remaining,
                },
            );
        }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });
}

    fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if matches!(*state, TimerState::Running) {
            *state = TimerState::Paused;
            *self.interval.lock().unwrap() = None; 
        }
    }

    fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        *state = TimerState::Idle; 

        let mut remaining_seconds = self.remaining_seconds.lock().unwrap();
        *remaining_seconds = 0; 

        let mut current_session = self.current_session.lock().unwrap();
        *current_session = 1; 

        *self.interval.lock().unwrap() = None; 

        let _ = self.app_handle.emit(
            "timer_updated",
            TimerUpdatePayload { state: "idle".into(), remaining_time: 0 },
    );
    }



}

lazy_static! {
    static ref POMODORO: Mutex<Option<PomodoroTimer>> = Mutex::new(None);
}

fn init_pomodoro(app_handle: AppHandle) {
    let mut timer = POMODORO.lock().unwrap();
    *timer = Some(PomodoroTimer::new(app_handle));
}

#[tauri::command]
async fn start_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.start().await;
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

#[tauri::command]
fn pause_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.pause();
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

#[tauri::command]
fn stop_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.stop();
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

fn command_start_pomodoro() -> Result<String, String> {
    async_runtime::block_on(start_timer()).map(|_| "pomodoro started".into())
}

fn command_pause_pomodoro() -> Result<String, String> {
    pause_timer().map(|_| "pomodoro paused".into())
}

fn command_stop_pomodoro() -> Result<String, String> {
    stop_timer().map(|_| "pomodoro stopped".into())
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

fn fetch_tasks(app: &AppHandle, day: &str, done: bool) -> Result<Vec<Task>, String> {
    let file = if done { DONE_FILE } else { TODO_FILE };
    let store = app.store(file).map_err(|e| e.to_string())?;
    let list: Vec<Task> = store.get("tasks")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(list.into_iter().filter(|t| t.created_at.starts_with(day)).collect())
}

// day will be used when flicking through tasks for various days
#[tauri::command]
fn get_tasks(app: AppHandle, day: String, done: bool) -> Result<Vec<Task>, String> {
    fetch_tasks(&app, &day, done)
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

fn command_completed() -> Result<String, String> {
    return Ok("success".into());
    }

// ───────────────────── palette parser ─────────────────────

#[tauri::command]
fn handle_palette_command(command: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    let trimmed_command = command.trim();
    let parts: Vec<&str> = trimmed_command.split_whitespace().collect();

    match parts.get(0) {
        Some(&"ping") => command_ping(),
        Some(&"date") => command_date(),
        Some(&"/theme") => command_theme(&parts, app_handle),
        Some(&"/todo") => command_todo(&parts, app_handle),
        Some(&"/doing") => command_doing(&parts, app_handle),
        Some(&"/done") => command_done(&parts, app_handle),
        Some(&"/break") => command_break(&parts, app_handle),
        Some(&"/completed") => command_completed(), 
        Some(&"/start") => command_start_pomodoro(),
        Some(&"/pause") => command_pause_pomodoro(),
        Some(&"/stop") => command_stop_pomodoro(),
        Some(unknown_cmd) => Err(format!("unknown command: {}", unknown_cmd)),
        None => Err("empty command received".into()), // Handle empty command string
    }
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
