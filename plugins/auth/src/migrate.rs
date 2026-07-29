use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::PLUGIN_NAME;

const FILENAME: &str = "auth.json";

pub(crate) fn auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<PathBuf> {
    let new_auth_path = new_auth_path(app)?;
    let legacy_auth_path = legacy_auth_path(app)?;
    let legacy_store_json_path = legacy_store_json_path(app)?;

    Ok(resolve_auth_path_from_paths(
        &legacy_auth_path,
        &legacy_store_json_path,
        &new_auth_path,
    ))
}

#[cfg(all(target_os = "linux", not(test)))]
const AUTH_SCOPE: &str = "auth";
#[cfg(all(target_os = "linux", not(test)))]
const AUTH_KEY: &str = "supabase-storage";

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn load_linux_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<HashMap<String, String>> {
    let auth_path = auth_path(app)?;

    // A locked or unavailable keyring must not abort plugin setup, otherwise the app
    // cannot start and the plaintext session can never be migrated. `Ok(None)` means
    // there is no secret yet; `Err` only means it could not be read this time, which
    // is a distinction the migration below depends on.
    let secure_read = tauri_plugin_store2::read_secret_blocking(app, AUTH_SCOPE, AUTH_KEY);
    let keyring_readable = secure_read.is_ok();
    let secure_data = match secure_read {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(%error, "failed_to_read_auth_from_secret_service");
            None
        }
    };

    if let Some(data) = secure_data {
        match serde_json::from_str::<HashMap<String, String>>(&data) {
            // Only drop the plaintext copy once the secure payload is known to be usable.
            Ok(auth) => {
                discard_plaintext_auth(&auth_path);
                return Ok(auth);
            }
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_secret_service_auth");
            }
        }
    }

    if !auth_path.is_file() {
        return Ok(HashMap::new());
    }

    let auth = match std::fs::read_to_string(&auth_path)
        .map_err(crate::Error::Io)
        .and_then(|data| Ok(serde_json::from_str::<HashMap<String, String>>(&data)?))
    {
        Ok(auth) => auth,
        Err(error) => {
            // Matches the keyring path: a corrupt or truncated auth.json costs the
            // session, but must not stop the app from launching. Dropping it also
            // breaks the loop where a half-removed file fails to parse every launch.
            tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
            discard_plaintext_auth(&auth_path);
            return Ok(HashMap::new());
        }
    };

    // Writing now could clobber a secret that exists but could not be read, so an
    // unreadable keyring keeps the plaintext copy and retries on a later launch.
    if !keyring_readable {
        return Ok(auth);
    }

    // persist_linux_auth drops the plaintext copy itself once the keyring owns it.
    if let Err(error) = persist_linux_auth(app, &auth) {
        tracing::warn!(%error, "failed_to_migrate_auth_to_secret_service");
    }

    Ok(auth)
}

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn persist_linux_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    auth: &HashMap<String, String>,
) -> crate::Result<()> {
    if auth.is_empty() {
        return clear_linux_auth(app);
    }

    let data = serde_json::to_string(auth)?;
    tauri_plugin_store2::write_secret_blocking(app, AUTH_SCOPE, AUTH_KEY, &data)
        .map_err(crate::Error::Storage)?;

    drop_plaintext_auth_for(app);
    Ok(())
}

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn clear_linux_auth<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<()> {
    drop_plaintext_auth_for(app);
    tauri_plugin_store2::delete_secret_blocking(app, AUTH_SCOPE, AUTH_KEY)
        .map_err(crate::Error::Storage)
}

// The keyring is authoritative once it has been written or cleared, so a plaintext
// copy left behind by a failed migration must not outlive it and silently restore
// the session on the next launch.
#[cfg(all(target_os = "linux", not(test)))]
fn drop_plaintext_auth_for<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    match auth_path(app) {
        Ok(path) => discard_plaintext_auth(&path),
        Err(error) => tracing::warn!(%error, "failed_to_resolve_auth_path"),
    }
}

// A leftover auth.json is less harmful than refusing a session the secure store
// already holds, so cleanup failures are reported rather than propagated.
#[cfg(any(all(target_os = "linux", not(test)), target_os = "windows", test))]
pub(crate) fn discard_plaintext_auth(path: &Path) {
    if let Err(error) = remove_plaintext_auth(path) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed_to_remove_plaintext_auth"
        );
    }
}

#[cfg(any(all(target_os = "linux", not(test)), target_os = "windows", test))]
pub(crate) fn remove_plaintext_auth(path: &Path) -> std::io::Result<()> {
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

fn new_auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::io::Result<PathBuf> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(invalid_data)?
        .join(FILENAME))
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_secure_auth_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> std::io::Result<PathBuf> {
    Ok(new_auth_path(app)?.with_file_name("auth.dpapi"))
}

fn legacy_auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::io::Result<PathBuf> {
    Ok(legacy_base_path(app)?.join(FILENAME))
}

fn legacy_store_json_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> std::io::Result<PathBuf> {
    Ok(legacy_base_path(app)?.join("store.json"))
}

#[cfg(target_os = "windows")]
pub(crate) fn discard_windows_plaintext_auth<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    for path in [new_auth_path(app), legacy_auth_path(app)] {
        match path {
            Ok(path) => discard_plaintext_auth(&path),
            Err(error) => tracing::warn!(%error, "failed_to_resolve_plaintext_auth_path"),
        }
    }

    match legacy_store_json_path(app) {
        Ok(path) => {
            if let Err(error) = remove_auth_from_store_json(&path) {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "failed_to_remove_auth_from_legacy_store"
                );
            }
        }
        Err(error) => tracing::warn!(%error, "failed_to_resolve_legacy_store_path"),
    }
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

    anlg_storage::fs::atomic_write(auth_path, &auth_str)?;
    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )?;

    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn remove_auth_from_store_json(store_json_path: &Path) -> std::io::Result<()> {
    if !store_json_path.is_file() {
        return Ok(());
    }

    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&content)
    {
        Ok(store) => store,
        Err(_) => {
            // The legacy store is already unreadable. Replace it so a partial
            // auth payload cannot survive after the encrypted store is authoritative.
            return anlg_storage::fs::atomic_write(store_json_path, "{}");
        }
    };

    if store.remove(PLUGIN_NAME).is_none() {
        return Ok(());
    }

    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )
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

    #[test]
    fn plaintext_removal_truncates_contents_before_unlinking() {
        let temp = tempdir().unwrap();
        let auth_path = temp.path().join(FILENAME);
        let surviving_link = temp.path().join("auth-backup.json");
        std::fs::write(&auth_path, auth_json("secret-token")).unwrap();
        std::fs::hard_link(&auth_path, &surviving_link).unwrap();

        remove_plaintext_auth(&auth_path).unwrap();

        assert!(!auth_path.exists());
        assert_eq!(std::fs::read(&surviving_link).unwrap(), b"");
    }

    #[test]
    fn plaintext_cleanup_removes_auth_from_legacy_store() {
        let temp = tempdir().unwrap();
        let store_path = temp.path().join("store.json");
        std::fs::write(
            &store_path,
            legacy_store_json(
                &auth_json("secret-token"),
                Some(("other", serde_json::json!("value"))),
            ),
        )
        .unwrap();

        remove_auth_from_store_json(&store_path).unwrap();

        let store: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(store_path).unwrap()).unwrap();
        assert!(store.get(PLUGIN_NAME).is_none());
        assert_eq!(store.get("other"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn plaintext_cleanup_scrubs_unreadable_legacy_store() {
        let temp = tempdir().unwrap();
        let store_path = temp.path().join("store.json");
        std::fs::write(&store_path, r#"{"auth":"partial-secret""#).unwrap();

        remove_auth_from_store_json(&store_path).unwrap();

        assert_eq!(std::fs::read_to_string(store_path).unwrap(), "{}");
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
