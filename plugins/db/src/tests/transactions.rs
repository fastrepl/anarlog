use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use sqlx::Connection;

use crate::{QueryEvent, TransactionStatement, runtime};

use super::support::{
    capture_channel, next_event, setup_runtime, setup_unmigrated_runtime,
    setup_unmigrated_runtime_with_max_connections,
};

#[tokio::test]
async fn execute_proxy_applies_app_schema_before_run() {
    let (_dir, runtime) = setup_unmigrated_runtime().await;

    runtime
        .execute_proxy(
            "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
            vec![json!("template-1"), json!("Template 1")],
            anlg_db_execute::ProxyQueryMethod::Run,
        )
        .await
        .unwrap();

    let rows = runtime
        .execute(
            "SELECT id, title FROM templates WHERE id = ?".to_string(),
            vec![json!("template-1")],
        )
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![json!({
            "id": "template-1",
            "title": "Template 1",
        })]
    );
}

#[tokio::test]
async fn execute_transaction_commits_every_statement_atomically() {
    let (_dir, runtime) = setup_unmigrated_runtime().await;

    let rows_affected = runtime
        .execute_transaction(vec![
            TransactionStatement {
                sql: "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                params: vec![json!("template-1"), json!("Template 1")],
                expected_rows_affected: None,
            },
            TransactionStatement {
                sql: "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                params: vec![json!("template-2"), json!("Template 2")],
                expected_rows_affected: None,
            },
        ])
        .await
        .unwrap();

    assert_eq!(rows_affected, vec![1, 1]);

    let rows = runtime
        .execute(
            "SELECT id FROM templates WHERE id IN (?, ?) ORDER BY id".to_string(),
            vec![json!("template-1"), json!("template-2")],
        )
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![json!({ "id": "template-1" }), json!({ "id": "template-2" })]
    );
}

#[tokio::test]
async fn execute_transaction_rolls_back_when_a_statement_fails() {
    let (_dir, runtime) = setup_runtime().await;

    let result = runtime
        .execute_transaction(vec![
            TransactionStatement {
                sql: "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                params: vec![json!("template-rollback"), json!("Rollback")],
                expected_rows_affected: None,
            },
            TransactionStatement {
                sql: "INSERT INTO missing_table (id) VALUES (?)".to_string(),
                params: vec![json!("fail")],
                expected_rows_affected: None,
            },
        ])
        .await;

    assert!(result.is_err());
    let rows = runtime
        .execute(
            "SELECT id FROM templates WHERE id = ?".to_string(),
            vec![json!("template-rollback")],
        )
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn execute_transaction_rolls_back_when_affected_rows_do_not_match() {
    let (_dir, runtime) = setup_runtime().await;

    let result = runtime
        .execute_transaction(vec![
            TransactionStatement {
                sql: "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                params: vec![json!("template-rollback"), json!("Rollback")],
                expected_rows_affected: Some(1),
            },
            TransactionStatement {
                sql: "UPDATE templates SET title = ? WHERE id = ?".to_string(),
                params: vec![json!("Missing"), json!("missing-template")],
                expected_rows_affected: Some(1),
            },
        ])
        .await;

    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("statement 1 affected 0 rows; expected 1")
    );
    let rows = runtime
        .execute(
            "SELECT id FROM templates WHERE id = ?".to_string(),
            vec![json!("template-rollback")],
        )
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn cancelled_transaction_returns_a_clean_connection_to_the_pool() {
    let (_dir, runtime) = setup_unmigrated_runtime_with_max_connections(1).await;
    sqlx::query("CREATE TEMP TABLE pooled_connection_marker (id INTEGER PRIMARY KEY)")
        .execute(runtime.pool())
        .await
        .unwrap();

    runtime.pause_next_transaction_after_begin();
    let transaction_runtime = Arc::clone(&runtime);
    let transaction_task = tokio::spawn(async move {
        transaction_runtime
            .execute_transaction(vec![TransactionStatement {
                sql: "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                params: vec![json!("cancelled"), json!("Cancelled")],
                expected_rows_affected: Some(1),
            }])
            .await
    });

    runtime.wait_for_transaction_after_begin().await;
    transaction_task.abort();
    assert!(transaction_task.await.unwrap_err().is_cancelled());

    let mut connection = tokio::time::timeout(Duration::from_secs(2), runtime.pool().acquire())
        .await
        .expect("timed out reacquiring the pooled connection")
        .unwrap();
    assert!(!connection.is_in_transaction());
    let marker_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pooled_connection_marker")
        .fetch_one(&mut *connection)
        .await
        .expect("the original pooled connection should be reused");
    assert_eq!(marker_count, 0);

    let mut transaction = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("a new immediate transaction should start");
    sqlx::query("INSERT INTO templates (id, title) VALUES (?, ?)")
        .bind("after-cancellation")
        .bind("After cancellation")
        .execute(&mut *transaction)
        .await
        .expect("a local write should succeed after cancellation");
    transaction.commit().await.unwrap();
    drop(connection);

    let rows = runtime
        .execute(
            "SELECT id FROM templates WHERE id = ?".to_string(),
            vec![json!("after-cancellation")],
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!({ "id": "after-cancellation" })]);
}

#[tokio::test]
async fn open_memory_app_db_subscribe_sees_app_schema() {
    let db = runtime::open_app_db(None).await.unwrap();
    let runtime = runtime::PluginDbRuntime::new(Arc::new(db));
    let (channel, events) = capture_channel();

    let registration = runtime
        .subscribe(
            "SELECT id, title FROM templates ORDER BY id".to_string(),
            vec![],
            runtime::QueryEventChannel::new(channel),
        )
        .await
        .unwrap();

    assert!(matches!(
        registration.analysis,
        anlg_db_reactive::DependencyAnalysis::Reactive { .. }
    ));

    let event = next_event(&events, 0).await.unwrap();
    assert!(matches!(event, QueryEvent::Result(rows) if !rows.is_empty()));
}
