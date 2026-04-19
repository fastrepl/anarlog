use tauri_plugin_auth::AuthPluginExt;
use tauri_plugin_permissions::PermissionsPluginExt;

use crate::error::Error;

pub fn access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.access_token().ok().flatten().filter(|t| !t.is_empty())
}

pub fn require_access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let token = app.access_token().map_err(|e| Error::Auth(e.to_string()))?;
    match token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(hypr_calendar::Error::NotAuthenticated.into()),
    }
}

pub async fn is_apple_authorized<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    {
        let status = app
            .permissions()
            .check(tauri_plugin_permissions::Permission::Calendar)
            .await
            .map_err(|e| hypr_calendar::Error::Api(e.to_string()))?;
        Ok(matches!(
            status,
            tauri_plugin_permissions::PermissionStatus::Authorized
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(false)
    }
}
