use std::{collections::HashMap, fs, path::{Path, PathBuf}, time::Duration};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use uuid::Uuid;
use lazy_static::lazy_static;
use directories::ProjectDirs;
use tokio::sync::Mutex as TokioMutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub id: Uuid,
    pub name: String,
    pub total_time: Duration,
}

type StateStore = HashMap<Uuid, State>;

fn load_store_for_static_init() -> StateStore {
    match load_json(&store_path()) {
        Ok(store) => store,
        Err(_) => {
            let empty: StateStore = HashMap::new();
            let _ = save_json(&store_path(), &empty);
            empty
        }
    }
}

lazy_static! {
    static ref STATE_STORE: TokioMutex<StateStore> = TokioMutex::new(load_store_for_static_init());
}

fn data_dir() -> PathBuf {
    ProjectDirs::from("com", "salieri", "salieri")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn store_path() -> PathBuf { data_dir().join("states_store.json") }

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

async fn persist_state_store() -> Result<(), String> {
    let store_guard = STATE_STORE.lock().await;
    let data = store_guard.clone();
    drop(store_guard);
    tauri::async_runtime::spawn_blocking(move || save_json(&store_path(), &data))
        .await
        .map_err(|e| format!("Failed to join save state: {}", e))?
        .map_err(|e| format!("Failed to save state store: {}", e))
}

#[tauri::command]
pub async fn create_state(name: String) -> Result<State, String> {
    let mut guard = STATE_STORE.lock().await;
    let state = State { id: Uuid::new_v4(), name, total_time: Duration::from_secs(0) };
    guard.insert(state.id, state.clone());
    drop(guard);
    persist_state_store().await?;
    Ok(state)
}

#[tauri::command]
pub async fn edit_state(id: String, name: String) -> Result<String, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let mut guard = STATE_STORE.lock().await;
    let Some(st) = guard.get_mut(&uuid) else { return Err("state not found".into()); };
    st.name = name;
    drop(guard);
    persist_state_store().await?;
    Ok("edited".into())
}

#[tauri::command]
pub async fn delete_state(id: String) -> Result<String, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let mut guard = STATE_STORE.lock().await;
    guard.remove(&uuid);
    drop(guard);
    persist_state_store().await?;
    Ok("deleted".into())
}

#[tauri::command]
pub async fn list_states() -> Result<Vec<State>, String> {
    let guard = STATE_STORE.lock().await;
    Ok(guard.values().cloned().collect())
}

pub async fn increment_total_time(id: &str, dur: Duration) -> Result<(), String> {
    let uuid = Uuid::parse_str(id).map_err(|e| e.to_string())?;
    let mut guard = STATE_STORE.lock().await;
    if let Some(st) = guard.get_mut(&uuid) {
        st.total_time += dur;
    }
    Ok(())
}

pub async fn persist_states() -> Result<(), String> { persist_state_store().await }
