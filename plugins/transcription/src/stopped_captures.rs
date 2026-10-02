use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};

use crate::api::{CaptureRecovery, StoppedCapture};

#[derive(Clone, Default)]
pub struct StoppedCaptureRegistry(Arc<StdMutex<HashMap<String, StoppedCapture>>>);

pub(crate) fn merge_capture_recoveries(
    stopped_session_ids: impl IntoIterator<Item = String>,
    active_session_id: Option<String>,
    finalizing_session_ids: impl IntoIterator<Item = String>,
    marker_session_ids: impl IntoIterator<Item = String>,
) -> Vec<CaptureRecovery> {
    fn add(
        recoveries: &mut Vec<CaptureRecovery>,
        indexes: &mut HashMap<String, usize>,
        session_id: String,
        process_stopped: bool,
    ) {
        if session_id.is_empty() {
            return;
        }

        if let Some(index) = indexes.get(&session_id) {
            recoveries[*index].process_stopped |= process_stopped;
            return;
        }

        indexes.insert(session_id.clone(), recoveries.len());
        recoveries.push(CaptureRecovery {
            session_id,
            process_stopped,
        });
    }

    let mut recoveries = Vec::new();
    let mut indexes = HashMap::new();

    for session_id in stopped_session_ids {
        add(&mut recoveries, &mut indexes, session_id, true);
    }
    if let Some(session_id) = active_session_id {
        add(&mut recoveries, &mut indexes, session_id, false);
    }
    for session_id in finalizing_session_ids {
        add(&mut recoveries, &mut indexes, session_id, false);
    }
    for session_id in marker_session_ids {
        add(&mut recoveries, &mut indexes, session_id, false);
    }

    recoveries
}

impl StoppedCaptureRegistry {
    pub fn record(&self, stopped_capture: StoppedCapture) {
        let mut captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let should_replace = captures
            .get(&stopped_capture.session_id)
            .is_none_or(|current| stopped_capture.stopped_at_ms >= current.stopped_at_ms);
        if should_replace {
            captures.insert(stopped_capture.session_id.clone(), stopped_capture);
        }
    }

    pub fn get(&self, session_id: &str) -> Option<StoppedCapture> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(session_id)
            .cloned()
    }

    pub fn list(&self) -> Vec<StoppedCapture> {
        let mut captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect::<Vec<_>>();
        captures.sort_by_key(|capture| (capture.stopped_at_ms, capture.session_id.clone()));
        captures
    }

    pub fn acknowledge(&self, session_id: &str, stopped_at_ms: i64) {
        let mut captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if captures
            .get(session_id)
            .is_some_and(|capture| capture.stopped_at_ms == stopped_at_ms)
        {
            captures.remove(session_id);
        }
    }

    pub fn clear_session(&self, session_id: &str) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_recoveries_keep_first_seen_order_and_stopped_status() {
        let recoveries = merge_capture_recoveries(
            ["shared", "stopped-only"].map(str::to_string),
            Some("shared".to_string()),
            ["finalizing-only"].map(str::to_string),
            ["shared", "marker-only"].map(str::to_string),
        );

        assert_eq!(
            recoveries,
            vec![
                CaptureRecovery {
                    session_id: "shared".to_string(),
                    process_stopped: true,
                },
                CaptureRecovery {
                    session_id: "stopped-only".to_string(),
                    process_stopped: true,
                },
                CaptureRecovery {
                    session_id: "finalizing-only".to_string(),
                    process_stopped: false,
                },
                CaptureRecovery {
                    session_id: "marker-only".to_string(),
                    process_stopped: false,
                },
            ]
        );
    }

    fn stopped_capture(stopped_at_ms: i64) -> StoppedCapture {
        StoppedCapture {
            session_id: "session".to_string(),
            stopped_at_ms,
            duration_seconds: 12.0,
            chunked_audio: true,
            audio_path: Some("/tmp/session.wav".to_string()),
            requested_live_transcription: true,
            live_transcription_active: true,
            error: None,
        }
    }

    #[test]
    fn stale_acknowledgement_keeps_newer_stop_and_matching_ack_removes_it() {
        let registry = StoppedCaptureRegistry::default();
        registry.record(stopped_capture(100));
        registry.record(stopped_capture(200));

        registry.acknowledge("session", 100);
        assert_eq!(registry.get("session"), Some(stopped_capture(200)));

        registry.acknowledge("session", 200);
        assert_eq!(registry.get("session"), None);
    }
}
