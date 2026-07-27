use db_core::Db;

async fn setup() -> Db {
    let db = Db::connect_memory().await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS probe (
            id TEXT PRIMARY KEY NOT NULL,
            body TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("probe", None, None).await.unwrap();
    db
}

async fn pending(db: &Db) -> (u64, u64) {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT rows, payload_size FROM cloudsync_payload_chunks")
            .fetch_all(db.pool())
            .await
            .unwrap();
    rows.iter().fold((0u64, 0u64), |(r, b), (rows, bytes)| {
        (r + *rows as u64, b + *bytes as u64)
    })
}

#[tokio::test]
async fn repeated_writes_to_one_row_do_not_multiply_the_sync_payload() {
    let body = "x".repeat(20_000);

    let once = setup().await;
    sqlx::query("INSERT INTO probe (id, body) VALUES ('note', ?)")
        .bind(&body)
        .execute(once.pool())
        .await
        .unwrap();
    let single = pending(&once).await;

    let many = setup().await;
    sqlx::query("INSERT INTO probe (id, body) VALUES ('note', '')")
        .execute(many.pool())
        .await
        .unwrap();
    for i in 0..50 {
        sqlx::query("UPDATE probe SET body = ? WHERE id = 'note'")
            .bind(format!("{body}{i}"))
            .execute(many.pool())
            .await
            .unwrap();
    }
    let repeated = pending(&many).await;

    // One pending row either way: the changeset carries the row's current
    // value, not an entry per write. Debouncing local writes therefore cannot
    // reduce what sync ships -- only the sync interval and the payload shape
    // can.
    assert_eq!(single.0, 1, "a single write should stage one row");
    assert_eq!(
        repeated.0, 1,
        "51 writes to one row should still stage one row"
    );
    assert!(
        repeated.1 < single.1 * 2,
        "payload should not grow with write count: {} vs {}",
        repeated.1,
        single.1
    );
}
