// TODO
// expand /todo functionality to allow for days to be specified 
// example: "/todo your mom 5-24-2025";
// checking tasks will need to also check dates
//
// consider storing all tasks in a hash map
// using the id
// for an immediate look up
// instead of this nonsense for loop every single time

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
    let tempTasks: Vec<Task> = store
    .get("tasks")
    .and_then(|v| serde_json::from_value(v.clone()).ok())
    .unwrap_or_default();
    let mut tasks: Vec<Task> = Vec::new();
    for i in 0..tempTasks.len() {
        if tempTasks[i].created_at.to_string().starts_with(&day) {
            tasks.push(tempTasks[i].clone())
        }
    }
    Ok(tasks)
}

// -- structs

// tasks
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Task {
    id: String,
    title: String,
    status: String,
    created_at: NaiveDate,
}


// ───────────────────── palette parser ─────────────────────


// note, convert these if statements into unique functions that
// handle_palette_command will call 
// we could probably just store these all in a seperate file. 
#[tauri::command]
fn handle_palette_command(command: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    if command == "ping" {
        return Ok("pong!".into());
    }

    if command == "date" {
        let now = Local::now();
        return Ok(now.format("%Y-%m-%d %H:%M:%S").to_string());
    }

    if command.starts_with("/theme") {
        let parts: Vec<&str> = command.split_whitespace().collect();

        return match parts.get(1) {
            Some(&"dark") => tauri::async_runtime::block_on(set_theme(app_handle.clone(), "dark".into()))
                .map(|_| "theme set to dark".into()),

            Some(&"light") => tauri::async_runtime::block_on(set_theme(app_handle.clone(), "light".into()))
                .map(|_| "theme set to light".into()),

            Some(&"toggle") => {
                match tauri::async_runtime::block_on(get_current_theme(app_handle.clone())) {
                    Ok(current) => {
                        let new_theme = if current == "dark" { "light" } else { "dark" };
                        tauri::async_runtime::block_on(set_theme(app_handle.clone(), new_theme.into()))
                            .map(|_| format!("theme toggled to {new_theme}"))
                    }
                    Err(e) => Err(format!("couldn't read theme: {e}")),
                }
            }

            Some(arg) => Err(format!("unknown /theme argument '{arg}'. use dark, light, or toggle.")),
            None       => Err("usage: /theme [dark|light|toggle]".into()),
        };
    }

    if command.starts_with("/todo ") {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Need tasks".to_string());
        }
        let title = parts[1..].join(" "); 
        let new_task = Task {
            id: uuid::Uuid::new_v4().to_string(), // generates unique id for each Task
            title, // same as title: title
            created_at: Local::now().date_naive(),
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

    return Ok(format!("added task: {}", new_task.title));
    }   

    if command.starts_with("/doing ") {
        let mut bool_done = false;
        let mut old_found = false;
        let mut old_index = 0;
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Need task".to_string());
        }
        let active_task = parts[1..].join(" ");
        let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
        let mut tasks: Vec<Task> = store
            .get("tasks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

            let old_active = {
                let guard = ACTIVE_TASK_ID.lock().unwrap();
                guard.clone()
            };

        
        for i in 0..tasks.len() {
                if tasks[i].id == old_active
                {
                    if tasks[i].title != active_task {
                        old_found = true;
                        old_index = i; 
                    }
                    else
                    {
                        return Err("already active task".to_string());
                    }
                }
                if tasks[i].title == active_task.to_string()
                {                        
                    tasks[i].status = "doing".to_string();
                    let mut guard = ACTIVE_TASK_ID.lock().unwrap();                            
                    *guard = tasks[i].id.clone();
                    bool_done = true;
                }
            }
                if bool_done == true {
                    if old_found == true {
                        tasks[old_index].status="todo".to_string();
                    }
                store.set("tasks", serde_json::to_value(&tasks).unwrap());
                store.save().map_err(|e| e.to_string())?;
                return Ok(format!("task active"));
            }
        }
    
    if command.starts_with("/done ") {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Need task".to_string());
        }
        let active_task = parts[1..].join(" ");
        let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
        let mut tasks: Vec<Task> = store
            .get("tasks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
    
        for i in 0..tasks.len() {
            if tasks[i].title == active_task {
                tasks[i].status = "done".into();
                store.set("tasks", serde_json::to_value(&tasks).unwrap());
                store.save().map_err(|e| e.to_string())?;
                return Ok(format!("task finished"));
            }
        }
    }

    // disable "doing" task without starting new task
    if command.starts_with("/break ") {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("Need task".to_string());
        }
        let active_task = parts[1..].join(" ");
        let store = app_handle.store("tasks.json").map_err(|e| e.to_string())?;
        let mut tasks: Vec<Task> = store
            .get("tasks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for i in 0..tasks.len() {
            if tasks[i].title == active_task && tasks[i].status == "doing" {
                tasks[i].status = "todo".into();
                store.set("tasks", serde_json::to_value(&tasks).unwrap());
                store.save().map_err(|e| e.to_string())?;
                return Ok(format!("task paused"));
            }
        }
        return Err("Task not active".to_string())
    }
    return Err(format!("unknown command: {command}"));

    }

// ───────────────────── tauri bootstrap ─────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let store = app.store(SETTINGS_STORE_FILENAME)?;
            



            let theme_value = match store.get(THEME_KEY) {
                Some(v) => v.as_str().map(|s| s.to_string()).unwrap_or_else(|| {
                    println!("Invalid theme value in store, resetting to default");
                    store.set(THEME_KEY, json!(DEFAULT_THEME));
                    let _ = store.save();
                    DEFAULT_THEME.to_string()
                }),
                None => {
                    println!("No theme found in store, initializing with default");
                    store.set(THEME_KEY, json!(DEFAULT_THEME));
                    let _ = store.save();
                    DEFAULT_THEME.to_string()
                }
            };

            println!("Initial theme value: {}", theme_value);
            
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