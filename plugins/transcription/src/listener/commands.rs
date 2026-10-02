use std::str::FromStr;

use crate::listener::ListenerPluginExt;
use crate::{
    CaptureConfigUpdate, CaptureParams, CaptureSnapshot, CaptureState, LiveTranscriptPersistence,
    StoppedCapture,
};
use anlg_transcript::{RenderTranscriptRequest, RenderedTranscriptSegment};
use anlg_transcription_core::listener::actors::recorder::{self, RecoveryAudioChunk};
use anlg_transcription_core::listener2 as listener2_core;
use tauri_plugin_fs_sync::FsSyncPluginExt;

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
) -> Result<std::collections::HashMap<String, String>, String> {
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
    error: String,
) -> Result<(), String> {
    use tauri::Manager;
    app.state::<crate::AudioCleanupStatus>()
        .acknowledge(&session_id, &error)
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
pub async fn delete_transcribed_capture_audio<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        let dir = session_audio_dir(&app, &session_id)?;
        recorder::delete_transcribed_capture_audio(&dir).map_err(|error| error.to_string())
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
    use crate::live_journal::{LiveJournalRegistry, register_app_journal};
    use tauri::Manager;

    let session_id = params.session_id.clone();
    let _audio_guard = app.fs_sync().lock_session_audio(&session_id).await;
    let live_transcript = params.live_transcript.clone();
    let registry = app.state::<LiveJournalRegistry>();
    let registration = live_transcript
        .map(|target| register_app_journal(&registry, app.clone(), session_id.clone(), target))
        .transpose()?;

    match (app.listener().start_capture(params).await, registration) {
        (Ok(()), Some((_, Some(previous)))) => previous.release(),
        (Ok(()), Some((_, None))) => {}
        (Ok(()), None) => registry.release_session(&session_id)?,
        (Err(error), Some((registered, previous))) => {
            let _ = registry.rollback_registration(&session_id, &registered, previous);
            return Err(error.to_string());
        }
        (Err(error), None) => return Err(error.to_string()),
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn flush_live_transcript<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<LiveTranscriptPersistence>, String> {
    use crate::live_journal::LiveJournalRegistry;
    use tauri::Manager;

    let registry = app.state::<LiveJournalRegistry>();
    let Some(journal) = registry.get(&session_id)? else {
        return Ok(None);
    };
    journal.flush().await.map(Some)
}

#[tauri::command]
#[specta::specta]
pub async fn release_live_transcript<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    transcript_id: String,
) -> Result<(), String> {
    use crate::live_journal::LiveJournalRegistry;
    use tauri::Manager;

    app.state::<LiveJournalRegistry>()
        .release(&session_id, &transcript_id)
}

#[tauri::command]
#[specta::specta]
pub async fn list_stopped_captures<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<StoppedCapture>, String> {
    use tauri::Manager;
    Ok(app.state::<crate::StoppedCaptureRegistry>().list())
}

#[tauri::command]
#[specta::specta]
pub async fn get_stopped_capture<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<StoppedCapture>, String> {
    use tauri::Manager;
    Ok(app
        .state::<crate::StoppedCaptureRegistry>()
        .get(&session_id))
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_stopped_capture<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    stopped_at_ms: i64,
) -> Result<(), String> {
    use tauri::Manager;
    app.state::<crate::StoppedCaptureRegistry>()
        .acknowledge(&session_id, stopped_at_ms);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_capture_lifecycle_marker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    marker: crate::CaptureLifecycleMarker,
    replace_transcript_id: Option<String>,
) -> Result<(), String> {
    let value_json = crate::capture_markers::serialize_marker(&marker)?;
    let replace_transcript_id =
        replace_transcript_id.unwrap_or_else(|| marker.transcript_id.clone());
    if !crate::capture_markers::save_marker(
        &app,
        &marker.session_id,
        &value_json,
        &replace_transcript_id,
    )
    .await?
    {
        return Err("capture lifecycle marker belongs to a different capture".to_string());
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_capture_lifecycle_marker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    transcript_id: String,
) -> Result<(), String> {
    if !crate::capture_markers::clear_marker(&app, &session_id, &transcript_id).await? {
        return Err(
            "capture lifecycle marker was not found or belongs to a different capture".to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_capture_lifecycle_marker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<crate::CaptureLifecycleMarker>, String> {
    crate::capture_markers::get_marker(&app, &session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_capture_lifecycle_markers<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<crate::CaptureLifecycleMarker>, String> {
    crate::capture_markers::list_markers(&app).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_capture_recoveries<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<crate::CaptureRecovery>, String> {
    use tauri::Manager;

    let (active_session_id, finalizing_session_ids) =
        match app.listener().get_capture_snapshot().await {
            Ok(snapshot) => (snapshot.active_session_id, snapshot.finalizing_session_ids),
            Err(error) => {
                tracing::warn!(?error, "failed to list active captures for recovery");
                (None, Vec::new())
            }
        };
    let stopped_session_ids =
        if let Some(registry) = app.try_state::<crate::StoppedCaptureRegistry>() {
            registry
                .list()
                .into_iter()
                .map(|capture| capture.session_id)
                .collect()
        } else {
            tracing::warn!("stopped capture registry is unavailable for recovery");
            Vec::new()
        };
    let marker_session_ids = match crate::capture_markers::list_markers(&app).await {
        Ok(markers) => markers
            .into_iter()
            .map(|marker| marker.session_id)
            .collect(),
        Err(error) => {
            tracing::warn!(?error, "failed to list capture markers for recovery");
            Vec::new()
        }
    };

    Ok(crate::stopped_captures::merge_capture_recoveries(
        stopped_session_ids,
        active_session_id,
        finalizing_session_ids,
        marker_session_ids,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn get_capture_audio_gaps<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<crate::CaptureAudioGaps>, String> {
    use tauri::Manager;

    app.try_state::<crate::CaptureGapRegistry>()
        .map(|registry| registry.get(&session_id))
        .ok_or_else(|| "capture gap registry is not available".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn mark_capture_audio_saved<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    crate::capture_markers::mark_audio_saved(&app, &session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_capture_audio_saved<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    crate::capture_markers::clear_audio_saved(&app, &session_id).await
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
pub async fn stop_capture_for_session<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<bool, String> {
    Ok(crate::stop_capture_for_session(&app, &session_id).await)
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
