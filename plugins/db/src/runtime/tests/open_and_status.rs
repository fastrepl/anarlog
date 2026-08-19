use super::*;

fn unavailable_extension_error() -> DbOpenError {
    DbOpenError::Io(std::io::Error::other("cloudsync extension unavailable"))
}

fn failed_extension_probe_error() -> DbOpenError {
    DbOpenError::Io(std::io::Error::other("cloudsync extension probe failed"))
}

async fn wait_for_unsent_changes(runtime: &PluginDbRuntime) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let status = runtime.cloudsync_status().await.unwrap();
            if status["has_unsent_changes"] == true {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("CloudSync status remained unenriched");
}

#[tokio::test]
async fn cloudsync_open_failure_falls_back_for_uninitialized_database() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");

    let db = open_app_db_without_cloudsync(
        DbStorage::Local(&db_path),
        unavailable_extension_error(),
        failed_extension_probe_error(),
        true,
    )
    .await
    .unwrap();

    assert!(!db.cloudsync_enabled());
    let sessions_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions'
            )",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(sessions_exists);
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn extension_open_without_initialized_tables_allows_plain_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    db.pool().close().await;
    drop(db);

    let db = open_app_db_without_cloudsync(
        DbStorage::Local(&db_path),
        unavailable_extension_error(),
        failed_extension_probe_error(),
        true,
    )
    .await
    .unwrap();

    assert!(!db.cloudsync_enabled());
    assert!(!database_uses_cloudsync_schema(&db).await.unwrap());
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn cloudsync_open_failure_does_not_migrate_initialized_replica_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE items (
                id TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL DEFAULT ''
            )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    db.pool().close().await;
    drop(db);

    let error = open_app_db_without_cloudsync(
        DbStorage::Local(&db_path),
        unavailable_extension_error(),
        failed_extension_probe_error(),
        true,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, crate::Error::Db(DbOpenError::Io(_))));
    let plain = Db::connect_local_plain(&db_path).await.unwrap();
    let sessions_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions'
            )",
    )
    .fetch_one(plain.pool())
    .await
    .unwrap();
    assert!(!sessions_exists);
}

#[tokio::test]
async fn cloudsync_open_fallback_propagates_schema_errors() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(app_db_open_options(DbStorage::Local(&db_path), false))
        .await
        .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "UPDATE app_settings
             SET value_json = 'not-json'
             WHERE id = 'cloudsync_workspace_binding'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.pool().close().await;
    drop(db);

    let error = open_app_db_without_cloudsync(
        DbStorage::Local(&db_path),
        unavailable_extension_error(),
        failed_extension_probe_error(),
        true,
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        crate::Error::AppSchema(anlg_db_app::AppSchemaError::CloudsyncWorkspace(
            anlg_db_app::CloudsyncWorkspaceError::InvalidBinding
        ))
    ));
}

#[tokio::test]
async fn failed_cloudsync_preflight_clears_new_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::new(db));
    let cancellation = crate::e2ee_witness::E2eeWitnessCancellation::default();

    runtime
        .prepare_cloudsync_config_fail_closed(
            anlg_db_core::CloudsyncRuntimeConfig {
                connection_string: "managed-database-id".to_string(),
                auth: anlg_db_core::CloudsyncAuth::Token {
                    token: "secret-token".to_string(),
                },
                tables: vec![anlg_db_core::CloudsyncTableSpec {
                    table_name: "missing_table".to_string(),
                    crdt_algo: None,
                    init_flags: None,
                    enabled: true,
                }],
                sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
                wait_ms: Some(5_000),
                max_retries: Some(3),
            },
            &cancellation,
        )
        .await
        .unwrap_err();

    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["configured"], false);
    assert_eq!(status["running"], false);
    assert_eq!(status["network_initialized"], false);
}

#[tokio::test]
async fn cloudsync_status_reports_pending_e2ee_dirty_rows() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('session-1', 'workspace-1', 'Local edit')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    wait_for_unsent_changes(&runtime).await;
}

#[tokio::test]
async fn cloudsync_status_does_not_treat_inbound_reconciliation_as_unsent() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_witness_records (
               workspace_id,
               record_id,
               revision,
               writer_id,
               payload_hash,
               payload,
               sequence
             )
             VALUES ('workspace-1', 'record-1', 1, 'writer-1', 'hash', 'payload', 1)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let status = runtime.cloudsync_status().await.unwrap();

    assert!(runtime.e2ee_sync_hook.reconciliation_requested());
    assert_ne!(status["has_unsent_changes"], true);
}

#[tokio::test]
async fn cloudsync_status_ignores_only_the_active_transcript() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES ('transcript-1', 'workspace-1', 'session-1', '[{\"text\":\"partial\"}]')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
    )
    .bind(
        serde_json::json!({
            "version": 1,
            "phase": "capturing",
            "sessionId": "session-1",
            "transcriptId": "transcript-1",
            "startedAt": 1_000,
            "createdAt": "2026-07-24T00:00:00.000Z",
            "audioOffsetMs": 0,
            "preserveExistingTranscript": false,
            "ownerUserId": "workspace-1",
            "memo": ""
        })
        .to_string(),
    )
    .execute(db.pool())
    .await
    .unwrap();

    let status = runtime.cloudsync_status().await.unwrap();
    assert_ne!(status["has_unsent_changes"], true);

    sqlx::query(
        "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    wait_for_unsent_changes(&runtime).await;
}

#[tokio::test]
async fn repeated_cloudsync_status_polling_returns_base_status_on_a_busy_pool() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    let mut held_connection = db.pool().acquire().await.unwrap();
    let started = std::time::Instant::now();

    for _ in 0..32 {
        let status = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            runtime.cloudsync_status(),
        )
        .await
        .expect("CloudSync status waited for a busy pool")
        .unwrap();
        assert_eq!(status["activity_paused"], false);
        assert_eq!(status["has_unsent_changes"], serde_json::Value::Null);
        assert!(status.get("recovery_pending").is_none());
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(1));

    held_connection.return_to_pool().await;
    for _ in 0..32 {
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.cloudsync_status(),
        )
        .await
        .expect("CloudSync status left SQLite work in flight")
        .unwrap();
    }
    assert_eq!(db.pool().num_idle(), 1);
    tokio::time::timeout(std::time::Duration::from_millis(100), db.pool().acquire())
        .await
        .expect("repeated CloudSync status polling exhausted the pool")
        .unwrap();
}
