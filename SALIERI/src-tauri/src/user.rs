use serde::{Serialize, Deserialize};
use tauri_plugin_store::StoreExt;
use tauri::AppHandle;

pub const USER_STORE: &str = "user.json";
pub const USER_KEY: &str = "user";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub tasks_done: u64,
    pub pomodoro_done: u64,
    pub time_in_salieri: u64,
}

impl User {
    pub fn load_user(app: &AppHandle) -> Result<User, String> {
        let store = app.store(USER_STORE).map_err(|e| e.to_string())?;
        let user = store
            .get(USER_KEY)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        Ok(user)
    }

    pub fn save_user(app: &AppHandle, user: &User) -> Result<(), String> {
        let store = app.store(USER_STORE).map_err(|e| e.to_string())?;
        store.set(USER_KEY, serde_json::to_value(user).unwrap());
        store.save().map_err(|e| e.to_string())
    }
}

pub fn increment_tasks_done(app: AppHandle) -> Result<String, String> {
    let mut user = User::load_user(&app)?;
    user.tasks_done += 1;
    User::save_user(&app, &user)?;
    Ok(format!("tasks done: {}", user.tasks_done))
}

pub fn increment_pomodoros_done(app: AppHandle) -> Result<String, String> {
    let mut user = User::load_user(&app)?;
    user.pomodoro_done += 1;
    User::save_user(&app, &user)?;
    Ok(format!("tasks done: {}", user.pomodoro_done))
}