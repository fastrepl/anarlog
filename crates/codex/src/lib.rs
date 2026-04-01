use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "thread-id")]
    pub thread_id: Option<String>,
    #[serde(rename = "turn-id")]
    pub turn_id: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "input-messages")]
    pub input_messages: Option<serde_json::Value>,
    #[serde(rename = "last-assistant-message")]
    pub last_assistant_message: Option<serde_json::Value>,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".codex")
        .join("config.toml")
}

pub fn read_config(path: &Path) -> Result<toml::Table, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => contents
            .parse::<toml::Table>()
            .map_err(|e| format!("failed to parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

pub fn write_config(path: &Path, table: &toml::Table) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let contents =
        toml::to_string_pretty(table).map_err(|e| format!("failed to serialize config: {e}"))?;
    std::fs::write(path, contents).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

pub fn set_notify(table: &mut toml::Table, command: Vec<String>) {
    let arr = command
        .into_iter()
        .map(toml::Value::String)
        .collect::<Vec<_>>();
    table.insert("notify".to_string(), toml::Value::Array(arr));
}

pub fn remove_notify(table: &mut toml::Table) {
    table.remove("notify");
}
