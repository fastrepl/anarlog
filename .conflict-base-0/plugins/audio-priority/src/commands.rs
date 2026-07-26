use crate::AudioPriorityPluginExt;
use hypr_audio_device::AudioDevice;

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_input_devices<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<AudioDevice>, String> {
    app.audio_priority()
        .list_input_devices()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_output_devices<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<AudioDevice>, String> {
    app.audio_priority()
        .list_output_devices()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_default_input_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    device_id: String,
) -> Result<(), String> {
    app.audio_priority()
        .set_default_input_device(&device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_default_output_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    device_id: String,
) -> Result<(), String> {
    app.audio_priority()
        .set_default_output_device(&device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_input_priorities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.audio_priority()
        .get_input_priorities()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_output_priorities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.audio_priority()
        .get_output_priorities()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save_input_priorities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    priorities: Vec<String>,
) -> Result<(), String> {
    app.audio_priority()
        .save_input_priorities(priorities)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn save_output_priorities<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    priorities: Vec<String>,
) -> Result<(), String> {
    app.audio_priority()
        .save_output_priorities(priorities)
        .await
        .map_err(|e| e.to_string())
}
