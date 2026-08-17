use std::collections::BTreeMap;

use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind, Participant, RecordingChunk,
    Speaker,
};
use anlg_session_ingest::{
    IngestAttachment, IngestParticipant, IngestSession, IngestSpeakerHint, IngestTranscript,
    IngestWord, SESSION_INGEST_SCHEMA_VERSION, SessionIngestEnvelope,
};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::capture::CaptureJob;

pub fn project(
    job: &CaptureJob,
    events: &[CaptureEvent],
) -> Result<SessionIngestEnvelope, ProjectionError> {
    if events.is_empty() {
        return Err(ProjectionError::NoEvents);
    }

    let mut state = BotState::Queued;
    let mut started_at = None;
    let mut ended_at = None;
    let mut speakers = BTreeMap::<String, Speaker>::new();
    let mut participants = BTreeMap::<String, ProjectedParticipant>::new();
    let mut segments = BTreeMap::new();
    let mut chunks = BTreeMap::<String, RecordingChunk>::new();

    for (expected_sequence, event) in events.iter().enumerate() {
        let expected_sequence = expected_sequence as u64;
        if event.sequence != expected_sequence {
            return Err(ProjectionError::NonContiguousSequence {
                expected: expected_sequence,
                actual: event.sequence,
            });
        }
        if event.bot_id != job.bot_id {
            return Err(ProjectionError::WrongBot);
        }
        if state.is_terminal() {
            return Err(ProjectionError::EventAfterTerminal);
        }

        match &event.payload {
            CaptureEventPayload::Lifecycle(transition) => {
                if transition.from != state {
                    return Err(ProjectionError::StateMismatch);
                }
                let validated = state
                    .transition_to(transition.to, transition.reason.clone())
                    .map_err(ProjectionError::InvalidTransition)?;
                if validated != *transition {
                    return Err(ProjectionError::StateMismatch);
                }
                state = transition.to;
                if started_at.is_none() && matches!(state, BotState::Joined | BotState::Capturing) {
                    started_at = Some(event.occurred_at);
                }
                if state.is_terminal() {
                    ended_at = Some(event.occurred_at);
                }
            }
            CaptureEventPayload::Transcript(segment) => {
                segments.insert(segment.id.clone(), segment.clone());
            }
            CaptureEventPayload::SpeakerUpserted(speaker) => {
                speakers.insert(speaker.id.clone(), speaker.clone());
            }
            CaptureEventPayload::ParticipantUpserted(participant) => {
                participants
                    .entry(participant.id.clone())
                    .and_modify(|projected| {
                        projected.participant = participant.clone();
                        projected.last_seen_at = event.occurred_at;
                    })
                    .or_insert_with(|| ProjectedParticipant {
                        participant: participant.clone(),
                        first_seen_at: event.occurred_at,
                        last_seen_at: event.occurred_at,
                        left_at: None,
                    });
            }
            CaptureEventPayload::ParticipantLeft { participant_id } => {
                let participant = participants
                    .entry(participant_id.clone())
                    .or_insert_with(|| ProjectedParticipant {
                        participant: Participant {
                            id: participant_id.clone(),
                            display_name: None,
                            email: None,
                        },
                        first_seen_at: event.occurred_at,
                        last_seen_at: event.occurred_at,
                        left_at: None,
                    });
                participant.last_seen_at = event.occurred_at;
                participant.left_at = Some(event.occurred_at);
            }
            CaptureEventPayload::RecordingChunkReady(chunk) => {
                chunks.insert(chunk.id.clone(), chunk.clone());
            }
        }
    }

    let updated_at = events.last().expect("events are not empty").occurred_at;
    let finalized = state.is_terminal();
    let attachments = project_attachments(&chunks, updated_at);
    let transcripts =
        project_transcript(&job.session_id, &segments, &speakers, &chunks, updated_at);

    Ok(SessionIngestEnvelope {
        schema_version: SESSION_INGEST_SCHEMA_VERSION,
        source_id: job.job_id.clone(),
        revision: events.len() as u64,
        finalized,
        attach_to_existing: false,
        workspace_id: job.workspace_id.clone(),
        owner_user_id: job.owner_user_id.clone(),
        session: IngestSession {
            id: job.session_id.clone(),
            title: job.session_title.clone(),
            status: session_status(state).into(),
            created_at: timestamp(job.created_at),
            updated_at: timestamp(updated_at),
            started_at: started_at.map(timestamp).unwrap_or_default(),
            ended_at: ended_at.map(timestamp).unwrap_or_default(),
            timezone: String::new(),
            language: String::new(),
            event_id: job.meeting.calendar_event_id.clone().unwrap_or_default(),
            external_event_id: job.meeting.external_id.clone().unwrap_or_default(),
            external_provider: platform_name(job.meeting.platform).into(),
            series_id: String::new(),
            event: serde_json::to_value(&job.meeting).expect("meeting reference serializes"),
            metadata: session_metadata(job, state, events),
        },
        documents: vec![],
        transcripts,
        participants: participants
            .into_values()
            .map(project_participant)
            .collect(),
        attachments,
    })
}

pub fn content_hash(envelope: &SessionIngestEnvelope) -> Result<String, serde_json::Error> {
    let serialized = serde_json::to_vec(envelope)?;
    Ok(hex_digest(Sha256::digest(serialized)))
}

fn project_transcript(
    session_id: &str,
    segments: &BTreeMap<String, anlg_meeting_capture::TranscriptSegment>,
    speakers: &BTreeMap<String, Speaker>,
    chunks: &BTreeMap<String, RecordingChunk>,
    updated_at: DateTime<Utc>,
) -> Vec<IngestTranscript> {
    if segments.is_empty() {
        return vec![];
    }

    let mut ordered = segments.values().collect::<Vec<_>>();
    ordered.sort_by_key(|segment| (segment.sequence, segment.id.as_str()));
    let words = ordered
        .iter()
        .map(|segment| {
            let speaker = segment.speaker.as_ref().map(|speaker| {
                speakers
                    .get(&speaker.id)
                    .and_then(|known| known.display_name.clone())
                    .or_else(|| speaker.display_name.clone())
                    .unwrap_or_else(|| speaker.id.clone())
            });
            IngestWord {
                id: stable_id("word", &segment.id),
                text: segment.text.clone(),
                start_ms: saturating_i64(segment.start_ms),
                end_ms: saturating_i64(segment.end_ms.unwrap_or(segment.start_ms)),
                channel: 0,
                speaker,
                metadata: map([
                    ("captureSegmentId", json!(segment.id)),
                    ("captureSequence", json!(segment.sequence)),
                    ("final", json!(segment.is_final)),
                ]),
            }
        })
        .collect::<Vec<_>>();
    let speaker_hints = ordered
        .iter()
        .filter_map(|segment| {
            segment.speaker.as_ref().map(|speaker| IngestSpeakerHint {
                id: stable_id("speaker-hint", &segment.id),
                word_id: stable_id("word", &segment.id),
                kind: "capture_speaker_id".into(),
                value: speaker.id.clone(),
            })
        })
        .collect();
    let ended_at_ms = ordered
        .iter()
        .filter_map(|segment| segment.end_ms)
        .max()
        .map(saturating_i64);
    let first_chunk = chunks.values().min_by_key(|chunk| chunk.sequence);

    vec![IngestTranscript {
        id: stable_id("transcript", session_id),
        provider: "enterprise_capture".into(),
        model: String::new(),
        language: String::new(),
        started_at_ms: ordered
            .iter()
            .map(|segment| segment.start_ms)
            .min()
            .map(saturating_i64)
            .unwrap_or_default(),
        ended_at_ms,
        audio_attachment_id: first_chunk
            .map(|chunk| stable_id("attachment", &chunk.id))
            .unwrap_or_default(),
        memo: String::new(),
        words,
        speaker_hints,
        metadata: Map::new(),
        created_at: timestamp(updated_at),
        updated_at: timestamp(updated_at),
    }]
}

fn project_attachments(
    chunks: &BTreeMap<String, RecordingChunk>,
    updated_at: DateTime<Utc>,
) -> Vec<IngestAttachment> {
    let mut ordered = chunks.values().collect::<Vec<_>>();
    ordered.sort_by_key(|chunk| (chunk.sequence, chunk.id.as_str()));
    ordered
        .into_iter()
        .map(|chunk| IngestAttachment {
            id: stable_id("attachment", &chunk.id),
            filename: format!("recording-{:06}.{}", chunk.sequence, extension(chunk)),
            content_type: chunk.content_type.clone(),
            size_bytes: chunk.size_bytes.unwrap_or_default(),
            sha256: chunk.sha256.clone().unwrap_or_default(),
            object_key: chunk.uri.clone(),
            metadata: map([
                ("captureChunkId", json!(chunk.id)),
                ("captureSequence", json!(chunk.sequence)),
                ("startMs", json!(chunk.start_ms)),
                ("durationMs", json!(chunk.duration_ms)),
            ]),
            created_at: timestamp(updated_at),
            updated_at: timestamp(updated_at),
        })
        .collect()
}

fn project_participant(participant: ProjectedParticipant) -> IngestParticipant {
    let mut metadata = Map::new();
    metadata.insert(
        "captureParticipantId".into(),
        json!(participant.participant.id),
    );
    if let Some(left_at) = participant.left_at {
        metadata.insert("leftAt".into(), json!(timestamp(left_at)));
    }
    IngestParticipant {
        id: stable_id("participant", &participant.participant.id),
        human_id: String::new(),
        display_name: participant.participant.display_name.unwrap_or_default(),
        email: participant.participant.email.unwrap_or_default(),
        role: String::new(),
        metadata,
        created_at: timestamp(participant.first_seen_at),
        updated_at: timestamp(participant.last_seen_at),
    }
}

fn session_metadata(
    job: &CaptureJob,
    state: BotState,
    events: &[CaptureEvent],
) -> Map<String, Value> {
    map([
        ("captureBotId", json!(job.bot_id)),
        ("captureProvider", json!(provider_name(job.provider))),
        ("captureState", json!(state_name(state))),
        ("requestingActorId", json!(job.requesting_actor_id)),
        (
            "lastCaptureSequence",
            json!(events.last().expect("events are not empty").sequence),
        ),
    ])
}

fn session_status(state: BotState) -> &'static str {
    match state {
        BotState::Completed => "completed",
        BotState::Failed => "failed",
        BotState::Canceled => "canceled",
        BotState::Stopping => "stopping",
        _ => "recording",
    }
}

fn provider_name(provider: CaptureProviderKind) -> &'static str {
    match provider {
        CaptureProviderKind::Anarlog => "anarlog",
        CaptureProviderKind::Recall => "recall",
        CaptureProviderKind::ZoomRtms => "zoom_rtms",
        CaptureProviderKind::MicrosoftGraph => "microsoft_graph",
        CaptureProviderKind::WebexMeetingsSdk => "webex_meetings_sdk",
    }
}

fn platform_name(platform: anlg_meeting_capture::MeetingPlatform) -> &'static str {
    use anlg_meeting_capture::MeetingPlatform;
    match platform {
        MeetingPlatform::GoogleMeet => "google_meet",
        MeetingPlatform::Zoom => "zoom",
        MeetingPlatform::MicrosoftTeams => "microsoft_teams",
        MeetingPlatform::Webex => "webex",
        MeetingPlatform::Jitsi => "jitsi",
    }
}

fn state_name(state: BotState) -> &'static str {
    match state {
        BotState::Queued => "queued",
        BotState::Launching => "launching",
        BotState::WaitingForAdmission => "waiting_for_admission",
        BotState::Joined => "joined",
        BotState::Capturing => "capturing",
        BotState::Stopping => "stopping",
        BotState::Completed => "completed",
        BotState::Failed => "failed",
        BotState::Canceled => "canceled",
    }
}

fn extension(chunk: &RecordingChunk) -> &'static str {
    match chunk.content_type.as_str() {
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

fn stable_id(prefix: &str, value: &str) -> String {
    format!("{prefix}-{}", hex_digest(Sha256::digest(value.as_bytes())))
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    digest
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        })
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

struct ProjectedParticipant {
    participant: Participant,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    left_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("capture projection requires at least one event")]
    NoEvents,
    #[error("capture event sequence is not contiguous: expected {expected}, got {actual}")]
    NonContiguousSequence { expected: u64, actual: u64 },
    #[error("capture event belongs to a different bot")]
    WrongBot,
    #[error("capture event was emitted after a terminal lifecycle event")]
    EventAfterTerminal,
    #[error("capture lifecycle event does not match the projected state")]
    StateMismatch,
    #[error("capture lifecycle transition is invalid")]
    InvalidTransition(#[source] anlg_meeting_capture::TransitionError),
}

#[cfg(test)]
mod tests {
    use anlg_meeting_capture::{
        BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind, MeetingPlatform,
        MeetingReference, Participant, RecordingChunk, Speaker, TerminalReason, TerminalReasonKind,
        TranscriptSegment,
    };
    use chrono::{DateTime, TimeDelta, Utc};

    use super::{ProjectionError, content_hash, project};
    use crate::capture::CaptureJob;

    #[test]
    fn replay_is_deterministic_and_only_terminal_events_finalize() {
        let job = job();
        let events = events();
        let in_progress = project(&job, &events[..events.len() - 1]).unwrap();
        let first = project(&job, &events).unwrap();
        let replayed = project(&job, &events).unwrap();

        assert!(!in_progress.finalized);
        assert!(first.finalized);
        assert_eq!(first, replayed);
        assert_eq!(
            content_hash(&first).unwrap(),
            content_hash(&replayed).unwrap()
        );
        assert_eq!(first.session.status, "completed");
        assert_eq!(first.transcripts.len(), 1);
        assert_eq!(first.transcripts[0].words.len(), 1);
        assert_eq!(
            first.transcripts[0].words[0].speaker.as_deref(),
            Some("Ada")
        );
        assert_eq!(first.participants.len(), 1);
        assert_eq!(first.attachments.len(), 1);
        assert_eq!(first.attachments[0].object_key, "recordings/job-a/0.webm");
        assert_eq!(first.attachments[0].size_bytes, 4_096);
    }

    #[test]
    fn rejects_non_contiguous_sequences_and_events_after_terminal_state() {
        let job = job();
        let mut gap = events();
        gap[1].sequence = 2;
        assert!(matches!(
            project(&job, &gap),
            Err(ProjectionError::NonContiguousSequence { .. })
        ));

        let mut after_terminal = events();
        after_terminal.push(event(
            8,
            CaptureEventPayload::ParticipantUpserted(Participant {
                id: "participant-b".into(),
                display_name: None,
                email: None,
            }),
        ));
        assert!(matches!(
            project(&job, &after_terminal),
            Err(ProjectionError::EventAfterTerminal)
        ));
    }

    fn job() -> CaptureJob {
        CaptureJob {
            workspace_id: "workspace-a".into(),
            job_id: "job-a".into(),
            bot_id: "bot-a".into(),
            owner_user_id: "owner-a".into(),
            requesting_actor_id: "actor-a".into(),
            session_id: "session-a".into(),
            session_title: "Architecture review".into(),
            provider: CaptureProviderKind::Anarlog,
            meeting: MeetingReference {
                platform: MeetingPlatform::GoogleMeet,
                url: "https://meet.google.com/abc-defg-hij".into(),
                external_id: Some("abc-defg-hij".into()),
                calendar_event_id: Some("calendar-event-a".into()),
            },
            created_at: at(0),
        }
    }

    fn events() -> Vec<CaptureEvent> {
        vec![
            lifecycle(0, BotState::Queued, BotState::Launching, None),
            lifecycle(1, BotState::Launching, BotState::Joined, None),
            lifecycle(2, BotState::Joined, BotState::Capturing, None),
            event(
                3,
                CaptureEventPayload::ParticipantUpserted(Participant {
                    id: "participant-a".into(),
                    display_name: Some("Ada".into()),
                    email: Some("ada@example.com".into()),
                }),
            ),
            event(
                4,
                CaptureEventPayload::SpeakerUpserted(Speaker {
                    id: "speaker-a".into(),
                    display_name: Some("Ada".into()),
                    participant_id: Some("participant-a".into()),
                }),
            ),
            event(
                5,
                CaptureEventPayload::Transcript(TranscriptSegment {
                    id: "segment-a".into(),
                    sequence: 0,
                    start_ms: 100,
                    end_ms: Some(450),
                    text: "Ship the deterministic projector.".into(),
                    speaker: Some(Speaker {
                        id: "speaker-a".into(),
                        display_name: None,
                        participant_id: Some("participant-a".into()),
                    }),
                    is_final: true,
                }),
            ),
            event(
                6,
                CaptureEventPayload::RecordingChunkReady(RecordingChunk {
                    id: "chunk-a".into(),
                    sequence: 0,
                    start_ms: 0,
                    duration_ms: 1_000,
                    content_type: "audio/webm".into(),
                    uri: "recordings/job-a/0.webm".into(),
                    size_bytes: Some(4_096),
                    sha256: Some("a".repeat(64)),
                }),
            ),
            lifecycle(
                7,
                BotState::Capturing,
                BotState::Completed,
                Some(TerminalReason {
                    kind: TerminalReasonKind::MeetingEnded,
                    message: None,
                    retryable: false,
                }),
            ),
        ]
    }

    fn lifecycle(
        sequence: u64,
        from: BotState,
        to: BotState,
        reason: Option<TerminalReason>,
    ) -> CaptureEvent {
        event(
            sequence,
            CaptureEventPayload::Lifecycle(from.transition_to(to, reason).unwrap()),
        )
    }

    fn event(sequence: u64, payload: CaptureEventPayload) -> CaptureEvent {
        CaptureEvent {
            id: format!("event-{sequence}"),
            bot_id: "bot-a".into(),
            sequence,
            occurred_at: at(sequence as i64),
            payload,
            metadata: Default::default(),
        }
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + TimeDelta::seconds(seconds)
    }
}
