use serde::{Serialize, Deserialize, de::DeserializeOwned};
use uuid::Uuid;
use lazy_static::lazy_static;
use std::time::Duration;
use std::{fs, path::{Path, PathBuf}};
use directories::ProjectDirs;
use tokio::sync::Mutex as TokioMutex;
use indexmap::IndexMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "duration_format")]
    pub total_time: Duration,
}

mod duration_format {
    use super::*;
    use serde::{Serializer, Deserializer};

    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

type Store = IndexMap<String, State>;

fn load_store_for_static_init() -> Store {
    match load_json(&store_path()) {
        Ok(store) => store,
        Err(_) => {
            let empty = Store::new();
            let _ = save_json(&store_path(), &empty);
            empty
        }
    }
}

lazy_static! {
    static ref STATE_STORE: TokioMutex<Store> = TokioMutex::new(load_store_for_static_init());
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

async fn persist_global_store() -> Result<(), String> {
    let guard = STATE_STORE.lock().await;
    let data = guard.clone();
    drop(guard);
    tauri::async_runtime::spawn_blocking(move || save_json(&store_path(), &data))
        .await
        .map_err(|e| format!("Failed to join save state: {}", e))?
        .map_err(|e| format!("Failed to save state: {}", e))
}

#[tauri::command]
pub async fn list_states() -> Result<Vec<State>, String> {
    let guard = STATE_STORE.lock().await;
    Ok(guard.values().cloned().collect())
}

#[tauri::command]
pub async fn create_state(name: String) -> Result<State, String> {
    let mut guard = STATE_STORE.lock().await;
    let state = State { id: Uuid::new_v4(), name, total_time: Duration::from_secs(0) };
    guard.insert(state.id.to_string(), state.clone());
    drop(guard);
    persist_global_store().await?;
    Ok(state)
}

#[tauri::command]
pub async fn edit_state(id: String, name: String) -> Result<(), String> {
    let mut guard = STATE_STORE.lock().await;
    if let Some(st) = guard.get_mut(&id) {
        st.name = name;
        drop(guard);
        persist_global_store().await
    } else {
        Err("state not found".into())
    }
}

#[tauri::command]
pub async fn delete_state(id: String) -> Result<(), String> {
    let mut guard = STATE_STORE.lock().await;
    guard.remove(&id);
    drop(guard);
    persist_global_store().await
}

pub async fn increment_total_time(id: &str, secs: u64) {
    let mut guard = STATE_STORE.lock().await;
    if let Some(st) = guard.get_mut(id) {
        st.total_time += Duration::from_secs(secs);
    }
}

pub async fn persist() -> Result<(), String> {
    persist_global_store().await
}
