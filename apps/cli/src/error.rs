#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(String),
    #[error("Anarlog database not found at {0}; start Anarlog once or pass --db-path")]
    DatabaseNotFound(std::path::PathBuf),
    #[error("{action} failed: {reason}")]
    Operation {
        action: &'static str,
        reason: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn operation(action: &'static str, reason: impl Into<String>) -> Self {
        Self::Operation {
            action,
            reason: reason.into(),
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::NotFound(_) => 2,
            Self::DatabaseNotFound(_) => 3,
            Self::Operation { .. } => 1,
        }
    }
}
