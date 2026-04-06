use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    #[serde(rename = "type")]
    pub hook_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookEntry>,
}

pub type HooksConfig = HashMap<String, Vec<HookMatcher>>;

pub fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".claude")
        .join("settings.json")
}

pub fn read_settings(path: &Path) -> Result<serde_json::Value, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|e| format!("failed to parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::Value::Object(serde_json::Map::new()))
        }
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

pub fn write_settings(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    std::fs::write(path, contents).map_err(|e| format!("failed to write {}: {e}", path.display()))
}
