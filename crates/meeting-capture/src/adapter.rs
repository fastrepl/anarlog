use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind, CaptureWorkerCheckpoint,
    CaptureWorkerCheckpointError, MeetingPlatform, Participant, ProviderMetadata, RecordingChunk,
    Speaker, TerminalReason, TranscriptSegment, TransitionError,
};

pub const MEETING_SDK_BRIDGE_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MeetingSdkBridgeCommand {
    Start(MeetingSdkBridgeStart),
    Stop { job_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingSdkBridgeStart {
    pub protocol_version: u16,
    pub checkpoint: CaptureWorkerCheckpoint,
    pub bot_name: String,
}

impl MeetingSdkBridgeStart {
    pub fn new(
        checkpoint: CaptureWorkerCheckpoint,
        bot_name: impl Into<String>,
    ) -> Result<Self, MeetingSdkBridgeError> {
        let start = Self {
            protocol_version: MEETING_SDK_BRIDGE_PROTOCOL_VERSION,
            checkpoint,
            bot_name: bot_name.into(),
        };
        start.validate()?;
        Ok(start)
    }

    pub fn validate(&self) -> Result<(), MeetingSdkBridgeError> {
        if self.protocol_version != MEETING_SDK_BRIDGE_PROTOCOL_VERSION {
            return Err(MeetingSdkBridgeError::UnsupportedProtocolVersion(
                self.protocol_version,
            ));
        }
        self.checkpoint.validate()?;
        validate_bridge_provider(self.checkpoint.provider, self.checkpoint.meeting.platform)?;
        validate_bridge_meeting_url(
            &self.checkpoint.meeting.url,
            self.checkpoint.meeting.platform,
        )?;
        if self.bot_name.is_empty()
            || self.bot_name.chars().count() > 80
            || self.bot_name.chars().any(char::is_control)
        {
            return Err(MeetingSdkBridgeError::InvalidBotName);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingSdkBridgeEvent {
    pub protocol_version: u16,
    pub sequence: u64,
    pub platform: MeetingPlatform,
    pub provider: CaptureProviderKind,
    pub payload: MeetingSdkBridgeEventPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MeetingSdkBridgeEventPayload {
    Ready,
    WaitingForAdmission,
    Joined,
    Capturing,
    Transcript(MeetingSdkBridgeTranscript),
    ParticipantUpserted(Participant),
    ParticipantLeft { participant_id: String },
    RecordingChunkReady(RecordingChunk),
    Terminal(MeetingSdkBridgeTerminal),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingSdkBridgeTranscript {
    pub start_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u64>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Speaker>,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingSdkBridgeTerminal {
    pub state: BotState,
    pub reason: TerminalReason,
}

pub struct MeetingSdkBridgeNormalizer {
    bot_id: String,
    platform: MeetingPlatform,
    provider: CaptureProviderKind,
    state: BotState,
    next_event_sequence: u64,
    next_bridge_sequence: u64,
}

impl MeetingSdkBridgeNormalizer {
    pub fn new(checkpoint: &CaptureWorkerCheckpoint) -> Result<Self, MeetingSdkBridgeError> {
        checkpoint.validate()?;
        validate_bridge_provider(checkpoint.provider, checkpoint.meeting.platform)?;
        validate_bridge_meeting_url(&checkpoint.meeting.url, checkpoint.meeting.platform)?;
        Ok(Self {
            bot_id: checkpoint.bot_id.clone(),
            platform: checkpoint.meeting.platform,
            provider: checkpoint.provider,
            state: checkpoint.state,
            next_event_sequence: checkpoint.next_sequence,
            next_bridge_sequence: 0,
        })
    }

    pub fn state(&self) -> BotState {
        self.state
    }

    pub fn accept(
        &mut self,
        event: MeetingSdkBridgeEvent,
        occurred_at: DateTime<Utc>,
    ) -> Result<CaptureEvent, MeetingSdkBridgeError> {
        if event.protocol_version != MEETING_SDK_BRIDGE_PROTOCOL_VERSION {
            return Err(MeetingSdkBridgeError::UnsupportedProtocolVersion(
                event.protocol_version,
            ));
        }
        if event.platform != self.platform || event.provider != self.provider {
            return Err(MeetingSdkBridgeError::IdentityMismatch);
        }
        if event.sequence != self.next_bridge_sequence {
            return Err(MeetingSdkBridgeError::SequenceMismatch {
                expected: self.next_bridge_sequence,
                actual: event.sequence,
            });
        }

        let payload = match event.payload {
            MeetingSdkBridgeEventPayload::Ready => self.transition(BotState::Launching, None)?,
            MeetingSdkBridgeEventPayload::WaitingForAdmission => {
                self.transition(BotState::WaitingForAdmission, None)?
            }
            MeetingSdkBridgeEventPayload::Joined => self.transition(BotState::Joined, None)?,
            MeetingSdkBridgeEventPayload::Capturing => {
                self.transition(BotState::Capturing, None)?
            }
            MeetingSdkBridgeEventPayload::Transcript(transcript) => {
                if self.state != BotState::Capturing
                    || transcript.text.trim().is_empty()
                    || transcript.text.len() > 64 * 1024
                    || transcript
                        .end_ms
                        .is_some_and(|end_ms| end_ms < transcript.start_ms)
                {
                    return Err(MeetingSdkBridgeError::InvalidTranscript);
                }
                CaptureEventPayload::Transcript(TranscriptSegment {
                    id: format!("sdk-bridge-segment-{}", self.next_event_sequence),
                    sequence: self.next_event_sequence,
                    start_ms: transcript.start_ms,
                    end_ms: transcript.end_ms,
                    text: transcript.text,
                    speaker: transcript.speaker,
                    is_final: transcript.is_final,
                })
            }
            MeetingSdkBridgeEventPayload::ParticipantUpserted(participant) => {
                require_capturing(self.state)?;
                CaptureEventPayload::ParticipantUpserted(participant)
            }
            MeetingSdkBridgeEventPayload::ParticipantLeft { participant_id } => {
                require_capturing(self.state)?;
                CaptureEventPayload::ParticipantLeft { participant_id }
            }
            MeetingSdkBridgeEventPayload::RecordingChunkReady(chunk) => {
                require_capturing(self.state)?;
                CaptureEventPayload::RecordingChunkReady(chunk)
            }
            MeetingSdkBridgeEventPayload::Terminal(terminal) => {
                if !terminal.state.is_terminal() {
                    return Err(MeetingSdkBridgeError::InvalidTerminalState(terminal.state));
                }
                self.transition(terminal.state, Some(terminal.reason))?
            }
        };

        let sequence = self.next_event_sequence;
        self.next_event_sequence = self
            .next_event_sequence
            .checked_add(1)
            .ok_or(MeetingSdkBridgeError::SequenceExhausted)?;
        self.next_bridge_sequence = self
            .next_bridge_sequence
            .checked_add(1)
            .ok_or(MeetingSdkBridgeError::SequenceExhausted)?;
        Ok(CaptureEvent {
            id: format!("capture-event-{sequence}"),
            bot_id: self.bot_id.clone(),
            sequence,
            occurred_at,
            payload,
            metadata: ProviderMetadata::default(),
        })
    }

    fn transition(
        &mut self,
        next: BotState,
        reason: Option<TerminalReason>,
    ) -> Result<CaptureEventPayload, TransitionError> {
        let transition = self.state.transition_to(next, reason)?;
        self.state = next;
        Ok(CaptureEventPayload::Lifecycle(transition))
    }
}

fn validate_bridge_provider(
    provider: CaptureProviderKind,
    platform: MeetingPlatform,
) -> Result<(), MeetingSdkBridgeError> {
    if !matches!(
        (provider, platform),
        (
            CaptureProviderKind::MicrosoftGraph,
            MeetingPlatform::MicrosoftTeams
        ) | (
            CaptureProviderKind::WebexMeetingsSdk,
            MeetingPlatform::Webex
        )
    ) {
        return Err(MeetingSdkBridgeError::UnsupportedProvider { provider, platform });
    }
    Ok(())
}

fn validate_bridge_meeting_url(
    value: &str,
    platform: MeetingPlatform,
) -> Result<(), MeetingSdkBridgeError> {
    let Some(remainder) = value.strip_prefix("https://") else {
        return Err(MeetingSdkBridgeError::InvalidMeetingUrl { platform });
    };
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains(['@', ':', '%'])
        || value.chars().any(char::is_whitespace)
    {
        return Err(MeetingSdkBridgeError::InvalidMeetingUrl { platform });
    }
    let host = authority.to_ascii_lowercase();
    let matches_platform = match platform {
        MeetingPlatform::MicrosoftTeams => {
            matches!(host.as_str(), "teams.microsoft.com" | "teams.live.com")
        }
        MeetingPlatform::Webex => host == "webex.com" || host.ends_with(".webex.com"),
        _ => false,
    };
    if !matches_platform {
        return Err(MeetingSdkBridgeError::InvalidMeetingUrl { platform });
    }
    Ok(())
}

fn require_capturing(state: BotState) -> Result<(), MeetingSdkBridgeError> {
    if state != BotState::Capturing {
        return Err(MeetingSdkBridgeError::PayloadBeforeCapture(state));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MeetingSdkBridgeError {
    #[error(transparent)]
    InvalidCheckpoint(#[from] CaptureWorkerCheckpointError),
    #[error("meeting SDK bridge bot name must contain 1-80 non-control characters")]
    InvalidBotName,
    #[error("provider {provider:?} on {platform:?} does not use the meeting SDK bridge")]
    UnsupportedProvider {
        provider: CaptureProviderKind,
        platform: MeetingPlatform,
    },
    #[error("meeting URL is not a canonical HTTPS URL for {platform:?}")]
    InvalidMeetingUrl { platform: MeetingPlatform },
    #[error("meeting SDK bridge protocol version {0} is unsupported")]
    UnsupportedProtocolVersion(u16),
    #[error("meeting SDK bridge platform or provider does not match the capture job")]
    IdentityMismatch,
    #[error("meeting SDK bridge sequence mismatch: expected {expected}, received {actual}")]
    SequenceMismatch { expected: u64, actual: u64 },
    #[error("meeting SDK bridge sequence was exhausted")]
    SequenceExhausted,
    #[error("meeting SDK bridge transcript is invalid for the current state")]
    InvalidTranscript,
    #[error("meeting SDK bridge emitted capture payload while state was {0:?}")]
    PayloadBeforeCapture(BotState),
    #[error("meeting SDK bridge terminal state {0:?} is not terminal")]
    InvalidTerminalState(BotState),
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeetingReference, TerminalReasonKind};

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn checkpoint(
        provider: CaptureProviderKind,
        platform: MeetingPlatform,
    ) -> CaptureWorkerCheckpoint {
        let url = match platform {
            MeetingPlatform::MicrosoftTeams => "https://teams.microsoft.com/l/meetup-join/test",
            MeetingPlatform::Webex => "https://anarlog.webex.com/meet/test",
            _ => "https://example.com/meeting",
        };
        CaptureWorkerCheckpoint {
            job_id: "job-1".into(),
            bot_id: "bot-1".into(),
            provider,
            meeting: MeetingReference {
                platform,
                url: url.into(),
                external_id: None,
                calendar_event_id: None,
            },
            state: BotState::Queued,
            next_sequence: 0,
        }
    }

    fn event(
        sequence: u64,
        platform: MeetingPlatform,
        provider: CaptureProviderKind,
        payload: MeetingSdkBridgeEventPayload,
    ) -> MeetingSdkBridgeEvent {
        MeetingSdkBridgeEvent {
            protocol_version: MEETING_SDK_BRIDGE_PROTOCOL_VERSION,
            sequence,
            platform,
            provider,
            payload,
        }
    }

    #[test]
    fn normalizes_teams_and_webex_sdk_lifecycles() {
        for (provider, platform) in [
            (
                CaptureProviderKind::MicrosoftGraph,
                MeetingPlatform::MicrosoftTeams,
            ),
            (
                CaptureProviderKind::WebexMeetingsSdk,
                MeetingPlatform::Webex,
            ),
        ] {
            let checkpoint = checkpoint(provider, platform);
            MeetingSdkBridgeStart::new(checkpoint.clone(), "Anarlog Notetaker").unwrap();
            let mut normalizer = MeetingSdkBridgeNormalizer::new(&checkpoint).unwrap();
            let payloads = [
                MeetingSdkBridgeEventPayload::Ready,
                MeetingSdkBridgeEventPayload::Joined,
                MeetingSdkBridgeEventPayload::Capturing,
                MeetingSdkBridgeEventPayload::Transcript(MeetingSdkBridgeTranscript {
                    start_ms: 10,
                    end_ms: Some(510),
                    text: "Shared notes are ready".into(),
                    speaker: None,
                    is_final: true,
                }),
                MeetingSdkBridgeEventPayload::Terminal(MeetingSdkBridgeTerminal {
                    state: BotState::Completed,
                    reason: TerminalReason {
                        kind: TerminalReasonKind::MeetingEnded,
                        message: None,
                        retryable: false,
                    },
                }),
            ];

            let events = payloads
                .into_iter()
                .enumerate()
                .map(|(sequence, payload)| {
                    normalizer
                        .accept(event(sequence as u64, platform, provider, payload), now())
                        .unwrap()
                })
                .collect::<Vec<_>>();

            assert_eq!(normalizer.state(), BotState::Completed);
            assert_eq!(
                events
                    .iter()
                    .map(|event| event.sequence)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2, 3, 4]
            );
            assert!(matches!(
                events[3].payload,
                CaptureEventPayload::Transcript(_)
            ));
        }
    }

    #[test]
    fn rejects_cross_platform_and_out_of_order_bridge_events() {
        let checkpoint = checkpoint(
            CaptureProviderKind::MicrosoftGraph,
            MeetingPlatform::MicrosoftTeams,
        );
        let mut normalizer = MeetingSdkBridgeNormalizer::new(&checkpoint).unwrap();
        assert!(matches!(
            normalizer.accept(
                event(
                    0,
                    MeetingPlatform::Webex,
                    CaptureProviderKind::WebexMeetingsSdk,
                    MeetingSdkBridgeEventPayload::Ready,
                ),
                now(),
            ),
            Err(MeetingSdkBridgeError::IdentityMismatch)
        ));
        assert!(matches!(
            normalizer.accept(
                event(
                    1,
                    MeetingPlatform::MicrosoftTeams,
                    CaptureProviderKind::MicrosoftGraph,
                    MeetingSdkBridgeEventPayload::Ready,
                ),
                now(),
            ),
            Err(MeetingSdkBridgeError::SequenceMismatch { .. })
        ));

        let unsafe_checkpoint = CaptureWorkerCheckpoint {
            meeting: MeetingReference {
                url: "https://teams.microsoft.com.evil.example/l/meetup-join/test".into(),
                ..checkpoint.meeting.clone()
            },
            ..checkpoint.clone()
        };
        assert!(matches!(
            MeetingSdkBridgeStart::new(unsafe_checkpoint, "Anarlog Notetaker"),
            Err(MeetingSdkBridgeError::InvalidMeetingUrl { .. })
        ));

        let mut start = MeetingSdkBridgeStart::new(checkpoint, "Anarlog Notetaker").unwrap();
        start.protocol_version += 1;
        assert!(matches!(
            start.validate(),
            Err(MeetingSdkBridgeError::UnsupportedProtocolVersion(_))
        ));
    }
}
