use super::*;

#[test]
fn shared_session_cache_is_plain_and_excluded_from_cloudsync() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260716173000_shared_session_cache")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "shared_session_cache")
    );
    assert!(!cloudsync_alter_guard_required("shared_session_cache"));
}

#[test]
fn shared_session_cache_migration_checksums_are_pinned() {
    assert_eq!(
        checksum_hex(&migration_checksum(LEGACY_SHARED_SESSION_CACHE_SQL)),
        LEGACY_SHARED_SESSION_CACHE_CHECKSUM
    );
    assert_eq!(
        checksum_hex(&migration_checksum(include_str!(
            "../../migrations/20260716173000_shared_session_cache.sql"
        ))),
        CURRENT_SHARED_SESSION_CACHE_CHECKSUM
    );
}

#[tokio::test]
async fn repairs_the_base_only_dev_cache_without_touching_notes() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(&db, schema_with_legacy_shared_session_cache(false))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-1', 'user-1', 'Important note')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, workspace_id, session_id, title, body)
             VALUES ('document-1', 'workspace-1', 'session-1', 'Notes', '{\"type\":\"doc\"}')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, words_json)
             VALUES ('transcript-1', 'workspace-1', 'user-1', 'session-1', '[{\"text\":\"hello\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shared_session_cache (
                share_id, workspace_id, session_id, content_revision,
                access_version, published_at
             ) VALUES (
                'share-1', 'workspace-1', 'shared-session-1', 1,
                1, '2026-07-18T00:00:00Z'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    prepare_schema(&db).await.unwrap();

    let source_rows: (i64, i64, i64) = sqlx::query_as(
        "SELECT
                (SELECT COUNT(*) FROM sessions WHERE id = 'session-1'),
                (SELECT COUNT(*) FROM session_documents WHERE id = 'document-1'),
                (SELECT COUNT(*) FROM transcripts WHERE id = 'transcript-1')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(source_rows, (1, 1, 1));
    let cache_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shared_session_cache")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(cache_rows, 0);
}

#[tokio::test]
async fn repairs_the_known_legacy_shared_session_cache_migration() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(&db, schema_with_legacy_shared_session_cache(true))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shared_session_cache (
                share_id, workspace_id, session_id, content_revision,
                access_version, published_at
             ) VALUES (
                'share-1', 'workspace-1', 'session-1', 1,
                1, '2026-07-18T00:00:00Z'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let legacy_checksum: String = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
             FROM _sqlx_migrations
             WHERE version = ?",
    )
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(legacy_checksum, LEGACY_SHARED_SESSION_CACHE_CHECKSUM);

    prepare_schema(&db).await.unwrap();
    prepare_schema(&db).await.unwrap();

    let checksum: String = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
             FROM _sqlx_migrations
             WHERE version = ?",
    )
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(checksum, CURRENT_SHARED_SESSION_CACHE_CHECKSUM);
    for column in [
        "viewer_user_id",
        "attachments_json",
        "web_editable",
        "web_edit_base_content_revision",
        "web_edit_base_title",
        "web_edit_base_body_json",
    ] {
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(
                        SELECT 1 FROM pragma_table_info('shared_session_cache')
                        WHERE name = ?
                    )",
            )
            .bind(column)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            "missing repaired column {column}"
        );
    }
    let primary_key_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name
             FROM pragma_table_info('shared_session_cache')
             WHERE pk > 0
             ORDER BY pk",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(primary_key_columns, ["viewer_user_id", "share_id"]);
    let cache_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shared_session_cache")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(cache_rows, 0);
}

#[tokio::test]
async fn current_shared_session_cache_is_left_untouched() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO shared_session_cache (
                share_id, viewer_user_id, workspace_id, session_id,
                content_revision, access_version, published_at
             ) VALUES (
                'share-1', 'user-1', 'workspace-1', 'session-1',
                1, 1, '2026-07-18T00:00:00Z'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    prepare_schema(&db).await.unwrap();

    let cache_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shared_session_cache
             WHERE viewer_user_id = 'user-1' AND share_id = 'share-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(cache_rows, 1);
}

#[tokio::test]
async fn unknown_shared_session_cache_checksum_fails_without_mutation() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO shared_session_cache (
                share_id, viewer_user_id, workspace_id, session_id,
                content_revision, access_version, published_at
             ) VALUES (
                'share-1', 'user-1', 'workspace-1', 'session-1',
                1, 1, '2026-07-18T00:00:00Z'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE _sqlx_migrations
             SET checksum = X'00'
             WHERE version = ?",
    )
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .execute(db.pool())
    .await
    .unwrap();

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(
        error,
        AppSchemaError::SharedSessionCacheRepair(
            "shared-session cache migration has an unknown checksum"
        )
    ));
    let cache_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shared_session_cache")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(cache_rows, 1);
    let checksum: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(checksum, [0]);
}

#[tokio::test]
async fn rejects_a_legacy_checksum_with_the_current_cache_schema() {
    let db = test_db().await;
    sqlx::query(
            "UPDATE _sqlx_migrations
             SET checksum = X'4813DB532E44E6DB8A3BA85E0B248FF99A927EC40B6AA971210452B588BCB2361D3661BD23D045A32AC3A9FCBED99B4A'
             WHERE version = ?",
        )
        .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
        .execute(db.pool())
        .await
        .unwrap();

    let error = prepare_schema(&db).await.unwrap_err();
    assert!(matches!(
        error,
        AppSchemaError::SharedSessionCacheRepair(
            "legacy shared-session cache migration has an unexpected schema"
        )
    ));
}

#[test]
fn session_share_sync_state_is_plain_and_excluded_from_replication() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717191000_session_share_sync_state")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "session_share_sync_state")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"session_share_sync_state"));
    assert!(!cloudsync_alter_guard_required("session_share_sync_state"));
}

#[tokio::test]
async fn session_share_sync_state_enforces_local_reconciliation_contract() {
    let db = test_db().await;
    let insert = "INSERT INTO session_share_sync_state (
            viewer_user_id,
            share_id,
            session_id,
            acknowledged_content_revision,
            baseline_source_hash,
            status
        ) VALUES (?, ?, ?, ?, ?, ?)";

    sqlx::query(insert)
        .bind("viewer-1")
        .bind("share-1")
        .bind("session-1")
        .bind(3_i64)
        .bind("a".repeat(64))
        .bind("clean")
        .execute(db.pool())
        .await
        .unwrap();

    for (viewer, share, session, revision, hash, status) in [
        ("", "share-2", "session-2", 1_i64, "b".repeat(64), "clean"),
        (
            "viewer-2",
            "share-2",
            "session-2",
            0_i64,
            "b".repeat(64),
            "clean",
        ),
        (
            "viewer-2",
            "share-2",
            "session-2",
            1_i64,
            "B".repeat(64),
            "clean",
        ),
        (
            "viewer-2",
            "share-2",
            "session-2",
            1_i64,
            "b".repeat(63),
            "clean",
        ),
        (
            "viewer-2",
            "share-2",
            "session-2",
            1_i64,
            "b".repeat(64),
            "pending",
        ),
    ] {
        assert!(
            sqlx::query(insert)
                .bind(viewer)
                .bind(share)
                .bind(session)
                .bind(revision)
                .bind(hash)
                .bind(status)
                .execute(db.pool())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn shared_session_cache_enforces_manager_web_edit_base() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717190000_shared_session_cache_web_edits")
        .unwrap();
    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);

    let db = test_db().await;
    let insert = "INSERT INTO shared_session_cache (
            share_id,
            viewer_user_id,
            workspace_id,
            session_id,
            content_revision,
            manage_access,
            access_version,
            web_editable,
            web_edit_base_content_revision,
            web_edit_base_title,
            web_edit_base_body_json,
            published_at
        ) VALUES (?, ?, 'workspace-1', 'session-1', 2, ?, 1, 0, ?, ?, ?, '2026-07-17T00:00:00Z')";
    let body = r#"{"type":"doc","content":[{"type":"paragraph"}]}"#;

    sqlx::query(insert)
        .bind("share-1")
        .bind("viewer-1")
        .bind(1_i64)
        .bind(1_i64)
        .bind("Before")
        .bind(body)
        .execute(db.pool())
        .await
        .unwrap();

    for (share, viewer, manager, revision, title, body) in [
        (
            "share-2",
            "viewer-2",
            0_i64,
            Some(1_i64),
            Some("Before"),
            Some(body),
        ),
        (
            "share-3",
            "viewer-3",
            1_i64,
            Some(2_i64),
            Some("Before"),
            Some(body),
        ),
        ("share-4", "viewer-4", 1_i64, Some(1_i64), None, Some(body)),
    ] {
        assert!(
            sqlx::query(insert)
                .bind(share)
                .bind(viewer)
                .bind(manager)
                .bind(revision)
                .bind(title)
                .bind(body)
                .execute(db.pool())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn shared_session_cache_attachment_manifest_is_local_and_bounded() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717172000_shared_session_cache_attachments")
        .unwrap();
    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);

    let db = test_db().await;
    sqlx::query(
        "INSERT INTO shared_session_cache (
                share_id,
                viewer_user_id,
                workspace_id,
                session_id,
                content_revision,
                access_version,
                published_at
            ) VALUES (
                '11111111-1111-4111-8111-111111111111',
                'viewer-1',
                '22222222-2222-4222-8222-222222222222',
                'session-1',
                1,
                1,
                '2026-07-17T00:00:00Z'
            )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let manifest: String = sqlx::query_scalar(
        "SELECT attachments_json FROM shared_session_cache
             WHERE viewer_user_id = 'viewer-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(manifest, "[]");

    let oversized = format!(
        "[{}]",
        (0..65)
            .map(|index| format!(r#"{{"id":{index}}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    let invalid = sqlx::query(
        "UPDATE shared_session_cache SET attachments_json = ?
             WHERE viewer_user_id = 'viewer-1'",
    )
    .bind(oversized)
    .execute(db.pool())
    .await;
    assert!(invalid.is_err());
}

#[tokio::test]
async fn shared_session_cache_enforces_snapshot_contract() {
    let db = test_db().await;
    let insert = "INSERT INTO shared_session_cache (
            share_id,
            viewer_user_id,
            workspace_id,
            session_id,
            schema_version,
            content_revision,
            title,
            body_json,
            capability,
            manage_access,
            access_version,
            published_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    sqlx::query(insert)
        .bind("share-1")
        .bind("viewer-1")
        .bind("workspace-1")
        .bind("session-1")
        .bind(1_i64)
        .bind(3_i64)
        .bind("Shared note")
        .bind(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#)
        .bind("commenter")
        .bind(1_i64)
        .bind(4_i64)
        .bind("2026-07-16T17:30:00.000Z")
        .execute(db.pool())
        .await
        .unwrap();

    let cached: (i64, String, i64, i64) = sqlx::query_as(
        "SELECT content_revision, capability, manage_access, access_version
             FROM shared_session_cache
             WHERE viewer_user_id = 'viewer-1' AND share_id = 'share-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(cached, (3, "commenter".to_string(), 1, 4));

    sqlx::query(insert)
        .bind("share-1")
        .bind("viewer-2")
        .bind("workspace-1")
        .bind("session-1")
        .bind(1_i64)
        .bind(3_i64)
        .bind("Shared note")
        .bind(r#"{"type":"doc"}"#)
        .bind("viewer")
        .bind(0_i64)
        .bind(4_i64)
        .bind("2026-07-16T17:30:00.000Z")
        .execute(db.pool())
        .await
        .unwrap();
    let viewer_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM shared_session_cache WHERE share_id = 'share-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(viewer_count, 2);

    for (
        share_id,
        viewer_user_id,
        workspace_id,
        session_id,
        schema_version,
        content_revision,
        title,
        body_json,
        capability,
        manage_access,
        access_version,
        published_at,
    ) in [
        (
            "share-schema",
            "viewer-1",
            "workspace-1",
            "session-1",
            2,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-revision",
            "viewer-1",
            "workspace-1",
            "session-1",
            1,
            0,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-body",
            "viewer-1",
            "workspace-1",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"paragraph"}"#,
            "viewer",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-capability",
            "viewer-1",
            "workspace-1",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "owner",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-manage",
            "viewer-1",
            "workspace-1",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            2,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-access-version",
            "viewer-1",
            "workspace-1",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            0,
            0,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-id",
            "viewer-1",
            " ",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
        (
            "share-viewer",
            " ",
            "workspace-1",
            "session-1",
            1,
            1,
            "Shared note",
            r#"{"type":"doc"}"#,
            "viewer",
            0,
            1,
            "2026-07-16T17:30:00.000Z",
        ),
    ] {
        let result = sqlx::query(insert)
            .bind(share_id)
            .bind(viewer_user_id)
            .bind(workspace_id)
            .bind(session_id)
            .bind(schema_version)
            .bind(content_revision)
            .bind(title)
            .bind(body_json)
            .bind(capability)
            .bind(manage_access)
            .bind(access_version)
            .bind(published_at)
            .execute(db.pool())
            .await;
        assert!(result.is_err(), "invalid cache row {share_id} was accepted");
    }

    let malformed_json = sqlx::query(insert)
        .bind("share-json")
        .bind("viewer-1")
        .bind("workspace-1")
        .bind("session-1")
        .bind(1_i64)
        .bind(1_i64)
        .bind("Shared note")
        .bind("not-json")
        .bind("viewer")
        .bind(0_i64)
        .bind(1_i64)
        .bind("2026-07-16T17:30:00.000Z")
        .execute(db.pool())
        .await;
    assert!(malformed_json.is_err());
}
