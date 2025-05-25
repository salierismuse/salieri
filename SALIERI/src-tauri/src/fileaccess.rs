use dirs_next::home_dir;
use std::path::PathBuf;
use tauri::AppHandle;

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
    let full_path = expand_tilde(&user_path)?;

    if !full_path.exists() {
        return Err(format!("file not found: {}", full_path.display()).into());
    }
    
    let contents = std::fs::read_to_string(&full_path).map_err(|e| e.to_string())?;
    Ok(contents)
}

#[tauri::command]
pub async fn command_code(path: &[&str], _app: AppHandle) -> Result<String, String> {
    let real_path = path[1..].join(" ");
    process_file(real_path)
}