use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotState {
    Queued,
    Launching,
    WaitingForAdmission,
    Joined,
    Capturing,
    Stopping,
    Completed,
    Failed,
    Canceled,
}

impl BotState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }

    pub fn transition_to(
        self,
        next: Self,
        reason: Option<TerminalReason>,
    ) -> Result<LifecycleTransition, TransitionError> {
        if !self.allows(next) {
            return Err(TransitionError::Invalid {
                from: self,
                to: next,
            });
        }
        if next.is_terminal() && reason.is_none() {
            return Err(TransitionError::MissingTerminalReason { state: next });
        }
        if !next.is_terminal() && reason.is_some() {
            return Err(TransitionError::UnexpectedTerminalReason { state: next });
        }

        Ok(LifecycleTransition {
            from: self,
            to: next,
            reason,
        })
    }

    fn allows(self, next: Self) -> bool {
        match self {
            Self::Queued => matches!(next, Self::Launching | Self::Canceled | Self::Failed),
            Self::Launching => matches!(
                next,
                Self::WaitingForAdmission
                    | Self::Joined
                    | Self::Stopping
                    | Self::Canceled
                    | Self::Failed
            ),
            Self::WaitingForAdmission => matches!(
                next,
                Self::Joined | Self::Stopping | Self::Canceled | Self::Failed
            ),
            Self::Joined => matches!(
                next,
                Self::Capturing | Self::Stopping | Self::Completed | Self::Failed
            ),
            Self::Capturing => {
                matches!(next, Self::Stopping | Self::Completed | Self::Failed)
            }
            Self::Stopping => matches!(next, Self::Completed | Self::Failed | Self::Canceled),
            Self::Completed | Self::Failed | Self::Canceled => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub from: BotState,
    pub to: BotState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<TerminalReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalReason {
    pub kind: TerminalReasonKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalReasonKind {
    MeetingEnded,
    StoppedByRequest,
    AdmissionDenied,
    AdmissionTimeout,
    NoOneJoined,
    EveryoneLeft,
    RemovedFromMeeting,
    RecordingPermissionDenied,
    InvalidMeeting,
    AuthenticationFailed,
    CapacityExceeded,
    NetworkLost,
    ProviderError,
    WorkerExited,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("invalid meeting bot state transition from {from:?} to {to:?}")]
    Invalid { from: BotState, to: BotState },
    #[error("terminal state {state:?} requires a terminal reason")]
    MissingTerminalReason { state: BotState },
    #[error("non-terminal state {state:?} cannot carry a terminal reason")]
    UnexpectedTerminalReason { state: BotState },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting_ended() -> TerminalReason {
        TerminalReason {
            kind: TerminalReasonKind::MeetingEnded,
            message: None,
            retryable: false,
        }
    }

    #[test]
    fn accepts_the_capture_happy_path() {
        let states = [
            BotState::Queued,
            BotState::Launching,
            BotState::WaitingForAdmission,
            BotState::Joined,
            BotState::Capturing,
            BotState::Completed,
        ];

        for pair in states.windows(2) {
            let reason = pair[1].is_terminal().then(meeting_ended);
            pair[0].transition_to(pair[1], reason).unwrap();
        }
    }

    #[test]
    fn rejects_skipping_from_queued_to_capturing() {
        assert_eq!(
            BotState::Queued
                .transition_to(BotState::Capturing, None)
                .unwrap_err(),
            TransitionError::Invalid {
                from: BotState::Queued,
                to: BotState::Capturing,
            }
        );
    }

    #[test]
    fn rejects_terminal_state_without_reason() {
        assert_eq!(
            BotState::Capturing
                .transition_to(BotState::Completed, None)
                .unwrap_err(),
            TransitionError::MissingTerminalReason {
                state: BotState::Completed,
            }
        );
    }

    #[test]
    fn rejects_transitions_out_of_terminal_states() {
        assert!(matches!(
            BotState::Failed.transition_to(BotState::Launching, None),
            Err(TransitionError::Invalid { .. })
        ));
    }
}
