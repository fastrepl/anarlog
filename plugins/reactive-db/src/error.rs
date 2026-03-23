#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not initialized — call init() first")]
    NotInitialized,

    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
