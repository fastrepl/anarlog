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

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeIdentityStatus {
    pub configured: bool,
    pub key_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeRecoveryKeyIdentity {
    pub key_id: String,
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

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncTokenConfigurationResult {
    Configured,
    AccountMismatch,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspaceProjection {
    pub account_user_id: String,
    pub personal_workspace_id: String,
    pub workspaces: Vec<CloudsyncWorkspaceProjectionEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspaceProjectionEntry {
    pub id: String,
    pub owner_user_id: String,
    pub kind: String,
    pub name: String,
    pub membership_id: String,
    pub role: String,
    pub membership_created_at: String,
    pub membership_updated_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<CloudsyncWorkspaceProjection> for hypr_db_app::CloudsyncWorkspaceProjection {
    fn from(projection: CloudsyncWorkspaceProjection) -> Self {
        Self {
            account_user_id: projection.account_user_id,
            personal_workspace_id: projection.personal_workspace_id,
            workspaces: projection
                .workspaces
                .into_iter()
                .map(|workspace| hypr_db_app::CloudsyncWorkspaceProjectionEntry {
                    id: workspace.id,
                    owner_user_id: workspace.owner_user_id,
                    kind: workspace.kind,
                    name: workspace.name,
                    membership_id: workspace.membership_id,
                    role: workspace.role,
                    membership_created_at: workspace.membership_created_at,
                    membership_updated_at: workspace.membership_updated_at,
                    created_at: workspace.created_at,
                    updated_at: workspace.updated_at,
                })
                .collect(),
        }
    }
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_meetings,
            commands::get_meeting,
            commands::get_meeting_transcript,
            commands::get_recurring_meeting_history,
            commands::execute,
            commands::execute_transaction,
            commands::execute_proxy,
            commands::get_legacy_import_report,
            commands::get_legacy_cleanup_status,
            commands::cleanup_legacy_files,
            commands::run_legacy_import,
            commands::get_e2ee_identity_status<tauri::Wry>,
            commands::inspect_e2ee_recovery_key,
            commands::create_e2ee_identity<tauri::Wry>,
            commands::import_e2ee_identity<tauri::Wry>,
            commands::subscribe,
            commands::unsubscribe,
            commands::configure_cloudsync,
            commands::bind_cloudsync_account,
            commands::configure_cloudsync_token<tauri::Wry>,
            commands::start_cloudsync,
            commands::stop_cloudsync,
            commands::suspend_cloudsync,
            commands::get_cloudsync_status,
            commands::sync_cloudsync_now,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(
    db: std::sync::Arc<hypr_db_core::Db>,
) -> tauri::plugin::TauriPlugin<R> {
    init_with_cloudsync(db, None)
}

pub fn init_with_cloudsync<R: tauri::Runtime>(
    db: std::sync::Arc<hypr_db_core::Db>,
    startup_config: Option<hypr_db_core::CloudsyncRuntimeConfig>,
) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _| {
            hypr_tauri_utils::block_on(hypr_db_app::prepare_schema(db.as_ref()))?;
            hypr_tauri_utils::block_on(import::import_legacy_data(app.app_handle(), db.pool()))?;
            if let Some(config) = startup_config.clone() {
                let migration_verified =
                    hypr_tauri_utils::block_on(import::legacy_migration_verified(db.pool()))?;
                if !migration_verified {
                    tracing::warn!(
                        "startup CloudSync configuration skipped until legacy migration is verified"
                    );
                } else if let Err(error) =
                    hypr_tauri_utils::block_on(db.cloudsync_configure(config))
                {
                    tracing::warn!(%error, "failed to configure startup cloudsync");
                } else {
                    let sync_db = std::sync::Arc::clone(&db);
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = sync_db.cloudsync_start().await {
                            tracing::warn!(%error, "failed to start cloudsync");
                            return;
                        }
                        if let Err(error) = sync_db.cloudsync_trigger_sync().await {
                            tracing::warn!(%error, "initial cloudsync failed");
                        }
                    });
                }
            }
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

    async fn setup_runtime_with_cloudsync(
        cloudsync_enabled: bool,
    ) -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();

        (dir, Arc::new(runtime::PluginDbRuntime::new(Arc::new(db))))
    }

    async fn setup_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
        setup_runtime_with_cloudsync(false).await
    }

    async fn setup_enabled_cloudsync_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>)
    {
        let (dir, runtime) = setup_runtime_with_cloudsync(true).await;
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        runtime
            .set_e2ee_recovery_key("user-a", &recovery_key)
            .unwrap();
        (dir, runtime)
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
    async fn cloudsync_transport_stays_inert_until_configured() {
        let (_dir, runtime) = setup_runtime().await;

        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["cloudsync_enabled"], false);
        assert_eq!(status["configured"], false);

        runtime
            .configure_cloudsync(
                serde_json::json!({
                    "connection_string": "managed-database-id",
                    "auth": { "type": "token", "token": "test-token" },
                    "tables": hypr_db_app::cloudsync_table_registry(),
                    "sync_interval_ms": 30_000,
                    "wait_ms": 5_000,
                    "max_retries": 3
                })
                .to_string(),
            )
            .await
            .unwrap();
        runtime.start_cloudsync().await.unwrap();

        let status = runtime.cloudsync_status().await.unwrap();
        assert_eq!(status["configured"], true);
        assert_eq!(status["running"], false);
        assert_eq!(status["network_initialized"], false);

        runtime.logout_cloudsync(false).await.unwrap();
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn cloudsync_waits_for_legacy_migration_verification() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        sqlx::query(
            "UPDATE storage_migration_state
             SET parity_verified = 0, last_error = 'completed_with_issues'
             WHERE id = 'legacy_v1'",
        )
        .execute(runtime.pool())
        .await
        .unwrap();

        let error = runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
            )
            .await
            .unwrap_err();

        assert!(matches!(&error, crate::Error::Io(_)));
        assert!(
            error
                .to_string()
                .contains("migration needs attention before CloudSync can start")
        );
        assert!(runtime.start_cloudsync().await.is_err());
        assert!(runtime.sync_cloudsync_now().await.is_err());
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn account_binding_is_durable_without_rekeying_local_rows() {
        let (_dir, runtime) = setup_runtime().await;
        let local_workspace = hypr_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, workspace_id, owner_user_id, name)
             VALUES (?, ?, ?, 'Local user')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(runtime.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', ?, ?, 'Session')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(runtime.pool())
        .await
        .unwrap();

        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        let binding: (String, String) = sqlx::query_as(
            "SELECT json_extract(value_json, '$.workspace_id'),
                    json_extract(value_json, '$.account_user_id')
             FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .fetch_one(runtime.pool())
        .await
        .unwrap();
        let human: (String, String, String) = sqlx::query_as(
            "SELECT id, workspace_id, owner_user_id FROM humans WHERE name = 'Local user'",
        )
        .fetch_one(runtime.pool())
        .await
        .unwrap();
        let session: (String, String) =
            sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
                .fetch_one(runtime.pool())
                .await
                .unwrap();

        assert_eq!(binding, (local_workspace.clone(), "user-a".to_string()));
        assert_eq!(
            human,
            (
                local_workspace.clone(),
                local_workspace.clone(),
                local_workspace.clone(),
            )
        );
        assert_eq!(session, (local_workspace.clone(), local_workspace));
        assert!(
            !runtime
                .bind_cloudsync_account("user-b".to_string())
                .await
                .unwrap()
        );
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn token_configuration_rejects_local_only_runtime_before_rekeying() {
        let (_dir, runtime) = setup_runtime().await;
        let local_workspace = hypr_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', ?, ?, 'Session')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(runtime.pool())
        .await
        .unwrap();

        let error = runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
            )
            .await
            .unwrap_err();

        let binding: (String, Option<String>) = sqlx::query_as(
            "SELECT json_extract(value_json, '$.workspace_id'),
                    json_extract(value_json, '$.account_user_id')
             FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .fetch_one(runtime.pool())
        .await
        .unwrap();
        let session: (String, String) =
            sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
                .fetch_one(runtime.pool())
                .await
                .unwrap();

        assert!(matches!(
            error,
            crate::Error::Cloudsync(hypr_db_core::CloudsyncRuntimeError::Unavailable)
        ));
        assert_eq!(binding, (local_workspace.clone(), None));
        assert_eq!(session, (local_workspace.clone(), local_workspace));
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn invalid_projection_does_not_claim_an_unbound_database() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        sqlx::query("DELETE FROM app_settings WHERE id = 'cloudsync_workspace_binding'")
            .execute(runtime.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions (id, title) VALUES ('session', 'Session')")
            .execute(runtime.pool())
            .await
            .unwrap();

        let error = runtime
            .configure_cloudsync_token_with_projection(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
                Some(hypr_db_app::CloudsyncWorkspaceProjection {
                    account_user_id: "user-a".to_string(),
                    personal_workspace_id: "user-a".to_string(),
                    workspaces: vec![],
                }),
            )
            .await
            .unwrap_err();

        let binding_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .fetch_one(runtime.pool())
        .await
        .unwrap();
        let session: (String, String) =
            sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
                .fetch_one(runtime.pool())
                .await
                .unwrap();

        assert!(matches!(
            error,
            crate::Error::CloudsyncWorkspace(
                hypr_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection
            )
        ));
        assert_eq!(binding_count, 0);
        assert_eq!(session, (String::new(), String::new()));
    }

    #[tokio::test]
    async fn token_configuration_claims_workspace_and_can_be_suspended() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        let local_workspace = hypr_db_app::ensure_cloudsync_workspace_binding(runtime.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', ?, ?, 'Session')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(runtime.pool())
        .await
        .unwrap();

        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        assert_eq!(
            runtime
                .configure_cloudsync_token(
                    "managed-database-id".to_string(),
                    "token".to_string(),
                    "user-a".to_string(),
                )
                .await
                .unwrap(),
            CloudsyncTokenConfigurationResult::Configured
        );

        let session: (String, String) =
            sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
                .fetch_one(runtime.pool())
                .await
                .unwrap();
        let binding: (String, String) = sqlx::query_as(
            "SELECT json_extract(value_json, '$.workspace_id'),
                    json_extract(value_json, '$.account_user_id')
             FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .fetch_one(runtime.pool())
        .await
        .unwrap();

        assert_eq!(session, ("user-a".to_string(), "user-a".to_string()));
        assert_eq!(binding, ("user-a".to_string(), "user-a".to_string()));
        runtime.suspend_cloudsync().await.unwrap();
    }

    #[tokio::test]
    async fn token_configuration_projects_server_workspaces_after_account_claim() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        let projection = hypr_db_app::CloudsyncWorkspaceProjection {
            account_user_id: "user-a".to_string(),
            personal_workspace_id: "user-a".to_string(),
            workspaces: vec![
                hypr_db_app::CloudsyncWorkspaceProjectionEntry {
                    id: "user-a".to_string(),
                    owner_user_id: "user-a".to_string(),
                    kind: "personal".to_string(),
                    name: "Personal".to_string(),
                    membership_id: "membership-personal".to_string(),
                    role: "owner".to_string(),
                    membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                    membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                    created_at: "2026-07-01T00:00:00Z".to_string(),
                    updated_at: "2026-07-16T00:00:00Z".to_string(),
                },
                hypr_db_app::CloudsyncWorkspaceProjectionEntry {
                    id: "workspace-shared".to_string(),
                    owner_user_id: "user-b".to_string(),
                    kind: "shared".to_string(),
                    name: "Shared".to_string(),
                    membership_id: "membership-shared".to_string(),
                    role: "member".to_string(),
                    membership_created_at: "2026-07-02T01:00:00Z".to_string(),
                    membership_updated_at: "2026-07-15T01:00:00Z".to_string(),
                    created_at: "2026-07-02T00:00:00Z".to_string(),
                    updated_at: "2026-07-15T00:00:00Z".to_string(),
                },
            ],
        };

        assert_eq!(
            runtime
                .configure_cloudsync_token_with_projection(
                    "managed-database-id".to_string(),
                    "token".to_string(),
                    "user-a".to_string(),
                    Some(projection),
                )
                .await
                .unwrap(),
            CloudsyncTokenConfigurationResult::Configured
        );

        let workspaces: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
                .fetch_all(runtime.pool())
                .await
                .unwrap();
        let memberships: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT workspace_id, user_id, role FROM workspace_memberships ORDER BY workspace_id",
        )
        .fetch_all(runtime.pool())
        .await
        .unwrap();
        let writable_workspace_ids: Vec<String> = sqlx::query_scalar(
            "SELECT allowed_workspace_id
             FROM cloudsync_writable_workspaces
             ORDER BY allowed_workspace_id",
        )
        .fetch_all(runtime.pool())
        .await
        .unwrap();

        assert_eq!(
            workspaces,
            vec![
                ("user-a".to_string(), "Personal".to_string()),
                ("workspace-shared".to_string(), "Shared".to_string()),
            ]
        );
        assert_eq!(
            memberships,
            vec![
                (
                    "user-a".to_string(),
                    "user-a".to_string(),
                    "owner".to_string(),
                ),
                (
                    "workspace-shared".to_string(),
                    "user-a".to_string(),
                    "member".to_string(),
                ),
            ]
        );
        assert_eq!(writable_workspace_ids, vec!["user-a".to_string()]);
        assert!(
            hypr_db_app::cloudsync_write_filter_installed(runtime.pool(), "user-a")
                .await
                .unwrap()
        );
        assert!(runtime.cloudsync_write_filters_match().await.unwrap());
    }

    #[tokio::test]
    async fn token_configuration_account_mismatch_preserves_workspace_projection() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );
        hypr_db_app::replace_cloudsync_workspace_projection(
            runtime.pool(),
            &hypr_db_app::CloudsyncWorkspaceProjection {
                account_user_id: "user-a".to_string(),
                personal_workspace_id: "user-a".to_string(),
                workspaces: vec![hypr_db_app::CloudsyncWorkspaceProjectionEntry {
                    id: "user-a".to_string(),
                    owner_user_id: "user-a".to_string(),
                    kind: "personal".to_string(),
                    name: "Existing".to_string(),
                    membership_id: "membership-existing".to_string(),
                    role: "owner".to_string(),
                    membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                    membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                    created_at: "2026-07-01T00:00:00Z".to_string(),
                    updated_at: "2026-07-16T00:00:00Z".to_string(),
                }],
            },
        )
        .await
        .unwrap();

        let error = runtime
            .configure_cloudsync_token_with_projection(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
                Some(hypr_db_app::CloudsyncWorkspaceProjection {
                    account_user_id: "user-b".to_string(),
                    personal_workspace_id: "user-b".to_string(),
                    workspaces: vec![hypr_db_app::CloudsyncWorkspaceProjectionEntry {
                        id: "user-b".to_string(),
                        owner_user_id: "user-b".to_string(),
                        kind: "personal".to_string(),
                        name: "Replacement".to_string(),
                        membership_id: "membership-replacement".to_string(),
                        role: "owner".to_string(),
                        membership_created_at: "2026-07-01T01:00:00Z".to_string(),
                        membership_updated_at: "2026-07-16T01:00:00Z".to_string(),
                        created_at: "2026-07-01T00:00:00Z".to_string(),
                        updated_at: "2026-07-16T00:00:00Z".to_string(),
                    }],
                }),
            )
            .await
            .unwrap_err();

        let workspaces: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
                .fetch_all(runtime.pool())
                .await
                .unwrap();
        assert!(matches!(
            error,
            crate::Error::CloudsyncWorkspace(
                hypr_db_app::CloudsyncWorkspaceError::InvalidWorkspaceProjection
            )
        ));
        assert_eq!(
            workspaces,
            vec![("user-a".to_string(), "Existing".to_string())]
        );
    }

    #[tokio::test]
    async fn token_configuration_rejects_foreign_workspace_rows() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', 'other-user')")
            .execute(runtime.pool())
            .await
            .unwrap();

        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        let result = runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(result, CloudsyncTokenConfigurationResult::AccountMismatch);
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn token_configuration_rejects_an_invalid_workspace_binding() {
        let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );
        sqlx::query(
            "UPDATE app_settings
             SET value_json = 'not-json'
             WHERE id = 'cloudsync_workspace_binding'",
        )
        .execute(runtime.pool())
        .await
        .unwrap();

        let result = runtime
            .configure_cloudsync_token(
                "managed-database-id".to_string(),
                "token".to_string(),
                "user-a".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(result, CloudsyncTokenConfigurationResult::AccountMismatch);
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn same_account_binding_is_idempotent() {
        let (_dir, runtime) = setup_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn account_switch_is_rejected_and_leaves_cloudsync_suspended() {
        let (_dir, runtime) = setup_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        let bound = runtime
            .bind_cloudsync_account("user-b".to_string())
            .await
            .unwrap();

        assert!(!bound);
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
    }

    #[tokio::test]
    async fn invalid_account_binding_remains_an_error() {
        let (_dir, runtime) = setup_runtime().await;
        assert!(
            runtime
                .bind_cloudsync_account("user-a".to_string())
                .await
                .unwrap()
        );

        assert!(
            runtime
                .bind_cloudsync_account(" ".to_string())
                .await
                .is_err()
        );
        assert_eq!(
            runtime.cloudsync_status().await.unwrap()["configured"],
            false
        );
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
