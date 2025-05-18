use serde_json::json;
use tauri_plugin_store::StoreExt;
use tauri::Emitter;

pub const THEME_KEY: &str = "current_theme";
pub const DEFAULT_THEME: &str = "dark";
pub const SETTINGS_STORE_FILENAME: &str = "settings.json";

#[derive(Clone, serde::Serialize)]
pub struct ThemeChangedPayload {
    pub theme: String,
}

#[tauri::command]
pub async fn set_theme(app_handle: tauri::AppHandle, theme_name: String) -> Result<(), String> {
    let store = app_handle.store(SETTINGS_STORE_FILENAME).map_err(|e| e.to_string())?;

    store.set(THEME_KEY, json!(theme_name.clone()));
    store.save().map_err(|e| e.to_string())?;

    app_handle
        .emit_to("main", "theme_changed", ThemeChangedPayload { theme: theme_name })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_current_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    let store = app_handle.store(SETTINGS_STORE_FILENAME).map_err(|e| e.to_string())?;

    let theme = store
        .get(THEME_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| DEFAULT_THEME.to_owned());

    Ok(theme)
}