use anlg_transcription_core::listener::ListenerRuntime;
use ractor::{ActorRef, call_t, registry};
use tauri_plugin_settings::SettingsPluginExt;
use tauri_specta::Event;

use crate::{
    CaptureDataEvent, CaptureLifecycleEvent, CaptureStatusEvent, SessionStateCache,
    SessionStateSnapshot,
};
use anlg_transcription_core::listener::State as RootState;
use anlg_transcription_core::listener::actors::{RootActor, RootMsg};

const LIVE_SEGMENT_SNAPSHOT_LIMIT: usize = 200;

pub struct TauriRuntime {
    pub app: tauri::AppHandle,
    pub session_state_cache: SessionStateCache,
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
                let requested_live_transcription = requested_transcription_mode
                    == anlg_transcription_core::listener::TranscriptionMode::Live;
                let live_transcription_active = current_transcription_mode
                    == anlg_transcription_core::listener::TranscriptionMode::Live;
                if let Ok(mut cache) = self.session_state_cache.lock() {
                    let state = cache.entry(session_id.clone()).or_default();
                    state.requested_live_transcription = requested_live_transcription;
                    state.live_transcription_active = live_transcription_active;
                }
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
                let (requested_live_transcription, live_transcription_active) = self
                    .session_state_cache
                    .lock()
                    .ok()
                    .and_then(|mut cache| cache.remove(&session_id))
                    .map(|state| {
                        (
                            state.requested_live_transcription,
                            state.live_transcription_active,
                        )
                    })
                    .unwrap_or((false, false));

                CaptureLifecycleEvent::Stopped {
                    session_id,
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
        if let Err(error) = CaptureStatusEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_progress_event");
        }
    }

    fn emit_error(&self, event: anlg_transcription_core::listener::SessionErrorEvent) {
        if let Err(error) = CaptureStatusEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_error_event");
        }
    }

    fn emit_data(&self, event: anlg_transcription_core::listener::SessionDataEvent) {
        if let anlg_transcription_core::listener::SessionDataEvent::TranscriptSegmentDelta {
            session_id,
            delta,
        } = &event
            && let Ok(mut cache) = self.session_state_cache.lock()
        {
            let state = cache
                .entry(session_id.clone())
                .or_insert_with(SessionStateSnapshot::default);
            apply_segment_delta(&mut state.live_segments, delta);
        }

        if let Err(error) = CaptureDataEvent::from(event).emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_data_event");
        }
    }
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

#[cfg(test)]
mod tests {
    use anlg_transcript::{ChannelProfile, SegmentKey};
    use anlg_transcription_core::listener::{LiveTranscriptSegment, LiveTranscriptSegmentDelta};

    use super::{LIVE_SEGMENT_SNAPSHOT_LIMIT, apply_segment_delta};

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

async fn current_root_state() -> RootState {
    let Some(cell) = registry::where_is(RootActor::name()) else {
        return RootState::Inactive;
    };

    let actor: ActorRef<RootMsg> = cell.into();
    call_t!(actor, RootMsg::GetState, 100).unwrap_or(RootState::Inactive)
}
