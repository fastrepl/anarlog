use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use sqlx::SqlitePool;
use tauri::Manager;
use tauri_plugin_fs_sync::FsSyncPluginExt;
use tauri_specta::Event;

use crate::{BatchSessionRegistry, ListenerPluginExt};

const SWEEP_INTERVAL: Duration = Duration::from_secs(60);
const VOICEPRINT_CLEANUP_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

const AUDIO_RETENTION_SETTING_ID: &str = "audio_retention";
const SAVE_RECORDINGS_SETTING_ID: &str = "save_recordings";
const LEGACY_SETTINGS_ID: &str = "legacy_settings_document";
const LEGACY_MAIN_VALUES_ID: &str = "legacy_main_values_document";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioRetentionPolicy {
    None,
    OneDay,
    ThreeDays,
    OneWeek,
    OneMonth,
    Forever,
}

impl AudioRetentionPolicy {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(false) => Some(Self::None),
            Value::Bool(true) => Some(Self::Forever),
            Value::String(value) => match value.as_str() {
                "none" => Some(Self::None),
                "oneDay" => Some(Self::OneDay),
                "threeDays" => Some(Self::ThreeDays),
                "oneWeek" => Some(Self::OneWeek),
                "oneMonth" => Some(Self::OneMonth),
                "forever" => Some(Self::Forever),
                _ => None,
            },
            _ => None,
        }
    }

    fn duration_ms(self) -> Option<i64> {
        match self {
            Self::None => Some(0),
            Self::OneDay => Some(DAY_MS),
            Self::ThreeDays => Some(3 * DAY_MS),
            Self::OneWeek => Some(7 * DAY_MS),
            Self::OneMonth => Some(30 * DAY_MS),
            Self::Forever => None,
        }
    }
}

#[derive(Debug, Default)]
struct StoredRetentionSettings {
    audio_retention: Option<String>,
    save_recordings: Option<String>,
    legacy_settings: Option<String>,
    legacy_main_values: Option<String>,
}

/// Mirrors the desktop settings resolution: direct row, then legacy documents,
/// then `save_recordings = false`, then the `forever` default.
fn resolve_policy(stored: &StoredRetentionSettings) -> AudioRetentionPolicy {
    let parse = |json: &Option<String>| {
        json.as_deref()
            .and_then(|json| serde_json::from_str::<Value>(json).ok())
    };

    if let Some(policy) = parse(&stored.audio_retention)
        .as_ref()
        .and_then(AudioRetentionPolicy::from_value)
    {
        return policy;
    }

    let legacy_settings = parse(&stored.legacy_settings).unwrap_or(Value::Null);
    let legacy_main_values = parse(&stored.legacy_main_values).unwrap_or(Value::Null);
    let legacy = [
        legacy_settings.pointer("/general/audio_retention"),
        legacy_settings.pointer("/general/saveAudioAfterMeeting"),
        legacy_settings.pointer("/general/save_recordings"),
        legacy_main_values.get("audio_retention"),
        legacy_main_values.get("save_recordings"),
    ];
    if let Some(policy) = legacy
        .into_iter()
        .flatten()
        .find_map(AudioRetentionPolicy::from_value)
    {
        return policy;
    }

    if parse(&stored.save_recordings) == Some(Value::Bool(false)) {
        return AudioRetentionPolicy::None;
    }
    AudioRetentionPolicy::Forever
}

async fn load_policy(pool: &SqlitePool) -> Result<AudioRetentionPolicy, sqlx::Error> {
    let get = |id| anlg_db_app::get_app_setting_json(pool, id);
    Ok(resolve_policy(&StoredRetentionSettings {
        audio_retention: get(AUDIO_RETENTION_SETTING_ID).await?,
        save_recordings: get(SAVE_RECORDINGS_SETTING_ID).await?,
        legacy_settings: get(LEGACY_SETTINGS_ID).await?,
        legacy_main_values: get(LEGACY_MAIN_VALUES_ID).await?,
    }))
}

fn is_expired(created_at_ms: Option<i64>, policy: AudioRetentionPolicy, now_ms: i64) -> bool {
    match (policy, policy.duration_ms(), created_at_ms) {
        (AudioRetentionPolicy::None, _, _) => true,
        (_, Some(duration), Some(created_at)) => now_ms >= created_at.saturating_add(duration),
        _ => false,
    }
}

fn retention_expired(
    candidate: &anlg_db_app::SessionAudioRetentionCandidate,
    policy: AudioRetentionPolicy,
    now_ms: i64,
) -> bool {
    if policy == AudioRetentionPolicy::None && !candidate.has_words {
        return false;
    }
    is_expired(candidate.created_at_ms, policy, now_ms)
}

async fn session_audio_is_expired(pool: &SqlitePool, session_id: &str) -> Result<bool, String> {
    let policy = load_policy(pool).await.map_err(|e| e.to_string())?;
    let candidate = anlg_db_app::get_session_audio_retention_candidate(pool, session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(candidate.is_some_and(|candidate| {
        retention_expired(&candidate, policy, chrono::Utc::now().timestamp_millis())
    }))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionAudioRetentionPhase {
    Deleting,
    Deleted,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event)]
pub struct SessionAudioRetentionEvent {
    pub session_id: String,
    pub phase: SessionAudioRetentionPhase,
}

fn db_pool<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<SqlitePool> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
}

fn batch_session_belongs_to(batch_session_id: &str, session_id: &str) -> bool {
    batch_session_id
        .strip_prefix(session_id)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
}

/// Conservative: any uncertainty about capture or batch state keeps the audio.
async fn session_audio_is_idle<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> bool {
    let Ok(snapshot) = app.listener().get_capture_snapshot().await else {
        return false;
    };
    if snapshot.active_session_id.as_deref() == Some(session_id)
        || snapshot
            .finalizing_session_ids
            .iter()
            .any(|id| id == session_id)
    {
        return false;
    }

    let registry = app.state::<Arc<BatchSessionRegistry>>();
    registry.sessions.lock().is_ok_and(|sessions| {
        !sessions
            .keys()
            .any(|id| batch_session_belongs_to(id, session_id))
    })
}

#[derive(Clone, Copy)]
enum Eligibility {
    Expired,
    Processed,
    LogicallyDeleted,
}

async fn delete_local_audio<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
    session_id: &str,
    eligibility: Eligibility,
) -> Result<bool, String> {
    let _guard = app.fs_sync().lock_session_audio(session_id).await;
    if !session_audio_is_idle(app, session_id).await {
        return Ok(false);
    }
    let eligible = match eligibility {
        Eligibility::Expired => session_audio_is_expired(pool, session_id).await?,
        Eligibility::Processed => anlg_db_app::session_audio_is_processed(pool, session_id)
            .await
            .map_err(|e| e.to_string())?,
        Eligibility::LogicallyDeleted => {
            anlg_db_app::session_audio_is_logically_deleted(pool, session_id)
                .await
                .map_err(|e| e.to_string())?
        }
    };
    if !eligible {
        return Ok(false);
    }

    let emit = |phase| {
        let _ = SessionAudioRetentionEvent {
            session_id: session_id.to_string(),
            phase,
        }
        .emit(app);
    };
    emit(SessionAudioRetentionPhase::Deleting);
    let deleted = app.fs_sync().delete_session_audio_locked(session_id)?;
    anlg_db_app::mark_session_audio_absent(pool, session_id)
        .await
        .map_err(|e| e.to_string())?;
    if deleted {
        emit(SessionAudioRetentionPhase::Deleted);
    }
    Ok(deleted)
}

async fn sweep<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
) -> Result<(), String> {
    let logically_deleted = anlg_db_app::list_logically_deleted_session_audio(pool)
        .await
        .map_err(|e| e.to_string())?;
    for session_id in logically_deleted {
        if let Err(error) =
            delete_local_audio(app, pool, &session_id, Eligibility::LogicallyDeleted).await
        {
            tracing::warn!(%session_id, %error, "deleted_session_audio_cleanup_failed");
        }
    }

    let policy = load_policy(pool).await.map_err(|e| e.to_string())?;
    if policy == AudioRetentionPolicy::Forever {
        return Ok(());
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let candidates = anlg_db_app::list_session_audio_retention_candidates(pool)
        .await
        .map_err(|e| e.to_string())?;
    for candidate in candidates {
        if !retention_expired(&candidate, policy, now_ms) {
            continue;
        }
        let session_id = candidate.session_id;
        if let Err(error) = delete_local_audio(app, pool, &session_id, Eligibility::Expired).await {
            tracing::warn!(%session_id, %error, "expired_session_audio_cleanup_failed");
        }
    }
    Ok(())
}

pub(crate) fn spawn<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_voiceprint_cleanup: Option<Instant> = None;
        loop {
            interval.tick().await;
            let Some(pool) = db_pool(&app) else {
                continue;
            };
            if let Err(error) = sweep(&app, &pool).await {
                tracing::warn!(%error, "audio_retention_sweep_failed");
            }
            if last_voiceprint_cleanup.is_none_or(|at| at.elapsed() >= VOICEPRINT_CLEANUP_INTERVAL)
            {
                last_voiceprint_cleanup = Some(Instant::now());
                if let Err(error) =
                    crate::voiceprint::cleanup_expired_voiceprint_candidates(&app, &pool).await
                {
                    tracing::warn!(%error, "voiceprint_candidate_cleanup_failed");
                }
            }
        }
    });
}

/// Deletes local audio right after processing when the retention policy is `none`.
#[tauri::command]
#[specta::specta]
pub async fn delete_processed_session_audio<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<bool, String> {
    let pool = db_pool(&app).ok_or_else(|| "database is not ready yet".to_string())?;
    if load_policy(&pool).await.map_err(|e| e.to_string())? != AudioRetentionPolicy::None {
        return Ok(false);
    }
    delete_local_audio(&app, &pool, &session_id, Eligibility::Processed).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(
        audio_retention: Option<&str>,
        save_recordings: Option<&str>,
        legacy_settings: Option<&str>,
        legacy_main_values: Option<&str>,
    ) -> StoredRetentionSettings {
        StoredRetentionSettings {
            audio_retention: audio_retention.map(str::to_string),
            save_recordings: save_recordings.map(str::to_string),
            legacy_settings: legacy_settings.map(str::to_string),
            legacy_main_values: legacy_main_values.map(str::to_string),
        }
    }

    #[test]
    fn retention_policy_never_deletes_audio_the_user_chose_to_keep() {
        use AudioRetentionPolicy as P;
        let cases = [
            (stored(None, None, None, None), P::Forever),
            (stored(Some(r#""unknown""#), None, None, None), P::Forever),
            (stored(Some("not json"), Some("false"), None, None), P::None),
            (
                stored(Some(r#""oneWeek""#), Some("false"), None, None),
                P::OneWeek,
            ),
            (stored(Some("true"), Some("false"), None, None), P::Forever),
            (stored(Some("false"), None, None, None), P::None),
            (
                stored(
                    None,
                    Some("false"),
                    Some(r#"{"general":{"saveAudioAfterMeeting":"threeDays"}}"#),
                    None,
                ),
                P::ThreeDays,
            ),
            (
                stored(None, None, None, Some(r#"{"save_recordings":false}"#)),
                P::None,
            ),
        ];
        for (settings, expected) in cases {
            assert_eq!(resolve_policy(&settings), expected, "{settings:?}");
        }
    }

    #[test]
    fn retention_window_starts_at_session_creation() {
        let created = Some(1_000);
        assert!(!is_expired(
            created,
            AudioRetentionPolicy::OneDay,
            1_000 + DAY_MS - 1
        ));
        assert!(is_expired(
            created,
            AudioRetentionPolicy::OneDay,
            1_000 + DAY_MS
        ));
        assert!(!is_expired(None, AudioRetentionPolicy::OneMonth, i64::MAX));
        assert!(!is_expired(
            created,
            AudioRetentionPolicy::Forever,
            i64::MAX
        ));
        assert!(is_expired(None, AudioRetentionPolicy::None, 0));
    }

    #[test]
    fn batch_sessions_match_their_capture_session_only() {
        assert!(batch_session_belongs_to("s1", "s1"));
        assert!(batch_session_belongs_to("s1:recovery", "s1"));
        assert!(!batch_session_belongs_to("s10", "s1"));
    }
}
