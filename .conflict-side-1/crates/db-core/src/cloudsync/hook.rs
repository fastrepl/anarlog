use std::future::Future;
use std::pin::Pin;

use sqlx::SqlitePool;

pub type CloudsyncHookFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), hypr_cloudsync::Error>> + Send + 'a>>;

pub trait CloudsyncSyncHook: Send + Sync + 'static {
    fn before_sync<'a>(&'a self, pool: &'a SqlitePool) -> CloudsyncHookFuture<'a>;
    fn after_sync<'a>(&'a self, pool: &'a SqlitePool) -> CloudsyncHookFuture<'a>;
}
