#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("invalid query method: {0}")]
    InvalidQueryMethod(String),
    #[error("transaction statement {statement_index} affected {actual} rows; expected {expected}")]
    UnexpectedRowsAffected {
        statement_index: usize,
        expected: u64,
        actual: u64,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
