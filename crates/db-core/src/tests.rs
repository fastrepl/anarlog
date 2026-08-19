use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::oneshot;

fn test_cloudsync_config() -> CloudsyncRuntimeConfig {
    CloudsyncRuntimeConfig {
        connection_string: "sqlitecloud://demo.invalid/app.db?apikey=demo".to_string(),
        auth: CloudsyncAuth::None,
        tables: vec![CloudsyncTableSpec {
            table_name: "test_sync".to_string(),
            crdt_algo: None,
            init_flags: None,
            enabled: true,
        }],
        sync_interval_ms: 30_000,
        wait_ms: Some(500),
        max_retries: Some(1),
    }
}

async fn assert_cloudsync_connection_ready(
    connection: &mut PoolConnection<Sqlite>,
    expected_site_id: &[u8],
    probe_id: &str,
) {
    let version: String = sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(&mut **connection)
        .await
        .unwrap();
    let stored_version: String = sqlx::query_scalar(
        "SELECT value FROM cloudsync_settings WHERE key = 'version' COLLATE NOCASE",
    )
    .fetch_one(&mut **connection)
    .await
    .unwrap();
    let stored_site_id: Vec<u8> =
        sqlx::query_scalar("SELECT site_id FROM cloudsync_site_id WHERE rowid = 0")
            .fetch_one(&mut **connection)
            .await
            .unwrap();
    let context_site_id: Vec<u8> = sqlx::query_scalar("SELECT cloudsync_siteid()")
        .fetch_one(&mut **connection)
        .await
        .unwrap();
    let sync_enabled: bool = sqlx::query_scalar("SELECT cloudsync_is_enabled('sync_items')")
        .fetch_one(&mut **connection)
        .await
        .unwrap();
    sqlx::query("BEGIN")
        .execute(&mut **connection)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sync_items (id, value) VALUES (?, 'probe-insert')")
        .bind(probe_id)
        .execute(&mut **connection)
        .await
        .unwrap_or_else(|error| panic!("{probe_id} insert failed: {error}"));
    sqlx::query("UPDATE sync_items SET value = 'probe-update' WHERE id = ?")
        .bind(probe_id)
        .execute(&mut **connection)
        .await
        .unwrap();
    let metadata_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM sync_items_cloudsync
         WHERE pk = cloudsync_pk_encode(?)",
    )
    .bind(probe_id)
    .fetch_one(&mut **connection)
    .await
    .unwrap();
    let change_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM cloudsync_changes
         WHERE tbl = 'sync_items' AND pk = cloudsync_pk_encode(?)",
    )
    .bind(probe_id)
    .fetch_one(&mut **connection)
    .await
    .unwrap();
    sqlx::query("ROLLBACK")
        .execute(&mut **connection)
        .await
        .unwrap();
    let persisted_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_items WHERE id = ?")
        .bind(probe_id)
        .fetch_one(&mut **connection)
        .await
        .unwrap();

    assert_eq!(version, anlg_cloudsync::CLOUDSYNC_VERSION);
    assert_eq!(stored_version, anlg_cloudsync::CLOUDSYNC_VERSION);
    assert_eq!(stored_site_id, expected_site_id);
    assert_eq!(context_site_id, expected_site_id);
    assert!(sync_enabled);
    assert!(metadata_rows > 0);
    assert!(change_rows > 0);
    assert_eq!(persisted_rows, 0);
}

#[tokio::test]
async fn connect_local_plain_creates_parent_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("nonexistent").join("nested").join("app.db");
    let db = Db::connect_local_plain(&db_path).await.unwrap();
    assert!(db_path.exists());
    drop(db);
}

#[tokio::test]
async fn connect_local_read_only_does_not_create_missing_database() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("missing.db");

    let result = Db::connect_local_read_only(&db_path).await;

    assert!(result.is_err());
    assert!(!db_path.exists());
}

#[tokio::test]
async fn connect_local_read_only_rejects_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("app.db");
    let writable = Db::connect_local_plain(&db_path).await.unwrap();
    sqlx::query("CREATE TABLE records (id TEXT PRIMARY KEY NOT NULL)")
        .execute(writable.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO records (id) VALUES ('existing')")
        .execute(writable.pool())
        .await
        .unwrap();
    writable.pool().close().await;

    let read_only = Db::connect_local_read_only(&db_path).await.unwrap();
    let rows: Vec<String> = sqlx::query_scalar("SELECT id FROM records ORDER BY id")
        .fetch_all(read_only.pool())
        .await
        .unwrap();
    let query_only: i64 = sqlx::query_scalar("PRAGMA query_only")
        .fetch_one(read_only.pool())
        .await
        .unwrap();
    let write_result = sqlx::query("INSERT INTO records (id) VALUES ('rejected')")
        .execute(read_only.pool())
        .await;

    assert_eq!(rows, vec!["existing"]);
    assert_eq!(query_only, 1);
    assert!(write_result.is_err());
}

#[tokio::test]
async fn open_applies_requested_pragmas() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("app.db");

    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(db.pool())
        .await
        .unwrap();

    assert_eq!(foreign_keys, 1);
    assert_eq!(journal_mode.to_lowercase(), "wal");
    assert_eq!(busy_timeout, SQLITE_BUSY_TIMEOUT.as_millis() as i64);
}

#[tokio::test]
async fn cloudsync_pool_serializes_and_validates_every_extension_load() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("cloudsync.db");
    let table = CloudsyncTableSpec {
        table_name: "sync_items".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    };

    for open_index in 0..4 {
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&database_path),
            cloudsync_enabled: true,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();

        if open_index == 0 {
            sqlx::query("CREATE TABLE sync_items (id TEXT PRIMARY KEY NOT NULL, value TEXT)")
                .execute(db.pool())
                .await
                .unwrap();
        }
        db.cloudsync_init_enabled_tables(std::slice::from_ref(&table))
            .await
            .unwrap();

        let expected_site_id: Vec<u8> =
            sqlx::query_scalar("SELECT site_id FROM cloudsync_site_id WHERE rowid = 0")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let mut writer = db.pool().acquire().await.unwrap();
        let discarded_first = db.pool().acquire().await.unwrap();
        let discarded_second = db.pool().acquire().await.unwrap();
        discarded_first.close().await.unwrap();
        discarded_second.close().await.unwrap();
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *writer)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sync_items (id, value) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET value = excluded.value",
        )
        .bind(format!("item-{open_index}"))
        .bind(format!("value-{open_index}"))
        .execute(&mut *writer)
        .await
        .unwrap();

        let first_pool = db.pool().clone();
        let second_pool = db.pool().clone();
        let first = tokio::spawn(async move { first_pool.acquire().await });
        let second = tokio::spawn(async move { second_pool.acquire().await });
        let mut first = first.await.unwrap().unwrap();
        let mut second = second.await.unwrap().unwrap();
        sqlx::query("COMMIT").execute(&mut *writer).await.unwrap();

        assert_cloudsync_connection_ready(
            &mut writer,
            &expected_site_id,
            &format!("writer-probe-{open_index}"),
        )
        .await;
        assert_cloudsync_connection_ready(
            &mut first,
            &expected_site_id,
            &format!("first-probe-{open_index}"),
        )
        .await;
        assert_cloudsync_connection_ready(
            &mut second,
            &expected_site_id,
            &format!("second-probe-{open_index}"),
        )
        .await;
        {
            let mut pinned = db.cloudsync_connection.lock().await;
            assert_cloudsync_connection_ready(
                pinned
                    .as_mut()
                    .expect("CloudSync connection must be pinned"),
                &expected_site_id,
                &format!("pinned-probe-{open_index}"),
            )
            .await;
        }

        drop(writer);
        drop(first);
        drop(second);
        db.cloudsync_close_connection().await.unwrap();
        db.pool().close().await;
    }
}

#[tokio::test]
async fn disabled_open_mode_keeps_cloudsync_inert() {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();

    assert!(!db.cloudsync_enabled());
    assert!(!db.has_cloudsync());

    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    db.cloudsync_start().await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(status.configured);
    assert!(!status.extension_loaded);
    assert!(!status.running);
    assert!(!status.network_initialized);
    assert!(!status.cloudsync_enabled);

    db.cloudsync_logout(false).await.unwrap();
    assert!(!db.cloudsync_status().await.unwrap().configured);
    db.cloudsync_stop().await.unwrap();
}

#[tokio::test]
async fn enabled_open_mode_requires_runtime_config_before_start() {
    let db = Db::connect_memory().await.unwrap();

    let error = db.cloudsync_start().await.unwrap_err();
    assert!(matches!(error, CloudsyncRuntimeError::NotConfigured));
}

#[tokio::test]
async fn memory_cloudsync_does_not_install_change_hooks() {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query("CREATE TABLE events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    let mut changes = db.change_notifier().subscribe();

    sqlx::query("INSERT INTO events (id) VALUES ('a')")
        .execute(db.pool())
        .await
        .unwrap();

    assert_eq!(db.change_notifier().current_seq(), 0);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cloudsync_and_change_notifier_share_transaction_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");

    sqlx::query(
        "CREATE TABLE test_sync (\
            id TEXT PRIMARY KEY NOT NULL, \
            value TEXT NOT NULL DEFAULT ''\
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE other_events (\
            id TEXT PRIMARY KEY NOT NULL, \
            value TEXT NOT NULL DEFAULT ''\
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("test_sync", None, None).await.unwrap();

    let notifier = db.change_notifier();
    let mut changes = notifier.subscribe();

    sqlx::query("INSERT INTO test_sync (id, value) VALUES ('a', 'one')")
        .execute(db.pool())
        .await
        .unwrap();
    let first_version: i64 = sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(first_version, 1);
    let first_change = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let change = changes.recv().await.unwrap();
            if change.table == "test_sync" {
                break change;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(first_change.kind, anlg_db_change::TableChangeKind::Insert);
    while changes.try_recv().is_ok() {}

    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE test_sync SET value = 'two' WHERE id = 'a'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("INSERT INTO other_events (id, value) VALUES ('a', 'two')")
        .execute(&mut *transaction)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
    transaction.commit().await.unwrap();

    let (sync_change, other_change) =
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            let mut sync_change = None;
            let mut other_change = None;
            while sync_change.is_none() || other_change.is_none() {
                let change = changes.recv().await.unwrap();
                match change.table.as_str() {
                    "test_sync" => sync_change = Some(change),
                    "other_events" => other_change = Some(change),
                    _ => {}
                }
            }
            (sync_change.unwrap(), other_change.unwrap())
        })
        .await
        .unwrap();
    assert_eq!(sync_change.kind, anlg_db_change::TableChangeKind::Update);
    assert_eq!(sync_change.seq, other_change.seq);
    let second_version: i64 = sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(second_version, 2);
    while changes.try_recv().is_ok() {}

    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE test_sync SET value = 'rolled-back' WHERE id = 'a'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE other_events SET value = 'rolled-back' WHERE id = 'a'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
    let version_after_rollback: i64 = sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(version_after_rollback, second_version);

    let failed = sqlx::query(
        "INSERT INTO test_sync (id, value) VALUES ('b', 'temporary'), ('a', 'duplicate')",
    )
    .execute(db.pool())
    .await;
    assert!(failed.is_err());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
    let version_after_failure: i64 = sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(version_after_failure, second_version);

    sqlx::query("UPDATE test_sync SET value = 'three' WHERE id = 'a'")
        .execute(db.pool())
        .await
        .unwrap();
    let final_change = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let change = changes.recv().await.unwrap();
            if change.table == "test_sync" {
                break change;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(final_change.kind, anlg_db_change::TableChangeKind::Update);
    let final_version: i64 = sqlx::query_scalar("SELECT cloudsync_db_version()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let value: String = sqlx::query_scalar("SELECT value FROM test_sync WHERE id = 'a'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let failed_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_sync WHERE id = 'b'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(final_version, 3);
    assert_eq!(value, "three");
    assert_eq!(failed_row_count, 0);
}

#[tokio::test]
async fn local_cloudsync_requires_wal() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");

    let error = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        DbOpenError::Cloudsync(anlg_cloudsync::Error::WalRequired)
    ));
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
async fn cloudsync_initialized_table_requires_extension_for_writes() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cloudsync.db");
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
    sqlx::query("INSERT INTO items (id, value) VALUES ('cloud', 'one')")
        .execute(db.pool())
        .await
        .unwrap();
    db.pool().close().await;
    drop(db);

    let plain = Db::connect_local_plain(&db_path).await.unwrap();
    let error = sqlx::query("INSERT INTO items (id, value) VALUES ('plain', 'two')")
        .execute(plain.pool())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("cloudsync_is_sync"));
}

#[tokio::test]
async fn configure_rejects_live_runtime_changes() {
    let db = Db::connect_memory_plain().await.unwrap();
    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    db.cloudsync_runtime.lock().unwrap().running = true;

    let error = db
        .cloudsync_configure(CloudsyncRuntimeConfig {
            connection_string: "sqlitecloud://demo.invalid/other.db?apikey=demo".to_string(),
            ..test_cloudsync_config()
        })
        .await
        .unwrap_err();

    assert!(matches!(error, CloudsyncRuntimeError::RestartRequired));
    assert_eq!(
        db.cloudsync_runtime
            .lock()
            .unwrap()
            .config
            .as_ref()
            .unwrap()
            .connection_string,
        "sqlitecloud://demo.invalid/app.db?apikey=demo"
    );
}

#[tokio::test]
async fn reconfigure_preserves_stopped_state_when_runtime_is_inert() {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    db.cloudsync_configure(test_cloudsync_config())
        .await
        .unwrap();
    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.network_initialized = true;
    }

    let next_config = CloudsyncRuntimeConfig {
        connection_string: "sqlitecloud://demo.invalid/reconfigured.db?apikey=demo".to_string(),
        sync_interval_ms: 2_000,
        ..test_cloudsync_config()
    };

    db.cloudsync_reconfigure(next_config.clone()).await.unwrap();

    let runtime = db.cloudsync_runtime.lock().unwrap();
    assert_eq!(runtime.config, Some(next_config));
    assert!(!runtime.running);
    assert!(!runtime.network_initialized);
}

#[tokio::test]
async fn dropping_db_stops_background_task_best_effort() {
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let db = Db::connect_memory_plain().await.unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let guard = DropFlag(Arc::clone(&dropped));
    let join_handle = tokio::spawn(async move {
        let _guard = guard;
        let _ = shutdown_rx.await;
    });

    {
        let mut runtime = db.cloudsync_runtime.lock().unwrap();
        runtime.running = true;
        runtime.task = Some(crate::cloudsync::CloudsyncBackgroundTask {
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        });
    }

    drop(db);

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn emits_table_changes_for_local_writes() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();
    let before = notifier.current_seq();

    sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
        .execute(db.pool())
        .await
        .unwrap();

    let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(change.table, "test_events");
    assert_eq!(change.kind, anlg_db_change::TableChangeKind::Insert);
    assert!(change.seq > before);
    assert_eq!(notifier.current_seq(), change.seq);
    assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
}

#[tokio::test]
async fn emits_table_changes_only_after_commit() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();
    let mut tx = db.pool().begin().await.unwrap();

    sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
        .execute(&mut *tx)
        .await
        .unwrap();

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );

    tx.commit().await.unwrap();

    let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(change.table, "test_events");
    assert_eq!(change.kind, anlg_db_change::TableChangeKind::Insert);
    assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
}

#[tokio::test]
async fn rollback_clears_pending_table_changes() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();
    let mut tx = db.pool().begin().await.unwrap();

    sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.rollback().await.unwrap();

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn coalesces_multiple_writes_in_a_transaction() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();
    let mut tx = db.pool().begin().await.unwrap();

    sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(change.table, "test_events");
    assert_eq!(change.kind, anlg_db_change::TableChangeKind::Update);
    assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn emits_update_and_delete_table_changes() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();

    sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM test_events WHERE id = 'a'")
        .execute(db.pool())
        .await
        .unwrap();

    let update = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();
    let delete = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(update.table, "test_events");
    assert_eq!(update.kind, anlg_db_change::TableChangeKind::Update);
    assert_eq!(delete.table, "test_events");
    assert_eq!(delete.kind, anlg_db_change::TableChangeKind::Delete);
    assert!(delete.seq > update.seq);
    assert_eq!(notifier.latest_table_seq("test_events"), Some(delete.seq));
}

#[tokio::test]
async fn emits_table_changes_across_multiple_connections() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");

    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE multi_conn_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let mut changes = notifier.subscribe();
    let mut conn_a = db.pool().acquire().await.unwrap();
    let mut conn_b = db.pool().acquire().await.unwrap();

    sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('a')")
        .execute(&mut *conn_a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('b')")
        .execute(&mut *conn_b)
        .await
        .unwrap();

    let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(first.table, "multi_conn_events");
    assert_eq!(second.table, "multi_conn_events");
    assert_ne!(first.seq, second.seq);
}

#[tokio::test]
async fn tracks_monotonic_change_sequences_per_table() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier();
    sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TABLE other_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let start = notifier.current_seq();
    let mut changes = notifier.subscribe();

    sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
        .execute(db.pool())
        .await
        .unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    sqlx::query("INSERT INTO test_events (id) VALUES ('b')")
        .execute(db.pool())
        .await
        .unwrap();
    let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    sqlx::query("INSERT INTO other_events (id) VALUES ('c')")
        .execute(db.pool())
        .await
        .unwrap();
    let third = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    assert!(first.seq > start);
    assert!(second.seq > first.seq);
    assert!(third.seq > second.seq);
    assert_eq!(notifier.current_seq(), third.seq);
    assert_eq!(notifier.latest_table_seq("test_events"), Some(second.seq));
    assert_eq!(notifier.latest_table_seq("other_events"), Some(third.seq));
    assert_eq!(notifier.latest_table_seq("missing_events"), None);
}

#[tokio::test]
async fn notifier_survives_db_drop() {
    let db = Db::connect_memory_plain().await.unwrap();
    let notifier = db.change_notifier().clone();
    sqlx::query("CREATE TABLE retained_events (id TEXT PRIMARY KEY NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();

    let pool = db.pool().clone();
    let mut changes = notifier.subscribe();
    drop(db);

    sqlx::query("INSERT INTO retained_events (id) VALUES ('a')")
        .execute(&pool)
        .await
        .unwrap();

    let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(change.table, "retained_events");
    assert_eq!(change.kind, anlg_db_change::TableChangeKind::Insert);
    assert_eq!(
        notifier.latest_table_seq("retained_events"),
        Some(change.seq)
    );
}

#[tokio::test]
async fn open_memory_clamps_max_connections_to_one() {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: false,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();

    let _conn = db.pool().acquire().await.unwrap();

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), db.pool().acquire())
            .await
            .is_err()
    );
}
