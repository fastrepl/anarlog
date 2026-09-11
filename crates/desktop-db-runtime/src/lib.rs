#![allow(clippy::module_name_repetitions)]

mod e2ee_witness;
pub mod error;
pub mod legacy;
pub mod runtime;

pub use anlg_db_reactive::QueryEventSink;
pub use error::{Error, Result};
pub use runtime::{DesktopDbRuntime, open_app_db, open_app_db_unmigrated};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatement {
    pub sql: String,
    pub params: Vec<serde_json::Value>,
    #[serde(default)]
    pub expected_rows_affected: Option<u64>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupPhase {
    PreparingDatabase,
    MigratingDatabase,
    ImportingLegacyData,
    ConfiguringCloudsync,
    Ready,
    Failed,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub phase: StartupPhase,
    pub migration_current: Option<u32>,
    pub migration_total: Option<u32>,
}

impl StartupStatus {
    pub fn for_phase(phase: StartupPhase) -> Self {
        Self {
            phase,
            migration_current: None,
            migration_total: None,
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportTargetReport {
    pub source_path: String,
    pub table_name: String,
    pub target_id: String,
    pub status: String,
    pub error: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportReport {
    pub state: StorageMigrationState,
    pub latest_run: Option<LegacyImportRun>,
    pub items: Vec<LegacyImportItemReport>,
    pub targets: Vec<LegacyImportTargetReport>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize)]
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCleanupResult {
    pub deleted_file_count: u64,
    pub deleted_bytes: u64,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionIngestApplyResult {
    Applied,
    AlreadyApplied,
    Rejected,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeIdentityStatus {
    pub configured: bool,
    pub key_id: Option<String>,
    pub member_public_key: Option<String>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeRecoveryKeyIdentity {
    pub key_id: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eeDeviceIdentity {
    pub public_key: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct E2eeDeviceEnrollmentPackage {
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudsyncWorkspaceKeyGrant {
    pub workspace_id: String,
    pub key_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
    pub is_active: bool,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceE2eeKeyRecipient {
    pub user_id: String,
    pub public_key: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceE2eeKeyGrantUpload {
    pub user_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SealedWorkspaceE2eeKey {
    pub key_id: String,
    pub grants: Vec<WorkspaceE2eeKeyGrantUpload>,
}

impl From<CloudsyncWorkspaceKeyGrant> for anlg_e2ee::WorkspaceKeyGrant {
    fn from(value: CloudsyncWorkspaceKeyGrant) -> Self {
        Self {
            key_id: value.key_id,
            ephemeral_public_key: value.ephemeral_public_key,
            nonce: value.nonce,
            ciphertext: value.ciphertext,
        }
    }
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExecuteProxyResult {
    pub rows: Vec<serde_json::Value>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "event", content = "data")]
pub enum QueryEvent {
    #[serde(rename = "result")]
    Result(Vec<serde_json::Value>),
    #[serde(rename = "error")]
    Error(String),
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncTokenConfigurationResult {
    Configured,
    AccountMismatch,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspaceProjection {
    pub account_user_id: String,
    pub personal_workspace_id: String,
    pub workspaces: Vec<CloudsyncWorkspaceProjectionEntry>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncE2eeWitness {
    pub endpoint: String,
    pub access_token: String,
}

impl From<CloudsyncE2eeWitness> for anlg_db_sync::E2eeWitnessConfig {
    fn from(value: CloudsyncE2eeWitness) -> Self {
        Self {
            endpoint: value.endpoint,
            access_token: value.access_token,
        }
    }
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
