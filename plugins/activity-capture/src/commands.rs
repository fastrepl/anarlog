use crate::{
    ActivityCapturePluginExt,
    events::{ActivityCaptureCapabilities, ActivityCaptureSnapshot},
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn capabilities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ActivityCaptureCapabilities, String> {
    Ok(app.activity_capture().capabilities())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ActivityCaptureSnapshot>, String> {
    app.activity_capture()
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture()
        .start()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.activity_capture().stop();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_running<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    Ok(app.activity_capture().is_running())
}
