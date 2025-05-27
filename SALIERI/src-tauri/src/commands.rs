use chrono::Local;
use tauri::AppHandle;

use crate::theme::{set_theme, get_current_theme};
use crate::tasks::{command_todo, command_doing, command_done, command_break, command_completed, command_deleteT};
use crate::pomodoro::{command_start_pomodoro, command_pause_pomodoro, command_stop_pomodoro, command_resume_pomodoro};
use crate::fileaccess::{command_code};

// file management



pub fn command_ping() -> Result<String, String> {
    Ok("pong!".into())
}

pub fn command_date() -> Result<String, String> {
    let now = Local::now();
    Ok(now.format("%Y-%m-%d %H:%M:%S").to_string())
}

pub async fn command_theme(parts: &[&str], app_handle: AppHandle) -> Result<String, String> {
    match parts.get(1) {
        Some(&"dark") => set_theme(app_handle, "dark".into()).await
            .map(|_| "theme set to dark".into()),

        Some(&"light") => set_theme(app_handle, "light".into()).await
            .map(|_| "theme set to light".into()),

        Some(&"toggle") => {
            match get_current_theme(app_handle.clone()).await {
                Ok(current) => {
                    let new_theme = if current == "dark" { "light" } else { "dark" };
                    set_theme(app_handle, new_theme.into()).await
                        .map(|_| format!("theme toggled to {new_theme}"))
                }
                Err(e) => Err(format!("couldn't read theme: {e}")),
            }
        }

        Some(arg) => Err(format!("unknown /theme argument '{arg}'. use dark, light, or toggle.")),
        None       => Err("usage: /theme [dark|light|toggle]".into()),
    }
}

fn command_wq() -> Result<String, String>
{
    Ok("file saved!".into())
}

#[tauri::command]
pub async fn handle_palette_command(command: String, app_handle: AppHandle) -> Result<String, String> {
    let trimmed_command = command.trim();
    let parts: Vec<&str> = trimmed_command.split_whitespace().collect();

    match parts.get(0) {
        Some(&"ping") => command_ping(),
        Some(&"date") => command_date(),
        Some(&"/theme") => command_theme(&parts, app_handle).await,
        Some(&"/todo") => command_todo(&parts, app_handle).await,
        Some(&"/doing") => command_doing(&parts, app_handle).await,
        Some(&"/done") => command_done(&parts, app_handle).await,
        Some(&"/break") => command_break(&parts, app_handle).await,
        Some(&"/deleteT") => command_deleteT(&parts, app_handle).await,
        Some(&"/completed") => command_completed(), 
        Some(&"/start") => command_start_pomodoro().await,
        Some(&"/pause") => command_pause_pomodoro().await,
        Some(&"/resume") => command_resume_pomodoro().await,
        Some(&"/stop") => command_stop_pomodoro().await,
        Some(&"/code") => command_code(&parts, app_handle).await,
        Some(&"/write") => command_code(&parts, app_handle).await,
        Some(&"/wq") => command_wq(),
        Some(unknown_cmd) => Err(format!("unknown command: {}", unknown_cmd)),
        None => Err("empty command received".into()), 
    }
}