use crate::{events::Phase, ext::DictationPluginExt};

#[tauri::command]
#[specta::specta]
pub(crate) async fn show<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().show().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn hide<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.dictation().hide().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_phase<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    phase: Phase,
) -> Result<(), String> {
    app.dictation().set_phase(phase).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn update_amplitude<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    amplitude: f32,
) -> Result<(), String> {
    app.dictation()
        .update_amplitude(amplitude)
        .map_err(|e| e.to_string())
}
