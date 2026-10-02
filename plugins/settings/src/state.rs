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
        anlg_storage::vault::compute_settings_path(&self.startup_vault_base)
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
        self.read_or_default().await.map(normalize_legacy_names)
    }

    pub async fn save(&self, settings: serde_json::Value) -> crate::Result<()> {
        let _guard = self.io_lock.write().await;

        let existing = self.read_or_default().await?;
        let merged = normalize_legacy_names(merge_settings(existing, settings));
        let content = serde_json::to_string_pretty(&merged)?;

        anlg_storage::fs::atomic_write_async(&self.settings_path(), &content).await?;
        Ok(())
    }

    pub fn reset(&self) -> crate::Result<()> {
        anlg_storage::fs::atomic_write(&self.settings_path(), "{}")?;
        Ok(())
    }
}

fn normalize_legacy_names(mut settings: serde_json::Value) -> serde_json::Value {
    let Some(ai) = settings
        .get_mut("ai")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return settings;
    };

    for key in ["current_llm_provider", "current_stt_provider"] {
        if ai.get(key).and_then(serde_json::Value::as_str) == Some("hyprnote") {
            ai.insert(
                key.to_string(),
                serde_json::Value::String("anarlog".to_string()),
            );
        }
    }

    settings
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
    async fn load_normalizes_legacy_provider_names() {
        let temp = tempdir().unwrap();
        let vault_base = temp.path().join("vault");
        std::fs::create_dir_all(&vault_base).unwrap();
        std::fs::write(
            anlg_storage::vault::compute_settings_path(&vault_base),
            r#"{"ai":{"current_llm_provider":"hyprnote"}}"#,
        )
        .unwrap();

        let snapshot = StartupSnapshot::new(vault_base);

        assert_eq!(
            snapshot.load().await.unwrap(),
            json!({"ai": {"current_llm_provider": "anarlog"}}),
        );
    }

    #[tokio::test]
    async fn save_migrates_legacy_provider_names() {
        let temp = tempdir().unwrap();
        let vault_base = temp.path().join("vault");
        std::fs::create_dir_all(&vault_base).unwrap();
        let snapshot = StartupSnapshot::new(vault_base);

        snapshot
            .save(json!({
                "ai": {
                    "current_llm_provider": "hyprnote",
                    "current_stt_provider": "hyprnote"
                }
            }))
            .await
            .unwrap();

        assert_eq!(
            snapshot.load().await.unwrap(),
            json!({
                "ai": {
                    "current_llm_provider": "anarlog",
                    "current_stt_provider": "anarlog"
                }
            }),
        );
    }

    #[test]
    fn merge_settings_overlays_objects_and_replaces_non_objects() {
        for (existing, incoming, expected) in [
            (
                json!({"a": 1, "b": 2}),
                json!({"b": 3, "c": 4}),
                json!({"a": 1, "b": 3, "c": 4}),
            ),
            (json!({}), json!({"a": 1}), json!({"a": 1})),
            (json!({"a": 1}), json!({}), json!({"a": 1})),
            (json!(null), json!({"a": 1}), json!({"a": 1})),
            (json!({"a": 1}), json!([1, 2, 3]), json!([1, 2, 3])),
        ] {
            assert_eq!(
                merge_settings(existing.clone(), incoming.clone()),
                expected,
                "unexpected merge for existing={existing} incoming={incoming}"
            );
        }
    }
}
