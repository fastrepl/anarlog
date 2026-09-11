use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] anlg_db_core::DbOpenError),
    #[error(transparent)]
    Migrate(#[from] anlg_db_migrate::MigrateError),
    #[error(transparent)]
    AppSchema(#[from] anlg_db_app::AppSchemaError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Execute(#[from] anlg_db_execute::Error),
    #[error(transparent)]
    Reactive(#[from] anlg_db_reactive::Error),
    #[error(transparent)]
    Cloudsync(#[from] anlg_db_core::CloudsyncRuntimeError),
    #[error(transparent)]
    CloudsyncWorkspace(#[from] anlg_db_app::CloudsyncWorkspaceError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("end-to-end encryption recovery key setup is required before CloudSync can start")]
    E2eeIdentityRequired,
    #[error("cloudsync_activity_deferred")]
    CloudsyncActivityDeferred,
    #[error("cloudsync_configuration_cancelled")]
    CloudsyncConfigurationCancelled,
    #[error("cloudsync_activity_drain_timeout")]
    CloudsyncActivityDrainTimeout,
    #[error("transaction statement {statement_index} affected {actual} rows; expected {expected}")]
    UnexpectedRowsAffected {
        statement_index: usize,
        expected: u64,
        actual: u64,
    },
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
