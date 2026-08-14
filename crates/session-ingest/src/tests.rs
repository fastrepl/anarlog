use std::collections::BTreeSet;

use anlg_agent_access::{GetMeetingInput, get_meeting, get_meeting_export};
use anlg_db_core::Db;
use serde_json::{Map, json};

use crate::{
    ApplyOutcome, DocumentFormat, DocumentKind, Error, IngestAttachment, IngestDocument,
    IngestParticipant, IngestSession, IngestSpeakerHint, IngestTranscript, IngestWord,
    SESSION_INGEST_SCHEMA_VERSION, SessionIngestEnvelope, apply_session_envelope,
};

async fn test_db() -> Db {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

fn envelope(finalized: bool) -> SessionIngestEnvelope {
    SessionIngestEnvelope {
        schema_version: SESSION_INGEST_SCHEMA_VERSION,
        source_id: "capture-job-1".to_string(),
        revision: 1,
        finalized,
        attach_to_existing: false,
        workspace_id: "workspace-1".to_string(),
        owner_user_id: "user-1".to_string(),
        session: IngestSession {
            id: "session-1".to_string(),
            title: "Weekly planning".to_string(),
            status: if finalized { "completed" } else { "active" }.to_string(),
            created_at: "2026-08-14T08:00:00.000Z".to_string(),
            updated_at: "2026-08-14T09:00:00.000Z".to_string(),
            started_at: "2026-08-14T08:00:00.000Z".to_string(),
            ended_at: if finalized {
                "2026-08-14T09:00:00.000Z".to_string()
            } else {
                String::new()
            },
            timezone: "Asia/Seoul".to_string(),
            language: "en".to_string(),
            event_id: "event-1".to_string(),
            external_event_id: "calendar-event-1".to_string(),
            external_provider: "calendar".to_string(),
            series_id: "series-1".to_string(),
            event: json!({ "summary": "Weekly planning" }),
            metadata: Map::from_iter([("room".to_string(), json!("planning"))]),
        },
        documents: vec![IngestDocument {
            id: "summary-1".to_string(),
            kind: DocumentKind::Summary,
            format: DocumentFormat::Markdown,
            template_id: String::new(),
            title: "Summary".to_string(),
            body: "We agreed on the launch plan.".to_string(),
            source_hash: "summary-hash".to_string(),
            generation_metadata: Map::new(),
            sort_order: 1,
            created_at: "2026-08-14T09:00:00.000Z".to_string(),
            updated_at: "2026-08-14T09:00:00.000Z".to_string(),
        }],
        transcripts: vec![IngestTranscript {
            id: "transcript-1".to_string(),
            provider: "customer_stt".to_string(),
            model: "model-1".to_string(),
            language: "en".to_string(),
            started_at_ms: 0,
            ended_at_ms: Some(1_000),
            audio_attachment_id: "audio-1".to_string(),
            memo: String::new(),
            words: vec![
                IngestWord {
                    id: "word-1".to_string(),
                    text: "Launch".to_string(),
                    start_ms: 0,
                    end_ms: 400,
                    channel: 0,
                    speaker: Some("Alice".to_string()),
                    metadata: Map::new(),
                },
                IngestWord {
                    id: "word-2".to_string(),
                    text: "Friday".to_string(),
                    start_ms: 500,
                    end_ms: 1_000,
                    channel: 0,
                    speaker: Some("Alice".to_string()),
                    metadata: Map::new(),
                },
            ],
            speaker_hints: vec![IngestSpeakerHint {
                id: "hint-1".to_string(),
                word_id: "word-1".to_string(),
                kind: "human_id".to_string(),
                value: "human-1".to_string(),
            }],
            metadata: Map::new(),
            created_at: "2026-08-14T08:00:00.000Z".to_string(),
            updated_at: "2026-08-14T09:00:00.000Z".to_string(),
        }],
        participants: vec![IngestParticipant {
            id: "participant-1".to_string(),
            human_id: "human-1".to_string(),
            display_name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            role: "host".to_string(),
            metadata: Map::new(),
            created_at: "2026-08-14T08:00:00.000Z".to_string(),
            updated_at: "2026-08-14T09:00:00.000Z".to_string(),
        }],
        attachments: vec![IngestAttachment {
            id: "audio-1".to_string(),
            filename: "meeting.webm".to_string(),
            content_type: "audio/webm".to_string(),
            size_bytes: 1_024,
            sha256: "audio-hash".to_string(),
            object_key: "workspace-1/capture-job-1/audio.webm".to_string(),
            metadata: Map::new(),
            created_at: "2026-08-14T08:00:00.000Z".to_string(),
            updated_at: "2026-08-14T09:00:00.000Z".to_string(),
        }],
    }
}

#[test]
fn only_database_failures_are_retryable() {
    assert!(
        !Error::DeletedSession {
            session_id: "session-1".to_string(),
        }
        .is_retryable()
    );
    assert!(Error::Database(sqlx::Error::RowNotFound).is_retryable());
}

#[tokio::test]
async fn applies_canonical_graph_idempotently_and_marks_rows_for_e2ee() {
    let db = test_db().await;
    let mut envelope = envelope(true);
    envelope
        .session
        .metadata
        .insert("alpha".to_string(), json!(1));
    envelope
        .session
        .metadata
        .insert("omega".to_string(), json!(2));

    assert_eq!(
        apply_session_envelope(db.pool(), "workspace-1", &envelope)
            .await
            .unwrap(),
        ApplyOutcome::Applied
    );
    let mut retry = envelope.clone();
    retry.session.metadata.clear();
    retry.session.metadata.insert("omega".to_string(), json!(2));
    retry
        .session
        .metadata
        .insert("room".to_string(), json!("planning"));
    retry.session.metadata.insert("alpha".to_string(), json!(1));
    assert_eq!(
        apply_session_envelope(db.pool(), "workspace-1", &retry)
            .await
            .unwrap(),
        ApplyOutcome::AlreadyApplied
    );

    let export = get_meeting_export(db.pool(), "session-1".to_string())
        .await
        .unwrap();
    assert_eq!(export.meeting.title, "Weekly planning");
    assert_eq!(
        export.meeting.summaries[0].markdown,
        "We agreed on the launch plan."
    );
    assert_eq!(export.meeting.participants[0].display_name, "Alice");
    assert_eq!(export.transcripts[0].text, "Launch Friday");

    let attachment = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT storage_kind, cloud_object_key, cloud_sync_enabled
         FROM session_attachments WHERE id = 'audio-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        attachment,
        (
            "remote_object".to_string(),
            "workspace-1/capture-job-1/audio.webm".to_string(),
            0,
        )
    );

    let dirty_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT table_name, row_id FROM e2ee_dirty_rows WHERE workspace_id = 'workspace-1'",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert!(dirty_rows.contains(&("sessions".to_string(), "session-1".to_string())));
    assert!(dirty_rows.contains(&("session_documents".to_string(), "summary-1".to_string())));
    assert!(dirty_rows.contains(&("transcripts".to_string(), "transcript-1".to_string())));
    assert!(dirty_rows.contains(&(
        "session_participants".to_string(),
        "participant-1".to_string()
    )));
    assert!(dirty_rows.contains(&("session_attachments".to_string(), "audio-1".to_string())));
    assert!(dirty_rows.contains(&("humans".to_string(), "human-1".to_string())));
}

#[tokio::test]
async fn rejects_cross_workspace_and_cross_owner_writes() {
    let db = test_db().await;
    let envelope = envelope(false);

    let error = apply_session_envelope(db.pool(), "workspace-other", &envelope)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::WorkspaceMismatch { .. }));

    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-1', 'user-other', 'Private')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let error = apply_session_envelope(db.pool(), "workspace-1", &envelope)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::OwnershipConflict {
            entity: "session",
            ..
        }
    ));
}

#[tokio::test]
async fn enforces_revision_and_finalization_rules() {
    let db = test_db().await;
    let first = envelope(false);
    apply_session_envelope(db.pool(), "workspace-1", &first)
        .await
        .unwrap();

    let mut conflicting = first.clone();
    conflicting.session.title = "Changed without a revision".to_string();
    let error = apply_session_envelope(db.pool(), "workspace-1", &conflicting)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::RevisionConflict { revision: 1 }));

    let mut final_revision = first.clone();
    final_revision.revision = 2;
    final_revision.finalized = true;
    final_revision.session.status = "completed".to_string();
    final_revision.session.ended_at = "2026-08-14T09:00:00.000Z".to_string();
    apply_session_envelope(db.pool(), "workspace-1", &final_revision)
        .await
        .unwrap();

    let error = apply_session_envelope(db.pool(), "workspace-1", &first)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::StaleRevision {
            actual: 1,
            applied: 2
        }
    ));

    let mut after_final = final_revision;
    after_final.revision = 3;
    let error = apply_session_envelope(db.pool(), "workspace-1", &after_final)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Finalized { revision: 2, .. }));
}

#[tokio::test]
async fn attaches_explicitly_without_overwriting_user_title() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (
            id, workspace_id, owner_user_id, title, status, created_at, updated_at, metadata_json
         ) VALUES (
            'session-1', 'workspace-1', 'user-1', 'My title', 'active',
            '2026-08-14T07:00:00.000Z', '2026-08-14T07:00:00.000Z', '{\"pinned\":true}'
         )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let mut envelope = envelope(false);

    let error = apply_session_envelope(db.pool(), "workspace-1", &envelope)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::SourceConflict { .. }));

    envelope.attach_to_existing = true;
    apply_session_envelope(db.pool(), "workspace-1", &envelope)
        .await
        .unwrap();
    envelope.revision = 2;
    envelope.session.title = "Capture title".to_string();
    envelope.session.updated_at = "2026-08-14T09:30:00.000Z".to_string();
    apply_session_envelope(db.pool(), "workspace-1", &envelope)
        .await
        .unwrap();
    let meeting = get_meeting(
        db.pool(),
        GetMeetingInput {
            meeting_id: "session-1".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(meeting.title, "My title");
    let metadata: serde_json::Value =
        sqlx::query_scalar("SELECT metadata_json FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .map(|value: String| serde_json::from_str(&value).unwrap())
            .unwrap();
    assert_eq!(metadata["pinned"], true);
    assert_eq!(
        metadata["anarlog_capture_ingest"]["source_id"],
        "capture-job-1"
    );
}

#[tokio::test]
async fn refuses_to_resurrect_deleted_sessions() {
    let db = test_db().await;
    let first = envelope(false);
    apply_session_envelope(db.pool(), "workspace-1", &first)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE sessions SET deleted_at = '2026-08-14T09:15:00.000Z' WHERE id = 'session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut second = first;
    second.revision = 2;
    second.session.title = "Must stay deleted".to_string();
    let error = apply_session_envelope(db.pool(), "workspace-1", &second)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::DeletedSession { .. }));
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT title, deleted_at FROM sessions WHERE id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.0, "Weekly planning");
    assert!(row.1.is_some());
}

#[tokio::test]
async fn reconciles_only_children_owned_by_the_capture_source() {
    let db = test_db().await;
    let first = envelope(false);
    apply_session_envelope(db.pool(), "workspace-1", &first)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (
            id, workspace_id, session_id, kind, body_format, body, generation_metadata_json
         ) VALUES (
            'user-note', 'workspace-1', 'session-1', 'note', 'markdown', 'Keep me', '{}'
         )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut second = first;
    second.revision = 2;
    second.documents.clear();
    apply_session_envelope(db.pool(), "workspace-1", &second)
        .await
        .unwrap();

    let rows = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, deleted_at FROM session_documents
         WHERE session_id = 'session-1' ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows[0].0, "summary-1");
    assert!(rows[0].1.is_some());
    assert_eq!(rows[1], ("user-note".to_string(), None));
}

#[tokio::test]
async fn refuses_to_overwrite_an_unowned_child_row() {
    let db = test_db().await;
    let first = envelope(false);
    apply_session_envelope(db.pool(), "workspace-1", &first)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (
            id, workspace_id, session_id, kind, body_format, body, generation_metadata_json
         ) VALUES (
            'user-note', 'workspace-1', 'session-1', 'note', 'markdown', 'Keep me', '{}'
         )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut second = first;
    second.revision = 2;
    second.session.title = "Must roll back".to_string();
    second.documents[0].id = "user-note".to_string();
    let error = apply_session_envelope(db.pool(), "workspace-1", &second)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::ChildSourceConflict {
            entity: "document",
            ..
        }
    ));

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(title, "Weekly planning");
    let body: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'user-note'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(body, "Keep me");
}
