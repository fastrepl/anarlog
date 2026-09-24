use anlg_meeting_capture::{ActiveSpeakers, Participant};
use serde::{Deserialize, Serialize};
use specta::Type;

pub const WINDOW_LABEL: &str = "meet-client";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum MeetClientState {
    Idle,
    Launching,
    WaitingForAdmission,
    Capturing,
    Stopping,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MeetParticipant {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

impl From<MeetParticipant> for Participant {
    fn from(participant: MeetParticipant) -> Self {
        Self {
            id: participant.id,
            display_name: participant.display_name,
            email: participant.email,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MeetActiveSpeakers {
    pub at_ms: u64,
    pub participant_ids: Vec<String>,
}

impl From<MeetActiveSpeakers> for ActiveSpeakers {
    fn from(active: MeetActiveSpeakers) -> Self {
        Self {
            at_ms: active.at_ms,
            participant_ids: active.participant_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MeetMeetingStatus {
    pub session_id: Option<String>,
    pub state: MeetClientState,
    pub participants: Vec<MeetParticipant>,
}

/// DOM selectors the injected observer uses to read the Meet web client. Google
/// ships obfuscated class names that rotate, so these are overridable at runtime
/// (`set_selectors`) instead of baked into the binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetSelectors {
    /// Elements that represent one participant each; must carry `participantIdAttribute`.
    pub tile: String,
    pub participant_id_attribute: String,
    /// Elements inside a tile whose text (or `nameAttribute`) is the display name.
    pub name: String,
    pub name_attribute: String,
    /// Speaking indicator inside a tile; considered active unless it has `silentClass`.
    pub speaking_indicator: String,
    pub silent_class: String,
    /// Present only while in the meeting (not on the lobby / "Ready to join" page).
    pub in_meeting: String,
    /// Present once the meeting has ended or the user has left.
    pub ended: String,
    pub poll_ms: u32,
}

impl Default for MeetSelectors {
    fn default() -> Self {
        Self {
            tile: "[data-participant-id]".into(),
            participant_id_attribute: "data-participant-id".into(),
            name: "[data-self-name], [data-tooltip]".into(),
            name_attribute: "data-self-name".into(),
            speaking_indicator: "[jscontroller][class*='gjg47c'], [jscontroller] > div[class] > div[class] > div[class]".into(),
            silent_class: "gjg47c".into(),
            in_meeting: "[data-participant-id], button[aria-label*='Leave call' i]".into(),
            ended: "[data-call-ended], a[href*='landing?']".into(),
            poll_ms: 250,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ObservedParticipant {
    pub id: String,
    pub display_name: Option<String>,
    pub speaking: bool,
}

/// One snapshot of the Meet DOM, posted by the injected observer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetObservation {
    pub in_meeting: bool,
    pub ended: bool,
    pub participants: Vec<ObservedParticipant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MeetClientEvent {
    StateChanged {
        session_id: String,
        state: MeetClientState,
        reason: Option<String>,
    },
    ParticipantUpserted {
        session_id: String,
        participant: MeetParticipant,
    },
    ParticipantLeft {
        session_id: String,
        participant_id: String,
    },
    ActiveSpeakers {
        session_id: String,
        speakers: MeetActiveSpeakers,
    },
    Error {
        session_id: String,
        message: String,
    },
}
