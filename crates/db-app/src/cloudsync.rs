use std::sync::LazyLock;

use anlg_db_core::CloudsyncTableSpec;

mod binding;
mod projection;
mod recovery;

pub use binding::{
    CLOUDSYNC_WORKSPACE_BINDING_ID, bind_cloudsync_account, claim_cloudsync_workspace,
    claim_cloudsync_workspace_cancellable, cloudsync_workspace_is_claimed_by,
    ensure_cloudsync_workspace_binding,
};
pub use projection::{
    CloudsyncWorkspaceProjection, CloudsyncWorkspaceProjectionEntry,
    CloudsyncWorkspaceReconciliationPlan, cloudsync_write_filter_installed,
    cloudsync_write_filter_version_current, commit_cloudsync_workspace_projection,
    commit_cloudsync_workspace_projection_cancellable, mark_cloudsync_write_filter_installed,
    replace_cloudsync_workspace_projection, set_cloudsync_personal_write_scope,
    stage_cloudsync_workspace_reconciliation, stage_cloudsync_workspace_reconciliation_cancellable,
    validate_cloudsync_workspace_projection,
};
pub use recovery::{
    CloudsyncFullResyncResetState, CloudsyncRecoveryPhase, CloudsyncRecoveryState,
    advance_cloudsync_recovery_phase, clear_cloudsync_full_resync_pending,
    cloudsync_encrypted_replica_is_empty, cloudsync_full_resync_generation,
    cloudsync_full_resync_requires_reset, cloudsync_full_resync_reset_state,
    cloudsync_recovery_barrier_is_exact, cloudsync_recovery_state, complete_cloudsync_recovery,
    delete_cloudsync_recovery_barrier, ensure_cloudsync_recovery_state,
    insert_cloudsync_recovery_barrier, mark_cloudsync_full_resync_receive_only_reset_applied,
    mark_cloudsync_full_resync_reset_applied, stage_cloudsync_poison_recovery,
};

#[cfg(test)]
use binding::{CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE, LEGACY_DEFAULT_USER_ID, USER_ID_REFERENCES};
#[cfg(test)]
use projection::CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE;

const CLOUDSYNC_FULL_RESYNC_PENDING_ID: &str = "cloudsync_full_resync_pending";
const CLOUDSYNC_FULL_RESYNC_RECOVERY_ID: &str = "cloudsync_full_resync_recovery_v1";
const CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID: &str = "cloudsync_full_resync_reset_applied";
const CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID: &str =
    "cloudsync_full_resync_receive_only_reset_applied";
const CLOUDSYNC_RECOVERY_PROTOCOL_VERSION: u32 = 1;
const CLOUDSYNC_RECOVERY_BARRIER_TABLE: &str = "__anarlog_cloudsync_control";
const CLOUDSYNC_RECOVERY_BARRIER_FIELD: &str = "$snapshot_barrier";

#[derive(Debug)]
pub enum CloudsyncWorkspaceError {
    Sqlx(sqlx::Error),
    ClaimCancelled,
    ProjectionCancelled,
    InvalidWorkspaceId,
    InvalidBinding,
    InvalidWorkspaceProjection,
    InvalidRecoveryState,
    RecoveryConflict,
    AccountMismatch,
    ForeignWorkspace { table: String },
}

impl std::fmt::Display for CloudsyncWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::ClaimCancelled => write!(f, "CloudSync workspace claim was cancelled"),
            Self::ProjectionCancelled => {
                write!(f, "CloudSync workspace projection was cancelled")
            }
            Self::InvalidWorkspaceId => write!(f, "workspace ID is invalid"),
            Self::InvalidBinding => write!(f, "local CloudSync workspace binding is invalid"),
            Self::InvalidWorkspaceProjection => {
                write!(f, "CloudSync workspace projection is invalid")
            }
            Self::InvalidRecoveryState => write!(f, "CloudSync recovery state is invalid"),
            Self::RecoveryConflict => write!(f, "CloudSync recovery state changed unexpectedly"),
            Self::AccountMismatch => write!(
                f,
                "this local database is already bound to a different account"
            ),
            Self::ForeignWorkspace { table } => write!(
                f,
                "{table} contains rows owned by a different CloudSync workspace"
            ),
        }
    }
}

impl std::error::Error for CloudsyncWorkspaceError {}

impl From<sqlx::Error> for CloudsyncWorkspaceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    [
        ("action_items", false),
        ("calendars", false),
        ("chat_groups", false),
        ("chat_messages", false),
        ("daily_notes", false),
        ("e2ee_records", true),
        ("entity_mentions", false),
        ("events", false),
        ("humans", false),
        ("organizations", false),
        ("session_attachments", false),
        ("session_documents", false),
        ("session_participants", false),
        ("session_tags", false),
        ("sessions", false),
        ("tags", false),
        ("templates", false),
        ("transcripts", false),
        ("workspace_memberships", false),
        ("workspaces", false),
    ]
    .into_iter()
    .map(|(table_name, enabled)| CloudsyncTableSpec {
        table_name: table_name.to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled,
    })
    .collect()
});

pub fn cloudsync_table_registry() -> &'static [CloudsyncTableSpec] {
    CLOUDSYNC_TABLE_REGISTRY.as_slice()
}

pub fn cloudsync_alter_guard_required(table_name: &str) -> bool {
    table_name == "e2ee_records" || crate::E2EE_DOMAIN_TABLES.contains(&table_name)
}

#[cfg(test)]
mod tests;
