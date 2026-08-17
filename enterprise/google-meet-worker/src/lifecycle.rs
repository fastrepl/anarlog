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

    pub fn resume(
        bot_id: impl Into<String>,
        state: BotState,
        next_sequence: u64,
    ) -> Result<Self, WorkerLifecycleResumeError> {
        if (state == BotState::Queued) != (next_sequence == 0) {
            return Err(WorkerLifecycleResumeError {
                state,
                next_sequence,
            });
        }
        Ok(Self {
            bot_id: bot_id.into(),
            state,
            next_sequence,
            admission: AdmissionClassifier::default(),
            runtime: RuntimeClassifier::default(),
        })
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

    pub fn worker_exited(
        &mut self,
        message: impl Into<String>,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        self.transition(
            BotState::Failed,
            Some(TerminalReason {
                kind: TerminalReasonKind::WorkerExited,
                message: Some(message.into()),
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
        let Some(outcome) = self.classify_runtime(snapshot, observed_at) else {
            return Ok(None);
        };
        self.apply_runtime_outcome(outcome, occurred_at).map(Some)
    }

    pub(crate) fn classify_runtime(
        &mut self,
        snapshot: &RuntimeSnapshot,
        observed_at: Instant,
    ) -> Option<RuntimeOutcome> {
        if !matches!(self.state, BotState::Joined | BotState::Capturing) {
            return None;
        }
        match self.runtime.classify(snapshot, observed_at) {
            RuntimeOutcome::Active
            | RuntimeOutcome::ConnectionInterrupted { .. }
            | RuntimeOutcome::Unknown { .. } => None,
            outcome => Some(outcome),
        }
    }

    pub(crate) fn apply_runtime_outcome(
        &mut self,
        outcome: RuntimeOutcome,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        match outcome {
            RuntimeOutcome::Removed(indicator) => self.transition(
                BotState::Failed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::RemovedFromMeeting,
                    message: Some(indicator),
                    retryable: false,
                }),
                occurred_at,
            ),
            RuntimeOutcome::MeetingEnded(indicator) => self.transition(
                BotState::Completed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::MeetingEnded,
                    message: Some(indicator),
                    retryable: false,
                }),
                occurred_at,
            ),
            RuntimeOutcome::NetworkLost(indicator) => self.transition(
                BotState::Failed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::NetworkLost,
                    message: Some(indicator),
                    retryable: true,
                }),
                occurred_at,
            ),
            RuntimeOutcome::StateLost => self.transition(
                BotState::Failed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::ProviderError,
                    message: Some(
                        "Google Meet runtime indicators disappeared beyond the grace period".into(),
                    ),
                    retryable: true,
                }),
                occurred_at,
            ),
            RuntimeOutcome::Active
            | RuntimeOutcome::ConnectionInterrupted { .. }
            | RuntimeOutcome::Unknown { .. } => {
                unreachable!("non-terminal runtime outcomes are not applied")
            }
        }
    }

    pub fn stopped_by_request(
        &mut self,
        occurred_at: DateTime<Utc>,
    ) -> Result<Vec<CaptureEvent>, TransitionError> {
        let reason = || TerminalReason {
            kind: TerminalReasonKind::StoppedByRequest,
            message: None,
            retryable: false,
        };
        match self.state {
            BotState::Queued => self
                .transition(BotState::Canceled, Some(reason()), occurred_at)
                .map(|event| vec![event]),
            BotState::Launching
            | BotState::WaitingForAdmission
            | BotState::Joined
            | BotState::Capturing => {
                let stopping = self.transition(BotState::Stopping, None, occurred_at)?;
                let completed =
                    self.transition(BotState::Completed, Some(reason()), occurred_at)?;
                Ok(vec![stopping, completed])
            }
            BotState::Stopping => self
                .transition(BotState::Completed, Some(reason()), occurred_at)
                .map(|event| vec![event]),
            BotState::Completed | BotState::Failed | BotState::Canceled => Ok(Vec::new()),
        }
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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("capture checkpoint state {state:?} is inconsistent with next sequence {next_sequence}")]
pub struct WorkerLifecycleResumeError {
    pub state: BotState,
    pub next_sequence: u64,
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
    fn resumes_at_the_durable_sequence_and_can_terminalize_an_interrupted_worker() {
        let mut lifecycle = WorkerLifecycle::resume("bot-1", BotState::Capturing, 7).unwrap();

        let event = lifecycle.worker_exited("worker restarted", now()).unwrap();
        let CaptureEventPayload::Lifecycle(transition) = event.payload else {
            panic!("expected lifecycle event")
        };

        assert_eq!(event.sequence, 7);
        assert_eq!(event.id, "capture-event-7");
        assert_eq!(transition.from, BotState::Capturing);
        assert_eq!(transition.to, BotState::Failed);
        assert_eq!(
            transition.reason.unwrap().kind,
            TerminalReasonKind::WorkerExited
        );
    }

    #[test]
    fn rejects_inconsistent_durable_sequence_origins() {
        assert!(WorkerLifecycle::resume("bot-1", BotState::Queued, 1).is_err());
        assert!(WorkerLifecycle::resume("bot-1", BotState::Launching, 0).is_err());
    }

    #[test]
    fn stop_is_phase_aware_before_launch_and_during_cleanup() {
        let mut queued = WorkerLifecycle::new("bot-queued");
        let canceled = queued.stopped_by_request(now()).unwrap();
        assert_eq!(canceled.len(), 1);
        assert_eq!(queued.state(), BotState::Canceled);

        let mut stopping = WorkerLifecycle::resume("bot-stopping", BotState::Stopping, 4).unwrap();
        let completed = stopping.stopped_by_request(now()).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(stopping.state(), BotState::Completed);
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
