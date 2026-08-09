mod commands;
mod e2ee_witness;
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
    pub migration_ready: bool,
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

#[derive(Debug, Clone, serde::Serialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeDeviceIdentity {
    pub public_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct E2eeDeviceEnrollmentPackage {
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

impl From<anlg_e2ee::DeviceEnrollmentPackage> for E2eeDeviceEnrollmentPackage {
    fn from(value: anlg_e2ee::DeviceEnrollmentPackage) -> Self {
        Self {
            ephemeral_public_key: value.ephemeral_public_key,
            nonce: value.nonce,
            ciphertext: value.ciphertext,
        }
    }
}

impl From<E2eeDeviceEnrollmentPackage> for anlg_e2ee::DeviceEnrollmentPackage {
    fn from(value: E2eeDeviceEnrollmentPackage) -> Self {
        Self {
            ephemeral_public_key: value.ephemeral_public_key,
            nonce: value.nonce,
            ciphertext: value.ciphertext,
        }
    }
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

#[derive(Clone, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncE2eeWitness {
    pub endpoint: String,
    pub access_token: String,
}

impl From<CloudsyncWorkspaceProjection> for anlg_db_app::CloudsyncWorkspaceProjection {
    fn from(projection: CloudsyncWorkspaceProjection) -> Self {
        Self {
            account_user_id: projection.account_user_id,
            personal_workspace_id: projection.personal_workspace_id,
            workspaces: projection
                .workspaces
                .into_iter()
                .map(|workspace| anlg_db_app::CloudsyncWorkspaceProjectionEntry {
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
            commands::get_or_create_e2ee_device_identity<tauri::Wry>,
            commands::seal_e2ee_recovery_key_for_device<tauri::Wry>,
            commands::import_e2ee_device_enrollment<tauri::Wry>,
            commands::subscribe,
            commands::unsubscribe,
            commands::configure_cloudsync,
            commands::bind_cloudsync_account,
            commands::configure_cloudsync_token<tauri::Wry>,
            commands::start_cloudsync,
            commands::stop_cloudsync,
            commands::suspend_cloudsync,
            commands::suspend_cloudsync_for_sign_out,
            commands::suspend_cloudsync_after_auth_loss,
            commands::get_cloudsync_status,
            commands::sync_cloudsync_now,
            commands::begin_cloudsync_activity,
            commands::end_cloudsync_activity,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(
    db: std::sync::Arc<anlg_db_core::Db>,
) -> tauri::plugin::TauriPlugin<R> {
    init_with_cloudsync(db, None)
}

pub fn init_with_cloudsync<R: tauri::Runtime>(
    db: std::sync::Arc<anlg_db_core::Db>,
    startup_config: Option<anlg_db_core::CloudsyncRuntimeConfig>,
) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _| {
            anlg_tauri_utils::block_on(anlg_db_app::prepare_schema(db.as_ref()))?;
            anlg_tauri_utils::block_on(import::import_legacy_data(app.app_handle(), db.pool()))?;
            if let Some(config) = startup_config.clone() {
                let migration_ready =
                    anlg_tauri_utils::block_on(import::legacy_migration_ready(db.pool()))?;
                if !migration_ready {
                    tracing::warn!(
                        "startup CloudSync configuration skipped until legacy migration is ready"
                    );
                } else if let Err(error) =
                    anlg_tauri_utils::block_on(db.cloudsync_configure(config))
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
mod tests;
