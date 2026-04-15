use hypr_db_core2::DbOpenError;

#[derive(Debug, thiserror::Error)]
pub enum AppDbOpenError {
    #[error(transparent)]
    Open(#[from] DbOpenError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Cloudsync(#[from] hypr_db_core2::Error),
    #[error("cloudsync-enabled databases cannot recreate storage after migration failure")]
    RecreateNotAllowedWithCloudsync,
    #[error("migration step {step_id} checksum changed after it was applied")]
    StepChecksumMismatch { step_id: &'static str },
    #[error("cloudsync alter step {step_id} targets non-synced table {table_name}")]
    InvalidCloudsyncStep {
        step_id: &'static str,
        table_name: &'static str,
    },
}
