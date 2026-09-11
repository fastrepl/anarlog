//! Tauri commands over `anlg_voiceprint_service`, with the plugin's own
//! secret store (`tauri-plugin-store2`) and mic-isolation memory.

use std::sync::Arc;

use anlg_voiceprint_service::{MIC_ISOLATION_STORE_SCOPE, SecretStore, Secrets};
use tauri::Manager;
use tauri_plugin_store2::Store2PluginExt;

struct Store2Secrets<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> SecretStore for Store2Secrets<R> {
    fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String> {
        tauri_plugin_store2::read_secret_blocking(&self.app, scope, key)
    }

    fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        tauri_plugin_store2::write_secret_blocking(&self.app, scope, key, value)
    }

    fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        tauri_plugin_store2::delete_secret_blocking(&self.app, scope, key)
    }
}

fn secrets<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Secrets {
    Arc::new(Store2Secrets { app: app.clone() })
}

fn pool<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<sqlx::SqlitePool, String> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn extract_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    transcript_id: String,
    audio_path: String,
) -> Result<u32, String> {
    let pool = pool(&app)?;
    let mic_isolated = mic_isolated_for_session(&app, &session_id);
    anlg_voiceprint_service::extract_candidates(
        &pool,
        &secrets(&app),
        session_id,
        transcript_id,
        audio_path,
        mic_isolated,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn promote_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    transcript_id: String,
    speaker_channel: i32,
    speaker_index: Option<i32>,
    human_id: String,
) -> Result<u32, String> {
    let pool = pool(&app)?;
    anlg_voiceprint_service::promote_candidates(
        &pool,
        &secrets(&app),
        transcript_id,
        speaker_channel,
        speaker_index,
        human_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_expired_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<u32, String> {
    let pool = pool(&app)?;
    anlg_voiceprint_service::cleanup_expired_candidates(&pool, &secrets(&app)).await
}

/// Remembers a stopped session's mic isolation on disk so extraction after a relaunch or crash
/// recovery still sees it; the in-memory cache alone dies with the process.
pub(crate) fn persist_mic_isolation<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
    value: Option<bool>,
) {
    let store = match app
        .store2()
        .scoped_store::<String>(MIC_ISOLATION_STORE_SCOPE)
    {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!(%error, "mic_isolation_store_unavailable");
            return;
        }
    };
    let result = match value {
        Some(value) => store.set(session_id.to_string(), value),
        None => store.delete(session_id.to_string()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "mic_isolation_persist_failed");
    }
}

fn mic_isolated_for_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Option<bool> {
    let cached = app
        .try_state::<crate::MicIsolationCache>()
        .and_then(|cache| cache.lock().ok()?.get(session_id).copied());
    if cached.is_some() {
        return cached;
    }
    app.store2()
        .scoped_store::<String>(MIC_ISOLATION_STORE_SCOPE)
        .ok()?
        .get::<bool>(session_id.to_string())
        .ok()
        .flatten()
}

/// Confirmed voiceprints of the session's participants, for the on-device
/// diarizer. Best effort: any storage problem yields an empty list rather than
/// blocking transcription.
pub(crate) async fn known_speakers_for_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Vec<anlg_transcription_core::listener2::KnownSpeaker> {
    let Ok(pool) = pool(app) else {
        return Vec::new();
    };
    anlg_voiceprint_service::known_speakers_for_session(&pool, &secrets(app), session_id)
        .await
        .into_iter()
        .map(|known| anlg_transcription_core::listener2::KnownSpeaker {
            id: known.human_id,
            embedding: known.embedding,
        })
        .collect()
}
