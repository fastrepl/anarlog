#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("invalid query method: {0}")]
    InvalidQueryMethod(String),
}

pub type Result<T> = std::result::Result<T, Error>;
