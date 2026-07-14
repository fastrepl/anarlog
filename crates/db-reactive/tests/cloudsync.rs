mod common;

use std::time::Duration;

use common::{TestEvent, TestSink, next_event};
use db_reactive::LiveQueryRuntime;
use hypr_db_core::{Db, DbOpenOptions, DbStorage};
use serde_json::json;

fn connection_string() -> String {
    std::env::var("SQLITECLOUD_URL").expect("SQLITECLOUD_URL must be set")
}

async fn setup_db() -> (tempfile::TempDir, Db) {
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
        "CREATE TABLE IF NOT EXISTS test_sync (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    db.cloudsync_init("test_sync", None, None).await.unwrap();
    db.cloudsync_network_init(&connection_string())
        .await
        .unwrap();

    (dir, db)
}

#[tokio::test]
#[ignore = "external verification only; requires SQLITECLOUD_URL and an explicit --ignored run"]
async fn cloudsync_pull_refreshes_live_query_subscriptions() {
    let marker = uuid::Uuid::new_v4().to_string();

    let (_dir_a, db_a) = setup_db().await;
    let (_dir_b, db_b) = setup_db().await;
    let pool_b = db_b.pool().clone();
    let runtime_b = LiveQueryRuntime::new(std::sync::Arc::new(db_b));
    let (sink, events) = TestSink::capture();

    runtime_b
        .subscribe(
            "SELECT id, value FROM test_sync WHERE value = ? ORDER BY id".to_string(),
            vec![json!(marker)],
            sink,
        )
        .await
        .unwrap();

    let initial = next_event(&events, 0, Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(initial, TestEvent::Result(Vec::new()));

    sqlx::query("INSERT INTO test_sync (id, value) VALUES (cloudsync_uuid(), ?)")
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();

    db_a.cloudsync_network_sync(Some(5000), Some(3))
        .await
        .unwrap();
    hypr_cloudsync::network_sync(&pool_b, Some(5000), Some(3))
        .await
        .unwrap();

    let event = next_event(&events, 1, Duration::from_secs(10))
        .await
        .unwrap();
    let TestEvent::Result(rows) = event else {
        panic!("expected result event");
    };

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], marker);
}
