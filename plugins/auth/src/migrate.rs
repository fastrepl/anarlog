#[cfg(all(target_os = "linux", not(test)))]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(all(target_os = "linux", not(test)))]
use anlg_desktop_auth::{LinuxSecurePersistence, Persistence, SecretStore};
use tauri::Manager;

const FILENAME: &str = anlg_desktop_auth::paths::FILENAME;

pub(crate) fn auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<PathBuf> {
    let new_auth_path = new_auth_path(app)?;
    let legacy_auth_path = legacy_auth_path(app)?;
    let legacy_store_json_path = legacy_store_json_path(app)?;

    Ok(anlg_desktop_auth::paths::resolve_auth_path_from_paths(
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
    let persistence = linux_persistence(app, auth_path(app)?);
    Ok(persistence.load()?)
}

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn persist_linux_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    auth: &HashMap<String, String>,
) -> crate::Result<()> {
    linux_persistence(app, auth_path(app)?).save(auth)?;
    Ok(())
}

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn clear_linux_auth<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<()> {
    linux_persistence(app, auth_path(app)?).clear()?;
    Ok(())
}

#[cfg(all(target_os = "linux", not(test)))]
fn linux_persistence<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    auth_path: PathBuf,
) -> LinuxSecurePersistence {
    LinuxSecurePersistence::new(Box::new(TauriSecretStore { app: app.clone() }), auth_path)
}

#[cfg(all(target_os = "linux", not(test)))]
struct TauriSecretStore<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

#[cfg(all(target_os = "linux", not(test)))]
impl<R: tauri::Runtime> SecretStore for TauriSecretStore<R> {
    fn read(&self) -> std::result::Result<Option<String>, String> {
        tauri_plugin_store2::read_secret_blocking(&self.app, AUTH_SCOPE, AUTH_KEY)
            .map_err(|error| error.to_string())
    }

    fn write(&self, value: &str) -> std::result::Result<(), String> {
        tauri_plugin_store2::write_secret_blocking(&self.app, AUTH_SCOPE, AUTH_KEY, value)
            .map_err(|error| error.to_string())
    }

    fn delete(&self) -> std::result::Result<(), String> {
        tauri_plugin_store2::delete_secret_blocking(&self.app, AUTH_SCOPE, AUTH_KEY)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
fn cli_fallback_auth_path(auth_path: &Path) -> PathBuf {
    anlg_desktop_auth::paths::cli_fallback_auth_path(auth_path)
}

// A leftover auth.json is less harmful than refusing a session the secure store
// already holds, so cleanup failures are reported rather than propagated.
#[cfg(target_os = "windows")]
pub(crate) fn discard_plaintext_auth(path: &Path) {
    if let Err(error) = remove_plaintext_auth(path) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed_to_remove_plaintext_auth"
        );
    }
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn remove_plaintext_auth(path: &Path) -> std::io::Result<()> {
    anlg_desktop_auth::paths::discard_plaintext_auth(path)
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
    Ok(anlg_storage::windows_auth::secure_path(&new_auth_path(
        app,
    )?))
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

#[cfg(test)]
fn migrate_auth_state(
    legacy_auth_path: &Path,
    legacy_store_json_path: &Path,
    new_auth_path: &Path,
) -> std::io::Result<()> {
    anlg_desktop_auth::paths::migrate_auth_state(
        legacy_auth_path,
        legacy_store_json_path,
        new_auth_path,
    )
}

#[cfg(test)]
fn resolve_auth_path_from_paths(
    legacy_auth_path: &Path,
    legacy_store_json_path: &Path,
    new_auth_path: &Path,
) -> PathBuf {
    anlg_desktop_auth::paths::resolve_auth_path_from_paths(
        legacy_auth_path,
        legacy_store_json_path,
        new_auth_path,
    )
}

#[cfg(any(target_os = "windows", test))]
fn remove_auth_from_store_json(store_json_path: &Path) -> std::io::Result<()> {
    anlg_desktop_auth::paths::remove_auth_from_store_json(store_json_path)
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
        assert!(migrated_store.get(crate::PLUGIN_NAME).is_none());
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
    fn cli_fallback_is_distinct_from_legacy_plaintext_auth() {
        let auth_path = Path::new("/data/com.hyprnote.stable/auth.json");

        assert_eq!(
            cli_fallback_auth_path(auth_path),
            Path::new("/data/com.hyprnote.stable/auth.cli.json")
        );
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
        assert!(store.get(crate::PLUGIN_NAME).is_none());
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
            crate::PLUGIN_NAME.to_string(),
            serde_json::Value::String(auth_json.to_string()),
        );
        if let Some((key, value)) = extra {
            store.insert(key.to_string(), value);
        }
        serde_json::to_string(&store).unwrap()
    }
}
