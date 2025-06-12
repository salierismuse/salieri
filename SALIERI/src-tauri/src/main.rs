#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod theme;
mod user;
mod pomodoro;
mod tasks;
mod commands;
mod fileaccess;
mod states;

use crate::theme::{set_theme, get_current_theme, ThemeChangedPayload, THEME_KEY, DEFAULT_THEME, SETTINGS_STORE_FILENAME};
use crate::tasks::{get_tasks, start_task_timer_loop, clear_active_startup, get_current_logical_day_key, create_task};
use crate::states::{create_state, edit_state, delete_state, list_states};
use crate::pomodoro::init_pomodoro;
use crate::commands::handle_palette_command;
use crate::fileaccess::save_file;

use serde_json::json;
use tauri_plugin_store::StoreExt;
use tauri::Emitter;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let bg_handle = app.handle().clone();

        tauri::async_runtime::spawn_blocking(move || {
            start_task_timer_loop(bg_handle);
        });

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

            tauri::async_runtime::block_on(clear_active_startup(app_handle.clone()));

            app.emit("theme_changed", ThemeChangedPayload { theme: theme_value })?;
            tauri::async_runtime::block_on(init_pomodoro(app_handle.clone()));  

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_theme,
            get_current_theme,
            handle_palette_command,
            get_tasks,
            get_current_logical_day_key,
            save_file,
            create_task,
            create_state,
            edit_state,
            delete_state,
            list_states,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}