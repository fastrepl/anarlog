#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    SqlxMigrate(#[from] sqlx::migrate::MigrateError),
    #[error(
        "migration step id {step_id} must match <VERSION>_<DESCRIPTION> with a positive integer version"
    )]
    InvalidStepId { step_id: &'static str },
    #[error("migration version {version} is declared by both {first_step_id} and {second_step_id}")]
    DuplicateStepVersion {
        version: i64,
        first_step_id: &'static str,
        second_step_id: &'static str,
    },
    #[error("cloudsync alter step {step_id} targets non-synced table {table_name}")]
    InvalidCloudsyncStep {
        step_id: &'static str,
        table_name: &'static str,
    },
}
