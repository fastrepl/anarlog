use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FILENAME: &str = "auth.json";
#[cfg(any(target_os = "linux", test))]
pub const CLI_FALLBACK_FILENAME: &str = "auth.cli.json";

pub fn resolve_auth_path_from_paths(
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
            %error,
            "failed to migrate auth state"
        );
    }
    if new_auth_path.is_file() {
        new_auth_path.to_path_buf()
    } else if legacy_auth_path.is_file() {
        legacy_auth_path.to_path_buf()
    } else {
        new_auth_path.to_path_buf()
    }
}

pub fn migrate_auth_state(
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

pub fn migrate_from_store_json(store_json_path: &Path, auth_path: &Path) -> std::io::Result<()> {
    if !store_json_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(invalid_data)?;
    let Some(auth_str) = store
        .remove("auth")
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
    else {
        return Ok(());
    };
    let _: HashMap<String, String> = serde_json::from_str(&auth_str).map_err(invalid_data)?;
    anlg_storage::fs::atomic_write(auth_path, &auth_str)?;
    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )?;
    Ok(())
}

pub fn remove_auth_from_store_json(store_json_path: &Path) -> std::io::Result<()> {
    if !store_json_path.is_file() {
        return Ok(());
    }
    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&content)
    {
        Ok(store) => store,
        Err(_) => return anlg_storage::fs::atomic_write(store_json_path, "{}"),
    };
    if store.remove("auth").is_none() {
        return Ok(());
    }
    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )
}

pub fn discard_plaintext_auth(path: &Path) -> std::io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)?;
    file.sync_all()?;
    std::fs::remove_file(path)
}

#[cfg(any(target_os = "linux", test))]
pub fn cli_fallback_auth_path(auth_path: &Path) -> PathBuf {
    auth_path.with_file_name(CLI_FALLBACK_FILENAME)
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_auth_file_to_new_path() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("legacy").join(FILENAME);
        let store = temp.path().join("legacy").join("store.json");
        let new = temp.path().join("new").join(FILENAME);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, r#"{"auth":"value"}"#).unwrap();

        assert_eq!(resolve_auth_path_from_paths(&legacy, &store, &new), new);
        assert!(!legacy.exists());
        assert_eq!(std::fs::read_to_string(new).unwrap(), r#"{"auth":"value"}"#);
    }

    #[test]
    fn migrates_auth_entry_from_legacy_store() {
        let temp = tempfile::tempdir().unwrap();
        let store = temp.path().join("store.json");
        let auth = temp.path().join("auth.json");
        std::fs::write(
            &store,
            r#"{"auth":"{\"session\":\"value\"}","other":"keep"}"#,
        )
        .unwrap();

        migrate_from_store_json(&store, &auth).unwrap();

        assert_eq!(
            std::fs::read_to_string(&auth).unwrap(),
            r#"{"session":"value"}"#
        );
        let migrated: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(store).unwrap()).unwrap();
        assert_eq!(migrated["other"], "keep");
        assert!(migrated.get("auth").is_none());
    }

    #[test]
    fn existing_new_auth_path_is_kept_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let legacy_base = temp.path().join("legacy");
        let local_base = temp.path().join("local").join("com.example.app");
        let legacy_auth = legacy_base.join(FILENAME);
        let legacy_store = legacy_base.join("store.json");
        let new_auth = local_base.join(FILENAME);
        std::fs::create_dir_all(&local_base).unwrap();
        std::fs::write(&new_auth, r#"{"session":"live"}"#).unwrap();

        assert_eq!(
            resolve_auth_path_from_paths(&legacy_auth, &legacy_store, &new_auth),
            new_auth
        );
        assert_eq!(
            std::fs::read_to_string(&new_auth).unwrap(),
            r#"{"session":"live"}"#
        );
        assert!(!legacy_auth.exists());
    }
}
