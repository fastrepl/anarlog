use std::time::Instant;

use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, LifecycleTransition, ProviderMetadata,
    TerminalReason, TerminalReasonKind, TransitionError,
};
use chrono::{DateTime, Utc};

use crate::{AdmissionClassifier, AdmissionOutcome, AdmissionRejectionReason, AdmissionSnapshot};

#[derive(Debug)]
pub struct WorkerLifecycle {
    bot_id: String,
    state: BotState,
    next_sequence: u64,
    admission: AdmissionClassifier,
}

impl WorkerLifecycle {
    pub fn new(bot_id: impl Into<String>) -> Self {
        Self {
            bot_id: bot_id.into(),
            state: BotState::Queued,
            next_sequence: 1,
            admission: AdmissionClassifier::default(),
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

    pub fn transition(
        &mut self,
        next: BotState,
        reason: Option<TerminalReason>,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, TransitionError> {
        let transition: LifecycleTransition = self.state.transition_to(next, reason)?;
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.state = next;
        Ok(CaptureEvent {
            id: format!("{}:lifecycle:{sequence}", self.bot_id),
            bot_id: self.bot_id.clone(),
            sequence,
            occurred_at,
            payload: CaptureEventPayload::Lifecycle(transition),
            metadata: ProviderMetadata::default(),
        })
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

        assert_eq!(launching.sequence, 1);
        assert_eq!(waiting.sequence, 2);
        assert_eq!(joined.sequence, 3);
        assert_eq!(capturing.sequence, 4);
        assert_eq!(lifecycle.state(), BotState::Capturing);
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
}
