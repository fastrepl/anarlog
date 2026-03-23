use db_watch::{TableDeps, extract_tables};
use sqlx::SqlitePool;

async fn test_pool() -> SqlitePool {
    let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
    hypr_db_app::migrate(db.pool()).await.unwrap();
    db.pool().clone()
}

#[tokio::test]
async fn full_cycle() {
    let pool = test_pool().await;
    let tables = extract_tables(&pool, "SELECT id FROM meetings WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["meetings"]).contains(&w));
    assert!(!deps.affected(&["words"]).contains(&w));
}

#[tokio::test]
async fn multi_table_join() {
    let pool = test_pool().await;
    let tables = extract_tables(
        &pool,
        "SELECT w.id FROM words w JOIN meetings m ON w.meeting_id = m.id",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["meetings"]).contains(&w));
    assert!(deps.affected(&["words"]).contains(&w));
    assert!(!deps.affected(&["chat_messages"]).contains(&w));
}

#[tokio::test]
async fn unregister_stops_notifications() {
    let pool = test_pool().await;
    let tables = extract_tables(&pool, "SELECT id FROM meetings WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["meetings"]).contains(&w));

    deps.unregister(w);
    assert!(!deps.affected(&["meetings"]).contains(&w));
}

#[tokio::test]
async fn overlapping_watches() {
    let pool = test_pool().await;

    let tables_a = extract_tables(
        &pool,
        "SELECT w.id FROM words w JOIN meetings m ON w.meeting_id = m.id",
    )
    .await
    .unwrap();

    let tables_b = extract_tables(
        &pool,
        "SELECT c.id FROM chat_messages c JOIN meetings m ON c.meeting_id = m.id",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let a = deps.register(tables_a);
    let b = deps.register(tables_b);

    let words_hit = deps.affected(&["words"]);
    assert!(words_hit.contains(&a));
    assert!(!words_hit.contains(&b));

    let chat_hit = deps.affected(&["chat_messages"]);
    assert!(!chat_hit.contains(&a));
    assert!(chat_hit.contains(&b));

    let meetings_hit = deps.affected(&["meetings"]);
    assert!(meetings_hit.contains(&a));
    assert!(meetings_hit.contains(&b));
}

#[tokio::test]
async fn fts_watch_cycle() {
    let pool = test_pool().await;
    let tables = extract_tables(
        &pool,
        "SELECT rowid FROM meetings_fts WHERE meetings_fts MATCH 'test'",
    )
    .await
    .unwrap();

    assert!(!tables.is_empty());

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["meetings_fts"]).contains(&w));
    assert!(!deps.affected(&["meetings"]).contains(&w));
}
