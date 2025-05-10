// Prevents the extra console window on Windows in release – DO NOT REMOVE!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use serde_json::json;
use tauri_plugin_store::{Builder as StorePlugin, StoreExt};
use tauri::{async_runtime, AppHandle, Emitter};  

const THEME_KEY: &str           = "current_theme";
const DEFAULT_THEME: &str       = "dark";
const SETTINGS_STORE_FILENAME: &str = "settings.json";

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

// ───────────────────── palette parser ─────────────────────

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
                    Err(e) => Err(format!("couldn’t read theme: {e}")),
                }
            }

            Some(arg) => Err(format!("unknown /theme argument '{arg}'. use dark, light, or toggle.")),
            None       => Err("usage: /theme [dark|light|toggle]".into()),
        };
    }

    Err(format!("unknown command: {command}"))
}

// ───────────────────── tauri bootstrap ─────────────────────

fn main() {
    tauri::Builder::default()
      .plugin(StorePlugin::default().build())
      .setup(|app| {
        // open (or create) the store
        let store = app.store(SETTINGS_STORE_FILENAME)?;
        // read or insert the default
        let theme_value = store
          .get(THEME_KEY)
          .and_then(|v| v.as_str().map(ToString::to_string))
          .unwrap_or_else(|| {
            store.set(THEME_KEY, json!(DEFAULT_THEME));
            let _ = store.save();
            DEFAULT_THEME.to_string()
          });
        // emit that first payload so the front-end sees it on startup
        app.emit_to("main", "theme_changed", ThemeChangedPayload { theme: theme_value })?;
        Ok(())
      })
      .invoke_handler(tauri::generate_handler![
        set_theme,
        get_current_theme,
        handle_palette_command
      ])
      .run(tauri::generate_context!())
      .expect("error while running tauri application");
  }