use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const NOTIFY_COMMAND: &[&str] = &["char", "codex", "notify"];

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
    if let Some(codex_home) = std::env::var_os("CODEX_HOME").filter(|path| !path.is_empty()) {
        return PathBuf::from(codex_home).join("config.toml");
    }

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

pub fn notify_command() -> Vec<String> {
    NOTIFY_COMMAND.iter().map(|part| part.to_string()).collect()
}

pub fn has_notify(table: &toml::Table, command: &[String]) -> bool {
    let Some(values) = table.get("notify").and_then(|value| value.as_array()) else {
        return false;
    };

    values.len() == command.len()
        && values
            .iter()
            .zip(command)
            .all(|(value, expected)| value.as_str() == Some(expected.as_str()))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{config_path, has_notify, notify_command, set_notify};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn config_path_uses_codex_home_when_present() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_dir = tempfile::tempdir().expect("tempdir");

        // SAFETY: tests serialize environment mutation with ENV_LOCK.
        unsafe {
            std::env::set_var("CODEX_HOME", temp_dir.path());
        }

        assert_eq!(config_path(), temp_dir.path().join("config.toml"));

        // SAFETY: tests serialize environment mutation with ENV_LOCK.
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }

    #[test]
    fn detects_matching_notify_command() {
        let mut table = toml::Table::new();
        let command = notify_command();

        set_notify(&mut table, command.clone());

        assert!(has_notify(&table, &command));
    }

    #[test]
    fn ignores_different_notify_command() {
        let mut table = toml::Table::new();
        let command = notify_command();

        set_notify(
            &mut table,
            vec![
                "other".to_string(),
                "notify".to_string(),
                "handler".to_string(),
            ],
        );

        assert!(!has_notify(&table, &command));
    }
}
