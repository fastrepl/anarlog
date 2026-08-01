use super::*;

#[tokio::test]
async fn malformed_sources_are_reported_without_aborting_other_imports() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tasks.json"), "{not-json").unwrap();
    std::fs::write(
        dir.path().join("daily_notes.json"),
        r#"{"daily-1":{"user_id":"user-1","date":"2026-07-10","content":"{}"}}"#,
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let status: String =
        sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "completed_with_issues");
    let error: String = sqlx::query_scalar(
        "SELECT error FROM migration_import_items \
         WHERE run_id = ? AND source_path = 'tasks.json'",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!error.is_empty());
    assert_eq!(row_count(&db, "SELECT COUNT(*) FROM daily_notes").await, 1);
}

#[tokio::test]
async fn shadow_import_is_non_destructive_idempotent_and_audited() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/work/session-1");
    std::fs::create_dir_all(session_dir.join("attachments")).unwrap();
    std::fs::create_dir_all(dir.path().join("humans")).unwrap();
    std::fs::create_dir_all(dir.path().join("organizations")).unwrap();
    std::fs::create_dir_all(dir.path().join("chats/chat-1")).unwrap();

    let meta = r#"{
      "id":"session-1",
      "user_id":"user-1",
      "created_at":"2026-07-10T01:00:00Z",
      "title":"Planning",
      "participants":[{"id":"participant-1","user_id":"user-1","human_id":"human-1","source":"manual"}],
      "key_facts":{"content":"Fact","source_hash":"source-hash"},
      "tags":["work"]
    }"#;
    std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
    std::fs::write(
        session_dir.join("_memo.md"),
        "---\nid: session-1\nsession_id: session-1\n---\n\nMeeting note",
    )
    .unwrap();
    std::fs::write(
        session_dir.join("transcript.json"),
        r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","started_at":0,"words":[{"text":"hello","start_ms":0,"end_ms":10,"channel":0}],"speaker_hints":[]}]}"#,
    )
    .unwrap();
    std::fs::write(session_dir.join("attachments/file.txt"), b"attachment").unwrap();
    std::fs::write(
        dir.path().join("humans/human-1.md"),
        "---\nuser_id: user-1\nname: Alice\nemails: [alice@example.com]\norg_id: org-1\n---\n\nMemo",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("organizations/org-1.md"),
        "---\nuser_id: user-1\nname: Acme\n---\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("tasks.json"),
        r#"{"task-1":{"user_id":"user-1","task_id":"task-1","source_id":"session-1","source_type":"session","source_order":0,"status":"todo","text_preview":"Follow up","body":[]}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("daily_notes.json"),
        r#"{"daily-1":{"user_id":"user-1","date":"2026-07-10","content":"{}"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("chats/chat-1/messages.json"),
        r#"{"chat_group":{"id":"chat-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Chat"},"messages":[{"id":"message-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","chat_group_id":"chat-1","role":"user","content":"Hi","metadata":{},"parts":[],"status":"ready"}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"general":{"theme":"dark"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calendars.json"),
        r##"{"calendar-1":{"tracking_id_calendar":"calendar-track-1","name":"Work","enabled":true,"provider":"google","source":"account","color":"#123456","connection_id":"connection-1"}}"##,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("events.json"),
        r#"{"event-1":{"tracking_id_event":"event-track-1","calendar_id":"calendar-1","title":"Planning","started_at":"2026-07-10T01:00:00Z","ended_at":"2026-07-10T02:00:00Z","provider":"google","participants":[]}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("templates.json"),
        r#"{"template-custom":{"title":"Custom","description":"Legacy template","sections":[]}}"#,
    )
    .unwrap();

    let source_before = std::fs::read(session_dir.join("_meta.json")).unwrap();
    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read(session_dir.join("_meta.json")).unwrap(),
        source_before
    );
    for (table, query, expected) in [
        ("sessions", "SELECT COUNT(*) FROM sessions", 1_i64),
        (
            "session_documents",
            "SELECT COUNT(*) FROM session_documents",
            2,
        ),
        ("transcripts", "SELECT COUNT(*) FROM transcripts", 1),
        (
            "session_participants",
            "SELECT COUNT(*) FROM session_participants",
            1,
        ),
        ("tags", "SELECT COUNT(*) FROM tags", 1),
        ("session_tags", "SELECT COUNT(*) FROM session_tags", 1),
        (
            "session_attachments",
            "SELECT COUNT(*) FROM session_attachments",
            1,
        ),
        ("humans", "SELECT COUNT(*) FROM humans", 1),
        ("organizations", "SELECT COUNT(*) FROM organizations", 1),
        ("action_items", "SELECT COUNT(*) FROM action_items", 1),
        ("daily_notes", "SELECT COUNT(*) FROM daily_notes", 1),
        ("chat_groups", "SELECT COUNT(*) FROM chat_groups", 1),
        ("chat_messages", "SELECT COUNT(*) FROM chat_messages", 1),
        ("app_settings", "SELECT COUNT(*) FROM app_settings", 2),
        ("calendars", "SELECT COUNT(*) FROM calendars", 1),
        ("events", "SELECT COUNT(*) FROM events", 1),
        ("templates", "SELECT COUNT(*) FROM templates", 18),
    ] {
        assert_eq!(row_count(&db, query).await, expected, "{table}");
    }
    assert_eq!(
        row_count(
            &db,
            "SELECT COUNT(*) FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .await,
        1,
    );
    assert_eq!(
        row_count(
            &db,
            "SELECT COUNT(*) FROM app_settings WHERE id = 'legacy_settings_document'",
        )
        .await,
        1,
    );

    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(run_count, 2);
    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_items")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(item_count, 26);

    let unchanged_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_import_items WHERE status = 'unchanged'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(unchanged_count, 12);

    let report = super::super::super::get_legacy_import_report(db.pool())
        .await
        .unwrap();
    assert_eq!(report.items.len(), 13);
    assert_eq!(
        report
            .items
            .iter()
            .filter(|item| item.status == "unchanged")
            .count(),
        12
    );
    assert_eq!(
        report
            .items
            .iter()
            .filter(|item| item.status == "complete")
            .count(),
        1
    );
    assert_eq!(report.targets.len(), 18);
    assert_eq!(
        report
            .targets
            .iter()
            .filter(|target| target.status == "unchanged")
            .count(),
        17
    );
    assert_eq!(
        report
            .targets
            .iter()
            .filter(|target| target.status == "matched")
            .count(),
        1
    );
    let latest_run = report.latest_run.unwrap();
    assert_eq!(latest_run.status, "completed");
    assert_eq!(latest_run.matched_count, 18);
}
