use std::path::{Path, PathBuf};

use anlg_desktop_identity::auth::{AuthPaths, FILENAME};
use tauri::Manager;

pub(crate) fn auth_paths<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<AuthPaths> {
    Ok(AuthPaths {
        current: new_auth_path(app)?,
        legacy: legacy_auth_path(app)?,
        legacy_store_json: legacy_store_json_path(app)?,
    })
}

#[cfg(any(not(any(target_os = "linux", target_os = "windows")), test))]
pub(crate) fn auth_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<PathBuf> {
    Ok(auth_paths(app)?.resolve())
}

/// Linux keeps the session in Secret Service and Windows in DPAPI, each through the shared
/// backend the native app also uses.
#[cfg(all(any(target_os = "linux", target_os = "windows"), not(test)))]
pub(crate) fn session_store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<anlg_desktop_identity::session::SessionStore> {
    Ok(anlg_desktop_identity::session::SessionStore::for_platform(
        auth_paths(app)?,
        &app.config().identifier,
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

fn invalid_data(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}
