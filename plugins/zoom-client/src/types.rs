use anlg_meeting_capture::{
    ActiveSpeakers, BotState, CaptureEventPayload, Participant, TerminalReason,
};
use serde::{Deserialize, Serialize};
use specta::Type;

pub const SIDECAR_NAME: &str = "char-sidecar-zoom";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ZoomClientState {
    Idle,
    Launching,
    WaitingForAdmission,
    Joined,
    Capturing,
    Stopping,
    Ended,
}

impl From<BotState> for ZoomClientState {
    fn from(state: BotState) -> Self {
        match state {
            BotState::Queued | BotState::Launching => Self::Launching,
            BotState::WaitingForAdmission => Self::WaitingForAdmission,
            BotState::Joined => Self::Joined,
            BotState::Capturing => Self::Capturing,
            BotState::Stopping => Self::Stopping,
            BotState::Completed | BotState::Failed | BotState::Canceled => Self::Ended,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ZoomParticipant {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

impl From<Participant> for ZoomParticipant {
    fn from(participant: Participant) -> Self {
        Self {
            id: participant.id,
            display_name: participant.display_name,
            email: participant.email,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ZoomActiveSpeakers {
    pub at_ms: u64,
    pub participant_ids: Vec<String>,
}

impl From<ActiveSpeakers> for ZoomActiveSpeakers {
    fn from(active: ActiveSpeakers) -> Self {
        Self {
            at_ms: active.at_ms,
            participant_ids: active.participant_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ZoomMeetingStatus {
    pub session_id: Option<String>,
    pub state: ZoomClientState,
    pub participants: Vec<ZoomParticipant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ZoomClientEvent {
    StateChanged {
        session_id: String,
        state: ZoomClientState,
        reason: Option<String>,
    },
    ParticipantUpserted {
        session_id: String,
        participant: ZoomParticipant,
    },
    ParticipantLeft {
        session_id: String,
        participant_id: String,
    },
    ActiveSpeakers {
        session_id: String,
        speakers: ZoomActiveSpeakers,
    },
    Error {
        session_id: String,
        message: String,
    },
}

impl ZoomClientEvent {
    pub fn from_capture(session_id: &str, payload: CaptureEventPayload) -> Option<Self> {
        let session_id = session_id.to_string();
        match payload {
            CaptureEventPayload::Lifecycle(transition) => Some(Self::StateChanged {
                session_id,
                state: transition.to.into(),
                reason: transition.reason.map(terminal_reason_text),
            }),
            CaptureEventPayload::ParticipantUpserted(participant) => {
                Some(Self::ParticipantUpserted {
                    session_id,
                    participant: participant.into(),
                })
            }
            CaptureEventPayload::ParticipantLeft { participant_id } => {
                Some(Self::ParticipantLeft {
                    session_id,
                    participant_id,
                })
            }
            CaptureEventPayload::ActiveSpeakers(active) => Some(Self::ActiveSpeakers {
                session_id,
                speakers: active.into(),
            }),
            CaptureEventPayload::Transcript(_)
            | CaptureEventPayload::SpeakerUpserted(_)
            | CaptureEventPayload::RecordingChunkReady(_) => None,
        }
    }
}

fn terminal_reason_text(reason: TerminalReason) -> String {
    let kind = serde_json::to_value(reason.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{:?}", reason.kind));
    match reason.message {
        Some(message) if !message.is_empty() => format!("{kind}: {message}"),
        _ => kind,
    }
}
