use super::*;

#[test]
fn transcript_import_synthesizes_stable_word_ids_without_reordering() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions/session-1/transcript.json");
    let batch = parse_transcript(
        dir.path(),
        &path,
        r#"{
          "transcripts": [{
            "id": "transcript-1",
            "session_id": "session-1",
            "started_at": 10.4,
            "words": [
              {"text":"hello","start_ms":10,"end_ms":20,"channel":0},
              {"id":"word-existing","text":"world","start_ms":21,"end_ms":30,"channel":0}
            ],
            "speaker_hints": []
          }]
        }"#,
        "transcript-hash",
    )
    .unwrap();

    let transcript = batch
        .rows
        .iter()
        .find_map(|row| match row {
            LegacyImportRow::Transcript(transcript) => Some(transcript),
            _ => None,
        })
        .expect("expected transcript");
    assert_eq!(transcript.started_at_ms, 10);
    let words: Vec<Value> = serde_json::from_str(&transcript.words_json).unwrap();
    assert_eq!(words[0]["id"], "transcript-1:word:0");
    assert_eq!(words[1]["id"], "word-existing");
    assert_eq!(words[0]["text"], "hello");
    assert_eq!(words[1]["text"], "world");
}

#[tokio::test]
async fn orphan_transcript_recovers_a_visible_session_losslessly_and_idempotently() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    let transcript_path = session_dir.join("transcript.json");
    std::fs::write(
        &transcript_path,
        r#"{"transcripts":[{"id":"transcript-1","user_id":"user-1","session_id":"session-1","created_at":"2026-07-11T09:30:00Z","started_at":10,"ended_at":30,"memo_md":"Recovered memo","words":[{"text":"hello","start_ms":10,"end_ms":20,"channel":0},{"text":"again","start_ms":21,"end_ms":30,"channel":0}],"speaker_hints":[]}]}"#,
    )
    .unwrap();
    let source_before = std::fs::read(&transcript_path).unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let run: (String, i64, i64) = sqlx::query_as(
        "SELECT status, imported_count, conflict_count FROM migration_import_runs WHERE id = ?",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(run, ("completed".to_string(), 2, 0));
    let session: (String, String) =
        sqlx::query_as("SELECT title, metadata_json FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(session.0, "Recovered transcript — 2026-07-11");
    assert_eq!(
        serde_json::from_str::<Value>(&session.1).unwrap()["legacy_recovery"]["reason"],
        "missing_session_metadata"
    );
    let transcript: (String, String, String) = sqlx::query_as(
        "SELECT memo, words_json, metadata_json FROM transcripts WHERE id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(transcript.0, "Recovered memo");
    assert_eq!(
        serde_json::from_str::<Vec<Value>>(&transcript.1)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        serde_json::from_str::<Value>(&transcript.2).unwrap()["legacy_recovery"]["reason"],
        "missing_session_metadata"
    );
    let targets: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
    )
    .bind(&run_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        targets,
        vec![
            (
                "sessions".to_string(),
                RECOVERED_MISSING_SESSION_METADATA.to_string()
            ),
            (
                "transcripts".to_string(),
                RECOVERED_MISSING_SESSION_METADATA.to_string()
            ),
        ]
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
    );

    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    assert_eq!(row_count(&db, "SELECT COUNT(*) FROM sessions").await, 1);
    assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 1);
    assert_eq!(std::fs::read(&transcript_path).unwrap(), source_before);
}

#[tokio::test]
async fn canonical_session_wins_when_an_orphan_transcript_is_recovered() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    let meta_path = session_dir.join("_meta.json");
    std::fs::write(
        &meta_path,
        r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Canonical title"}"#,
    )
    .unwrap();
    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    std::fs::remove_file(meta_path).unwrap();
    std::fs::write(
        session_dir.join("transcript.json"),
        r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","created_at":"2026-07-12T09:30:00Z","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "Canonical title"
    );
    assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 1);
    let targets: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
    )
    .bind(&run_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        targets,
        vec![
            ("sessions".to_string(), "retained_existing".to_string()),
            (
                "transcripts".to_string(),
                RECOVERED_MISSING_SESSION_METADATA.to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn canonical_metadata_replaces_a_recovered_placeholder() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("transcript.json"),
        r#"{"transcripts":[{"id":"transcript-1","user_id":"transcript-user","session_id":"session-1","created_at":"2026-07-11T09:30:00Z","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
    )
    .unwrap();
    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","user_id":"canonical-user","created_at":"2026-07-10T08:00:00Z","title":"Canonical title"}"#,
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let session: (String, String, String, String) = sqlx::query_as(
        "SELECT owner_user_id, title, created_at, metadata_json FROM sessions WHERE id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        session,
        (
            "canonical-user".to_string(),
            "Canonical title".to_string(),
            "2026-07-10T08:00:00Z".to_string(),
            "{}".to_string(),
        )
    );
    let target_status: String = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets WHERE run_id = ? AND source_kind = 'session_meta'",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_status, "filled_from_legacy");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM migration_import_runs WHERE id = ?",)
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "completed"
    );
}

#[tokio::test]
async fn deleted_session_is_not_reused_for_an_orphan_transcript() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    let meta_path = session_dir.join("_meta.json");
    std::fs::write(
        &meta_path,
        r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Deleted"}"#,
    )
    .unwrap();
    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET deleted_at = '2026-07-12T00:00:00Z' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    std::fs::remove_file(meta_path).unwrap();
    let transcript_path = session_dir.join("transcript.json");
    std::fs::write(
        &transcript_path,
        r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 0);
    assert!(transcript_path.is_file());
    let run: (String, i64) =
        sqlx::query_as("SELECT status, skipped_count FROM migration_import_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(run, ("completed_with_issues".to_string(), 2));
    let target_statuses: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
    )
    .bind(&run_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        target_statuses,
        vec![
            ("sessions".to_string(), "missing_dependency".to_string()),
            ("transcripts".to_string(), "missing_dependency".to_string()),
        ]
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
}
