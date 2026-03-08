use std::path::Path;

use super::path::VAULT_PATH_KEY;
use crate::fs::copy_dir_recursive;

const VAULT_DIRECTORIES: &[&str] = &["sessions", "humans", "organizations", "chats", "prompts"];

const VAULT_FILES: &[&str] = &[
    "settings.json",
    "events.json",
    "calendars.json",
    "templates.json",
    "chat_shortcuts.json",
];

pub async fn copy_vault_items(src: &Path, dst: &Path) -> std::io::Result<()> {
    for dir_name in VAULT_DIRECTORIES {
        let src_dir = src.join(dir_name);
        let dst_dir = dst.join(dir_name);

        if src_dir.exists() && src_dir.is_dir() {
            tokio::fs::create_dir_all(&dst_dir).await?;
            copy_dir_recursive(&src_dir, &dst_dir, None).await?;
        }
    }

    for file_name in VAULT_FILES {
        let src_file = src.join(file_name);
        let dst_file = dst.join(file_name);

        if src_file.exists() && src_file.is_file() {
            tokio::fs::copy(&src_file, &dst_file).await?;
        }
    }

    Ok(())
}

pub fn set_vault_path(config: &mut serde_json::Value, path: &Path) {
    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            VAULT_PATH_KEY.to_string(),
            serde_json::Value::String(path.to_string_lossy().to_string()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn copy_vault_items_copies_only_vault() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::create_dir_all(src.join("sessions")).unwrap();
        fs::write(src.join("sessions").join("test.json"), "session").unwrap();
        fs::create_dir_all(src.join("humans")).unwrap();
        fs::write(src.join("humans").join("person.md"), "human").unwrap();
        fs::write(src.join("events.json"), "events").unwrap();
        fs::write(src.join("settings.json"), "settings").unwrap();

        fs::write(src.join("store.json"), "store").unwrap();
        fs::create_dir_all(src.join("models")).unwrap();
        fs::write(src.join("models").join("model.gguf"), "model").unwrap();

        copy_vault_items(&src, &dst).await.unwrap();

        assert!(dst.join("sessions").join("test.json").exists());
        assert!(dst.join("humans").join("person.md").exists());
        assert!(dst.join("events.json").exists());
        assert!(dst.join("settings.json").exists());

        assert!(!dst.join("store.json").exists());
        assert!(!dst.join("models").exists());
    }

    #[tokio::test]
    async fn copy_vault_items_handles_missing_items() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::write(src.join("events.json"), "events").unwrap();

        copy_vault_items(&src, &dst).await.unwrap();

        assert!(dst.join("events.json").exists());
        assert!(!dst.join("sessions").exists());
    }

    #[test]
    fn set_vault_path_sets_path() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("vault");

        let mut config = serde_json::json!({});
        set_vault_path(&mut config, &path);

        assert_eq!(
            config.get(VAULT_PATH_KEY).and_then(|v| v.as_str()),
            Some(path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn set_vault_path_preserves_existing_fields() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("vault");

        let mut config = serde_json::json!({"theme": "dark", "language": "en"});
        set_vault_path(&mut config, &path);

        assert_eq!(config.get("theme").and_then(|v| v.as_str()), Some("dark"));
        assert_eq!(config.get("language").and_then(|v| v.as_str()), Some("en"));
        assert_eq!(
            config.get(VAULT_PATH_KEY).and_then(|v| v.as_str()),
            Some(path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn set_vault_path_overwrites_existing() {
        let temp = tempdir().unwrap();
        let old_path = temp.path().join("old");
        let new_path = temp.path().join("new");

        let mut config = serde_json::json!({ VAULT_PATH_KEY: old_path.to_string_lossy() });
        set_vault_path(&mut config, &new_path);

        assert_eq!(
            config.get(VAULT_PATH_KEY).and_then(|v| v.as_str()),
            Some(new_path.to_string_lossy().as_ref())
        );
    }
}
