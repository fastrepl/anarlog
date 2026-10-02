use std::str::FromStr;

use anlg_transcription_core::listener2 as core;
use tauri::Manager;

use crate::TranscriptionParams;
use crate::listener2::Listener2PluginExt;
use tauri_plugin_fs_sync::FsSyncPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn start_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: TranscriptionParams,
) -> Result<(), String> {
    let audio_session_id = params.session_id.split(':').next().unwrap_or_default();
    let _audio_guard = app.fs_sync().lock_session_audio(audio_session_id).await;
    app.listener2()
        .start_transcription(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    app.listener2().stop_transcription(session_id).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_transcription_sessions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<crate::TranscriptionSession>, String> {
    app.listener2()
        .list_transcription_sessions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_completed_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<Option<crate::CompletedTranscription>, String> {
    app.listener2()
        .get_completed_transcription(session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_completed_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    app.listener2()
        .acknowledge_completed_transcription(session_id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn parse_subtitle<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<core::Subtitle, String> {
    app.listener2().parse_subtitle(path)
}

#[tauri::command]
#[specta::specta]
pub async fn export_to_vtt<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    words: Vec<core::VttWord>,
) -> Result<String, String> {
    app.listener2().export_to_vtt(session_id, words)
}

#[tauri::command]
#[specta::specta]
pub async fn is_supported_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: String,
    model: Option<String>,
    languages: Vec<String>,
) -> Result<bool, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| anlg_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    core::is_supported_languages_batch(&provider, model.as_deref(), &languages_parsed)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_providers_for_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    languages: Vec<String>,
) -> Result<Vec<String>, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| anlg_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    Ok(core::suggest_providers_for_languages_batch(
        &languages_parsed,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn list_documented_language_codes_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(core::list_documented_language_codes_batch())
}

#[tauri::command]
#[specta::specta]
pub async fn refine_batch_transcript(
    request: anlg_transcript::BatchRefinementRequest,
) -> Result<anlg_transcript::BatchRefinementOutcome, String> {
    Ok(anlg_transcript::refine_batch_transcript(request))
}

#[tauri::command]
#[specta::specta]
pub async fn save_batch_transcript<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::batch_transcript::SaveBatchTranscriptRequest,
) -> Result<crate::batch_transcript::SaveBatchTranscriptOutcome, String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::batch_transcript::save_batch_transcript(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn render_session_transcript<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::session_transcript::RenderSessionTranscriptRequest,
) -> Result<Option<crate::session_transcript::RenderedSessionTranscript>, String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    crate::session_transcript::render_session_transcript(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn reconcile_refined_speaker_clusters(
    request: anlg_transcript::SpeakerClusterReconciliationRequest,
) -> Result<Vec<anlg_transcript::StoredSpeakerHint>, String> {
    Ok(anlg_transcript::reconcile_refined_speaker_clusters(
        &request.source,
        &request.words,
        request.hints,
    ))
}
