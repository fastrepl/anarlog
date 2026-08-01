use super::*;
use crate::{CloudsyncAuth, CloudsyncTableSpec, DbOpenOptions, DbStorage};

mod lifecycle;
mod native_interrupt;
mod orchestration;
mod result_state;
mod shutdown;
mod status;

pub(super) fn test_cloudsync_config() -> CloudsyncRuntimeConfig {
    CloudsyncRuntimeConfig {
        connection_string: "sqlitecloud://demo.invalid/app.db?apikey=demo".to_string(),
        auth: CloudsyncAuth::None,
        tables: Vec::new(),
        sync_interval_ms: 30_000,
        wait_ms: Some(500),
        max_retries: Some(1),
    }
}

pub(super) async fn db_with_local_unsent_changes() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
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
    db.cloudsync_init_enabled_tables(&[CloudsyncTableSpec {
        table_name: "items".to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled: true,
    }])
    .await
    .unwrap();
    let mut connection = db.pool().acquire().await.unwrap();
    sqlx::query("INSERT INTO items (id, value) VALUES ('item', 'pending')")
        .execute(&mut *connection)
        .await
        .unwrap();
    connection.return_to_pool().await;
    assert!(
        db.pool().num_idle() > 0,
        "cloudsync test pool has no idle connections (size={}, max={})",
        db.pool().size(),
        db.pool().options().get_max_connections(),
    );
    (dir, db)
}
