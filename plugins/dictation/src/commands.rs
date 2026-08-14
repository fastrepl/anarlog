use std::sync::Arc;

use tauri::Manager;

use crate::{
    events::Phase,
    ext::DictationPluginExt,
    recorder::{RecordedAudio, Recorder},
};

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
) -> Result<(), String> {
    let audio = app
        .state::<Arc<dyn anlg_audio::AudioProvider>>()
        .inner()
        .clone();
    app.state::<Recorder>()
        .start(audio, microphone_device)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<RecordedAudio, String> {
    app.state::<Recorder>()
        .stop()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_recording<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.state::<Recorder>()
        .cancel()
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
