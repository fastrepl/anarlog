mod commands;
mod error;
mod import;
mod runtime;

pub use error::{Error, Result};
pub use runtime::open_app_db;
use tauri::Manager;

const PLUGIN_NAME: &str = "db";

pub type ManagedState = std::sync::Arc<runtime::PluginDbRuntime>;

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatement {
    pub sql: String,
    pub params: Vec<serde_json::Value>,
    #[serde(default)]
    pub expected_rows_affected: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationState {
    pub phase: String,
    pub latest_run_id: String,
    pub parity_verified: bool,
    pub cutover_at: Option<String>,
    pub rollback_until: Option<String>,
    pub last_error: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportRun {
    pub id: String,
    pub importer_version: i64,
    pub source_root: String,
    pub dry_run: bool,
    pub status: String,
    pub discovered_count: i64,
    pub imported_count: i64,
    pub matched_count: i64,
    pub skipped_count: i64,
    pub conflict_count: i64,
    pub error_count: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportItemReport {
    pub source_path: String,
    pub source_kind: String,
    pub source_sha256: String,
    pub status: String,
    pub discovered_count: i64,
    pub imported_count: i64,
    pub matched_count: i64,
    pub skipped_count: i64,
    pub conflict_count: i64,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportTargetReport {
    pub source_path: String,
    pub table_name: String,
    pub target_id: String,
    pub status: String,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportReport {
    pub state: StorageMigrationState,
    pub latest_run: Option<LegacyImportRun>,
    pub items: Vec<LegacyImportItemReport>,
    pub targets: Vec<LegacyImportTargetReport>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCleanupStatus {
    pub migration_verified: bool,
    pub available: bool,
    pub already_cleaned: bool,
    pub file_count: u64,
    pub total_bytes: u64,
    pub source_root: String,
    pub blocking_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCleanupResult {
    pub deleted_file_count: u64,
    pub deleted_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq)]
pub struct ExecuteProxyResult {
    rows: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq)]
#[serde(tag = "event", content = "data")]
pub enum QueryEvent {
    #[serde(rename = "result")]
    Result(Vec<serde_json::Value>),
    #[serde(rename = "error")]
    Error(String),
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::execute,
            commands::execute_transaction,
            commands::execute_proxy,
            commands::get_legacy_import_report,
            commands::get_legacy_cleanup_status,
            commands::cleanup_legacy_files,
            commands::run_legacy_import,
            commands::subscribe,
            commands::unsubscribe,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(
    db: std::sync::Arc<hypr_db_core::Db>,
) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _| {
            hypr_tauri_utils::block_on(hypr_db_app::prepare_schema(db.as_ref()))?;
            hypr_tauri_utils::block_on(import::import_legacy_data(app.app_handle(), db.pool()))?;
            app.manage(std::sync::Arc::new(runtime::PluginDbRuntime::new(db)));
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use hypr_db_reactive::QueryEventSink;
    use serde_json::json;
    use tauri::ipc::{Channel, InvokeResponseBody};

    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn capture_channel() -> (Channel<QueryEvent>, Arc<Mutex<Vec<QueryEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(payload) = body else {
                return Ok(());
            };
            let event: QueryEvent =
                serde_json::from_str(&payload).expect("channel payload should parse");
            captured.lock().unwrap().push(event);
            Ok(())
        });
        (channel, events)
    }

    async fn next_event(
        events: &Arc<Mutex<Vec<QueryEvent>>>,
        index: usize,
    ) -> anyhow::Result<QueryEvent> {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(event) = events.lock().unwrap().get(index).cloned() {
                    return event;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(anyhow::Error::from)
    }

    async fn setup_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        (dir, Arc::new(runtime::PluginDbRuntime::new(Arc::new(db))))
    }

    async fn setup_unmigrated_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();

        (dir, Arc::new(runtime::PluginDbRuntime::new(Arc::new(db))))
    }

    #[tokio::test]
    async fn query_event_channel_sends_result_payload() {
        let (channel, events) = capture_channel();
        let sink = runtime::QueryEventChannel::new(channel);

        sink.send_result(vec![json!({ "id": "note-1" })]).unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Result(vec![json!({ "id": "note-1" })]));
    }

    #[tokio::test]
    async fn query_event_channel_sends_error_payload() {
        let (channel, events) = capture_channel();
        let sink = runtime::QueryEventChannel::new(channel);

        sink.send_error("boom".to_string()).unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Error("boom".to_string()));
    }

    #[test]
    fn query_event_serializes_with_tagged_shape() {
        let result =
            serde_json::to_value(QueryEvent::Result(vec![json!({ "id": "note-1" })])).unwrap();
        let error = serde_json::to_value(QueryEvent::Error("boom".to_string())).unwrap();

        assert_eq!(
            result,
            json!({ "event": "result", "data": [{ "id": "note-1" }] })
        );
        assert_eq!(error, json!({ "event": "error", "data": "boom" }));
    }

    #[tokio::test]
    async fn subscribe_sends_initial_result_through_channel() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        runtime
            .subscribe(
                "SELECT id, title FROM templates WHERE id = 'missing-template' ORDER BY id"
                    .to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Result(Vec::new()));
    }

    #[tokio::test]
    async fn execute_proxy_applies_app_schema_before_run() {
        let (_dir, runtime) = setup_unmigrated_runtime().await;

        runtime
            .execute_proxy(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                vec![json!("template-1"), json!("Template 1")],
                hypr_db_execute::ProxyQueryMethod::Run,
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
            hypr_db_reactive::DependencyAnalysis::Reactive { .. }
        ));

        let event = next_event(&events, 0).await.unwrap();
        assert!(matches!(event, QueryEvent::Result(rows) if !rows.is_empty()));
    }

    #[tokio::test]
    async fn invalid_sql_sends_error_through_channel() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        runtime
            .subscribe(
                "SELECT * FROM missing_table".to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert!(matches!(event, QueryEvent::Error(_)));
    }
}
