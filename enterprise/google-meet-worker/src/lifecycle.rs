use std::time::Instant;

use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, LifecycleTransition, ProviderMetadata,
    TerminalReason, TerminalReasonKind, TransitionError,
};
use chrono::{DateTime, Utc};

use crate::{
    AdmissionClassifier, AdmissionOutcome, AdmissionRejectionReason, AdmissionSnapshot,
    RuntimeClassifier, RuntimeOutcome, RuntimeSnapshot,
};

#[derive(Debug)]
pub struct WorkerLifecycle {
    bot_id: String,
    state: BotState,
    next_sequence: u64,
    admission: AdmissionClassifier,
    runtime: RuntimeClassifier,
}

impl WorkerLifecycle {
    pub fn new(bot_id: impl Into<String>) -> Self {
        Self {
            bot_id: bot_id.into(),
            state: BotState::Queued,
            next_sequence: 0,
            admission: AdmissionClassifier::default(),
            runtime: RuntimeClassifier::default(),
        }
    }

    pub fn state(&self) -> BotState {
        self.state
    }

    pub fn launch_started(
        &mut self,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        self.transition(BotState::Launching, None, occurred_at)
    }

    pub fn observe_admission(
        &mut self,
        snapshot: &AdmissionSnapshot,
        observed_at: Instant,
        occurred_at: DateTime<Utc>,
    ) -> Result<Option<CaptureEvent>, TransitionError> {
        let outcome = self.admission.classify(snapshot, observed_at);
        match outcome {
            AdmissionOutcome::WaitingForAdmission
            | AdmissionOutcome::ConsentRequired
            | AdmissionOutcome::CaptchaChallenge { .. }
                if self.state == BotState::Launching =>
            {
                self.transition(BotState::WaitingForAdmission, None, occurred_at)
                    .map(Some)
            }
            AdmissionOutcome::Admitted
                if matches!(
                    self.state,
                    BotState::Launching | BotState::WaitingForAdmission
                ) =>
            {
                self.transition(BotState::Joined, None, occurred_at)
                    .map(Some)
            }
            AdmissionOutcome::Rejected(rejection)
                if matches!(
                    self.state,
                    BotState::Launching | BotState::WaitingForAdmission
                ) =>
            {
                let kind = match rejection.reason {
                    AdmissionRejectionReason::HostDenied => TerminalReasonKind::AdmissionDenied,
                    AdmissionRejectionReason::CaptchaUnsolved => {
                        TerminalReasonKind::AuthenticationFailed
                    }
                    AdmissionRejectionReason::ErrorPage => TerminalReasonKind::ProviderError,
                };
                self.transition(
                    BotState::Failed,
                    Some(TerminalReason {
                        kind,
                        message: Some(rejection.indicator),
                        retryable: false,
                    }),
                    occurred_at,
                )
                .map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn capture_started(
        &mut self,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        self.transition(BotState::Capturing, None, occurred_at)
    }

    pub fn admission_timed_out(
        &mut self,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        self.transition(
            BotState::Failed,
            Some(TerminalReason {
                kind: TerminalReasonKind::AdmissionTimeout,
                message: Some("Google Meet host did not admit the bot before the deadline".into()),
                retryable: true,
            }),
            occurred_at,
        )
    }

    pub fn observe_runtime(
        &mut self,
        snapshot: &RuntimeSnapshot,
        observed_at: Instant,
        occurred_at: DateTime<Utc>,
    ) -> Result<Option<CaptureEvent>, TransitionError> {
        if !matches!(self.state, BotState::Joined | BotState::Capturing) {
            return Ok(None);
        }
        match self.runtime.classify(snapshot, observed_at) {
            RuntimeOutcome::Removed(indicator) => self
                .transition(
                    BotState::Failed,
                    Some(TerminalReason {
                        kind: TerminalReasonKind::RemovedFromMeeting,
                        message: Some(indicator),
                        retryable: false,
                    }),
                    occurred_at,
                )
                .map(Some),
            RuntimeOutcome::MeetingEnded(indicator) => self
                .transition(
                    BotState::Completed,
                    Some(TerminalReason {
                        kind: TerminalReasonKind::MeetingEnded,
                        message: Some(indicator),
                        retryable: false,
                    }),
                    occurred_at,
                )
                .map(Some),
            RuntimeOutcome::NetworkLost(indicator) => self
                .transition(
                    BotState::Failed,
                    Some(TerminalReason {
                        kind: TerminalReasonKind::NetworkLost,
                        message: Some(indicator),
                        retryable: true,
                    }),
                    occurred_at,
                )
                .map(Some),
            RuntimeOutcome::StateLost => self
                .transition(
                    BotState::Failed,
                    Some(TerminalReason {
                        kind: TerminalReasonKind::ProviderError,
                        message: Some(
                            "Google Meet runtime indicators disappeared beyond the grace period"
                                .into(),
                        ),
                        retryable: true,
                    }),
                    occurred_at,
                )
                .map(Some),
            RuntimeOutcome::Active
            | RuntimeOutcome::ConnectionInterrupted { .. }
            | RuntimeOutcome::Unknown { .. } => Ok(None),
        }
    }

    pub fn stopped_by_request(
        &mut self,
        occurred_at: DateTime<Utc>,
    ) -> Result<Vec<CaptureEvent>, TransitionError> {
        let stopping = self.transition(BotState::Stopping, None, occurred_at)?;
        let completed = self.transition(
            BotState::Completed,
            Some(TerminalReason {
                kind: TerminalReasonKind::StoppedByRequest,
                message: None,
                retryable: false,
            }),
            occurred_at,
        )?;
        Ok(vec![stopping, completed])
    }

    pub fn transition(
        &mut self,
        next: BotState,
        reason: Option<TerminalReason>,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        let transition: LifecycleTransition = self.state.transition_to(next, reason)?;
        self.state = next;
        Ok(self.emit_payload(CaptureEventPayload::Lifecycle(transition), occurred_at))
    }

    pub fn emit_payload(
        &mut self,
        payload: CaptureEventPayload,
        occurred_at: DateTime<Utc>,
    ) -> CaptureEvent {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        CaptureEvent {
            id: format!("capture-event-{sequence}"),
            bot_id: self.bot_id.clone(),
            sequence,
            occurred_at,
            payload,
            metadata: ProviderMetadata::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn emits_the_normalized_join_and_capture_path() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");

        let launching = lifecycle.launch_started(now()).unwrap();
        let waiting = lifecycle
            .observe_admission(
                &AdmissionSnapshot {
                    waiting_room_visible: true,
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap()
            .unwrap();
        let joined = lifecycle
            .observe_admission(
                &AdmissionSnapshot {
                    participant_tile_labels: vec!["Ada Lovelace".into()],
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap()
            .unwrap();
        let capturing = lifecycle.capture_started(now()).unwrap();

        assert_eq!(launching.sequence, 0);
        assert_eq!(waiting.sequence, 1);
        assert_eq!(joined.sequence, 2);
        assert_eq!(capturing.sequence, 3);
        assert_eq!(lifecycle.state(), BotState::Capturing);
    }

    #[test]
    fn shares_one_sequence_across_lifecycle_and_capture_payloads() {
        use anlg_meeting_capture::TranscriptSegment;

        let mut lifecycle = WorkerLifecycle::new("bot-1");
        let launching = lifecycle.launch_started(now()).unwrap();
        let transcript = lifecycle.emit_payload(
            CaptureEventPayload::Transcript(TranscriptSegment {
                id: "segment-1".into(),
                sequence: 1,
                start_ms: 0,
                end_ms: Some(100),
                text: "hello".into(),
                speaker: None,
                is_final: true,
            }),
            now(),
        );

        assert_eq!(launching.sequence, 0);
        assert_eq!(transcript.sequence, 1);
        assert_eq!(transcript.id, "capture-event-1");
        assert!(
            transcript.id.len() <= 128
                && transcript
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        );
    }

    #[test]
    fn host_denial_is_terminal_and_non_retryable() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(now()).unwrap();

        let event = lifecycle
            .observe_admission(
                &AdmissionSnapshot {
                    explicit_denial_indicator: Some("denied your request".into()),
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap()
            .unwrap();

        let CaptureEventPayload::Lifecycle(transition) = event.payload else {
            panic!("expected lifecycle event")
        };
        assert_eq!(transition.to, BotState::Failed);
        assert_eq!(
            transition.reason.unwrap().kind,
            TerminalReasonKind::AdmissionDenied
        );
    }

    #[test]
    fn repeated_waiting_observations_do_not_duplicate_transitions() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(now()).unwrap();
        let snapshot = AdmissionSnapshot {
            waiting_room_visible: true,
            ..Default::default()
        };

        assert!(
            lifecycle
                .observe_admission(&snapshot, Instant::now(), now())
                .unwrap()
                .is_some()
        );
        assert!(
            lifecycle
                .observe_admission(&snapshot, Instant::now(), now())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn admission_timeout_is_terminal_and_retry_honest() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(now()).unwrap();

        let event = lifecycle.admission_timed_out(now()).unwrap();
        let CaptureEventPayload::Lifecycle(transition) = event.payload else {
            panic!("expected lifecycle event")
        };
        let reason = transition.reason.unwrap();
        assert_eq!(reason.kind, TerminalReasonKind::AdmissionTimeout);
        assert!(reason.retryable);
    }

    #[test]
    fn removal_and_meeting_end_map_to_distinct_terminal_reasons() {
        for (snapshot, expected_state, expected_reason) in [
            (
                RuntimeSnapshot {
                    removal_indicator: Some("you were removed".into()),
                    ..Default::default()
                },
                BotState::Failed,
                TerminalReasonKind::RemovedFromMeeting,
            ),
            (
                RuntimeSnapshot {
                    meeting_ended_indicator: Some("meeting ended".into()),
                    ..Default::default()
                },
                BotState::Completed,
                TerminalReasonKind::MeetingEnded,
            ),
        ] {
            let mut lifecycle = WorkerLifecycle::new("bot-1");
            lifecycle.launch_started(now()).unwrap();
            lifecycle
                .observe_admission(
                    &AdmissionSnapshot {
                        self_name_nodes: 1,
                        ..Default::default()
                    },
                    Instant::now(),
                    now(),
                )
                .unwrap();
            lifecycle.capture_started(now()).unwrap();

            let event = lifecycle
                .observe_runtime(&snapshot, Instant::now(), now())
                .unwrap()
                .unwrap();
            let CaptureEventPayload::Lifecycle(transition) = event.payload else {
                panic!("expected lifecycle event")
            };
            assert_eq!(transition.to, expected_state);
            assert_eq!(transition.reason.unwrap().kind, expected_reason);
        }
    }

    #[test]
    fn requested_stop_emits_stopping_before_completed() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(now()).unwrap();
        lifecycle
            .observe_admission(
                &AdmissionSnapshot {
                    self_name_nodes: 1,
                    ..Default::default()
                },
                Instant::now(),
                now(),
            )
            .unwrap();
        lifecycle.capture_started(now()).unwrap();

        let events = lifecycle.stopped_by_request(now()).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(lifecycle.state(), BotState::Completed);
    }
}
