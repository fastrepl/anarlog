use anlg_transcription_core::listener::ListenerRuntime;
use ractor::{ActorRef, call_t, registry};
use tauri::Manager;
use tauri_plugin_settings::SettingsPluginExt;
use tauri_specta::Event;

use crate::{
    CaptureDataEvent, CaptureGapRegistry, CaptureLifecycleEvent, CaptureStatusEvent,
    MicIsolationCache, SessionStateCache, SessionStateSnapshot, StoppedCapture,
    StoppedCaptureRegistry,
};
use anlg_transcription_core::listener::State as RootState;
use anlg_transcription_core::listener::actors::{RootActor, RootMsg};

const LIVE_SEGMENT_SNAPSHOT_LIMIT: usize = 200;

pub struct TauriRuntime {
    pub audio_cleanup_status: crate::AudioCleanupStatus,
    pub app: tauri::AppHandle,
    pub session_state_cache: SessionStateCache,
    pub mic_isolation_cache: MicIsolationCache,
    pub stopped_capture_registry: StoppedCaptureRegistry,
    pub capture_gap_registry: CaptureGapRegistry,
}

impl anlg_storage::StorageRuntime for TauriRuntime {
    fn global_base(&self) -> Result<std::path::PathBuf, anlg_storage::Error> {
        self.app
            .settings()
            .global_base()
            .map(|p| p.into_std_path_buf())
            .map_err(|_| anlg_storage::Error::DataDirUnavailable)
    }

    fn vault_base(&self) -> Result<std::path::PathBuf, anlg_storage::Error> {
        self.app
            .settings()
            .vault_base()
            .map(|p| p.into_std_path_buf())
            .map_err(|_| anlg_storage::Error::DataDirUnavailable)
    }
}

impl ListenerRuntime for TauriRuntime {
    fn emit_lifecycle(&self, event: anlg_transcription_core::listener::SessionLifecycleEvent) {
        use tauri_plugin_tray::TrayPluginExt;
        match &event {
            anlg_transcription_core::listener::SessionLifecycleEvent::Active { error, .. } => {
                let _ = self.app.tray().set_start_disabled(true);
                let _ = self.app.tray().set_degraded(error.is_some());
                let _ = self.app.tray().set_recording(true);
            }
            anlg_transcription_core::listener::SessionLifecycleEvent::Inactive { .. } => {
                let app = self.app.clone();
                tauri::async_runtime::spawn(async move {
                    match current_root_state().await {
                        RootState::Active => {}
                        RootState::Finalizing => {
                            let _ = app.tray().set_start_disabled(false);
                            let _ = app.tray().set_recording(false);
                        }
                        RootState::Inactive => {
                            let _ = app.tray().set_start_disabled(false);
                            let _ = app.tray().set_recording(false);
                            let _ = app.tray().set_degraded(false);
                        }
                    }
                });
            }
            anlg_transcription_core::listener::SessionLifecycleEvent::Finalizing { .. } => {}
        }

        let capture_event = match event {
            anlg_transcription_core::listener::SessionLifecycleEvent::Active {
                session_id,
                requested_transcription_mode,
                current_transcription_mode,
                error,
            } => {
                self.stopped_capture_registry.clear_session(&session_id);
                let requested_live_transcription = requested_transcription_mode
                    == anlg_transcription_core::listener::TranscriptionMode::Live;
                let live_transcription_active = current_transcription_mode
                    == anlg_transcription_core::listener::TranscriptionMode::Live;
                let now_ms = crate::capture_gaps::unix_now_ms();
                let new_capture = if let Ok(mut cache) = self.session_state_cache.lock() {
                    let state = cache.entry(session_id.clone()).or_default();
                    let new_capture = state.started_at_ms.is_none();
                    state.requested_live_transcription = requested_live_transcription;
                    state.live_transcription_active = live_transcription_active;
                    state.started_at_ms.get_or_insert(now_ms);
                    state.degraded = error.clone();
                    new_capture
                } else {
                    false
                };
                self.capture_gap_registry.start(
                    &session_id,
                    now_ms,
                    requested_live_transcription,
                    live_transcription_active,
                    new_capture,
                );
                CaptureLifecycleEvent::Started {
                    session_id,
                    requested_live_transcription,
                    live_transcription_active,
                    degraded: error,
                }
            }
            anlg_transcription_core::listener::SessionLifecycleEvent::Finalizing { session_id } => {
                CaptureLifecycleEvent::Finalizing { session_id }
            }
            anlg_transcription_core::listener::SessionLifecycleEvent::Inactive {
                session_id,
                audio_path,
                error,
            } => {
                let (snapshot, started_at_ms) = self
                    .session_state_cache
                    .lock()
                    .map(|mut cache| {
                        let started_at_ms =
                            cache.get(&session_id).and_then(|state| state.started_at_ms);
                        (cache.remove(&session_id), started_at_ms)
                    })
                    .unwrap_or((None, None));
                let stopped_at_ms = crate::capture_gaps::unix_now_ms();
                self.capture_gap_registry
                    .stopped(&session_id, stopped_at_ms);
                let duration_seconds = started_at_ms
                    .map(|started_at_ms| {
                        stopped_at_ms.saturating_sub(started_at_ms).max(0) as f64 / 1_000.0
                    })
                    .unwrap_or_default();
                let (requested_live_transcription, live_transcription_active) = snapshot
                    .as_ref()
                    .map(|state| {
                        (
                            state.requested_live_transcription,
                            state.live_transcription_active,
                        )
                    })
                    .unwrap_or((false, false));
                let mic_isolated = snapshot.and_then(|state| state.mic_isolated);
                if let Ok(mut isolation) = self.mic_isolation_cache.lock() {
                    match mic_isolated {
                        Some(value) => {
                            isolation.insert(session_id.clone(), value);
                        }
                        None => {
                            isolation.remove(&session_id);
                        }
                    }
                }
                crate::voiceprint::persist_mic_isolation(&self.app, &session_id, mic_isolated);
                self.stopped_capture_registry.record(StoppedCapture {
                    session_id: session_id.clone(),
                    stopped_at_ms,
                    duration_seconds,
                    chunked_audio: true,
                    audio_path: audio_path.clone(),
                    requested_live_transcription,
                    live_transcription_active,
                    error: error.clone(),
                });

                CaptureLifecycleEvent::Stopped {
                    session_id,
                    stopped_at_ms,
                    chunked_audio: true,
                    audio_path,
                    requested_live_transcription,
                    live_transcription_active,
                    error,
                }
            }
        };

        if let Err(error) = capture_event.emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_lifecycle_event");
        }
    }

    fn emit_progress(&self, event: anlg_transcription_core::listener::SessionProgressEvent) {
        if let anlg_transcription_core::listener::SessionProgressEvent::Connected {
            session_id,
            ..
        } = &event
        {
            self.capture_gap_registry
                .connected(session_id, crate::capture_gaps::unix_now_ms());
        }
        if let Err(error) = CaptureStatusEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_progress_event");
        }
    }

    fn emit_error(&self, event: anlg_transcription_core::listener::SessionErrorEvent) {
        match &event {
            anlg_transcription_core::listener::SessionErrorEvent::ConnectionError {
                session_id,
                ..
            } => self
                .capture_gap_registry
                .interrupted(session_id, crate::capture_gaps::unix_now_ms()),
            anlg_transcription_core::listener::SessionErrorEvent::AudioError {
                session_id,
                error,
                ..
            } if error.starts_with("audio_storage_") => self
                .capture_gap_registry
                .storage_failed(session_id, crate::capture_gaps::unix_now_ms()),
            _ => {}
        }
        update_audio_cleanup_status(&self.audio_cleanup_status, &event);
        if let Err(error) = CaptureStatusEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_error_event");
        }
    }

    fn emit_data(&self, event: anlg_transcription_core::listener::SessionDataEvent) {
        match &event {
            anlg_transcription_core::listener::SessionDataEvent::TranscriptDelta {
                session_id,
                delta,
            } => {
                if (!delta.new_words.is_empty() || !delta.replaced_ids.is_empty())
                    && let Some(registry) = self
                        .app
                        .try_state::<crate::live_journal::LiveJournalRegistry>()
                {
                    registry.append(session_id, delta.as_ref().clone());
                }
            }
            anlg_transcription_core::listener::SessionDataEvent::TranscriptSegmentDelta {
                session_id,
                delta,
            } => {
                if let Ok(mut cache) = self.session_state_cache.lock() {
                    let state = cache
                        .entry(session_id.clone())
                        .or_insert_with(SessionStateSnapshot::default);
                    apply_segment_delta(&mut state.live_segments, delta);
                }
            }
            anlg_transcription_core::listener::SessionDataEvent::MicIsolated {
                session_id,
                value,
            } => {
                if let Ok(mut cache) = self.session_state_cache.lock() {
                    let state = cache
                        .entry(session_id.clone())
                        .or_insert_with(SessionStateSnapshot::default);
                    state.mic_isolated = Some(merge_mic_isolation(state.mic_isolated, *value));
                }
            }
            _ => {}
        }

        if let Err(error) = CaptureDataEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_data_event");
        }
    }
}

/// A recording is isolated only if every stream was. Unplugging headphones mid-meeting flips it
/// to false for good, so no far-end speech can be mistaken for the local user later.
fn merge_mic_isolation(previous: Option<bool>, value: bool) -> bool {
    previous.unwrap_or(true) && value
}

fn apply_segment_delta(
    segments: &mut Vec<anlg_transcription_core::listener::LiveTranscriptSegment>,
    delta: &anlg_transcription_core::listener::LiveTranscriptSegmentDelta,
) {
    let changed_ids = delta
        .removed_ids
        .iter()
        .map(String::as_str)
        .chain(delta.upserts.iter().map(|segment| segment.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    segments.retain(|segment| !changed_ids.contains(segment.id.as_str()));
    segments.extend(delta.upserts.iter().cloned());
    segments.sort_by(|left, right| {
        left.start_ms
            .cmp(&right.start_ms)
            .then_with(|| left.end_ms.cmp(&right.end_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    if segments.len() > LIVE_SEGMENT_SNAPSHOT_LIMIT {
        segments.drain(0..segments.len() - LIVE_SEGMENT_SNAPSHOT_LIMIT);
    }
}

async fn current_root_state() -> RootState {
    let Some(cell) = registry::where_is(RootActor::name()) else {
        return RootState::Inactive;
    };

    let actor: ActorRef<RootMsg> = cell.into();
    call_t!(actor, RootMsg::GetState, 100).unwrap_or(RootState::Inactive)
}

#[cfg(test)]
mod tests {
    use anlg_transcript::{ChannelProfile, SegmentKey};
    use anlg_transcription_core::listener::{LiveTranscriptSegment, LiveTranscriptSegmentDelta};

    use super::{LIVE_SEGMENT_SNAPSHOT_LIMIT, apply_segment_delta, merge_mic_isolation};

    #[test]
    fn mic_isolation_sticks_to_false_once_any_stream_was_shared() {
        assert!(merge_mic_isolation(None, true));
        assert!(!merge_mic_isolation(None, false));
        assert!(merge_mic_isolation(Some(true), true));
        assert!(!merge_mic_isolation(Some(true), false));
        assert!(!merge_mic_isolation(Some(false), true));
    }

    fn segment(id: &str, start_ms: i64) -> LiveTranscriptSegment {
        LiveTranscriptSegment {
            id: id.to_string(),
            key: SegmentKey {
                channel: ChannelProfile::DirectMic,
                speaker_index: None,
                speaker_human_id: None,
            },
            start_ms,
            end_ms: start_ms + 100,
            text: id.to_string(),
            words: Vec::new(),
        }
    }

    #[test]
    fn reconstructs_the_latest_segment_snapshot_from_deltas() {
        let mut segments = Vec::new();
        apply_segment_delta(
            &mut segments,
            &LiveTranscriptSegmentDelta {
                upserts: vec![segment("later", 200), segment("earlier", 100)],
                removed_ids: Vec::new(),
            },
        );
        apply_segment_delta(
            &mut segments,
            &LiveTranscriptSegmentDelta {
                upserts: vec![segment("later", 150)],
                removed_ids: vec!["earlier".to_string()],
            },
        );

        assert_eq!(segments, vec![segment("later", 150)]);
    }

    #[test]
    fn bounds_the_cached_segment_snapshot() {
        let mut segments = Vec::new();
        apply_segment_delta(
            &mut segments,
            &LiveTranscriptSegmentDelta {
                upserts: (0..LIVE_SEGMENT_SNAPSHOT_LIMIT + 5)
                    .map(|index| segment(&format!("segment-{index}"), index as i64))
                    .collect(),
                removed_ids: Vec::new(),
            },
        );

        assert_eq!(segments.len(), LIVE_SEGMENT_SNAPSHOT_LIMIT);
        assert_eq!(
            segments.first().map(|segment| segment.id.as_str()),
            Some("segment-5")
        );
    }
}

fn update_audio_cleanup_status(
    cache: &crate::AudioCleanupStatus,
    event: &anlg_transcription_core::listener::SessionErrorEvent,
) {
    let anlg_transcription_core::listener::SessionErrorEvent::AudioError {
        session_id, error, ..
    } = event
    else {
        return;
    };
    if !(error.starts_with("audio_deletion_failed:")
        || error.starts_with("audio_recovery_failed:")
        || error == "audio_deletion_completed"
        || error == "audio_recovery_completed")
    {
        return;
    }
    if let Ok(mut cache) = cache.0.lock() {
        cache.insert(session_id.clone(), error.clone());
    }
}

#[cfg(test)]
mod cleanup_status_tests {
    use super::*;
    #[test]
    fn acknowledged_status_is_evicted_without_losing_other_or_newer_failures() {
        let cache: crate::AudioCleanupStatus = Default::default();
        cache.0.lock().unwrap().extend([
            ("completed".into(), "audio_deletion_completed".into()),
            ("failed".into(), "audio_recovery_failed: denied".into()),
            (
                "unacknowledged".into(),
                "audio_deletion_failed: denied".into(),
            ),
        ]);
        cache
            .acknowledge("completed", "audio_deletion_completed")
            .unwrap();
        cache
            .acknowledge("failed", "audio_recovery_completed")
            .unwrap();
        assert_eq!(
            cache.0.lock().unwrap().get("failed").map(String::as_str),
            Some("audio_recovery_failed: denied")
        );
        cache
            .acknowledge("failed", "audio_recovery_failed: denied")
            .unwrap();
        let status = cache.0.lock().unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(
            status.get("unacknowledged").map(String::as_str),
            Some("audio_deletion_failed: denied")
        );
    }

    #[test]
    fn cleanup_status_survives_until_the_frontend_subscribes() {
        let cache: crate::AudioCleanupStatus = Default::default();
        for error in [
            "audio_deletion_failed: denied",
            "audio_deletion_completed",
            "audio_recovery_failed: denied",
            "audio_recovery_completed",
        ] {
            update_audio_cleanup_status(
                &cache,
                &anlg_transcription_core::listener::SessionErrorEvent::AudioError {
                    session_id: "session".into(),
                    error: error.into(),
                    device: None,
                    is_fatal: false,
                },
            );
            assert_eq!(
                cache.0.lock().unwrap().get("session").map(String::as_str),
                Some(error)
            );
        }
    }
}
