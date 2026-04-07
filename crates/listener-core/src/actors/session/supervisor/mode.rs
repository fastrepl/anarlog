use ractor::ActorCell;

use crate::actors::source::ListenerRouting;
use crate::{DegradedError, SessionLifecycleEvent, TranscriptionMode};

#[derive(Debug, Clone, Copy)]
pub(super) struct SessionModeState {
    requested_transcription_mode: TranscriptionMode,
    current_transcription_mode: TranscriptionMode,
    listener_buffering_enabled: bool,
}

impl SessionModeState {
    pub(super) fn new(requested_transcription_mode: TranscriptionMode) -> Self {
        Self {
            requested_transcription_mode,
            current_transcription_mode: requested_transcription_mode,
            listener_buffering_enabled: requested_transcription_mode == TranscriptionMode::Live,
        }
    }

    pub(super) fn should_spawn_listener(&self) -> bool {
        self.requested_transcription_mode == TranscriptionMode::Live
    }

    pub(super) fn on_listener_attached(&mut self) {
        self.current_transcription_mode = TranscriptionMode::Live;
    }

    pub(super) fn enter_batch_fallback(&mut self) {
        self.current_transcription_mode = TranscriptionMode::Batch;
        self.listener_buffering_enabled = false;
    }

    pub(super) fn listener_routing(&self, listener_cell: Option<&ActorCell>) -> ListenerRouting {
        if let Some(cell) = listener_cell {
            ListenerRouting::Attached(cell.clone().into())
        } else if self.listener_buffering_enabled {
            ListenerRouting::Buffering
        } else {
            ListenerRouting::Dropped
        }
    }

    pub(super) fn active_event(
        &self,
        session_id: String,
        error: Option<DegradedError>,
    ) -> SessionLifecycleEvent {
        SessionLifecycleEvent::Active {
            session_id,
            requested_transcription_mode: self.requested_transcription_mode,
            current_transcription_mode: self.current_transcription_mode,
            error,
        }
    }
}

pub(super) fn classify_connection_failure(base_url: &str) -> String {
    if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
        "Local transcription server is not running".to_string()
    } else {
        format!("Cannot reach transcription server at {}", base_url)
    }
}

pub(super) fn parse_degraded_reason(reason: Option<&String>) -> DegradedError {
    reason
        .and_then(|r| serde_json::from_str::<DegradedError>(r).ok())
        .unwrap_or_else(|| DegradedError::StreamError {
            message: reason
                .cloned()
                .unwrap_or_else(|| "listener terminated without reason".to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_degraded_reason_uses_json_payload() {
        let reason = serde_json::to_string(&DegradedError::ConnectionTimeout).unwrap();
        let parsed = parse_degraded_reason(Some(&reason));
        assert!(matches!(parsed, DegradedError::ConnectionTimeout));
    }

    #[test]
    fn parse_degraded_reason_falls_back_for_missing_reason() {
        let parsed = parse_degraded_reason(None);
        assert!(matches!(parsed, DegradedError::StreamError { .. }));
    }

    #[test]
    fn parse_degraded_reason_falls_back_for_invalid_json() {
        let reason = "not-json".to_string();
        let parsed = parse_degraded_reason(Some(&reason));
        assert!(matches!(parsed, DegradedError::StreamError { .. }));
    }

    #[test]
    fn batch_mode_starts_with_dropped_listener_routing() {
        let state = SessionModeState::new(TranscriptionMode::Batch);
        assert!(matches!(
            state.listener_routing(None),
            ListenerRouting::Dropped
        ));
    }

    #[test]
    fn entering_batch_fallback_disables_buffering() {
        let mut state = SessionModeState::new(TranscriptionMode::Live);
        state.enter_batch_fallback();

        assert_eq!(state.current_transcription_mode, TranscriptionMode::Batch);
        assert!(matches!(
            state.listener_routing(None),
            ListenerRouting::Dropped
        ));
    }
}
