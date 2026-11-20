use crate::CameraWatermarkPluginExt;
use crate::camera::CameraWatermarkState;
use crate::prefs::CameraBackground;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_state<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> CameraWatermarkState {
    let camera = app.camera_watermark();
    camera.refreshed_state().await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
) -> Result<CameraWatermarkState, String> {
    app.camera_watermark()
        .set_enabled(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_auto_switch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
) -> Result<CameraWatermarkState, String> {
    app.camera_watermark()
        .set_auto_switch(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_background<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    background: CameraBackground,
) -> Result<CameraWatermarkState, String> {
    app.camera_watermark()
        .set_background(background)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add_extension<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> CameraWatermarkState {
    let camera = app.camera_watermark();
    camera.add_extension().await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn allow_camera<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> CameraWatermarkState {
    app.camera_watermark().allow_camera()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn open_extension_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.camera_watermark()
        .open_extension_settings()
        .map_err(|e| e.to_string())
}
