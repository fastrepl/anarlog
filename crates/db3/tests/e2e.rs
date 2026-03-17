use db3::Db3;

fn connection_string() -> String {
    std::env::var("SQLITECLOUD_URL").expect("SQLITECLOUD_URL must be set")
}

async fn setup_db() -> Db3 {
    let db = Db3::connect_memory().await.unwrap();

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

    db
}

#[tokio::test]
async fn sync_roundtrip() {
    let marker = uuid::Uuid::new_v4().to_string();

    let db_a = setup_db().await;

    sqlx::query("INSERT INTO test_sync (id, value) VALUES (cloudsync_uuid(), ?)")
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();

    db_a.cloudsync_network_sync(Some(5000), Some(3))
        .await
        .unwrap();

    let db_b = setup_db().await;

    db_b.cloudsync_network_sync(Some(5000), Some(3))
        .await
        .unwrap();

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, value FROM test_sync")
        .fetch_all(db_b.pool())
        .await
        .unwrap();

    assert!(
        rows.iter().any(|(_, v)| v == &marker),
        "db_b should contain the row inserted by db_a"
    );
}
