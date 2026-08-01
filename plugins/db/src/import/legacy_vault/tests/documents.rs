use super::*;

#[tokio::test]
async fn divergent_duplicate_document_ids_are_recovered_with_provenance() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Planning"}"#,
    )
    .unwrap();
    let first = "---\nid: shared-document\nsession_id: session-1\n---\n\nFirst body";
    let second = "---\nid: shared-document\nsession_id: session-1\n---\n\nSecond body";
    std::fs::write(session_dir.join("a.md"), first).unwrap();
    std::fs::write(session_dir.join("b.md"), second).unwrap();
    let recovered_id = stable_id(&format!(
        "legacy-recovered-document:shared-document:sessions/session-1/b.md:{}",
        sha256(second.as_bytes())
    ));

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let documents: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, body, generation_metadata_json FROM session_documents ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(documents.len(), 2);
    assert_eq!(
        documents
            .iter()
            .find(|document| document.0 == "shared-document")
            .unwrap()
            .1,
        "First body"
    );
    let recovered = documents
        .iter()
        .find(|document| document.0 == recovered_id)
        .unwrap();
    assert_eq!(recovered.1, "Second body");
    let provenance = serde_json::from_str::<Value>(&recovered.2).unwrap();
    assert_eq!(
        provenance["legacy_recovery"]["reason"],
        "duplicate_document_id"
    );
    assert_eq!(
        provenance["legacy_recovery"]["original_id"],
        "shared-document"
    );
    let recovered_status: String = sqlx::query_scalar(
        "SELECT status FROM migration_import_targets WHERE run_id = ? AND target_id = ?",
    )
    .bind(&run_id)
    .bind(&recovered_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(recovered_status, RECOVERED_DUPLICATE_DOCUMENT_ID);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM migration_import_runs WHERE id = ?",)
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "completed"
    );

    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    assert_eq!(
        row_count(&db, "SELECT COUNT(*) FROM session_documents").await,
        2
    );
}

#[tokio::test]
async fn identical_duplicate_document_ids_do_not_fork() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Planning"}"#,
    )
    .unwrap();
    let document = "---\nid: shared-document\nsession_id: session-1\n---\n\nSame body";
    std::fs::write(session_dir.join("a.md"), document).unwrap();
    std::fs::write(session_dir.join("b.md"), document).unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    assert_eq!(
        row_count(&db, "SELECT COUNT(*) FROM session_documents").await,
        1
    );
    let run: (String, i64) =
        sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(run, ("completed".to_string(), 0));
}

#[tokio::test]
async fn nonempty_canonical_summary_shadows_empty_hidden_artifact() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
    )
    .unwrap();
    std::fs::write(
        session_dir.join(".md"),
        "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\n",
    )
    .unwrap();
    std::fs::write(
        session_dir.join("_summary.md"),
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCurrent summary",
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
    let body: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let hidden_item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_import_items \
         WHERE run_id = ? AND source_path = 'sessions/session-1/.md'",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(status, "completed");
    assert_eq!(body, "Current summary");
    assert_eq!(hidden_item_count, 0);
}

#[tokio::test]
async fn nonempty_hidden_summary_is_not_shadowed_by_empty_canonical_summary() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
    )
    .unwrap();
    std::fs::write(
        session_dir.join(".md"),
        "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\nKeep this summary",
    )
    .unwrap();
    std::fs::write(
        session_dir.join("_summary.md"),
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\n",
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let run: (String, i64) =
        sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let body: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let hidden_item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_import_items
         WHERE run_id = ? AND source_path = 'sessions/session-1/.md'",
    )
    .bind(&run_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(run, ("completed".to_string(), 0));
    assert_eq!(body, "Keep this summary");
    assert_eq!(hidden_item_count, 1);
}

#[tokio::test]
async fn divergent_summaries_preserve_both_documents_without_conflict() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
    )
    .unwrap();
    std::fs::write(
        session_dir.join(".md"),
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nHidden recovery copy",
    )
    .unwrap();
    std::fs::write(
        session_dir.join("_summary.md"),
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCanonical summary",
    )
    .unwrap();

    let run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let run: (String, i64) =
        sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let bodies: Vec<String> =
        sqlx::query_scalar("SELECT body FROM session_documents ORDER BY body")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let targets: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_path, status FROM migration_import_targets
         WHERE run_id = ? AND table_name = 'session_documents'
         ORDER BY source_path",
    )
    .bind(&run_id)
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(run, ("completed".to_string(), 0));
    assert_eq!(bodies, vec!["Canonical summary", "Hidden recovery copy"]);
    assert_eq!(
        targets,
        vec![
            (
                "sessions/session-1/.md".to_string(),
                RECOVERED_DUPLICATE_DOCUMENT_ID.to_string()
            ),
            (
                "sessions/session-1/_summary.md".to_string(),
                "inserted".to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn empty_session_title_is_recovered_from_summary_heading() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("_meta.json"),
        r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":""}"#,
    )
    .unwrap();
    std::fs::write(
        session_dir.join("_summary.md"),
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\n# Transcript Test Utterances\n\nDetails",
    )
    .unwrap();

    import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();

    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(title, "Transcript Test Utterances");
}

#[tokio::test]
async fn divergent_nonempty_summaries_recover_after_a_partial_import() {
    let db = test_db().await;
    let dir = tempfile::tempdir().unwrap();
    let session_dir = dir.path().join("sessions/session-1");
    std::fs::create_dir_all(&session_dir).unwrap();
    let meta = r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#;
    let hidden = "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\nStale summary";
    let canonical =
        "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCurrent summary";
    std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
    std::fs::write(session_dir.join(".md"), hidden).unwrap();
    std::fs::write(session_dir.join("_summary.md"), canonical).unwrap();

    let failed_run_id = "failed-run";
    anlg_db_app::begin_legacy_import_run(
        db.pool(),
        failed_run_id,
        &dir.path().to_string_lossy(),
        false,
    )
    .await
    .unwrap();
    for (item_id, source_path, source_kind, source_hash, batch) in [
        (
            "meta-item",
            "sessions/session-1/_meta.json",
            "session_meta",
            sha256(meta.as_bytes()),
            parse_session_meta(dir.path(), &session_dir.join("_meta.json"), meta).unwrap(),
        ),
        (
            "hidden-item",
            "sessions/session-1/.md",
            "session_document",
            sha256(hidden.as_bytes()),
            parse_session_document(
                dir.path(),
                &session_dir.join(".md"),
                hidden,
                &sha256(hidden.as_bytes()),
            )
            .unwrap(),
        ),
        (
            "canonical-item",
            "sessions/session-1/_summary.md",
            "session_document",
            sha256(canonical.as_bytes()),
            parse_session_document(
                dir.path(),
                &session_dir.join("_summary.md"),
                canonical,
                &sha256(canonical.as_bytes()),
            )
            .unwrap(),
        ),
    ] {
        anlg_db_app::apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: item_id,
                run_id: failed_run_id,
                source_path,
                source_kind,
                source_sha256: &source_hash,
            },
            &batch,
            false,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        anlg_db_app::finish_legacy_import_run(db.pool(), failed_run_id)
            .await
            .unwrap(),
        "completed_with_conflicts"
    );
    sqlx::query(
        "UPDATE migration_import_runs
         SET status = 'completed_with_issues'
         WHERE id = ?",
    )
    .bind(failed_run_id)
    .execute(db.pool())
    .await
    .unwrap();

    let recovery_run_id = import_legacy_vault(db.pool(), dir.path(), false)
        .await
        .unwrap();
    let bodies: Vec<String> =
        sqlx::query_scalar("SELECT body FROM session_documents ORDER BY body")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let status: String =
        sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
            .bind(&recovery_run_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let parity_verified: bool = sqlx::query_scalar(
        "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(bodies, vec!["Current summary", "Stale summary"]);
    assert_eq!(status, "completed");
    assert!(parity_verified);
}
