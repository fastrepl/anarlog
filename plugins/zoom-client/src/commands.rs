use crate::{ZoomClientPluginExt, ZoomMeetingStatus};

#[tauri::command]
#[specta::specta]
pub(crate) fn is_available() -> bool {
    crate::is_enabled()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn join_meeting<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    meeting_url: String,
    display_name: String,
) -> Result<String, String> {
    app.zoom_client()
        .join(meeting_url, display_name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn leave_meeting<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.zoom_client().leave().map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_status<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> ZoomMeetingStatus {
    app.zoom_client().status()
}
