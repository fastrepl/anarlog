#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{action} failed: {reason}")]
    OperationFailed {
        action: &'static str,
        reason: String,
    },
    #[error("{what} not found")]
    NotFound { what: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn operation_failed(action: &'static str, reason: impl Into<String>) -> Self {
        Self::OperationFailed {
            action,
            reason: reason.into(),
        }
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }
}
