use anlg_db_core::Db;
use sqlx::{Row, SqlitePool};

use crate::{
    conflicts::{resolve_session_conflict, resolve_session_conflicts},
    creation::{
        CreateEventSessionRequest, CreateSessionRequest, EventParticipantIdentity, create_session,
        create_session_for_event,
    },
    deletion::{
        RestoreDeletedSessionOutcome, TombstoneSessionRequest, restore_deleted_session,
        soft_delete_session,
    },
    move_contents::{MoveSessionContentsRequest, move_session_contents},
    participants::{
        AddSessionParticipantRequest, RemoveSessionParticipantRequest, add_session_participant,
        remove_session_participant,
    },
    proposals::{
        PersistChatSessionProposalRequest, SetSessionProposalStatusRequest,
        persist_chat_session_proposal, set_session_proposal_status,
    },
};

async fn test_db() -> Db {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

async fn insert_session(pool: &SqlitePool, id: &str, owner_user_id: &str) {
    sqlx::query("INSERT INTO sessions (id, owner_user_id) VALUES (?, ?)")
        .bind(id)
        .bind(owner_user_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn insert_event(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO events (
            id, tracking_id_event, calendar_id, title, started_at, ended_at,
            location, meeting_link, description, recurrence_series_id,
            has_recurrence_rules, is_all_day, provider
        ) VALUES (
            'event-1', 'external-event-1', 'calendar-1', 'Planning',
            '2026-07-10T09:00:00.000Z', '2026-07-10T10:00:00.000Z',
            'Room 1', 'https://meet.example/1', 'Plan', 'series-1',
            1, 0, 'google'
        )",
    )
    .execute(pool)
    .await
    .unwrap();
}

fn create_request() -> CreateSessionRequest {
    CreateSessionRequest {
        title: "Welcome".to_string(),
        user_id: "user-1".to_string(),
        event_json: "{\"tracking_id\":\"welcome\"}".to_string(),
        folder_path: "CS 101".to_string(),
        raw_md: "{\"type\":\"doc\"}".to_string(),
    }
}

fn participant(email: &str, name: &str, company_name: Option<&str>) -> EventParticipantIdentity {
    EventParticipantIdentity {
        email: email.to_string(),
        name: name.to_string(),
        company_name: company_name.map(str::to_string),
    }
}

fn event_request(participants: Vec<EventParticipantIdentity>) -> CreateEventSessionRequest {
    CreateEventSessionRequest {
        event_id: "event-1".to_string(),
        user_id: "user-1".to_string(),
        title: None,
        participants,
    }
}

#[tokio::test]
async fn create_session_persists_session_note_owner_and_participant() {
    let db = test_db().await;
    let session_id = create_session(db.pool(), create_request()).await.unwrap();

    let session = sqlx::query(
        "SELECT title, owner_user_id, event_json, folder_path, created_at, updated_at
         FROM sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(session.get::<String, _>("title"), "Welcome");
    assert_eq!(session.get::<String, _>("owner_user_id"), "user-1");
    assert_eq!(
        session.get::<String, _>("event_json"),
        "{\"tracking_id\":\"welcome\"}"
    );
    assert_eq!(session.get::<String, _>("folder_path"), "CS 101");
    for column in ["created_at", "updated_at"] {
        let value: String = session.get(column);
        assert!(
            value.ends_with('Z') && value.contains('.'),
            "{column}={value}"
        );
    }

    let note = sqlx::query(
        "SELECT body, body_format, kind FROM session_documents WHERE id = ? AND kind = 'note'",
    )
    .bind(&session_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(note.get::<String, _>("body"), "{\"type\":\"doc\"}");

    let owner: String =
        sqlx::query_scalar("SELECT id FROM humans WHERE id = 'user-1' AND deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(owner, "user-1");

    let participant =
        sqlx::query("SELECT human_id, source FROM session_participants WHERE session_id = ?")
            .bind(&session_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(participant.get::<String, _>("human_id"), "user-1");
    assert_eq!(participant.get::<String, _>("source"), "manual");
}

#[tokio::test]
async fn create_session_keeps_personal_workspace_inside_team_folder() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO folders (id, workspace_id, path) VALUES ('folder-1', 'team-ws', 'CS 101')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let session_id = create_session(db.pool(), create_request()).await.unwrap();
    let workspace: String = sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_ne!(workspace, "team-ws");
}

#[tokio::test]
async fn create_session_for_event_writes_event_json_byte_identical_to_js() {
    let db = test_db().await;
    insert_event(db.pool()).await;

    let result = create_session_for_event(
        db.pool(),
        event_request(vec![participant(
            "alice@example.com",
            "Alice",
            Some("Example"),
        )]),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.created);

    let event_json: String = sqlx::query_scalar("SELECT event_json FROM sessions WHERE id = ?")
        .bind(&result.session_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        event_json,
        "{\"tracking_id\":\"external-event-1\",\"calendar_id\":\"calendar-1\",\"title\":\"Planning\",\"started_at\":\"2026-07-10T09:00:00.000Z\",\"ended_at\":\"2026-07-10T10:00:00.000Z\",\"is_all_day\":false,\"has_recurrence_rules\":true,\"location\":\"Room 1\",\"meeting_link\":\"https://meet.example/1\",\"description\":\"Plan\",\"recurrence_series_id\":\"series-1\"}"
    );

    let session = sqlx::query(
        "SELECT title, event_id, external_event_id, series_id FROM sessions WHERE id = ?",
    )
    .bind(&result.session_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(session.get::<String, _>("title"), "Planning");
    assert_eq!(session.get::<String, _>("event_id"), "event-1");

    let participant_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_participants WHERE session_id = ? AND source = 'auto'",
    )
    .bind(&result.session_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(participant_count, 1);
    let human = sqlx::query(
        "SELECT name, email, organization_id FROM humans WHERE email = 'alice@example.com'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(human.get::<String, _>("name"), "Alice");
    assert_ne!(human.get::<String, _>("organization_id"), "");
    let org: String = sqlx::query_scalar("SELECT name FROM organizations WHERE name = 'Example'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(org, "Example");
}

#[tokio::test]
async fn create_session_for_event_reuses_existing_session_and_dedupes_participants() {
    let db = test_db().await;
    insert_event(db.pool()).await;
    insert_session(db.pool(), "session-existing", "user-1").await;
    sqlx::query(
        "UPDATE sessions
        SET event_id = 'old-event',
            external_event_id = 'external-event-1',
            external_provider = 'google',
            event_json = '{\"calendar_id\":\"calendar-1\"}'
        WHERE id = 'session-existing'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let request = event_request(vec![
        participant("alice@example.com", "Alice", Some("Example")),
        participant("Alice@Example.com", "Alice", Some("Example")),
    ]);
    let first = create_session_for_event(db.pool(), request.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.session_id, "session-existing");
    assert!(!first.created);

    let second = create_session_for_event(db.pool(), request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.session_id, "session-existing");
    assert!(!second.created);

    let event_id: String =
        sqlx::query_scalar("SELECT event_id FROM sessions WHERE id = 'session-existing'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(event_id, "event-1");

    let participant_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_participants WHERE session_id = 'session-existing'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let human_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE lower(email) = 'alice@example.com'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(participant_count, 1);
    assert_eq!(human_count, 1);
}

#[tokio::test]
async fn create_session_for_event_does_not_reuse_tracking_id_from_another_calendar() {
    let db = test_db().await;
    insert_event(db.pool()).await;
    sqlx::query(
        "INSERT INTO events (
            id, tracking_id_event, calendar_id, title, started_at, ended_at,
            meeting_link, provider
        ) VALUES (
            'event-2', 'external-event-1', 'calendar-2', 'Other planning',
            '2026-07-10T09:00:00.000Z', '2026-07-10T10:00:00.000Z',
            'https://meet.example/2', 'google'
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let first = create_session_for_event(db.pool(), event_request(Vec::new()))
        .await
        .unwrap()
        .unwrap();
    let mut second_request = event_request(Vec::new());
    second_request.event_id = "event-2".to_string();
    let second = create_session_for_event(db.pool(), second_request)
        .await
        .unwrap()
        .unwrap();

    assert!(first.created);
    assert!(second.created);
    assert_ne!(first.session_id, second.session_id);
    let associations: Vec<(String, String)> =
        sqlx::query_as("SELECT id, event_id FROM sessions WHERE id IN (?, ?) ORDER BY event_id")
            .bind(&first.session_id)
            .bind(&second.session_id)
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        associations,
        vec![
            (first.session_id, "event-1".to_string()),
            (second.session_id, "event-2".to_string()),
        ]
    );
}

#[tokio::test]
async fn create_session_for_event_enriches_placeholder_human() {
    let db = test_db().await;
    insert_event(db.pool()).await;
    insert_session(db.pool(), "session-existing", "user-1").await;
    sqlx::query("UPDATE sessions SET event_id = 'event-1' WHERE id = 'session-existing'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO humans (id, name, email) VALUES ('human-1', 'alice@example.com', 'alice@example.com')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    create_session_for_event(
        db.pool(),
        event_request(vec![participant(
            "alice@example.com",
            "Alice",
            Some("Example"),
        )]),
    )
    .await
    .unwrap();

    let human = sqlx::query("SELECT name, organization_id FROM humans WHERE id = 'human-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(human.get::<String, _>("name"), "Alice");
    let org_id = human.get::<String, _>("organization_id");
    let org_name: String = sqlx::query_scalar("SELECT name FROM organizations WHERE id = ?")
        .bind(&org_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(org_name, "Example");
}

#[tokio::test]
async fn create_session_for_event_returns_none_when_event_missing() {
    let db = test_db().await;
    let result = create_session_for_event(db.pool(), event_request(vec![]))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn soft_delete_session_tombstones_children_with_one_timestamp() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query("UPDATE sessions SET title = 'Planning' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, kind) VALUES ('note-1', 'session-1', 'note')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO transcripts (id, session_id) VALUES ('transcript-1', 'session-1')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_participants (id, session_id, human_id) VALUES ('p-1', 'session-1', 'human-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO entity_mentions (id, source_type, source_id, target_type, target_id)
         VALUES ('mention-1', 'session', 'session-1', 'human', 'human-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let tombstone = "2026-07-10T12:00:00.000Z";
    let deleted = soft_delete_session(
        db.pool(),
        TombstoneSessionRequest {
            session_id: "session-1".to_string(),
            tombstone: tombstone.to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(deleted.id, "session-1");
    assert_eq!(deleted.title, "Planning");

    for (table, where_clause) in [
        ("session_documents", "session_id = 'session-1'"),
        ("transcripts", "session_id = 'session-1'"),
        ("session_participants", "session_id = 'session-1'"),
        ("entity_mentions", "id = 'mention-1'"),
        ("sessions", "id = 'session-1'"),
    ] {
        let deleted_at: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT deleted_at FROM {table} WHERE {where_clause}"
        )))
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(deleted_at.as_deref(), Some(tombstone), "{table}");
    }
}

#[tokio::test]
async fn soft_delete_session_returns_none_when_already_deleted() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query("UPDATE sessions SET deleted_at = '2026-01-01' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO transcripts (id, session_id) VALUES ('t-1', 'session-1')")
        .execute(db.pool())
        .await
        .unwrap();

    let deleted = soft_delete_session(
        db.pool(),
        TombstoneSessionRequest {
            session_id: "session-1".to_string(),
            tombstone: "2026-07-10T12:00:00.000Z".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(deleted.is_none());
    let transcript_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM transcripts WHERE id = 't-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(transcript_deleted, None);
}

#[tokio::test]
async fn restore_deleted_session_restores_only_matching_tombstone() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query("INSERT INTO transcripts (id, session_id) VALUES ('t-1', 'session-1')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE sessions SET deleted_at = '2026-07-10T12:00:00.000Z' WHERE id = 'session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE transcripts SET deleted_at = '2026-07-10T12:00:00.000Z' WHERE id = 't-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (id, deleted_at) VALUES ('other', '2026-01-01')")
        .execute(db.pool())
        .await
        .unwrap();

    let outcome = restore_deleted_session(
        db.pool(),
        TombstoneSessionRequest {
            session_id: "session-1".to_string(),
            tombstone: "2026-07-10T12:00:00.000Z".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome, RestoreDeletedSessionOutcome::Restored);

    let session_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let transcript_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM transcripts WHERE id = 't-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let other_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM sessions WHERE id = 'other'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(session_deleted, None);
    assert_eq!(transcript_deleted, None);
    assert_eq!(other_deleted.as_deref(), Some("2026-01-01"));
}

#[tokio::test]
async fn restore_deleted_session_reports_alive_and_not_deleted() {
    let db = test_db().await;
    insert_session(db.pool(), "alive-1", "user-1").await;
    insert_session(db.pool(), "stale-1", "user-1").await;
    sqlx::query("UPDATE sessions SET deleted_at = '2026-02-01' WHERE id = 'stale-1'")
        .execute(db.pool())
        .await
        .unwrap();

    let alive = restore_deleted_session(
        db.pool(),
        TombstoneSessionRequest {
            session_id: "alive-1".to_string(),
            tombstone: "2026-07-10T12:00:00.000Z".to_string(),
        },
    )
    .await
    .unwrap();
    let not_deleted = restore_deleted_session(
        db.pool(),
        TombstoneSessionRequest {
            session_id: "stale-1".to_string(),
            tombstone: "2026-07-10T12:00:00.000Z".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(alive, RestoreDeletedSessionOutcome::Alive);
    assert_eq!(not_deleted, RestoreDeletedSessionOutcome::NotDeleted);
}

#[tokio::test]
async fn add_session_participant_links_human_and_revives_excluded() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query(
        "INSERT INTO humans (id, name, email) VALUES ('human-1', 'Alice', 'alice@example.com')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    add_session_participant(
        db.pool(),
        AddSessionParticipantRequest {
            session_id: "session-1".to_string(),
            human_id: "human-1".to_string(),
            source: "manual".to_string(),
        },
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT human_id, source, display_name, email FROM session_participants WHERE session_id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("human_id"), "human-1");
    assert_eq!(row.get::<String, _>("source"), "manual");
    assert_eq!(row.get::<String, _>("display_name"), "Alice");

    add_session_participant(
        db.pool(),
        AddSessionParticipantRequest {
            session_id: "session-1".to_string(),
            human_id: "human-1".to_string(),
            source: "manual".to_string(),
        },
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_participants WHERE session_id = 'session-1' AND deleted_at IS NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn add_session_participant_is_noop_on_deleted_session() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query("UPDATE sessions SET deleted_at = '2026-01-01' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO humans (id) VALUES ('human-1')")
        .execute(db.pool())
        .await
        .unwrap();

    add_session_participant(
        db.pool(),
        AddSessionParticipantRequest {
            session_id: "session-1".to_string(),
            human_id: "human-1".to_string(),
            source: "manual".to_string(),
        },
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_participants")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn remove_session_participant_tombstones_manual_and_excludes_auto() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query(
        "INSERT INTO session_participants (id, session_id, human_id, source) VALUES
            ('manual-1', 'session-1', 'human-1', 'manual'),
            ('auto-1', 'session-1', 'human-2', 'auto')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    for mapping in ["manual-1", "auto-1"] {
        remove_session_participant(
            db.pool(),
            RemoveSessionParticipantRequest {
                mapping_id: mapping.to_string(),
            },
        )
        .await
        .unwrap();
    }

    let manual =
        sqlx::query("SELECT source, deleted_at FROM session_participants WHERE id = 'manual-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(manual.get::<String, _>("source"), "manual");
    assert!(manual.get::<Option<String>, _>("deleted_at").is_some());

    let auto =
        sqlx::query("SELECT source, deleted_at FROM session_participants WHERE id = 'auto-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(auto.get::<String, _>("source"), "excluded");
    assert_eq!(auto.get::<Option<String>, _>("deleted_at"), None);
}

#[tokio::test]
async fn persist_chat_session_proposal_stores_passed_base_timestamp() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;

    persist_chat_session_proposal(
        db.pool(),
        PersistChatSessionProposalRequest {
            id: "proposal-1".to_string(),
            session_id: "session-1".to_string(),
            kind: "summary_replace".to_string(),
            target_id: "summary-1".to_string(),
            base_updated_at: "2026-08-26T00:00:00.000Z".to_string(),
            current_markdown: "Current".to_string(),
            proposed_markdown: "Proposed".to_string(),
            source: "chat".to_string(),
        },
    )
    .await
    .unwrap();

    let proposal = sqlx::query(
        "SELECT kind, target_id, base_updated_at, status, source FROM session_proposals WHERE id = 'proposal-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        proposal.get::<String, _>("base_updated_at"),
        "2026-08-26T00:00:00.000Z"
    );
    assert_eq!(proposal.get::<String, _>("status"), "pending");
    assert_eq!(proposal.get::<String, _>("source"), "chat");
}

#[tokio::test]
async fn set_session_proposal_status_only_transitions_pending() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1", "user-1").await;
    sqlx::query(
        "INSERT INTO session_proposals (id, session_id, status) VALUES ('p-1', 'session-1', 'pending'), ('p-2', 'session-1', 'applied')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    for proposal in ["p-1", "p-2"] {
        set_session_proposal_status(
            db.pool(),
            SetSessionProposalStatusRequest {
                proposal_id: proposal.to_string(),
                status: "declined".to_string(),
            },
        )
        .await
        .unwrap();
    }

    let first: String = sqlx::query_scalar("SELECT status FROM session_proposals WHERE id = 'p-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let second: String =
        sqlx::query_scalar("SELECT status FROM session_proposals WHERE id = 'p-2'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let updated_at: String =
        sqlx::query_scalar("SELECT updated_at FROM session_proposals WHERE id = 'p-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(first, "declined");
    assert_eq!(second, "applied");
    assert!(updated_at.ends_with('Z') && updated_at.contains('.'));
}

#[tokio::test]
async fn resolve_session_conflicts_marks_note_conflicts_resolved() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO e2ee_field_conflicts (id, table_name, row_id, field_name) VALUES
            ('c-body', 'session_documents', 'session-1', 'body'),
            ('c-title', 'sessions', 'session-1', 'title'),
            ('c-other', 'session_documents', 'session-1', 'title'),
            ('c-other-session', 'session_documents', 'session-2', 'body')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    resolve_session_conflicts(
        db.pool(),
        crate::conflicts::ResolveSessionConflictsRequest {
            session_id: "session-1".to_string(),
        },
    )
    .await
    .unwrap();
    resolve_session_conflict(
        db.pool(),
        crate::conflicts::ResolveSessionConflictRequest {
            conflict_id: "c-other-session".to_string(),
        },
    )
    .await
    .unwrap();

    for (id, expected) in [
        ("c-body", true),
        ("c-title", true),
        ("c-other", false),
        ("c-other-session", true),
    ] {
        let resolved_at: Option<String> =
            sqlx::query_scalar("SELECT resolved_at FROM e2ee_field_conflicts WHERE id = ?")
                .bind(id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(resolved_at.is_some(), expected, "{id}");
    }
}

#[tokio::test]
async fn move_session_contents_moves_rows_and_rewrites_notes() {
    let db = test_db().await;
    insert_session(db.pool(), "source", "user-1").await;
    insert_session(db.pool(), "target", "user-1").await;
    sqlx::query(
        "INSERT INTO transcripts (id, session_id, audio_attachment_id) VALUES
            ('t-1', 'source', 'session-audio:source')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, kind) VALUES
            ('source', 'source', 'note'),
            ('target', 'target', 'note'),
            ('summary-1', 'source', 'summary')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO action_items (id, session_id) VALUES ('a-1', 'source')")
        .execute(db.pool())
        .await
        .unwrap();

    move_session_contents(
        db.pool(),
        MoveSessionContentsRequest {
            source_session_id: "source".to_string(),
            target_session_id: "target".to_string(),
            rewrite_audio_ids: true,
            next_target_note: Some("{\"type\":\"doc\",\"content\":[]}".to_string()),
            empty_source_note: Some("{\"type\":\"doc\",\"content\":[]}".to_string()),
        },
    )
    .await
    .unwrap();

    let transcript =
        sqlx::query("SELECT session_id, audio_attachment_id FROM transcripts WHERE id = 't-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(transcript.get::<String, _>("session_id"), "target");
    assert_eq!(
        transcript.get::<String, _>("audio_attachment_id"),
        "session-audio:target"
    );
    let summary_session: String =
        sqlx::query_scalar("SELECT session_id FROM session_documents WHERE id = 'summary-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let action_session: String =
        sqlx::query_scalar("SELECT session_id FROM action_items WHERE id = 'a-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(summary_session, "target");
    assert_eq!(action_session, "target");
    let source_note: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'source'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(source_note, "{\"type\":\"doc\",\"content\":[]}");
}

#[tokio::test]
async fn move_session_contents_rolls_back_when_note_row_count_mismatches() {
    let db = test_db().await;
    insert_session(db.pool(), "source", "user-1").await;
    insert_session(db.pool(), "target", "user-1").await;
    sqlx::query("INSERT INTO transcripts (id, session_id) VALUES ('t-1', 'source')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, kind) VALUES ('source', 'source', 'note')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let error = move_session_contents(
        db.pool(),
        MoveSessionContentsRequest {
            source_session_id: "source".to_string(),
            target_session_id: "target".to_string(),
            rewrite_audio_ids: false,
            next_target_note: Some("{}".to_string()),
            empty_source_note: Some("{}".to_string()),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(error, "transaction statement 5 affected 0 rows; expected 1");
    let session_id: String =
        sqlx::query_scalar("SELECT session_id FROM transcripts WHERE id = 't-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(session_id, "source");
}
