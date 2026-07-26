use crate::{Permission, PermissionsPluginExt};

#[tauri::command]
#[specta::specta]
pub(crate) async fn open_permission<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    permission: Permission,
) -> Result<(), String> {
    app.permissions()
        .open(permission)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn check_permission<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    permission: Permission,
) -> Result<crate::PermissionStatus, String> {
    app.permissions()
        .check(permission)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn request_permission<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    permission: Permission,
) -> Result<(), String> {
    app.permissions()
        .request(permission)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn reset_permission<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    permission: Permission,
) -> Result<(), String> {
    app.permissions()
        .reset(permission)
        .await
        .map_err(|e| e.to_string())
}
