use super::*;

#[test]
fn attachment_local_state_is_plain_and_excluded_from_cloudsync() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717140000_attachment_local_state")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "attachment_local_state")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"attachment_local_state"));
    assert!(!cloudsync_alter_guard_required("attachment_local_state"));
}

#[test]
fn attachment_transfer_jobs_are_plain_and_excluded_from_cloudsync() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717150000_attachment_transfer_jobs")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "attachment_transfer_jobs")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"attachment_transfer_jobs"));
    assert!(!cloudsync_alter_guard_required("attachment_transfer_jobs"));
}

#[test]
fn attachment_transfer_jobs_migration_checksums_are_pinned() {
    assert_eq!(
        checksum_hex(&migration_checksum(&legacy_attachment_transfer_jobs_sql())),
        LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM
    );
    assert_eq!(
        checksum_hex(&migration_checksum(include_str!(
            "../../migrations/20260717150000_attachment_transfer_jobs.sql"
        ))),
        CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM
    );
    assert_ne!(
        normalize_schema_sql("WHERE direction = 'delete'"),
        normalize_schema_sql("WHERE direction = 'DELETE'")
    );
}

#[tokio::test]
async fn repairs_known_attachment_transfer_job_indexes_without_dropping_jobs() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(&db, schema_with_legacy_attachment_transfer_jobs())
        .await
        .unwrap();
    assert_eq!(
        attachment_transfer_jobs_schema_checksum_for_test(&db).await,
        LEGACY_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM
    );
    sqlx::query(
        "INSERT INTO attachment_transfer_jobs (
                id, attachment_id, session_id, workspace_id, direction,
                expected_sha256, expected_size_bytes, ciphertext_sha256,
                ciphertext_size_bytes, remote_object_id, object_key, cache_id,
                phase, attempt_count, next_attempt_at, last_attempt_at,
                last_error, created_at, updated_at, completed_at
             ) VALUES (
                'job-1', 'attachment-1', 'session-1', 'workspace-1', 'upload',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                42,
                'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                99, 'object-1', 'objects/object-1', 'cache-1', 'completed', 2,
                '2026-07-17T01:00:00.000Z', '2026-07-17T00:30:00.000Z',
                'previous retry', '2026-07-17T00:00:00.000Z',
                '2026-07-17T00:45:00.000Z', '2026-07-17T00:45:00.000Z'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let row_snapshot_sql = "SELECT json_object(
            'id', id,
            'attachment_id', attachment_id,
            'session_id', session_id,
            'workspace_id', workspace_id,
            'direction', direction,
            'expected_sha256', expected_sha256,
            'expected_size_bytes', expected_size_bytes,
            'ciphertext_sha256', ciphertext_sha256,
            'ciphertext_size_bytes', ciphertext_size_bytes,
            'remote_object_id', remote_object_id,
            'object_key', object_key,
            'cache_id', cache_id,
            'phase', phase,
            'attempt_count', attempt_count,
            'next_attempt_at', next_attempt_at,
            'last_attempt_at', last_attempt_at,
            'last_error', last_error,
            'created_at', created_at,
            'updated_at', updated_at,
            'completed_at', completed_at
         ) FROM attachment_transfer_jobs WHERE id = 'job-1'";
    let row_before: String = sqlx::query_scalar(row_snapshot_sql)
        .fetch_one(db.pool())
        .await
        .unwrap();

    prepare_schema(&db).await.unwrap();
    prepare_schema(&db).await.unwrap();

    let checksum: String = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
             FROM _sqlx_migrations WHERE version = ?",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(checksum, CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM);
    assert_eq!(
        attachment_transfer_jobs_schema_checksum_for_test(&db).await,
        CURRENT_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM
    );
    let row_after: String = sqlx::query_scalar(row_snapshot_sql)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row_after, row_before);

    sqlx::query(
        "INSERT INTO attachment_transfer_jobs (
                id, attachment_id, session_id, workspace_id, direction,
                expected_sha256, remote_object_id
             ) VALUES (
                'job-2', 'attachment-2', 'session-1', 'workspace-1', 'upload',
                'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                'object-1'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachment_transfer_jobs")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(jobs, 2);
}

#[tokio::test]
async fn unknown_attachment_transfer_jobs_checksum_fails_without_mutation() {
    let db = test_db().await;
    let original_index_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master
             WHERE type = 'index'
               AND name = 'idx_attachment_transfer_jobs_upload_object_id'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE _sqlx_migrations SET checksum = X'00'
             WHERE version = ?",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .execute(db.pool())
    .await
    .unwrap();

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(
        error,
        AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs migration has an unknown checksum"
        )
    ));
    let index_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master
             WHERE type = 'index'
               AND name = 'idx_attachment_transfer_jobs_upload_object_id'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(index_sql, original_index_sql);
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(checksum, [0]);
}

#[tokio::test]
async fn legacy_attachment_transfer_checksum_rejects_a_changed_schema() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(&db, schema_with_legacy_attachment_transfer_jobs())
        .await
        .unwrap();
    sqlx::raw_sql(
        "DROP INDEX idx_attachment_transfer_jobs_due;
             CREATE INDEX idx_attachment_transfer_jobs_due
             ON attachment_transfer_jobs(created_at, phase, next_attempt_at);",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let changed_schema = attachment_transfer_jobs_schema_checksum_for_test(&db).await;

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(
        error,
        AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs migration has an unexpected schema"
        )
    ));
    assert_eq!(
        attachment_transfer_jobs_schema_checksum_for_test(&db).await,
        changed_schema
    );
    let checksum: String = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
             FROM _sqlx_migrations WHERE version = ?",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(checksum, LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM);
}

#[tokio::test]
async fn legacy_attachment_transfer_checksum_rejects_current_indexes() {
    let db = test_db().await;
    sqlx::query(
            "UPDATE _sqlx_migrations
             SET checksum = X'FD846A54C653D8E737371A950747B442BBA2DA055566DBCD35907B36FE7829A3A09C81052346BB05100BA71BE552EFC0'
             WHERE version = ?",
        )
        .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
        .execute(db.pool())
        .await
        .unwrap();

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(
        error,
        AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs migration has an unexpected schema"
        )
    ));
    assert_eq!(
        attachment_transfer_jobs_schema_checksum_for_test(&db).await,
        CURRENT_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM
    );
}

#[tokio::test]
async fn attachment_transfer_job_index_repair_rolls_back_when_checksum_update_fails() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(&db, schema_with_legacy_attachment_transfer_jobs())
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TRIGGER block_attachment_transfer_jobs_checksum_update
             BEFORE UPDATE OF checksum ON _sqlx_migrations
             WHEN OLD.version = 20260717150000
             BEGIN
               SELECT RAISE(ABORT, 'blocked checksum update');
             END;",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(error, AppSchemaError::Sqlx(_)));
    assert_eq!(
        attachment_transfer_jobs_schema_checksum_for_test(&db).await,
        LEGACY_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM
    );
    let checksum: String = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
             FROM _sqlx_migrations WHERE version = ?",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(checksum, LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM);
}

#[tokio::test]
async fn attachment_cloud_sync_intent_is_a_synced_additive_column() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717170000_attachment_cloud_sync_intent")
        .unwrap();

    assert_eq!(
        migration.scope,
        anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_attachments",
        }
    );

    let db = test_db().await;
    sqlx::query(
        "INSERT INTO session_attachments (id, workspace_id, session_id)
             VALUES ('attachment-intent', 'workspace-1', 'session-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let enabled: i64 = sqlx::query_scalar(
        "SELECT cloud_sync_enabled FROM session_attachments
             WHERE id = 'attachment-intent'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(enabled, 0);

    let invalid = sqlx::query(
        "UPDATE session_attachments SET cloud_sync_enabled = 2
             WHERE id = 'attachment-intent'",
    )
    .execute(db.pool())
    .await;
    assert!(invalid.is_err());
}

#[test]
fn shared_attachment_cache_is_plain_and_excluded_from_cloudsync() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717171000_shared_session_attachment_cache")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "shared_session_attachment_cache")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"shared_session_attachment_cache"));
    assert!(!cloudsync_alter_guard_required(
        "shared_session_attachment_cache"
    ));
}

#[tokio::test]
async fn attachment_transfer_jobs_deduplicate_work_without_collapsing_old_objects() {
    let db = test_db().await;
    let insert = "INSERT INTO attachment_transfer_jobs (
            id,
            attachment_id,
            session_id,
            workspace_id,
            direction,
            expected_sha256,
            expected_size_bytes,
            remote_object_id,
            object_key
        ) VALUES (?, 'attachment-1', 'session-1', 'workspace-1', ?, ?, 42, ?, ?)";
    let sha256 = "a".repeat(64);

    sqlx::query(insert)
        .bind("upload-1")
        .bind("upload")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("")
        .execute(db.pool())
        .await
        .unwrap();

    let duplicate_upload = sqlx::query(insert)
        .bind("upload-2")
        .bind("upload")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("")
        .execute(db.pool())
        .await;
    assert!(duplicate_upload.is_err());

    let competing_download = sqlx::query(insert)
        .bind("download-1")
        .bind("download")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("private/current-object")
        .execute(db.pool())
        .await;
    assert!(competing_download.is_err());

    sqlx::query("UPDATE attachment_transfer_jobs SET phase = 'completed' WHERE id = 'upload-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(insert)
        .bind("download-1")
        .bind("download")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("private/current-object")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE attachment_transfer_jobs SET phase = 'completed' WHERE id = 'download-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(insert)
        .bind("upload-2")
        .bind("upload")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE attachment_transfer_jobs SET phase = 'completed' WHERE id = 'upload-2'")
        .execute(db.pool())
        .await
        .unwrap();

    for (id, remote_object_id, object_key) in [
        ("delete-old-1", "remote-object-1", "private/old-object-1"),
        ("delete-old-2", "remote-object-2", "private/old-object-2"),
    ] {
        sqlx::query(insert)
            .bind(id)
            .bind("delete")
            .bind(&sha256)
            .bind(remote_object_id)
            .bind(object_key)
            .execute(db.pool())
            .await
            .unwrap();
    }

    let delete_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attachment_transfer_jobs
             WHERE attachment_id = 'attachment-1' AND direction = 'delete'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(delete_count, 2);

    let duplicate_delete = sqlx::query(insert)
        .bind("delete-old-1-again")
        .bind("delete")
        .bind(&sha256)
        .bind("remote-object-3")
        .bind("private/old-object-1")
        .execute(db.pool())
        .await;
    assert!(duplicate_delete.is_err());

    let duplicate_delete_object = sqlx::query(insert)
        .bind("delete-old-1-different-key")
        .bind("delete")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("private/other-key")
        .execute(db.pool())
        .await;
    assert!(duplicate_delete_object.is_err());

    sqlx::query(
        "UPDATE attachment_transfer_jobs SET phase = 'completed' WHERE id = 'delete-old-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(insert)
        .bind("delete-old-1-again")
        .bind("delete")
        .bind(&sha256)
        .bind("remote-object-1")
        .bind("private/old-object-1")
        .execute(db.pool())
        .await
        .unwrap();

    for (id, object_key) in [
        ("delete-by-key-1", "private/key-only-1"),
        ("delete-by-key-2", "private/key-only-2"),
    ] {
        sqlx::query(insert)
            .bind(id)
            .bind("delete")
            .bind(&sha256)
            .bind("")
            .bind(object_key)
            .execute(db.pool())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn attachment_transfer_jobs_require_complete_ciphertext_metadata() {
    let db = test_db().await;
    let sha256 = "a".repeat(64);
    sqlx::query(
        "INSERT INTO attachment_transfer_jobs (
                id,
                attachment_id,
                session_id,
                workspace_id,
                direction,
                expected_sha256,
                expected_size_bytes
            ) VALUES ('upload-1', 'attachment-1', 'session-1', 'workspace-1', 'upload', ?, 42)",
    )
    .bind(&sha256)
    .execute(db.pool())
    .await
    .unwrap();

    let incomplete = sqlx::query(
        "UPDATE attachment_transfer_jobs
             SET ciphertext_sha256 = ?
             WHERE id = 'upload-1'",
    )
    .bind(&sha256)
    .execute(db.pool())
    .await;
    assert!(incomplete.is_err());

    sqlx::query(
        "UPDATE attachment_transfer_jobs
             SET ciphertext_sha256 = ?, ciphertext_size_bytes = 84
             WHERE id = 'upload-1'",
    )
    .bind(&sha256)
    .execute(db.pool())
    .await
    .unwrap();

    let oversized = sqlx::query(
        "UPDATE attachment_transfer_jobs
             SET ciphertext_size_bytes = 545259521
             WHERE id = 'upload-1'",
    )
    .execute(db.pool())
    .await;
    assert!(oversized.is_err());
}
