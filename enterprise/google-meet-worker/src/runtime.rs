// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{CdpError, CdpPage};

pub const DEFAULT_CONNECTION_GRACE: Duration = Duration::from_secs(30);
pub const DEFAULT_RUNTIME_UNKNOWN_GRACE: Duration = Duration::from_secs(30);
pub const RUNTIME_PROBE_EXPRESSION: &str = include_str!("runtime_probe.js");

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub removal_indicator: Option<String>,
    pub meeting_ended_indicator: Option<String>,
    pub connection_problem_indicator: Option<String>,
    #[serde(default)]
    pub participant_tile_labels: Vec<String>,
    pub self_name_nodes: usize,
    pub visible_meeting_controls: usize,
}

impl RuntimeSnapshot {
    fn has_active_signal(&self) -> bool {
        self.self_name_nodes > 0
            || self.visible_meeting_controls > 0
            || self.participant_tile_labels.iter().any(|label| {
                let label = label.trim().to_lowercase();
                !label.is_empty()
                    && !label.contains("visual_effects")
                    && !label.contains("backgrounds and effects")
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeOutcome {
    Active,
    ConnectionInterrupted { remaining: Duration },
    Removed(String),
    MeetingEnded(String),
    NetworkLost(String),
    Unknown { remaining: Duration },
    StateLost,
}

#[derive(Debug)]
pub struct RuntimeClassifier {
    connection_grace: Duration,
    connection_problem_since: Option<Instant>,
    unknown_grace: Duration,
    unknown_since: Option<Instant>,
}

impl Default for RuntimeClassifier {
    fn default() -> Self {
        Self::with_grace(DEFAULT_CONNECTION_GRACE, DEFAULT_RUNTIME_UNKNOWN_GRACE)
    }
}

impl RuntimeClassifier {
    pub fn new(connection_grace: Duration) -> Self {
        Self::with_grace(connection_grace, DEFAULT_RUNTIME_UNKNOWN_GRACE)
    }

    pub fn with_grace(connection_grace: Duration, unknown_grace: Duration) -> Self {
        Self {
            connection_grace,
            connection_problem_since: None,
            unknown_grace,
            unknown_since: None,
        }
    }

    pub fn classify(&mut self, snapshot: &RuntimeSnapshot, now: Instant) -> RuntimeOutcome {
        if let Some(indicator) = snapshot.removal_indicator.as_ref() {
            self.connection_problem_since = None;
            self.unknown_since = None;
            return RuntimeOutcome::Removed(indicator.clone());
        }
        if let Some(indicator) = snapshot.meeting_ended_indicator.as_ref() {
            self.connection_problem_since = None;
            self.unknown_since = None;
            return RuntimeOutcome::MeetingEnded(indicator.clone());
        }
        if let Some(indicator) = snapshot.connection_problem_indicator.as_ref() {
            self.unknown_since = None;
            let since = *self.connection_problem_since.get_or_insert(now);
            let elapsed = now.saturating_duration_since(since);
            if elapsed < self.connection_grace {
                return RuntimeOutcome::ConnectionInterrupted {
                    remaining: self.connection_grace - elapsed,
                };
            }
            self.connection_problem_since = None;
            return RuntimeOutcome::NetworkLost(indicator.clone());
        }
        self.connection_problem_since = None;
        if snapshot.has_active_signal() {
            self.unknown_since = None;
            RuntimeOutcome::Active
        } else {
            let since = *self.unknown_since.get_or_insert(now);
            let elapsed = now.saturating_duration_since(since);
            if elapsed < self.unknown_grace {
                RuntimeOutcome::Unknown {
                    remaining: self.unknown_grace - elapsed,
                }
            } else {
                self.unknown_since = None;
                RuntimeOutcome::StateLost
            }
        }
    }
}

impl CdpPage {
    pub async fn probe_runtime(&mut self) -> Result<RuntimeSnapshot, CdpError> {
        self.evaluate(RUNTIME_PROBE_EXPRESSION).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_removal_wins_over_ended_and_connection_copy() {
        let mut classifier = RuntimeClassifier::default();
        assert_eq!(
            classifier.classify(
                &RuntimeSnapshot {
                    removal_indicator: Some("you were removed".into()),
                    meeting_ended_indicator: Some("meeting ended".into()),
                    connection_problem_indicator: Some("connection lost".into()),
                    ..Default::default()
                },
                Instant::now()
            ),
            RuntimeOutcome::Removed("you were removed".into())
        );
    }

    #[test]
    fn transient_reconnection_has_a_bounded_grace_period() {
        let grace = Duration::from_secs(30);
        let started = Instant::now();
        let snapshot = RuntimeSnapshot {
            connection_problem_indicator: Some("reconnecting".into()),
            ..Default::default()
        };
        let mut classifier = RuntimeClassifier::new(grace);

        assert_eq!(
            classifier.classify(&snapshot, started),
            RuntimeOutcome::ConnectionInterrupted { remaining: grace }
        );
        assert_eq!(
            classifier.classify(&snapshot, started + grace),
            RuntimeOutcome::NetworkLost("reconnecting".into())
        );
    }

    #[test]
    fn effects_preview_is_not_an_active_meeting_signal() {
        let mut classifier = RuntimeClassifier::default();
        let outcome = classifier.classify(
            &RuntimeSnapshot {
                participant_tile_labels: vec!["visual_effects Backgrounds and effects".into()],
                ..Default::default()
            },
            Instant::now(),
        );

        assert_eq!(
            outcome,
            RuntimeOutcome::Unknown {
                remaining: DEFAULT_RUNTIME_UNKNOWN_GRACE
            }
        );
    }

    #[test]
    fn unknown_runtime_state_cannot_poll_forever() {
        let grace = Duration::from_secs(30);
        let started = Instant::now();
        let mut classifier = RuntimeClassifier::with_grace(grace, grace);
        let snapshot = RuntimeSnapshot::default();

        assert!(matches!(
            classifier.classify(&snapshot, started),
            RuntimeOutcome::Unknown { remaining } if remaining == grace
        ));
        assert_eq!(
            classifier.classify(&snapshot, started + grace),
            RuntimeOutcome::StateLost
        );
    }
}
