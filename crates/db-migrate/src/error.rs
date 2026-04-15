#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Cloudsync(#[from] hypr_db_core2::Error),
    #[error("migration step {step_id} checksum changed after it was applied")]
    StepChecksumMismatch { step_id: &'static str },
    #[error("cloudsync alter step {step_id} targets non-synced table {table_name}")]
    InvalidCloudsyncStep {
        step_id: &'static str,
        table_name: &'static str,
    },
}
