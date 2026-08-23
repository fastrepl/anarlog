#![forbid(unsafe_code)]

mod e2ee_sync;
mod replica_sync;
mod witness;
mod witness_watch;

pub use e2ee_sync::{E2eeSyncHook, ReplicaSyncOutcome, ReplicaSyncStatus};
pub use replica_sync::{ReplicaSyncTask, spawn_replica_sync};
pub use witness::{E2eeWitnessCancellation, E2eeWitnessClient, E2eeWitnessConfig};
pub use witness_watch::{WitnessWatchTask, spawn_witness_watch};

static RUNTIME: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();

pub fn set_runtime_handle(handle: tokio::runtime::Handle) {
    let _ = RUNTIME.set(handle);
}

pub fn runtime_handle() -> Option<tokio::runtime::Handle> {
    tokio::runtime::Handle::try_current()
        .ok()
        .or_else(|| RUNTIME.get().cloned())
}

pub(crate) fn spawn_background(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(fut);
        }
        Err(_) => {
            RUNTIME.get().expect("db-sync runtime handle").spawn(fut);
        }
    }
}

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
