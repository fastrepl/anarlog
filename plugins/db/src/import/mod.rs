mod calendars;
mod cleanup;
mod events;
mod legacy_vault;
mod templates;

use std::path::PathBuf;

use sqlx::SqlitePool;

pub async fn import_legacy_data<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if !legacy_import_required(pool).await? {
        return Ok(());
    }

    let vault_base = resolve_startup_vault_base(app)?;
    let run_id = legacy_vault::import_legacy_vault(pool, &vault_base, false).await?;
    require_startup_ready_import(pool, &run_id).await
}

async fn require_startup_ready_import(pool: &SqlitePool, run_id: &str) -> crate::Result<()> {
    if legacy_import_required(pool).await? {
        return Err(std::io::Error::other(format!(
            "legacy import {run_id} did not pass parity verification; source files were left unchanged",
        ))
        .into());
    }

    Ok(())
}

async fn legacy_import_required(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let startup_ready: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM storage_migration_state AS state
           LEFT JOIN migration_import_runs AS run ON run.id = state.latest_run_id
           WHERE state.id = 'legacy_v1'
             AND (
               (state.importer_version = ? AND state.parity_verified = 1)
               OR (
                 run.importer_version = ?
                 AND run.dry_run = 0
                 AND run.status = 'completed_with_conflicts'
                 AND run.conflict_count > 0
                 AND run.skipped_count = 0
                 AND run.error_count = 0
               )
             )
         )",
    )
    .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
    .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
    .fetch_one(pool)
    .await?;

    Ok(!startup_ready)
}

pub async fn rerun_legacy_import(pool: &SqlitePool, dry_run: bool) -> crate::Result<String> {
    let source_root = sqlx::query_scalar::<_, String>(
        "SELECT source_root
         FROM migration_import_runs
         WHERE dry_run = 0 AND source_root <> ''
         ORDER BY started_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| std::io::Error::other("no legacy import source has been recorded"))?;

    legacy_vault::import_legacy_vault(pool, std::path::Path::new(&source_root), dry_run).await
}

pub async fn get_legacy_import_report(
    pool: &SqlitePool,
) -> crate::Result<crate::LegacyImportReport> {
    let state = sqlx::query_as::<_, crate::StorageMigrationState>(
        "SELECT phase, latest_run_id, parity_verified, cutover_at, rollback_until, last_error, updated_at
         FROM storage_migration_state
         WHERE id = 'legacy_v1'",
    )
    .fetch_one(pool)
    .await?;

    let latest_run = if state.latest_run_id.is_empty() {
        None
    } else {
        sqlx::query_as::<_, crate::LegacyImportRun>(
            "SELECT id, importer_version, source_root, dry_run, status, discovered_count,
                    imported_count, matched_count, skipped_count, conflict_count, error_count, started_at,
                    completed_at, error
             FROM migration_import_runs
             WHERE id = ?",
        )
        .bind(&state.latest_run_id)
        .fetch_optional(pool)
        .await?
    };

    let items = if state.latest_run_id.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, crate::LegacyImportItemReport>(
            "SELECT source_path, source_kind, source_sha256, status, discovered_count,
                    imported_count, matched_count, skipped_count, conflict_count, error
             FROM migration_import_items
             WHERE run_id = ?
             ORDER BY source_path",
        )
        .bind(&state.latest_run_id)
        .fetch_all(pool)
        .await?
    };

    let targets = if state.latest_run_id.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, crate::LegacyImportTargetReport>(
            "SELECT source_path, table_name, target_id, status, error
             FROM migration_import_targets
             WHERE run_id = ?
             ORDER BY table_name, target_id, source_path",
        )
        .bind(&state.latest_run_id)
        .fetch_all(pool)
        .await?
    };

    Ok(crate::LegacyImportReport {
        state,
        latest_run,
        items,
        targets,
    })
}

pub async fn get_legacy_cleanup_status(
    pool: &SqlitePool,
) -> crate::Result<crate::LegacyCleanupStatus> {
    cleanup::get_status(pool).await
}

pub async fn cleanup_legacy_files(pool: &SqlitePool) -> crate::Result<crate::LegacyCleanupResult> {
    cleanup::execute(pool).await
}

fn resolve_startup_vault_base<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<PathBuf> {
    let bundle_id: &str = app.config().identifier.as_ref();
    let settings_base = hypr_storage::global::compute_default_base(bundle_id)
        .ok_or(std::io::Error::other("settings base unavailable"))?;
    std::fs::create_dir_all(&settings_base)?;

    Ok(hypr_storage::vault::resolve_base(
        &settings_base,
        &settings_base,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn finish_issue_run(
        pool: &SqlitePool,
        run_id: &str,
        item_status: &str,
        skipped_count: i64,
        conflict_count: i64,
    ) -> String {
        hypr_db_app::begin_legacy_import_run(pool, run_id, "/vault", false)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO migration_import_items
             (id, run_id, source_path, source_kind, source_sha256, status,
              discovered_count, skipped_count, conflict_count, error, completed_at)
             VALUES (?, ?, 'source.json', 'test', 'hash', ?, 1, ?, ?, '',
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        )
        .bind(format!("item-{run_id}"))
        .bind(run_id)
        .bind(item_status)
        .bind(skipped_count)
        .bind(conflict_count)
        .execute(pool)
        .await
        .unwrap();

        hypr_db_app::finish_legacy_import_run(pool, run_id)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn verified_current_import_is_not_repeated_at_startup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());

        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();

        assert!(!legacy_import_required(db.pool()).await.unwrap());
        require_startup_ready_import(db.pool(), "verified-run")
            .await
            .unwrap();

        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = importer_version - 1
             WHERE id = 'legacy_v1'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());
    }

    #[tokio::test]
    async fn incomplete_import_still_blocks_startup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        let error = require_startup_ready_import(db.pool(), "run-with-errors")
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("did not pass parity verification")
        );
        assert!(
            error
                .to_string()
                .contains("source files were left unchanged")
        );
    }

    #[tokio::test]
    async fn conflict_only_import_allows_startup_without_verifying_parity() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        assert_eq!(
            finish_issue_run(db.pool(), "conflict-run", "conflict", 0, 1).await,
            "completed_with_conflicts"
        );

        let parity_verified: bool = sqlx::query_scalar(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(!parity_verified);
        assert!(!legacy_import_required(db.pool()).await.unwrap());
        require_startup_ready_import(db.pool(), "conflict-run")
            .await
            .unwrap();

        let cleanup_status = cleanup::get_status(db.pool()).await.unwrap();
        assert!(!cleanup_status.migration_verified);
        assert!(!cleanup_status.available);
        assert!(cleanup_status.blocking_reason.is_some());
    }

    #[tokio::test]
    async fn legacy_completed_with_issues_conflicts_remain_retryable() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        finish_issue_run(db.pool(), "legacy-conflict-run", "conflict", 0, 1).await;
        sqlx::query(
            "UPDATE migration_import_runs SET status = 'completed_with_issues' WHERE id = ?",
        )
        .bind("legacy-conflict-run")
        .execute(db.pool())
        .await
        .unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());
        assert!(
            require_startup_ready_import(db.pool(), "legacy-conflict-run")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn legacy_conflict_only_run_is_retried_once_and_then_allows_startup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let sqlite_document = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"SQLite note"}]}]}"#;
        let sqlite_words =
            r#"[{"id":"sqlite-word","text":"SQLite words","start_ms":0,"end_ms":10,"channel":0}]"#;
        sqlx::query(
            "INSERT INTO sessions
             (id, owner_user_id, title, created_at, started_at, ended_at, event_id,
              external_event_id, external_provider, series_id, event_json, folder_path)
             VALUES ('session-1', 'user-1', 'Planning', '2026-07-10T01:00:00Z',
                     '', '', '', '', '', '', '', '')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents
             (id, session_id, kind, body_format, body, created_at, updated_at)
             VALUES ('note-1', 'session-1', 'note', 'prosemirror_json', ?,
                     '2026-07-10T01:00:00Z', '2026-07-10T02:00:00Z')",
        )
        .bind(sqlite_document)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts
             (id, owner_user_id, session_id, started_at_ms, memo, words_json,
              speaker_hints_json, created_at)
             VALUES ('transcript-1', 'user-1', 'session-1', 0, 'SQLite memo', ?,
                     '[]', '2026-07-10T01:00:00Z')",
        )
        .bind(sqlite_words)
        .execute(db.pool())
        .await
        .unwrap();

        let vault = tempfile::tempdir().unwrap();
        let session_dir = vault.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta = br#"{"id":"session-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Planning"}"#;
        let note = b"---\nid: note-1\nsession_id: session-1\n---\n\nLegacy note";
        let transcript = br#"{"transcripts":[{"id":"transcript-1","user_id":"user-1","session_id":"session-1","created_at":"2026-07-10T01:00:00Z","started_at":0,"memo_md":"Legacy memo","words":[{"text":"Legacy words","start_ms":0,"end_ms":10,"channel":0}],"speaker_hints":[]}] }"#;
        std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
        std::fs::write(session_dir.join("_memo.md"), note).unwrap();
        std::fs::write(session_dir.join("transcript.json"), transcript).unwrap();

        let legacy_run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
            .await
            .unwrap();
        let legacy_run: (String, i64, i64, i64) = sqlx::query_as(
            "SELECT status, conflict_count, skipped_count, error_count
             FROM migration_import_runs WHERE id = ?",
        )
        .bind(&legacy_run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            legacy_run,
            ("completed_with_conflicts".to_string(), 2, 0, 0)
        );
        sqlx::query(
            "UPDATE migration_import_runs
             SET status = 'completed_with_issues'
             WHERE id = ?",
        )
        .bind(&legacy_run_id)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "UPDATE storage_migration_state
             SET last_error = 'completed_with_issues'
             WHERE id = 'legacy_v1' AND latest_run_id = ?",
        )
        .bind(&legacy_run_id)
        .execute(db.pool())
        .await
        .unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());
        let recovery_run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
            .await
            .unwrap();
        require_startup_ready_import(db.pool(), &recovery_run_id)
            .await
            .unwrap();

        let recovery_run: (String, i64, i64, i64) = sqlx::query_as(
            "SELECT status, conflict_count, skipped_count, error_count
             FROM migration_import_runs WHERE id = ?",
        )
        .bind(&recovery_run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            recovery_run,
            ("completed_with_conflicts".to_string(), 2, 0, 0)
        );
        let recovery_run_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM migration_import_runs
             WHERE status = 'completed_with_conflicts'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(recovery_run_count, 1);
        let total_run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(total_run_count, 2);

        let document_body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let transcript_data: (String, String) =
            sqlx::query_as("SELECT memo, words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(document_body, sqlite_document);
        assert_eq!(
            transcript_data,
            ("SQLite memo".to_string(), sqlite_words.to_string())
        );
        assert_eq!(std::fs::read(session_dir.join("_meta.json")).unwrap(), meta);
        assert_eq!(std::fs::read(session_dir.join("_memo.md")).unwrap(), note);
        assert_eq!(
            std::fs::read(session_dir.join("transcript.json")).unwrap(),
            transcript
        );

        let cleanup_status = cleanup::get_status(db.pool()).await.unwrap();
        assert!(!cleanup_status.migration_verified);
        assert!(!cleanup_status.available);
        assert!(cleanup_status.blocking_reason.is_some());

        assert!(!legacy_import_required(db.pool()).await.unwrap());
        let run_count_before_second_startup: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
                .fetch_one(db.pool())
                .await
                .unwrap();
        if legacy_import_required(db.pool()).await.unwrap() {
            let run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
                .await
                .unwrap();
            require_startup_ready_import(db.pool(), &run_id)
                .await
                .unwrap();
        }
        let run_count_after_second_startup: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            run_count_after_second_startup,
            run_count_before_second_startup
        );
    }

    #[tokio::test]
    async fn skipped_and_error_imports_remain_blocking_and_retryable() {
        for (run_id, item_status, skipped_count) in
            [("skipped-run", "partial", 1), ("error-run", "error", 0)]
        {
            let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
            hypr_db_app::prepare_schema(&db).await.unwrap();

            assert_eq!(
                finish_issue_run(db.pool(), run_id, item_status, skipped_count, 0).await,
                "completed_with_issues"
            );
            assert!(legacy_import_required(db.pool()).await.unwrap());
            assert!(
                require_startup_ready_import(db.pool(), run_id)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn orphaned_session_children_remain_blocking_and_retryable() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let vault = tempfile::tempdir().unwrap();
        let session_dir = vault.path().join("sessions/missing-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_memo.md"),
            "---\nid: orphan-note\nsession_id: missing-session\n---\n\nOrphaned note",
        )
        .unwrap();

        let run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
            .await
            .unwrap();
        let run: (String, i64, i64, i64) = sqlx::query_as(
            "SELECT status, skipped_count, conflict_count, error_count
             FROM migration_import_runs WHERE id = ?",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let target_status: String = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets
             WHERE run_id = ? AND target_id = 'orphan-note'",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(run, ("completed_with_issues".to_string(), 1, 0, 1));
        assert_eq!(target_status, "missing_dependency");
        assert!(legacy_import_required(db.pool()).await.unwrap());
        assert!(
            require_startup_ready_import(db.pool(), &run_id)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn document_and_transcript_conflicts_preserve_both_stores_and_allow_startup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let sqlite_document = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"SQLite note"}]}]}"#;
        let sqlite_words =
            r#"[{"id":"sqlite-word","text":"SQLite words","start_ms":0,"end_ms":10,"channel":0}]"#;
        sqlx::query(
            "INSERT INTO sessions
             (id, owner_user_id, title, created_at, started_at, ended_at, event_id,
              external_event_id, external_provider, series_id, event_json, folder_path)
             VALUES ('session-1', 'user-1', 'Planning', '2026-07-10T01:00:00Z',
                     '', '', '', '', '', '', '', '')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents
             (id, session_id, kind, body_format, body, created_at, updated_at)
             VALUES ('note-1', 'session-1', 'note', 'prosemirror_json',
                     ?,
                     '2026-07-10T01:00:00Z', '2026-07-10T02:00:00Z')",
        )
        .bind(sqlite_document)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts
             (id, owner_user_id, session_id, started_at_ms, memo, words_json,
              speaker_hints_json, created_at)
             VALUES ('transcript-1', 'user-1', 'session-1', 0, 'SQLite memo',
                     ?,
                     '[]', '2026-07-10T01:00:00Z')",
        )
        .bind(sqlite_words)
        .execute(db.pool())
        .await
        .unwrap();

        let vault = tempfile::tempdir().unwrap();
        let session_dir = vault.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta = br#"{"id":"session-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Planning"}"#;
        let note = b"---\nid: note-1\nsession_id: session-1\n---\n\nLegacy note";
        let transcript = br#"{"transcripts":[{"id":"transcript-1","user_id":"user-1","session_id":"session-1","created_at":"2026-07-10T01:00:00Z","started_at":0,"memo_md":"Legacy memo","words":[{"text":"Legacy words","start_ms":0,"end_ms":10,"channel":0}],"speaker_hints":[]}]}"#;
        std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
        std::fs::write(session_dir.join("_memo.md"), note).unwrap();
        std::fs::write(session_dir.join("transcript.json"), transcript).unwrap();

        let run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
            .await
            .unwrap();
        let run: (String, i64, i64, i64) = sqlx::query_as(
            "SELECT status, conflict_count, skipped_count, error_count
             FROM migration_import_runs WHERE id = ?",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(run, ("completed_with_conflicts".to_string(), 2, 0, 0));

        let targets: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name, status FROM migration_import_targets
             WHERE run_id = ? ORDER BY table_name",
        )
        .bind(&run_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            targets,
            vec![
                ("session_documents".to_string(), "conflict".to_string()),
                ("sessions".to_string(), "matched".to_string()),
                ("transcripts".to_string(), "conflict".to_string()),
            ]
        );

        let document_body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'note-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let transcript_memo: String =
            sqlx::query_scalar("SELECT memo FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let transcript_words: String =
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(document_body, sqlite_document);
        assert_eq!(transcript_memo, "SQLite memo");
        assert_eq!(transcript_words, sqlite_words);
        assert_eq!(std::fs::read(session_dir.join("_meta.json")).unwrap(), meta);
        assert_eq!(std::fs::read(session_dir.join("_memo.md")).unwrap(), note);
        assert_eq!(
            std::fs::read(session_dir.join("transcript.json")).unwrap(),
            transcript
        );

        let parity_verified: bool = sqlx::query_scalar(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(!parity_verified);
        assert!(!legacy_import_required(db.pool()).await.unwrap());
        require_startup_ready_import(db.pool(), &run_id)
            .await
            .unwrap();

        let cleanup_status = cleanup::get_status(db.pool()).await.unwrap();
        assert!(!cleanup_status.migration_verified);
        assert!(!cleanup_status.available);
        assert!(cleanup_status.blocking_reason.is_some());
    }

    #[tokio::test]
    async fn stale_snapshots_for_preexisting_sqlite_domains_do_not_block_cutover() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
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

        let vault = tempfile::tempdir().unwrap();
        std::fs::write(
            vault.path().join("calendars.json"),
            r##"{
              "calendar-1": {
                "tracking_id_calendar": "tracking-1",
                "name": "Work",
                "enabled": true,
                "provider": "google",
                "source": "work@example.com",
                "color": "#123456",
                "connection_id": "connection-1"
              }
            }"##,
        )
        .unwrap();
        std::fs::write(
            vault.path().join("events.json"),
            r#"{
              "event-1": {
                "tracking_id_event": "tracking-event-1",
                "calendar_id": "calendar-1",
                "title": "Stale title",
                "started_at": "2026-07-11T09:00:00Z",
                "ended_at": "2026-07-11T10:00:00Z",
                "provider": "google",
                "participants": []
              }
            }"#,
        )
        .unwrap();

        let run_id = legacy_vault::import_legacy_vault(db.pool(), vault.path(), false)
            .await
            .unwrap();

        require_startup_ready_import(db.pool(), &run_id)
            .await
            .unwrap();
        let target_statuses: Vec<String> = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets WHERE run_id = ? ORDER BY target_id",
        )
        .bind(&run_id)
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
        let event_title: String =
            sqlx::query_scalar("SELECT title FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(!calendar_enabled);
        assert_eq!(event_title, "Updated title");
    }

    #[tokio::test]
    async fn verified_file_import_survives_cloudsync_reopen_without_rerun() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let vault = dir.path().join("vault");
        let session_dir = vault.join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Imported before restart"}"#,
        )
        .unwrap();

        let db = crate::runtime::open_app_db(Some(&db_path)).await.unwrap();
        assert!(db.cloudsync_enabled());
        assert!(legacy_import_required(db.pool()).await.unwrap());

        let run_id = legacy_vault::import_legacy_vault(db.pool(), &vault, false)
            .await
            .unwrap();
        let run_status: String =
            sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let state_before: (String, bool, i64) = sqlx::query_as(
            "SELECT latest_run_id, parity_verified, importer_version
             FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let run_count_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
                .fetch_one(db.pool())
                .await
                .unwrap();

        assert_eq!(run_status, "completed");
        assert_eq!(state_before.0, run_id);
        assert!(state_before.1);
        assert_eq!(state_before.2, hypr_db_app::LEGACY_IMPORTER_VERSION);
        assert!(!legacy_import_required(db.pool()).await.unwrap());

        db.pool().close().await;
        drop(db);

        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Changed after verified import"}"#,
        )
        .unwrap();

        let reopened = crate::runtime::open_app_db(Some(&db_path)).await.unwrap();
        assert!(reopened.cloudsync_enabled());

        if legacy_import_required(reopened.pool()).await.unwrap() {
            legacy_vault::import_legacy_vault(reopened.pool(), &vault, false)
                .await
                .unwrap();
        }

        let state_after: (String, bool, i64) = sqlx::query_as(
            "SELECT latest_run_id, parity_verified, importer_version
             FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(reopened.pool())
        .await
        .unwrap();
        let run_count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
            .fetch_one(reopened.pool())
            .await
            .unwrap();
        let stored_title: String =
            sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
                .fetch_one(reopened.pool())
                .await
                .unwrap();

        assert_eq!(state_after, state_before);
        assert_eq!(run_count_after, run_count_before);
        assert_eq!(stored_title, "Imported before restart");
        assert!(!legacy_import_required(reopened.pool()).await.unwrap());
        require_startup_ready_import(reopened.pool(), &run_id)
            .await
            .unwrap();
    }
}
