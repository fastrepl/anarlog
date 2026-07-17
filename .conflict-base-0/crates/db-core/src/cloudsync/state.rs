use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{CloudsyncNetworkResult, CloudsyncRuntimeConfig};

#[derive(Default, Debug)]
pub(crate) struct CloudsyncRuntimeState {
    pub(crate) config: Option<CloudsyncRuntimeConfig>,
    pub(crate) running: bool,
    pub(crate) network_initialized: bool,
    pub(crate) task: Option<CloudsyncBackgroundTask>,
    pub(crate) last_sync: Option<CloudsyncNetworkResult>,
    pub(crate) last_sync_at_ms: Option<u64>,
    pub(crate) last_error: Option<String>,
    pub(crate) last_error_kind: Option<hypr_cloudsync::ErrorKind>,
    pub(crate) consecutive_failures: u32,
}

#[derive(Debug)]
pub(crate) struct CloudsyncBackgroundTask {
    pub(crate) shutdown_tx: Option<oneshot::Sender<()>>,
    pub(crate) join_handle: JoinHandle<()>,
}
