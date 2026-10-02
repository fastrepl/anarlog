use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex, MutexGuard},
};

use crate::api::{CaptureAudioGap, CaptureAudioGaps};

const MAX_CAPTURE_GAPS: usize = 128;
const MAX_CAPTURE_GAP_SESSIONS: usize = 32;

#[derive(Clone, Default)]
pub struct CaptureGapRegistry(Arc<StdMutex<HashMap<String, Ledger>>>);

#[derive(Clone)]
struct Ledger {
    capture_started_at_ms: i64,
    gaps: Vec<(i64, i64)>,
    open_gap_started_at_ms: Option<i64>,
    awaiting_connection: bool,
    storage_failed: bool,
    confirmed_through_ms: Option<i64>,
}

impl Ledger {
    fn new(capture_started_at_ms: i64) -> Self {
        Self {
            capture_started_at_ms,
            gaps: Vec::new(),
            open_gap_started_at_ms: None,
            awaiting_connection: false,
            storage_failed: false,
            confirmed_through_ms: None,
        }
    }

    fn open_gap(&mut self) {
        self.open_gap_started_at_ms.get_or_insert_with(|| {
            self.capture_started_at_ms.saturating_add(
                self.confirmed_through_ms
                    .unwrap_or_default()
                    .saturating_sub(1_000)
                    .max(0),
            )
        });
    }

    fn close_gap(&mut self, end_ms: i64) {
        let Some(start_ms) = self.open_gap_started_at_ms.take() else {
            return;
        };
        if end_ms <= start_ms {
            return;
        }

        if let Some((last_start_ms, last_end_ms)) = self.gaps.last_mut()
            && start_ms <= *last_end_ms
            && end_ms >= *last_start_ms
        {
            *last_start_ms = (*last_start_ms).min(start_ms);
            *last_end_ms = (*last_end_ms).max(end_ms);
        } else {
            self.gaps.push((start_ms, end_ms));
        }

        if self.gaps.len() > MAX_CAPTURE_GAPS {
            let first_start_ms = self.gaps.first().map(|gap| gap.0).unwrap_or_default();
            let last_end_ms = self.gaps.last().map(|gap| gap.1).unwrap_or_default();
            self.gaps.clear();
            self.gaps.push((first_start_ms, last_end_ms));
        }
    }
}

impl CaptureGapRegistry {
    fn ledgers(&self) -> MutexGuard<'_, HashMap<String, Ledger>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn start(
        &self,
        session_id: &str,
        now_ms: i64,
        requested_live: bool,
        live_active: bool,
        new_capture: bool,
    ) {
        let mut ledgers = self.ledgers();
        if new_capture || !ledgers.contains_key(session_id) {
            ledgers.insert(session_id.to_string(), Ledger::new(now_ms));
            while ledgers.len() > MAX_CAPTURE_GAP_SESSIONS {
                let oldest_session_id = ledgers
                    .iter()
                    .min_by_key(|(_, ledger)| ledger.capture_started_at_ms)
                    .map(|(session_id, _)| session_id.clone());
                let Some(oldest_session_id) = oldest_session_id else {
                    break;
                };
                ledgers.remove(&oldest_session_id);
            }
        }

        let Some(ledger) = ledgers.get_mut(session_id) else {
            return;
        };
        if live_active {
            ledger.close_gap(now_ms);
            ledger.awaiting_connection = false;
        } else if requested_live {
            ledger.open_gap();
            ledger.awaiting_connection = true;
        } else {
            ledger.open_gap();
            ledger.awaiting_connection = false;
        }
    }

    pub fn interrupted(&self, session_id: &str, _now_ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.open_gap();
            ledger.awaiting_connection = true;
        }
    }

    pub fn connected(&self, session_id: &str, now_ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.close_gap(now_ms);
            ledger.awaiting_connection = false;
        }
    }

    pub fn storage_failed(&self, session_id: &str, _now_ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.storage_failed = true;
            ledger.open_gap();
        }
    }

    pub fn persisted_through(&self, session_id: &str, ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.confirmed_through_ms = Some(
                ledger
                    .confirmed_through_ms
                    .map_or(ms, |confirmed_through_ms| confirmed_through_ms.max(ms)),
            );
        }
    }

    pub fn persistence_failed(&self, session_id: &str, now_ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.open_gap();
            ledger.close_gap(now_ms);
        }
    }

    pub fn stopped(&self, session_id: &str, now_ms: i64) {
        if let Some(ledger) = self.ledgers().get_mut(session_id) {
            ledger.close_gap(now_ms);
        }
    }

    pub fn get(&self, session_id: &str) -> Option<CaptureAudioGaps> {
        self.ledgers()
            .get(session_id)
            .map(|ledger| CaptureAudioGaps {
                capture_started_at_ms: ledger.capture_started_at_ms,
                gaps: ledger
                    .gaps
                    .iter()
                    .map(|(start_ms, end_ms)| CaptureAudioGap {
                        start_ms: *start_ms,
                        end_ms: *end_ms,
                    })
                    .collect(),
                open_gap_started_at_ms: ledger.open_gap_started_at_ms,
                awaiting_connection: ledger.awaiting_connection,
                storage_failed: ledger.storage_failed,
                confirmed_through_ms: ledger.confirmed_through_ms,
            })
    }
}

pub(crate) fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_events_close_the_confirmed_outage_interval() {
        let registry = CaptureGapRegistry::default();
        registry.start("session", 1_000_000, true, true, true);
        registry.persisted_through("session", 20_000);

        registry.interrupted("session", 1_030_000);
        assert!(registry.get("session").unwrap().awaiting_connection);

        registry.connected("session", 1_090_000);
        let gaps = registry.get("session").unwrap();
        assert_eq!(
            gaps.gaps,
            vec![CaptureAudioGap {
                start_ms: 1_019_000,
                end_ms: 1_090_000,
            }]
        );
        assert!(!gaps.awaiting_connection);
    }

    #[test]
    fn persistence_failures_merge_and_start_only_resets_new_captures() {
        let registry = CaptureGapRegistry::default();
        registry.start("session", 1_000_000, true, true, true);

        registry.persistence_failed("session", 1_000_010);
        registry.persistence_failed("session", 1_000_020);
        let gaps = registry.get("session").unwrap().gaps;
        assert_eq!(
            gaps,
            vec![CaptureAudioGap {
                start_ms: 1_000_000,
                end_ms: 1_000_020,
            }]
        );

        registry.start("session", 1_100_000, true, true, false);
        let retained = registry.get("session").unwrap();
        assert_eq!(retained.capture_started_at_ms, 1_000_000);
        assert_eq!(retained.gaps, gaps);

        registry.start("session", 1_200_000, true, true, true);
        let reset = registry.get("session").unwrap();
        assert_eq!(reset.capture_started_at_ms, 1_200_000);
        assert!(reset.gaps.is_empty());
    }

    #[test]
    fn stopped_closes_gaps_and_storage_failure_preserves_connection_state() {
        let registry = CaptureGapRegistry::default();
        registry.start("session", 1_000, true, false, true);
        registry.storage_failed("session", 2_000);

        let open = registry.get("session").unwrap();
        assert!(open.awaiting_connection);
        assert!(open.storage_failed);

        registry.stopped("session", 5_000);
        let stopped = registry.get("session").unwrap();
        assert!(stopped.awaiting_connection);
        assert!(stopped.storage_failed);
        assert_eq!(stopped.open_gap_started_at_ms, None);
        assert_eq!(
            stopped.gaps,
            vec![CaptureAudioGap {
                start_ms: 1_000,
                end_ms: 5_000,
            }]
        );
    }

    #[test]
    fn the_129th_disjoint_gap_collapses_the_gap_list() {
        let registry = CaptureGapRegistry::default();
        let capture_started_at_ms = 1_000_000;
        registry.start("session", capture_started_at_ms, true, true, true);

        for index in 0..129_i64 {
            let offset_ms = index * 2_000;
            registry.persisted_through("session", offset_ms + 1_000);
            registry.interrupted("session", capture_started_at_ms + offset_ms);
            registry.connected("session", capture_started_at_ms + offset_ms + 100);
        }

        let gaps = registry.get("session").unwrap().gaps;
        assert_eq!(
            gaps,
            vec![CaptureAudioGap {
                start_ms: capture_started_at_ms,
                end_ms: capture_started_at_ms + 128 * 2_000 + 100,
            }]
        );
    }
}
