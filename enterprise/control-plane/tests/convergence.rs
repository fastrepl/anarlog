use std::collections::BTreeSet;

use anarlog_enterprise_control_plane::{capture::CaptureJob, projector::project};
use anlg_agent_access::get_meeting_export;
use anlg_db_core::Db;
use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind, MeetingPlatform,
    MeetingReference, Participant, RecordingChunk, Speaker, TerminalReason, TerminalReasonKind,
    TranscriptSegment,
};
use anlg_session_ingest::{ApplyOutcome, apply_session_envelope};
use chrono::{DateTime, TimeDelta, Utc};

#[tokio::test]
async fn finalized_projection_converges_across_two_device_databases() {
    let envelope = project(&capture_job(), &capture_events()).unwrap();
    assert!(envelope.finalized);

    let device_a = test_db().await;
    let device_b = test_db().await;
    for device in [&device_a, &device_b] {
        assert_eq!(
            apply_session_envelope(device.pool(), "workspace-a", &envelope)
                .await
                .unwrap(),
            ApplyOutcome::Applied
        );
        assert_eq!(
            apply_session_envelope(device.pool(), "workspace-a", &envelope)
                .await
                .unwrap(),
            ApplyOutcome::AlreadyApplied
        );
    }

    let export_a = get_meeting_export(device_a.pool(), envelope.session.id.clone())
        .await
        .unwrap();
    let export_b = get_meeting_export(device_b.pool(), envelope.session.id.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(export_a).unwrap(),
        serde_json::to_value(export_b).unwrap()
    );
    assert_eq!(session_count(&device_a).await, 1);
    assert_eq!(session_count(&device_b).await, 1);
    assert_eq!(dirty_rows(&device_a).await, dirty_rows(&device_b).await);
    assert_eq!(
        ingest_marker(&device_a).await,
        ingest_marker(&device_b).await
    );
    assert_eq!(ingest_marker(&device_a).await["finalized"], true);
    assert_eq!(ingest_marker(&device_a).await["revision"], 8);
}

async fn test_db() -> Db {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

async fn session_count(db: &Db) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sessions WHERE id = 'session-a'")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn dirty_rows(db: &Db) -> BTreeSet<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT table_name, row_id FROM e2ee_dirty_rows WHERE workspace_id = 'workspace-a'",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
    .into_iter()
    .collect()
}

async fn ingest_marker(db: &Db) -> serde_json::Value {
    let metadata: String =
        sqlx::query_scalar("SELECT metadata_json FROM sessions WHERE id = 'session-a'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["anarlog_capture_ingest"].clone()
}

fn capture_job() -> CaptureJob {
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

fn capture_events() -> Vec<CaptureEvent> {
    vec![
        lifecycle(0, BotState::Queued, BotState::Launching, None),
        lifecycle(1, BotState::Launching, BotState::Joined, None),
        lifecycle(2, BotState::Joined, BotState::Capturing, None),
        capture_event(
            3,
            CaptureEventPayload::ParticipantUpserted(Participant {
                id: "participant-a".into(),
                display_name: Some("Ada".into()),
                email: Some("ada@example.com".into()),
            }),
        ),
        capture_event(
            4,
            CaptureEventPayload::SpeakerUpserted(Speaker {
                id: "speaker-a".into(),
                display_name: Some("Ada".into()),
                participant_id: Some("participant-a".into()),
            }),
        ),
        capture_event(
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
        capture_event(
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
    capture_event(
        sequence,
        CaptureEventPayload::Lifecycle(from.transition_to(to, reason).unwrap()),
    )
}

fn capture_event(sequence: u64, payload: CaptureEventPayload) -> CaptureEvent {
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
