use std::path::PathBuf;
use tokio::sync::RwLock;

pub struct State {
    vault_base: PathBuf,
    lock: RwLock<()>,
}

impl State {
    pub fn new(vault_base: PathBuf) -> Self {
        Self {
            vault_base,
            lock: RwLock::new(()),
        }
    }

    fn path(&self) -> PathBuf {
        hypr_storage::vault::compute_settings_path(&self.vault_base)
    }

    pub fn vault_base(&self) -> &PathBuf {
        &self.vault_base
    }

    async fn read_or_default(&self) -> crate::Result<serde_json::Value> {
        match tokio::fs::read_to_string(self.path()).await {
            Ok(content) => Ok(serde_json::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn load(&self) -> crate::Result<serde_json::Value> {
        let _guard = self.lock.read().await;
        self.read_or_default().await
    }

    pub async fn save(&self, settings: serde_json::Value) -> crate::Result<()> {
        let _guard = self.lock.write().await;

        let existing = self.read_or_default().await?;
        let merged = merge_settings(existing, settings);
        let content = serde_json::to_string_pretty(&merged)?;

        hypr_storage::fs::atomic_write_async(&self.path(), &content).await?;
        Ok(())
    }

    pub fn reset(&self) -> crate::Result<()> {
        hypr_storage::fs::atomic_write(&self.path(), "{}")?;
        Ok(())
    }
}

fn merge_settings(existing: serde_json::Value, incoming: serde_json::Value) -> serde_json::Value {
    match (existing, incoming) {
        (serde_json::Value::Object(mut existing_map), serde_json::Value::Object(incoming_map)) => {
            for (key, value) in incoming_map {
                existing_map.insert(key, value);
            }
            serde_json::Value::Object(existing_map)
        }
        (_, incoming) => incoming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_both_objects() {
        let existing = json!({"a": 1, "b": 2});
        let incoming = json!({"b": 3, "c": 4});
        let result = merge_settings(existing, incoming);
        assert_eq!(result, json!({"a": 1, "b": 3, "c": 4}));
    }

    #[test]
    fn merge_empty_existing() {
        let existing = json!({});
        let incoming = json!({"a": 1});
        let result = merge_settings(existing, incoming);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn merge_empty_incoming() {
        let existing = json!({"a": 1});
        let incoming = json!({});
        let result = merge_settings(existing, incoming);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn merge_incoming_replaces_non_object_existing() {
        let existing = json!(null);
        let incoming = json!({"a": 1});
        let result = merge_settings(existing, incoming);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn merge_non_object_incoming_replaces_existing() {
        let existing = json!({"a": 1});
        let incoming = json!([1, 2, 3]);
        let result = merge_settings(existing, incoming);
        assert_eq!(result, json!([1, 2, 3]));
    }
}
