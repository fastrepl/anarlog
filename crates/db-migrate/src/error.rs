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
    // The desktop startup dialog classifies this failure by matching
    // "created by a newer version of Anarlog" in the rendered message.
    #[error(
        "the database was created by a newer version of Anarlog: it requires migration {min_supported_version}, but this build only includes migrations up to {max_known_version}"
    )]
    SchemaFromNewerApp {
        min_supported_version: i64,
        max_known_version: i64,
    },
    // The desktop startup dialog classifies this failure by matching
    // "created by a newer version of Anarlog" in the rendered message.
    #[error(
        "migration {missing_version} cannot be applied because the database was created by a newer version of Anarlog (max applied migration {max_applied_version})"
    )]
    DatabaseAhead {
        missing_version: i64,
        max_applied_version: i64,
    },
}
