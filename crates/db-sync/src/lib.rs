#![forbid(unsafe_code)]

mod e2ee_sync;
mod replica_sync;
mod witness;
mod witness_watch;

pub use e2ee_sync::{E2eeSyncHook, ReplicaSyncOutcome, ReplicaSyncStatus};
pub use replica_sync::{ReplicaSyncTask, spawn_replica_sync};
pub use witness::{E2eeWitnessCancellation, E2eeWitnessClient, E2eeWitnessConfig};
pub use witness_watch::{WitnessWatchTask, spawn_witness_watch};

pub fn is_permanent_cloudsync_workspace_rejection(
    error: &anlg_db_app::CloudsyncWorkspaceError,
) -> bool {
    matches!(
        error,
        anlg_db_app::CloudsyncWorkspaceError::InvalidWorkspaceId
            | anlg_db_app::CloudsyncWorkspaceError::InvalidBinding
            | anlg_db_app::CloudsyncWorkspaceError::AccountMismatch
            | anlg_db_app::CloudsyncWorkspaceError::ForeignWorkspace { .. }
    )
}
