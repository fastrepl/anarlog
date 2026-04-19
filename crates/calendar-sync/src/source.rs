use std::future::Future;
use std::pin::Pin;

use crate::runtime::SyncReason;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncOutcome {
    pub data_changed: bool,
}

pub trait CalendarSyncSource: Send + Sync + 'static {
    fn sync(
        &self,
        reasons: Vec<SyncReason>,
    ) -> Pin<Box<dyn Future<Output = Result<SyncOutcome, BoxError>> + Send + '_>>;
}
