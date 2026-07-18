use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

pub struct StartupSnapshot {
    startup_vault_base: PathBuf,
    io_lock: RwLock<()>,
}

impl StartupSnapshot {
    pub fn new(startup_vault_base: PathBuf) -> Self {
        Self {
            startup_vault_base,
            io_lock: RwLock::new(()),
        }
    }

    fn settings_path(&self) -> PathBuf {
        hypr_storage::vault::compute_settings_path(&self.startup_vault_base)
    }

    pub fn startup_vault_base(&self) -> &PathBuf {
        &self.startup_vault_base
    }

    async fn read_at(path: &Path) -> crate::Result<Option<serde_json::Value>> {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_or_default_at(path: &Path) -> crate::Result<serde_json::Value> {
        Ok(Self::read_at(path)
            .await?
            .unwrap_or_else(|| serde_json::json!({})))
    }

    async fn read_or_default(&self) -> crate::Result<serde_json::Value> {
        Self::read_or_default_at(&self.settings_path()).await
    }

    pub async fn load(&self) -> crate::Result<serde_json::Value> {
        let _guard = self.io_lock.read().await;
        self.read_or_default().await
    }

    pub async fn load_with_legacy_fallback(
        &self,
        legacy_base: &Path,
    ) -> crate::Result<serde_json::Value> {
        let _guard = self.io_lock.read().await;
        if let Some(settings) = Self::read_at(&self.settings_path()).await? {
            return Ok(settings);
        }

        let legacy_path = hypr_storage::vault::compute_settings_path(legacy_base);
        if legacy_path == self.settings_path() {
            return Ok(serde_json::json!({}));
        }

        Ok(match Self::read_or_default_at(&legacy_path).await {
            Ok(legacy) if is_non_empty_object(&legacy) => legacy,
            _ => serde_json::json!({}),
        })
    }

    pub async fn save(&self, settings: serde_json::Value) -> crate::Result<()> {
        let _guard = self.io_lock.write().await;

        let existing = self.read_or_default().await?;
        let merged = merge_settings(existing, settings);
        let content = serde_json::to_string_pretty(&merged)?;

        hypr_storage::fs::atomic_write_async(&self.settings_path(), &content).await?;
        Ok(())
    }

    pub fn reset(&self) -> crate::Result<()> {
        hypr_storage::fs::atomic_write(&self.settings_path(), "{}")?;
        Ok(())
    }
}

fn is_non_empty_object(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|object| !object.is_empty())
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_uses_global_legacy_settings_when_custom_vault_is_missing() {
        let temp = tempdir().unwrap();
        let vault_base = temp.path().join("vault");
        let global_base = temp.path().join("global");
        std::fs::create_dir_all(&vault_base).unwrap();
        std::fs::create_dir_all(&global_base).unwrap();
        std::fs::write(
            hypr_storage::vault::compute_settings_path(&global_base),
            r#"{"ai":{"current_llm_provider":"hyprnote"}}"#,
        )
        .unwrap();

        let snapshot = StartupSnapshot::new(vault_base);

        assert_eq!(
            snapshot
                .load_with_legacy_fallback(&global_base)
                .await
                .unwrap(),
            json!({"ai": {"current_llm_provider": "hyprnote"}}),
        );
    }

    #[tokio::test]
    async fn load_preserves_an_explicit_custom_vault_reset() {
        let temp = tempdir().unwrap();
        let vault_base = temp.path().join("vault");
        let global_base = temp.path().join("global");
        std::fs::create_dir_all(&vault_base).unwrap();
        std::fs::create_dir_all(&global_base).unwrap();
        std::fs::write(
            hypr_storage::vault::compute_settings_path(&vault_base),
            "{}",
        )
        .unwrap();
        std::fs::write(
            hypr_storage::vault::compute_settings_path(&global_base),
            r#"{"general":{"theme":"light"}}"#,
        )
        .unwrap();

        let snapshot = StartupSnapshot::new(vault_base);

        assert_eq!(
            snapshot
                .load_with_legacy_fallback(&global_base)
                .await
                .unwrap(),
            json!({}),
        );
    }

    #[tokio::test]
    async fn load_prefers_non_empty_custom_vault_settings() {
        let temp = tempdir().unwrap();
        let vault_base = temp.path().join("vault");
        let global_base = temp.path().join("global");
        std::fs::create_dir_all(&vault_base).unwrap();
        std::fs::create_dir_all(&global_base).unwrap();
        std::fs::write(
            hypr_storage::vault::compute_settings_path(&vault_base),
            r#"{"general":{"theme":"dark"}}"#,
        )
        .unwrap();
        std::fs::write(
            hypr_storage::vault::compute_settings_path(&global_base),
            r#"{"general":{"theme":"light"}}"#,
        )
        .unwrap();

        let snapshot = StartupSnapshot::new(vault_base);

        assert_eq!(
            snapshot
                .load_with_legacy_fallback(&global_base)
                .await
                .unwrap(),
            json!({"general": {"theme": "dark"}}),
        );
    }

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
