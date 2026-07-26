use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::PLUGIN_NAME;

const FILENAME: &str = "auth.json";

pub(crate) fn auth_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> std::result::Result<PathBuf, String> {
    let new_auth_path = new_auth_path(app).map_err(|e| e.to_string())?;
    let legacy_auth_path = legacy_auth_path(app).map_err(|e| e.to_string())?;
    let legacy_store_json_path = legacy_store_json_path(app).map_err(|e| e.to_string())?;

    Ok(resolve_auth_path_from_paths(
        &legacy_auth_path,
        &legacy_store_json_path,
        &new_auth_path,
    ))
}

fn new_auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::io::Result<PathBuf> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(invalid_data)?
        .join(FILENAME))
}

fn legacy_auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::io::Result<PathBuf> {
    Ok(legacy_base_path(app)?.join(FILENAME))
}

fn legacy_store_json_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> std::io::Result<PathBuf> {
    Ok(legacy_base_path(app)?.join("store.json"))
}

fn legacy_base_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::io::Result<PathBuf> {
    use tauri_plugin_settings::SettingsPluginExt;

    let base = app.settings().global_base().map_err(invalid_data)?;
    Ok(Path::new(base.as_str()).to_path_buf())
}

fn migrate_auth_state(
    legacy_auth_path: &Path,
    legacy_store_json_path: &Path,
    new_auth_path: &Path,
) -> std::io::Result<()> {
    if let Some(parent) = new_auth_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if legacy_auth_path.is_file() {
        std::fs::rename(legacy_auth_path, new_auth_path)?;
        return Ok(());
    }

    if new_auth_path.is_file() {
        return Ok(());
    }

    migrate_from_store_json(legacy_store_json_path, new_auth_path)
}

fn resolve_auth_path_from_paths(
    legacy_auth_path: &Path,
    legacy_store_json_path: &Path,
    new_auth_path: &Path,
) -> PathBuf {
    if let Err(error) = migrate_auth_state(legacy_auth_path, legacy_store_json_path, new_auth_path)
    {
        tracing::warn!(
            legacy_auth_path = %legacy_auth_path.display(),
            legacy_store_json_path = %legacy_store_json_path.display(),
            new_auth_path = %new_auth_path.display(),
            error = %error,
            "failed to migrate auth state"
        );
    }

    if new_auth_path.is_file() {
        return new_auth_path.to_path_buf();
    }

    if legacy_auth_path.is_file() {
        return legacy_auth_path.to_path_buf();
    }

    new_auth_path.to_path_buf()
}

fn migrate_from_store_json(store_json_path: &Path, auth_path: &Path) -> std::io::Result<()> {
    if !store_json_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(invalid_data)?;

    let auth_str = match store
        .remove(PLUGIN_NAME)
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
    {
        Some(s) => s,
        None => return Ok(()),
    };

    let _: HashMap<String, String> = serde_json::from_str(&auth_str).map_err(invalid_data)?;

    hypr_storage::fs::atomic_write(auth_path, &auth_str)?;
    hypr_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )?;

    Ok(())
}

fn invalid_data(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migration_moves_legacy_auth_file() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert!(!legacy_auth_path.exists());
        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
    }

    #[test]
    fn migration_overwrites_new_auth_path_when_legacy_exists() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(new_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();
        std::fs::write(&new_auth_path, "{}").unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert!(!legacy_auth_path.exists());
        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
    }

    #[test]
    fn migration_is_noop_when_new_auth_path_exists_without_legacy_auth() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(new_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(&new_auth_path, auth_json("new-token")).unwrap();
        std::fs::write(&legacy_store_json_path, "{ invalid json").unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("new-token")
        );
    }

    #[test]
    fn migration_moves_auth_out_of_legacy_store_json() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy_store_json_path,
            legacy_store_json(
                &auth_json("legacy-token"),
                Some(("other", serde_json::json!("value"))),
            ),
        )
        .unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
        let migrated_store: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&legacy_store_json_path).unwrap())
                .unwrap();
        assert!(migrated_store.get(PLUGIN_NAME).is_none());
        assert_eq!(
            migrated_store.get("other").unwrap(),
            &serde_json::json!("value")
        );
    }

    #[test]
    fn migration_creates_new_auth_parent_directory() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp
            .path()
            .join("nested")
            .join("com.hyprnote.stable")
            .join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert!(new_auth_path.exists());
    }

    #[test]
    fn migration_does_not_clone_shared_auth_into_second_bundle() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let stable_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);
        let nightly_auth_path = temp.path().join("com.hyprnote.nightly").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        migrate_auth_state(
            &legacy_auth_path,
            &legacy_store_json_path,
            &stable_auth_path,
        )
        .unwrap();
        migrate_auth_state(
            &legacy_auth_path,
            &legacy_store_json_path,
            &nightly_auth_path,
        )
        .unwrap();

        assert!(stable_auth_path.exists());
        assert!(!nightly_auth_path.exists());
    }

    #[test]
    fn resolve_auth_path_ignores_invalid_legacy_store_json() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_store_json_path, "{ invalid json").unwrap();

        let resolved = resolve_auth_path_from_paths(
            &legacy_auth_path,
            &legacy_store_json_path,
            &new_auth_path,
        );

        assert_eq!(resolved, new_auth_path);
        assert!(!legacy_auth_path.exists());
        assert!(!new_auth_path.exists());
    }

    #[test]
    fn resolve_auth_path_falls_back_to_legacy_auth_when_rename_fails() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&new_auth_path).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        let resolved = resolve_auth_path_from_paths(
            &legacy_auth_path,
            &legacy_store_json_path,
            &new_auth_path,
        );

        assert_eq!(resolved, legacy_auth_path);
        assert_eq!(
            std::fs::read_to_string(&legacy_auth_path).unwrap(),
            auth_json("legacy-token")
        );
        assert!(new_auth_path.is_dir());
    }

    fn auth_json(token: &str) -> String {
        serde_json::to_string(&serde_json::json!({
            "sb-project-auth-token": token,
        }))
        .unwrap()
    }

    fn legacy_store_json(auth_json: &str, extra: Option<(&str, serde_json::Value)>) -> String {
        let mut store = serde_json::Map::new();
        store.insert(
            PLUGIN_NAME.to_string(),
            serde_json::Value::String(auth_json.to_string()),
        );
        if let Some((key, value)) = extra {
            store.insert(key.to_string(), value);
        }
        serde_json::to_string(&store).unwrap()
    }
}
