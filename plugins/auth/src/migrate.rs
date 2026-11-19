#[cfg(all(target_os = "linux", not(test)))]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(all(target_os = "linux", not(test)))]
use anlg_desktop_identity::auth::{
    LINUX_SECRET_KEY as AUTH_KEY, LINUX_SECRET_SCOPE as AUTH_SCOPE, cli_fallback_path,
};
use anlg_desktop_identity::auth::{AuthPaths, FILENAME};
use tauri::Manager;

pub(crate) fn auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<PathBuf> {
    Ok(AuthPaths {
        current: new_auth_path(app)?,
        legacy: legacy_auth_path(app)?,
        legacy_store_json: legacy_store_json_path(app)?,
    }
    .resolve())
}

#[cfg(all(target_os = "linux", not(test)))]
pub(crate) fn load_linux_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<HashMap<String, String>> {
    let auth_path = auth_path(app)?;
    let cli_fallback_path = cli_fallback_path(&auth_path);

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

    if cli_fallback_path.is_file() {
        match read_auth_file(&cli_fallback_path) {
            Ok(auth) => {
                if keyring_readable {
                    if let Err(error) = persist_linux_auth(app, &auth) {
                        tracing::warn!(%error, "failed_to_reconcile_cli_auth_with_secret_service");
                    }
                }
                return Ok(auth);
            }
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_cli_auth_fallback");
                discard_plaintext_auth(&cli_fallback_path);
            }
        }
    }

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

    let auth = match read_auth_file(&auth_path) {
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
    tauri_plugin_store2::delete_secret_blocking(app, AUTH_SCOPE, AUTH_KEY)
        .map_err(crate::Error::Storage)?;
    drop_plaintext_auth_for(app);
    Ok(())
}

// The keyring is authoritative once it has been written or cleared, so a plaintext
// copy left behind by a failed migration must not outlive it and silently restore
// the session on the next launch.
#[cfg(all(target_os = "linux", not(test)))]
fn drop_plaintext_auth_for<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    match auth_path(app) {
        Ok(path) => {
            discard_plaintext_auth(&path);
            discard_plaintext_auth(&cli_fallback_path(&path));
        }
        Err(error) => tracing::warn!(%error, "failed_to_resolve_auth_path"),
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn read_auth_file(path: &Path) -> crate::Result<HashMap<String, String>> {
    let data = std::fs::read_to_string(path).map_err(crate::Error::Io)?;
    Ok(serde_json::from_str(&data)?)
}

// A leftover auth.json is less harmful than refusing a session the secure store
// already holds, so cleanup failures are reported rather than propagated.
#[cfg(any(all(target_os = "linux", not(test)), target_os = "windows"))]
pub(crate) fn discard_plaintext_auth(path: &Path) {
    if let Err(error) = anlg_desktop_identity::auth::remove_plaintext(path) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed_to_remove_plaintext_auth"
        );
    }
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
            if let Err(error) = anlg_desktop_identity::auth::remove_auth_from_store_json(&path) {
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

fn invalid_data(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}
