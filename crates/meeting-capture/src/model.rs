use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{BotState, LifecycleTransition, ProviderMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingPlatform {
    GoogleMeet,
    Zoom,
    MicrosoftTeams,
    Webex,
    Jitsi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureProviderKind {
    Anarlog,
    Recall,
    ZoomRtms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingReference {
    pub platform: MeetingPlatform,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartCaptureRequest {
    pub meeting: MeetingReference,
    pub bot_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: ProviderMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingBot {
    pub id: String,
    pub provider: CaptureProviderKind,
    pub meeting: MeetingReference,
    pub state: BotState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: ProviderMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Speaker {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub sequence: u64,
    pub start_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u64>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Speaker>,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingChunk {
    pub id: String,
    pub sequence: u64,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub content_type: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub id: String,
    pub bot_id: String,
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub payload: CaptureEventPayload,
    #[serde(default)]
    pub metadata: ProviderMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum CaptureEventPayload {
    Lifecycle(LifecycleTransition),
    Transcript(TranscriptSegment),
    SpeakerUpserted(Speaker),
    ParticipantUpserted(Participant),
    ParticipantLeft { participant_id: String },
    RecordingChunkReady(RecordingChunk),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TerminalReason, TerminalReasonKind};

    #[test]
    fn serializes_a_normalized_lifecycle_event() {
        let transition = BotState::Capturing
            .transition_to(
                BotState::Completed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::MeetingEnded,
                    message: None,
                    retryable: false,
                }),
            )
            .unwrap();
        let event = CaptureEvent {
            id: "event-1".into(),
            bot_id: "bot-1".into(),
            sequence: 4,
            occurred_at: DateTime::parse_from_rfc3339("2026-08-14T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            payload: CaptureEventPayload::Lifecycle(transition),
            metadata: ProviderMetadata::default(),
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "id": "event-1",
                "bot_id": "bot-1",
                "sequence": 4,
                "occurred_at": "2026-08-14T00:00:00Z",
                "payload": {
                    "type": "lifecycle",
                    "data": {
                        "from": "capturing",
                        "to": "completed",
                        "reason": {
                            "kind": "meeting_ended",
                            "retryable": false
                        }
                    }
                },
                "metadata": {}
            })
        );
    }
}
