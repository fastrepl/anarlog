use std::future::Future;
use std::pin::Pin;

use sqlx::SqlitePool;

use super::CloudsyncNetworkResult;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CloudsyncSyncDirective {
    #[default]
    SendAndReceive,
    ReceiveOnly,
    Deferred,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CloudsyncHookOutcome {
    pub local_work_remaining: bool,
    pub deferred: bool,
}

pub type CloudsyncBeforeHookFuture<'a> = Pin<
    Box<dyn Future<Output = Result<CloudsyncSyncDirective, anlg_cloudsync::Error>> + Send + 'a>,
>;
pub type CloudsyncHookFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CloudsyncHookOutcome, anlg_cloudsync::Error>> + Send + 'a>>;

pub trait CloudsyncSyncHook: Send + Sync + 'static {
    fn activity_paused(&self) -> bool {
        false
    }

    fn cancel_active_sync(&self) {}

    fn before_sync<'a>(&'a self, pool: &'a SqlitePool) -> CloudsyncBeforeHookFuture<'a>;
    fn after_sync<'a>(
        &'a self,
        pool: &'a SqlitePool,
        result: &'a CloudsyncNetworkResult,
    ) -> CloudsyncHookFuture<'a>;
}
