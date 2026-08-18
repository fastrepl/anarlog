use super::*;
use anlg_db_core::Db;

async fn test_db() -> Db {
    let db = Db::connect_memory_plain().await.unwrap();
    crate::prepare_schema(&db).await.unwrap();
    db
}

fn session_batch() -> LegacyImportBatch {
    LegacyImportBatch {
        rows: vec![LegacyImportRow::Session(LegacySession {
            id: "session-1".to_string(),
            owner_user_id: "user-1".to_string(),
            title: "Planning".to_string(),
            created_at: "2026-07-10T12:00:00Z".to_string(),
            started_at: String::new(),
            ended_at: String::new(),
            event_id: String::new(),
            external_event_id: String::new(),
            external_provider: String::new(),
            series_id: String::new(),
            event_json: String::new(),
            metadata_json: "{}".to_string(),
            folder_path: "work".to_string(),
            recovery_status: None,
        })],
        ..Default::default()
    }
}

async fn begin_run_with_session(db: &Db, run_id: &str) {
    begin_legacy_import_run(db.pool(), run_id, "/vault", false)
        .await
        .unwrap();
    apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "session-item",
            run_id,
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "session-hash",
        },
        &session_batch(),
        false,
    )
    .await
    .unwrap();
}

fn document_row(id: &str, body_format: &str, body: &str) -> LegacyImportRow {
    LegacyImportRow::Document(LegacyDocument {
        id: id.to_string(),
        session_id: "session-1".to_string(),
        kind: "note".to_string(),
        template_id: String::new(),
        title: String::new(),
        body_format: body_format.to_string(),
        body: body.to_string(),
        source_hash: format!("{id}-hash"),
        sort_order: 0,
        created_by: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
        generation_metadata_json: "{}".to_string(),
        recovery_status: None,
    })
}

fn transcript_row(id: &str, words_json: &str) -> LegacyImportRow {
    LegacyImportRow::Transcript(LegacyTranscript {
        id: id.to_string(),
        owner_user_id: "user-1".to_string(),
        session_id: "session-1".to_string(),
        started_at_ms: 10,
        ended_at_ms: Some(20),
        memo: String::new(),
        words_json: words_json.to_string(),
        speaker_hints_json: "[]".to_string(),
        created_at: "2026-07-10T12:00:00Z".to_string(),
        metadata_json: "{}".to_string(),
        recovery_status: None,
    })
}

#[tokio::test]
async fn import_fails_closed_without_a_workspace_binding() {
    let db = test_db().await;
    sqlx::query("DELETE FROM app_settings WHERE id = 'cloudsync_workspace_binding'")
        .execute(db.pool())
        .await
        .unwrap();
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();

    let error = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-1",
        },
        &session_batch(),
        false,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, sqlx::Error::Database(_)));
    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_items")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(session_count, 0);
    assert_eq!(item_count, 0);
}

#[tokio::test]
async fn import_item_is_atomic_and_existing_sqlite_rows_win() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO app_settings (id, value_json) \
         VALUES ('cloudsync_workspace_binding', \
           '{\"workspace_id\":\"workspace-1\",\"account_user_id\":\"user-1\"}') \
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
    )
    .execute(db.pool())
    .await
    .unwrap();
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-1",
        },
        &session_batch(),
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.imported_count, 1);
    assert_eq!(result.conflict_count, 0);
    let workspace_id: String =
        sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(workspace_id, "workspace-1");

    begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
        .await
        .unwrap();
    let mut conflicting_batch = session_batch();
    let LegacyImportRow::Session(session) = &mut conflicting_batch.rows[0] else {
        panic!("expected session");
    };
    session.title = "Conflicting legacy title".to_string();
    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-2",
            run_id: "run-2",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-2",
        },
        &conflicting_batch,
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.conflict_count, 1);
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
        .bind("session-1")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(title, "Planning");
}

#[tokio::test]
async fn orphaned_session_document_is_a_blocking_missing_dependency() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/missing/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![document_row("document-1", "markdown", "Orphaned note")],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.matched_count, 0);
    assert_eq!(result.conflict_count, 0);
    assert_eq!(result.skipped_count, 1);
    let target_status: String = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets
         WHERE run_id = 'run-1' AND target_id = 'document-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_status, "missing_dependency");
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed_with_issues"
    );
}

#[tokio::test]
async fn nonempty_legacy_document_fills_an_empty_sqlite_document() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO session_documents
         (id, session_id, kind, title, body_format, body)
         VALUES ('document-1', 'session-1', 'note', 'Keep this title',
                 'prosemirror_json',
                 '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut incoming = document_row("document-1", "markdown", "Legacy note");
    let LegacyImportRow::Document(row) = &mut incoming else {
        unreachable!();
    };
    row.title = "  \n".to_string();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/session-1/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![incoming],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 1);
    assert_eq!(result.conflict_count, 0);
    let document: (String, String, String) = sqlx::query_as(
        "SELECT title, body_format, body FROM session_documents WHERE id = 'document-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        document,
        (
            "Keep this title".to_string(),
            "markdown".to_string(),
            "Legacy note".to_string()
        )
    );
    let target_status: String = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets
         WHERE run_id = 'run-1' AND target_id = 'document-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_status, "filled_from_legacy");
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed"
    );
}

#[tokio::test]
async fn nonempty_sqlite_document_wins_over_an_empty_legacy_document() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO session_documents
         (id, session_id, kind, body_format, body)
         VALUES ('document-1', 'session-1', 'note', 'markdown', 'SQLite note')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut incoming = document_row("document-1", "markdown", "  \n");
    let LegacyImportRow::Document(row) = &mut incoming else {
        unreachable!();
    };
    row.title = "Legacy title".to_string();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/session-1/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![incoming],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 1);
    assert_eq!(result.conflict_count, 0);
    let document: (String, String) =
        sqlx::query_as("SELECT title, body FROM session_documents WHERE id = 'document-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(document, (String::new(), "SQLite note".to_string()));
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed"
    );
}

#[tokio::test]
async fn document_title_is_filled_only_when_the_existing_title_is_empty() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO session_documents
         (id, session_id, kind, title, body_format, body)
         VALUES ('document-1', 'session-1', 'note', '', 'markdown', 'Same note')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let mut incoming = document_row("document-1", "markdown", "Same note");
    let LegacyImportRow::Document(row) = &mut incoming else {
        unreachable!();
    };
    row.title = "Legacy title".to_string();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/session-1/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![incoming],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 1);
    assert_eq!(result.conflict_count, 0);
    let title: String =
        sqlx::query_scalar("SELECT title FROM session_documents WHERE id = 'document-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title, "Legacy title");
}

#[tokio::test]
async fn divergent_nonempty_document_titles_remain_conflicted() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO session_documents
         (id, session_id, kind, title, body_format, body)
         VALUES ('document-1', 'session-1', 'note', 'SQLite title',
                 'markdown', 'Same note')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let mut incoming = document_row("document-1", "markdown", "Same note");
    let LegacyImportRow::Document(row) = &mut incoming else {
        unreachable!();
    };
    row.title = "Legacy title".to_string();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/session-1/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![incoming],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 0);
    assert_eq!(result.conflict_count, 1);
    let title: String =
        sqlx::query_scalar("SELECT title FROM session_documents WHERE id = 'document-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title, "SQLite title");
}

#[tokio::test]
async fn divergent_nonempty_documents_remain_conflicted_and_unchanged() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO session_documents
         (id, session_id, kind, body_format, body)
         VALUES ('document-1', 'session-1', 'note', 'markdown', 'SQLite note')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "document-item",
            run_id: "run-1",
            source_path: "sessions/session-1/_memo.md",
            source_kind: "session_document",
            source_sha256: "document-hash",
        },
        &LegacyImportBatch {
            rows: vec![document_row("document-1", "markdown", "Legacy note")],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 0);
    assert_eq!(result.conflict_count, 1);
    let body: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'document-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(body, "SQLite note");
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed_with_conflicts"
    );
    let parity_verified: bool = sqlx::query_scalar(
        "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!parity_verified);
}

#[tokio::test]
async fn run_outcome_and_migration_state_pointer_stay_consistent() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;

    let status = finish_legacy_import_run(db.pool(), "run-1").await.unwrap();
    let (latest_run_id, parity_verified, last_error): (String, bool, String) = sqlx::query_as(
        "SELECT latest_run_id, parity_verified, last_error
         FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(latest_run_id, "run-1");
    assert_eq!(parity_verified, status == "completed");
    let run_status: String =
        sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = 'run-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(run_status, status);
    assert_eq!(last_error.is_empty(), status == "completed");

    begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
        .await
        .unwrap();
    fail_legacy_import_run(db.pool(), "run-2", "disk full")
        .await
        .unwrap();
    let (latest_run_id, last_error): (String, String) = sqlx::query_as(
        "SELECT latest_run_id, last_error
         FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let (run_status, run_error): (String, String) =
        sqlx::query_as("SELECT status, error FROM migration_import_runs WHERE id = 'run-2'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(latest_run_id, "run-2");
    assert_eq!(run_status, "failed");
    assert_eq!(last_error, run_error);
    assert_eq!(last_error, "disk full");
}

#[tokio::test]
async fn dry_runs_never_touch_migration_state() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "dry-run-1", "/legacy", true)
        .await
        .unwrap();
    fail_legacy_import_run(db.pool(), "dry-run-1", "dry failure")
        .await
        .unwrap();

    let (latest_run_id, last_error): (String, String) = sqlx::query_as(
        "SELECT latest_run_id, last_error
         FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(latest_run_id, "");
    assert_eq!(last_error, "");
}

#[tokio::test]
async fn transcript_reconciliation_only_fills_an_empty_payload() {
    let db = test_db().await;
    begin_run_with_session(&db, "run-1").await;
    sqlx::query(
        "INSERT INTO transcripts
         (id, session_id, words_json, speaker_hints_json)
         VALUES
         ('fill', 'session-1', '[]', '[]'),
         ('retain', 'session-1', '[{\"id\":\"sqlite\"}]', '[]'),
         ('equivalent', 'session-1', '[ { \"id\": \"same\" } ]', '[]'),
         ('conflict', 'session-1', '[{\"id\":\"sqlite\"}]', '[]')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "transcript-item",
            run_id: "run-1",
            source_path: "sessions/session-1/transcript.json",
            source_kind: "transcript",
            source_sha256: "transcript-hash",
        },
        &LegacyImportBatch {
            rows: vec![
                transcript_row("fill", "[{\"id\":\"legacy\"}]"),
                transcript_row("retain", "[]"),
                transcript_row("equivalent", "[{\"id\":\"same\"}]"),
                transcript_row("conflict", "[{\"id\":\"legacy\"}]"),
            ],
            ..Default::default()
        },
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 3);
    assert_eq!(result.conflict_count, 1);
    let payloads: Vec<(String, String)> =
        sqlx::query_as("SELECT id, words_json FROM transcripts ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        payloads,
        vec![
            ("conflict".to_string(), "[{\"id\":\"sqlite\"}]".to_string()),
            (
                "equivalent".to_string(),
                "[ { \"id\": \"same\" } ]".to_string()
            ),
            ("fill".to_string(), "[{\"id\":\"legacy\"}]".to_string()),
            ("retain".to_string(), "[{\"id\":\"sqlite\"}]".to_string()),
        ]
    );
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed_with_conflicts"
    );
}

#[tokio::test]
async fn imported_session_children_inherit_parent_workspace() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO app_settings (id, value_json) \
         VALUES ('cloudsync_workspace_binding', \
           '{\"workspace_id\":\"workspace-1\",\"account_user_id\":\"user-1\"}') \
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
    )
    .execute(db.pool())
    .await
    .unwrap();
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();

    let mut batch = session_batch();
    batch.rows.extend([
        LegacyImportRow::Document(LegacyDocument {
            id: "document-1".to_string(),
            session_id: "session-1".to_string(),
            kind: "note".to_string(),
            template_id: String::new(),
            title: String::new(),
            body_format: "markdown".to_string(),
            body: "Notes".to_string(),
            source_hash: String::new(),
            sort_order: 0,
            created_by: "user-1".to_string(),
            created_at: "2026-07-10T12:00:00Z".to_string(),
            updated_at: "2026-07-10T12:00:00Z".to_string(),
            generation_metadata_json: "{}".to_string(),
            recovery_status: None,
        }),
        LegacyImportRow::Transcript(LegacyTranscript {
            id: "transcript-1".to_string(),
            owner_user_id: "user-1".to_string(),
            session_id: "session-1".to_string(),
            started_at_ms: 0,
            ended_at_ms: Some(1000),
            memo: String::new(),
            words_json: "[]".to_string(),
            speaker_hints_json: "[]".to_string(),
            created_at: "2026-07-10T12:00:00Z".to_string(),
            metadata_json: "{}".to_string(),
            recovery_status: None,
        }),
        LegacyImportRow::Participant(LegacyParticipant {
            id: "participant-1".to_string(),
            owner_user_id: "user-1".to_string(),
            session_id: "session-1".to_string(),
            human_id: "human-1".to_string(),
            source: "manual".to_string(),
        }),
        LegacyImportRow::ActionItem(LegacyActionItem {
            id: "action-1".to_string(),
            owner_user_id: "user-1".to_string(),
            session_id: "session-1".to_string(),
            source_type: "session".to_string(),
            source_id: "session-1".to_string(),
            source_order: 0,
            status: "todo".to_string(),
            text: "Follow up".to_string(),
            body_json: "{}".to_string(),
            due_at: String::new(),
        }),
        LegacyImportRow::Attachment(LegacyAttachment {
            id: "attachment-1".to_string(),
            session_id: "session-1".to_string(),
            filename: "notes.txt".to_string(),
            relative_path: "notes.txt".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: 5,
            sha256: "hash".to_string(),
            source_id: "legacy-1".to_string(),
        }),
    ]);

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1",
            source_kind: "session",
            source_sha256: "hash-1",
        },
        &batch,
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.imported_count, 6);
    for table in [
        "sessions",
        "session_documents",
        "transcripts",
        "session_participants",
        "action_items",
        "session_attachments",
    ] {
        let sql = format!("SELECT workspace_id FROM {table} LIMIT 1");
        let workspace_id: String = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(workspace_id, "workspace-1", "table {table}");
    }
}

#[tokio::test]
async fn preexisting_sqlite_domains_retain_newer_rows_without_blocking_parity() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO calendars \
         (id, tracking_id_calendar, name, enabled, provider, source, color, connection_id) \
         VALUES ('calendar-1', 'tracking-1', 'Work', 0, 'google', 'work@example.com', '#123456', 'connection-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO events \
         (id, tracking_id_event, calendar_id, title, started_at, ended_at, location, \
          meeting_link, description, note, recurrence_series_id, has_recurrence_rules, \
          is_all_day, provider, participants_json) \
         VALUES ('event-1', 'tracking-event-1', 'calendar-1', 'Updated title', \
                 '2026-07-11T10:00:00Z', '2026-07-11T11:00:00Z', '', '', '', '', '', 0, 0, \
                 'google', '[]')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();
    let batch = LegacyImportBatch {
        rows: vec![
            LegacyImportRow::Calendar(LegacyCalendar {
                id: "calendar-1".to_string(),
                tracking_id_calendar: "tracking-1".to_string(),
                name: "Work".to_string(),
                enabled: true,
                provider: "google".to_string(),
                source: "work@example.com".to_string(),
                color: "#123456".to_string(),
                connection_id: "connection-1".to_string(),
            }),
            LegacyImportRow::Event(LegacyEvent {
                id: "event-1".to_string(),
                tracking_id_event: "tracking-event-1".to_string(),
                calendar_id: "calendar-1".to_string(),
                title: "Stale title".to_string(),
                started_at: "2026-07-11T09:00:00Z".to_string(),
                ended_at: "2026-07-11T10:00:00Z".to_string(),
                location: String::new(),
                meeting_link: String::new(),
                description: String::new(),
                note: String::new(),
                recurrence_series_id: String::new(),
                has_recurrence_rules: false,
                is_all_day: false,
                provider: "google".to_string(),
                participants_json: Some("[]".to_string()),
            }),
        ],
        ..Default::default()
    };

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "calendar-data.json",
            source_kind: "calendar_data",
            source_sha256: "hash-1",
        },
        &batch,
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.matched_count, 2);
    assert_eq!(result.conflict_count, 0);
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed"
    );

    let target_statuses: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets WHERE run_id = 'run-1' ORDER BY target_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        target_statuses,
        vec!["retained_existing", "retained_existing"]
    );

    let calendar_enabled: bool =
        sqlx::query_scalar("SELECT enabled FROM calendars WHERE id = 'calendar-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let event_title: String = sqlx::query_scalar("SELECT title FROM events WHERE id = 'event-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(!calendar_enabled);
    assert_eq!(event_title, "Updated title");
}

#[tokio::test]
async fn dry_run_records_counts_without_writing_domain_rows() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "run-1", "/vault", true)
        .await
        .unwrap();

    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-1",
        },
        &session_batch(),
        true,
    )
    .await
    .unwrap();

    assert_eq!(result.discovered_count, 1);
    assert_eq!(result.imported_count, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn completed_source_hash_is_restartable() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();
    apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-1",
        },
        &session_batch(),
        false,
    )
    .await
    .unwrap();
    finish_legacy_import_run(db.pool(), "run-1").await.unwrap();

    assert!(
        legacy_source_already_imported(db.pool(), "sessions/session-1/_meta.json", "hash-1")
            .await
            .unwrap()
    );
    assert!(
        !legacy_source_already_imported(db.pool(), "sessions/session-1/_meta.json", "changed")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn successful_run_marks_import_parity_verified() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();
    apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "sessions/session-1/_meta.json",
            source_kind: "session_meta",
            source_sha256: "hash-1",
        },
        &session_batch(),
        false,
    )
    .await
    .unwrap();

    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
        "completed"
    );

    let parity_verified: bool = sqlx::query_scalar(
        "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(parity_verified);
}

#[tokio::test]
async fn identical_shared_rows_are_matched_instead_of_reported_as_conflicts() {
    let db = test_db().await;
    let batch = LegacyImportBatch {
        rows: vec![LegacyImportRow::Tag(LegacyTag {
            id: "work".to_string(),
            owner_user_id: "user-1".to_string(),
            name: "work".to_string(),
        })],
        ..Default::default()
    };

    for run in 1..=2 {
        let run_id = format!("run-{run}");
        let item_id = format!("item-{run}");
        let source_path = format!("sessions/session-{run}/_meta.json");
        let source_hash = format!("hash-{run}");
        begin_legacy_import_run(db.pool(), &run_id, "/vault", false)
            .await
            .unwrap();
        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: &item_id,
                run_id: &run_id,
                source_path: &source_path,
                source_kind: "session_meta",
                source_sha256: &source_hash,
            },
            &batch,
            false,
        )
        .await
        .unwrap();

        if run == 1 {
            assert_eq!(result.imported_count, 1);
            assert_eq!(result.matched_count, 0);
        } else {
            assert_eq!(result.imported_count, 0);
            assert_eq!(result.matched_count, 1);
            assert_eq!(result.conflict_count, 0);
        }
    }
}

#[tokio::test]
async fn first_import_restores_legacy_edits_over_untouched_default_templates() {
    let db = test_db().await;
    begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
        .await
        .unwrap();
    let batch = LegacyImportBatch {
        rows: vec![LegacyImportRow::Template(LegacyTemplate {
            id: "default-daily-standup".to_string(),
            title: "My Standup".to_string(),
            description: "User-edited legacy template".to_string(),
            pinned: true,
            pin_order: Some(1),
            category: Some("Custom".to_string()),
            targets_json: Some("[\"Engineering\"]".to_string()),
            sections_json: "[]".to_string(),
        })],
        ..Default::default()
    };

    apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-1",
            run_id: "run-1",
            source_path: "templates.json",
            source_kind: "template",
            source_sha256: "hash-1",
        },
        &batch,
        false,
    )
    .await
    .unwrap();

    let title: String =
        sqlx::query_scalar("SELECT title FROM templates WHERE id = 'default-daily-standup'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title, "My Standup");
    finish_legacy_import_run(db.pool(), "run-1").await.unwrap();

    sqlx::query(
        "UPDATE templates SET updated_at = '2099-01-01T00:00:00Z' \
         WHERE id = 'default-daily-standup'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
        .await
        .unwrap();
    let changed_batch = LegacyImportBatch {
        rows: vec![LegacyImportRow::Template(LegacyTemplate {
            id: "default-daily-standup".to_string(),
            title: "Stale Legacy Title".to_string(),
            description: String::new(),
            pinned: false,
            pin_order: None,
            category: None,
            targets_json: None,
            sections_json: "[]".to_string(),
        })],
        ..Default::default()
    };
    let result = apply_legacy_import_item(
        db.pool(),
        LegacyImportItem {
            id: "item-2",
            run_id: "run-2",
            source_path: "templates.json",
            source_kind: "template",
            source_sha256: "hash-2",
        },
        &changed_batch,
        false,
    )
    .await
    .unwrap();

    assert_eq!(result.matched_count, 1);
    assert_eq!(result.conflict_count, 0);
    let target_status: String = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets WHERE run_id = 'run-2' AND target_id = 'default-daily-standup'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_status, "retained_existing");
    assert_eq!(
        finish_legacy_import_run(db.pool(), "run-2").await.unwrap(),
        "completed"
    );

    let title: String =
        sqlx::query_scalar("SELECT title FROM templates WHERE id = 'default-daily-standup'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title, "My Standup");
}
