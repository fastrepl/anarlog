use anlg_meeting_capture::{
    BotState, CaptureEventPayload, CaptureProviderKind, CaptureWorkerCheckpoint,
    MEETING_SDK_BRIDGE_PROTOCOL_VERSION, MeetingPlatform, MeetingReference, MeetingSdkBridgeEvent,
    MeetingSdkBridgeEventPayload, MeetingSdkBridgeNormalizer, MeetingSdkBridgeTerminal,
    MeetingSdkBridgeTranscript, TerminalReason, TerminalReasonKind,
};
use chrono::{DateTime, Utc};

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-08-21T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn checkpoint() -> CaptureWorkerCheckpoint {
    CaptureWorkerCheckpoint {
        job_id: "job-teams".into(),
        bot_id: "bot-teams".into(),
        provider: CaptureProviderKind::MicrosoftGraph,
        meeting: MeetingReference {
            platform: MeetingPlatform::MicrosoftTeams,
            url: "https://teams.microsoft.com/l/meetup-join/reliability".into(),
            external_id: Some("meeting-1".into()),
            calendar_event_id: None,
        },
        state: BotState::Queued,
        next_sequence: 0,
    }
}

fn event(sequence: u64, payload: MeetingSdkBridgeEventPayload) -> MeetingSdkBridgeEvent {
    MeetingSdkBridgeEvent {
        protocol_version: MEETING_SDK_BRIDGE_PROTOCOL_VERSION,
        sequence,
        platform: MeetingPlatform::MicrosoftTeams,
        provider: CaptureProviderKind::MicrosoftGraph,
        payload,
    }
}

#[test]
fn teams_reliability_matrix_covers_lobby_policy_and_meeting_end() {
    let cases: [(&str, Vec<MeetingSdkBridgeEventPayload>, TerminalReasonKind); 4] = [
        (
            "lobby-timeout",
            vec![
                MeetingSdkBridgeEventPayload::Ready,
                MeetingSdkBridgeEventPayload::WaitingForAdmission,
                MeetingSdkBridgeEventPayload::Terminal(MeetingSdkBridgeTerminal {
                    state: BotState::Failed,
                    reason: TerminalReason {
                        kind: TerminalReasonKind::AdmissionTimeout,
                        message: Some("organizer did not admit the media bot".into()),
                        retryable: true,
                    },
                }),
            ],
            TerminalReasonKind::AdmissionTimeout,
        ),
        (
            "organizer-denied",
            vec![
                MeetingSdkBridgeEventPayload::Ready,
                MeetingSdkBridgeEventPayload::WaitingForAdmission,
                MeetingSdkBridgeEventPayload::Terminal(MeetingSdkBridgeTerminal {
                    state: BotState::Failed,
                    reason: TerminalReason {
                        kind: TerminalReasonKind::AdmissionDenied,
                        message: Some("organizer policy denied the media bot".into()),
                        retryable: false,
                    },
                }),
            ],
            TerminalReasonKind::AdmissionDenied,
        ),
        (
            "removed-by-organizer",
            vec![
                MeetingSdkBridgeEventPayload::Ready,
                MeetingSdkBridgeEventPayload::Joined,
                MeetingSdkBridgeEventPayload::Capturing,
                MeetingSdkBridgeEventPayload::Terminal(MeetingSdkBridgeTerminal {
                    state: BotState::Failed,
                    reason: TerminalReason {
                        kind: TerminalReasonKind::RemovedFromMeeting,
                        message: Some("organizer removed the media bot".into()),
                        retryable: false,
                    },
                }),
            ],
            TerminalReasonKind::RemovedFromMeeting,
        ),
        (
            "meeting-ended",
            vec![
                MeetingSdkBridgeEventPayload::Ready,
                MeetingSdkBridgeEventPayload::Joined,
                MeetingSdkBridgeEventPayload::Capturing,
                MeetingSdkBridgeEventPayload::Transcript(MeetingSdkBridgeTranscript {
                    start_ms: 0,
                    end_ms: Some(1_200),
                    text: "Action items are in the notes".into(),
                    speaker: None,
                    is_final: true,
                }),
                MeetingSdkBridgeEventPayload::Terminal(MeetingSdkBridgeTerminal {
                    state: BotState::Completed,
                    reason: TerminalReason {
                        kind: TerminalReasonKind::MeetingEnded,
                        message: Some("Teams meeting ended".into()),
                        retryable: false,
                    },
                }),
            ],
            TerminalReasonKind::MeetingEnded,
        ),
    ];

    for (name, payloads, expected) in cases {
        let mut normalizer = MeetingSdkBridgeNormalizer::new(&checkpoint()).unwrap();
        let mut last_kind = None;
        for (sequence, payload) in payloads.into_iter().enumerate() {
            let accepted = normalizer
                .accept(event(sequence as u64, payload), now())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            if let CaptureEventPayload::Lifecycle(transition) = &accepted.payload {
                if let Some(reason) = &transition.reason {
                    last_kind = Some(reason.kind);
                }
            }
        }
        assert_eq!(last_kind, Some(expected), "{name}");
        assert!(normalizer.state().is_terminal(), "{name}");
    }
}
