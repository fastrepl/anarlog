use std::path::{Path, PathBuf};

pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .expect("could not determine config directory")
        .join("amp")
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{read_settings, write_settings};

    #[test]
    fn read_settings_returns_empty_object_when_missing() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("settings.json");

        assert_eq!(
            read_settings(&path).expect("settings"),
            serde_json::Value::Object(serde_json::Map::new())
        );
    }

    #[test]
    fn write_settings_round_trips() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("nested").join("settings.json");
        let settings = json!({
            "theme": "dark",
            "permissions": ["allow"]
        });

        write_settings(&path, &settings).expect("write");
        assert_eq!(read_settings(&path).expect("read"), settings);
    }
}
