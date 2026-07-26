use crate::ScreenPluginExt;
use crate::ext::{WindowCaptureTarget, WindowContextCapture, WindowContextCaptureOptions};

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_frontmost_window_context<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    options: Option<WindowContextCaptureOptions>,
) -> Result<WindowContextCapture, String> {
    app.screen()
        .capture_frontmost_window_context(options.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_target_window_context<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: WindowCaptureTarget,
    options: Option<WindowContextCaptureOptions>,
) -> Result<WindowContextCapture, String> {
    app.screen()
        .capture_target_window_context(target, options.unwrap_or_default())
        .map_err(|e| e.to_string())
}
