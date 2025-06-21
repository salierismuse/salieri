use dirs_next::home_dir;
use std::io::prelude::*;
use std::path::PathBuf;
use tauri::AppHandle;
use std::fs::File;

// function to turn "~/...." filepath into proper path
fn expand_tilde(path: &str) -> Result<PathBuf, String>{
    if path == "~" || path.starts_with("~/")
    {
        let home = dirs_next::home_dir().ok_or("no home directory located")?;
        if path == "~"
        {
            return Ok(home);
        }
        else {
            let rest_of_path = &path[2..];
            return Ok(home.join(rest_of_path));
        }
    }
    Err("something weird happened".to_string())
}

fn process_file(user_path: String) -> Result<String, String>
{
    let full_path;
    let full_string;
    if (user_path.chars().next().unwrap() != '~') { 
        if (user_path.len() > 0)
        {
            full_string = ["~/", "salieri_files/", &user_path].join("");
            full_path = expand_tilde(&full_string)?;
        }
        else {
            return Err(format!("no good!"));
        }
    }
    else {
        full_path = expand_tilde(&user_path)?;
    }
    
    if !full_path.exists() {
        let path = std::path::Path::new(&full_path);
        let prefix = path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();
        std::fs::write(&full_path, "").map_err(|e| e.to_string())?;
    }
    
    let contents = std::fs::read_to_string(&full_path).map_err(|e| e.to_string())?;
    Ok(contents)
}

#[tauri::command]
pub async fn command_code(path: &[&str], _app: AppHandle) -> Result<String, String> {
    let real_path = path[1..].join(" ");
    process_file(real_path)
}

#[tauri::command]
pub async fn save_file(user_path: String, information: String) -> Result<String, String> {
    let actual_path;
    let full_string;
    if (user_path.chars().next().unwrap() != '~') { 
        if (user_path.len() > 0)
        {
            full_string = ["~/", "salieri_files/", &user_path].join("");
            actual_path = expand_tilde(&full_string)?;
        }
        else {
            return Err(format!("no good!"));
        }
    }
    else {
        actual_path = expand_tilde(&user_path)?;
    }

    // create parent directories if they don't exist
    if let Some(parent) = actual_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create directories: {}", e))?;
    }
    
    let mut file = File::create(&actual_path)
        .map_err(|e| format!("failed to create/open file '{}': {}", actual_path.display(), e))?;
    
    file.write_all(information.as_bytes())
        .map_err(|e| format!("failed to write to file '{}': {}", actual_path.display(), e))?;
    
    file.flush()
        .map_err(|e| format!("failed to flush file '{}': {}", actual_path.display(), e))?;
    
    Ok(format!("file '{}' saved successfully.", actual_path.display()))
}