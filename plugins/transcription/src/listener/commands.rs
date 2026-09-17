use std::str::FromStr;

use crate::listener::ListenerPluginExt;
use crate::{CaptureConfigUpdate, CaptureParams, CaptureSnapshot, CaptureState};
use anlg_transcript::{RenderTranscriptRequest, RenderedTranscriptSegment};
use anlg_transcription_core::listener::actors::recorder::{self, RecoveryAudioChunk};
use anlg_transcription_core::listener2 as listener2_core;

fn session_audio_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    use tauri_plugin_settings::SettingsPluginExt;
    uuid::Uuid::parse_str(session_id).map_err(|_| "Invalid session ID".to_string())?;
    let base = app
        .settings()
        .vault_base()
        .map_err(|error| error.to_string())?;
    Ok(recorder::find_session_dir(
        base.join("sessions").as_std_path(),
        session_id,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn get_capture_audio_cleanup_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<std::collections::HashMap<String, bool>, String> {
    use tauri::Manager;
    app.state::<crate::AudioCleanupStatus>()
        .0
        .lock()
        .map(|status| status.clone())
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_capture_audio_cleanup_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    failed: bool,
) -> Result<(), String> {
    use tauri::Manager;
    app.state::<crate::AudioCleanupStatus>()
        .acknowledge(&session_id, failed)
}

#[tauri::command]
#[specta::specta]
pub async fn list_capture_audio_chunks<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Vec<RecoveryAudioChunk>, String> {
    tokio::task::spawn_blocking(move || {
        let dir = session_audio_dir(&app, &session_id)?;
        recorder::list_recovery_chunks(&dir).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_capture_audio_chunk<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    chunk_id: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let dir = session_audio_dir(&app, &session_id)?;
        recorder::acknowledge_recovery_chunk(&dir, &chunk_id).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub async fn list_microphone_devices<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.listener()
        .list_microphone_devices()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_microphone_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.listener()
        .get_current_microphone_device()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_mic_muted<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    Ok(app.listener().get_mic_muted().await)
}

#[tauri::command]
#[specta::specta]
pub async fn set_mic_muted<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    muted: bool,
) -> Result<(), String> {
    app.listener().set_mic_muted(muted).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn start_capture<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: CaptureParams,
) -> Result<(), String> {
    app.listener()
        .start_capture(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_capture<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    use crate::Listener2PluginExt;
    if let Ok(snapshot) = app.listener().get_capture_snapshot().await
        && let Some(session_id) = snapshot.active_session_id
    {
        app.listener2()
            .stop_transcription(format!("{session_id}:recovery"))
            .await;
    }
    app.listener().stop_capture().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_capture_config<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    update: CaptureConfigUpdate,
) -> Result<(), String> {
    app.listener().update_capture_config(update).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_capture_state<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<CaptureState, String> {
    Ok(app.listener().get_capture_state().await)
}

#[tauri::command]
#[specta::specta]
pub async fn get_capture_snapshot<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<CaptureSnapshot, String> {
    app.listener()
        .get_capture_snapshot()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn is_supported_languages_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: String,
    model: Option<String>,
    languages: Vec<String>,
) -> Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    let languages_parsed = languages
        .iter()
        .map(|s| anlg_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    listener2_core::is_supported_languages_live(&provider, model.as_deref(), &languages_parsed)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_providers_for_languages_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    languages: Vec<String>,
) -> Result<Vec<String>, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| anlg_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    Ok(listener2_core::suggest_providers_for_languages_live(
        &languages_parsed,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn list_documented_language_codes_live<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(owhisper_client::documented_language_codes_live())
}

#[tauri::command]
#[specta::specta]
pub async fn render_transcript_segments(
    params: RenderTranscriptRequest,
) -> Result<Vec<RenderedTranscriptSegment>, String> {
    Ok(anlg_transcript::render_transcript_segments(params))
}

#[tauri::command]
#[specta::specta]
pub async fn update_capture_credentials<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    session_id: String,
    api_key: String,
) -> Result<(), String> {
    use anlg_transcription_core::listener::actors::{SessionMsg, session_supervisor_name};
    let cell = ractor::registry::where_is(session_supervisor_name(&session_id))
        .ok_or("Capture is not active")?;
    let actor: ractor::ActorRef<SessionMsg> = cell.into();
    actor
        .cast(SessionMsg::UpdateCredentials(api_key))
        .map_err(|_| "Capture credentials could not be updated".to_string())
}
