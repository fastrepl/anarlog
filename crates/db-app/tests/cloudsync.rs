use std::time::{Duration, SystemTime, UNIX_EPOCH};

use db_app::{cloudsync_table_registry, prepare_schema};
use hypr_db_core::{CloudsyncAuth, CloudsyncRuntimeConfig, Db, DbOpenOptions, DbStorage};

const SYNC_TIMEOUT: Duration = Duration::from_secs(90);

fn cloudsync_config() -> CloudsyncRuntimeConfig {
    CloudsyncRuntimeConfig {
        connection_string: std::env::var("ANARLOG_CLOUDSYNC_DATABASE_ID")
            .expect("ANARLOG_CLOUDSYNC_DATABASE_ID must be set"),
        auth: CloudsyncAuth::ApiKey {
            api_key: std::env::var("ANARLOG_CLOUDSYNC_API_KEY")
                .expect("ANARLOG_CLOUDSYNC_API_KEY must be set"),
        },
        tables: cloudsync_table_registry().to_vec(),
        sync_interval_ms: 300_000,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    }
}

async fn setup_db() -> Db {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();

    prepare_schema(&db).await.unwrap();
    db.cloudsync_configure(cloudsync_config()).unwrap();
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_start())
        .await
        .expect("cloudsync start timed out")
        .unwrap();
    db
}

#[tokio::test]
#[ignore = "external verification only; requires the anarlog-dev SQLite Cloud credentials"]
async fn core_session_syncs_between_two_clients() {
    let marker = format!(
        "anarlog-cloudsync-e2e-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let db_a = setup_db().await;

    sqlx::query("INSERT INTO sessions (id, title) VALUES (cloudsync_uuid(), ?)")
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();
    tokio::time::timeout(SYNC_TIMEOUT, db_a.cloudsync_trigger_sync())
        .await
        .expect("first client sync timed out")
        .unwrap();

    let db_b = setup_db().await;
    for _ in 0..2 {
        tokio::time::timeout(SYNC_TIMEOUT, db_b.cloudsync_trigger_sync())
            .await
            .expect("second client sync timed out")
            .unwrap();
    }

    let title: Option<String> =
        sqlx::query_scalar("SELECT title FROM sessions WHERE title = ? LIMIT 1")
            .bind(&marker)
            .fetch_optional(db_b.pool())
            .await
            .unwrap();

    assert_eq!(title.as_deref(), Some(marker.as_str()));

    sqlx::query("DELETE FROM sessions WHERE title = ?")
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();
    tokio::time::timeout(SYNC_TIMEOUT, db_a.cloudsync_trigger_sync())
        .await
        .expect("cleanup sync timed out")
        .unwrap();

    tokio::time::timeout(Duration::from_secs(15), db_a.cloudsync_stop())
        .await
        .expect("first client stop timed out")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), db_b.cloudsync_stop())
        .await
        .expect("second client stop timed out")
        .unwrap();
}
