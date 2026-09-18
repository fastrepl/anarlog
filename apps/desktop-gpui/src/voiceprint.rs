//! `services/voiceprint.ts` + the `transcription` plugin's voiceprint
//! commands for the shell: candidate extraction after a transcript lands,
//! promotion on a manual speaker assignment, the expiry sweep, the known
//! speakers for on-device diarization, and the mic-isolation memory.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_voiceprint_service::{KnownVoiceprint, SecretStore, Secrets};

use crate::db::Store;
use crate::store_file::StoreFile;

/// `CLEANUP_INTERVAL_MS`: expired candidates are a privacy cleanup, not a
/// hot path, so the sweep runs a few times a day.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// `crate::secrets` behind the service's store trait: the same OS credential
/// entries the Tauri app writes through `tauri-plugin-store2`.
struct KeyringSecrets {
    app_id: String,
}

impl SecretStore for KeyringSecrets {
    fn read(&self, scope: &str, key: &str) -> Result<Option<String>, String> {
        crate::secrets::read(&self.app_id, scope, key)
    }

    fn write(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        crate::secrets::write(&self.app_id, scope, key, value)
    }

    fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        crate::secrets::write(&self.app_id, scope, key, "")
    }
}

pub fn secrets(store: &Store) -> Secrets {
    Arc::new(KeyringSecrets {
        app_id: store.identifier().to_string(),
    })
}

/// `MicIsolationCache` + the `transcription.mic_isolation` store scope: a
/// stopped session's isolation survives a relaunch for a late extraction.
#[derive(Default)]
pub struct MicIsolation {
    cache: Mutex<HashMap<String, bool>>,
}

/// `merge_mic_isolation`: a recording is isolated only if every stream was.
/// Unplugging headphones mid-meeting flips it to false for good.
pub fn merge_mic_isolation(previous: Option<bool>, value: bool) -> bool {
    previous.unwrap_or(true) && value
}

impl MicIsolation {
    /// `persist_mic_isolation`
    pub fn persist(&self, store_file: &StoreFile, session_id: &str, value: Option<bool>) {
        if let Ok(mut cache) = self.cache.lock() {
            match value {
                Some(value) => {
                    cache.insert(session_id.to_string(), value);
                }
                None => {
                    cache.remove(session_id);
                }
            }
        }
        let scope = anlg_voiceprint_service::MIC_ISOLATION_STORE_SCOPE;
        let result = match value {
            Some(value) => store_file.set_scoped(scope, session_id, value),
            None => store_file.delete_scoped(scope, session_id),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "mic_isolation_persist_failed");
        }
    }

    /// `mic_isolated_for_session`
    pub fn get(&self, store_file: &StoreFile, session_id: &str) -> Option<bool> {
        let cached = self
            .cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(session_id).copied());
        if cached.is_some() {
            return cached;
        }
        store_file.scoped_bool(
            anlg_voiceprint_service::MIC_ISOLATION_STORE_SCOPE,
            session_id,
        )
    }
}

/// `maybeExtractVoiceprintCandidates`: runs after a transcript is persisted
/// and before audio retention may delete the recording. Failures are logged
/// and swallowed; losing candidates for one session is acceptable, blocking
/// transcription completion is not.
pub fn maybe_extract_candidates(
    store: &Store,
    enabled: bool,
    session_id: String,
    transcript_id: String,
    audio_path: Option<String>,
    mic_isolated: Option<bool>,
) -> tokio::task::JoinHandle<()> {
    let pool = store.pool().clone();
    let secrets = secrets(store);
    store.runtime().spawn(async move {
        let Some(audio_path) = audio_path.filter(|_| enabled) else {
            return;
        };
        if let Err(error) = anlg_voiceprint_service::extract_candidates(
            &pool,
            &secrets,
            session_id,
            transcript_id,
            audio_path,
            mic_isolated,
        )
        .await
        {
            tracing::error!(%error, "[voiceprint] candidate extraction failed");
        }
    })
}

/// `promoteVoiceprintCandidates` after an "all" speaker assignment.
pub fn promote_candidates(
    store: &Store,
    transcript_id: String,
    channel: i32,
    speaker_index: Option<i32>,
    human_id: String,
) -> tokio::task::JoinHandle<()> {
    let pool = store.pool().clone();
    let secrets = secrets(store);
    store.runtime().spawn(async move {
        if let Err(error) = anlg_voiceprint_service::promote_candidates(
            &pool,
            &secrets,
            transcript_id,
            channel,
            speaker_index,
            human_id,
        )
        .await
        {
            tracing::error!(%error, "[voiceprint] promotion failed");
        }
    })
}

/// `cleanupExpiredVoiceprintCandidates`, on a `CLEANUP_INTERVAL` loop like
/// the task manager's retention tick.
pub fn spawn_cleanup_loop(store: &Store) {
    let pool = store.pool().clone();
    let secrets = secrets(store);
    store.runtime().spawn(async move {
        loop {
            if let Err(error) =
                anlg_voiceprint_service::cleanup_expired_candidates(&pool, &secrets).await
            {
                tracing::error!(%error, "[voiceprint] candidate cleanup failed");
            }
            tokio::time::sleep(CLEANUP_INTERVAL).await;
        }
    });
}

/// `known_speakers_for_session` for the batch params.
pub fn known_speakers(
    store: &Store,
    session_id: String,
) -> tokio::task::JoinHandle<Vec<KnownVoiceprint>> {
    let pool = store.pool().clone();
    let secrets = secrets(store);
    store.runtime().spawn(async move {
        anlg_voiceprint_service::known_speakers_for_session(&pool, &secrets, &session_id).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_isolation_sticks_to_false_once_any_stream_was_shared() {
        assert!(merge_mic_isolation(None, true));
        assert!(!merge_mic_isolation(None, false));
        assert!(merge_mic_isolation(Some(true), true));
        assert!(!merge_mic_isolation(Some(true), false));
        assert!(!merge_mic_isolation(Some(false), true));
    }

    #[test]
    fn mic_isolation_round_trips_through_the_store_scope() {
        let dir = tempfile::tempdir().unwrap();
        let store_file = StoreFile::next_to(&dir.path().join("app.db"));
        let isolation = MicIsolation::default();
        isolation.persist(&store_file, "s1", Some(true));
        assert_eq!(isolation.get(&store_file, "s1"), Some(true));
        // A fresh process only has the file.
        let fresh = MicIsolation::default();
        assert_eq!(fresh.get(&store_file, "s1"), Some(true));
        isolation.persist(&store_file, "s1", None);
        assert_eq!(MicIsolation::default().get(&store_file, "s1"), None);
        assert_eq!(isolation.get(&store_file, "s1"), None);
    }
}
