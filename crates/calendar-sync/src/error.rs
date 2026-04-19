use thiserror::Error;

#[derive(Debug, Clone, Copy, Error)]
#[error("calendar sync worker is not accepting requests")]
pub struct RequestSyncError;
