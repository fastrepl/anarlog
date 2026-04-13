#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),
    #[error("failed to send query event: {0}")]
    Sink(String),
}

pub type Result<T> = std::result::Result<T, Error>;
