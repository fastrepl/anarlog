use std::sync::Arc;

use tauri::Manager;

use crate::{
    events::Phase,
    ext::DictationPluginExt,
    recorder::{RecordedAudio, Recorder},
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_target() -> Result<String, String> {
    crate::insertion::capture().await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn insert_text(target: String, text: String) -> Result<(), String> {
    if text.contains('\0') {
        return Err("Dictation text contains an unsupported NUL character".into());
    }
    if text.is_empty() || text.len() > 100_000 {
        return Err("Dictation text is empty or too long".into());
    }
    crate::insertion::insert(target, text).await
}

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

#[tauri::command]
#[specta::specta]
pub(crate) async fn start_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    microphone_device: Option<String>,
    owner: String,
) -> Result<(), String> {
    let audio = app
        .state::<Arc<dyn anlg_audio::AudioProvider>>()
        .inner()
        .clone();
    app.state::<Recorder>()
        .start(audio, microphone_device, owner)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    owner: String,
) -> Result<RecordedAudio, String> {
    app.state::<Recorder>()
        .stop(&owner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    owner: String,
) -> Result<(), String> {
    app.state::<Recorder>()
        .cancel(&owner)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn discard_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    file_path: String,
) -> Result<(), String> {
    app.state::<Recorder>()
        .discard(file_path)
        .map_err(|error| error.to_string())
}
