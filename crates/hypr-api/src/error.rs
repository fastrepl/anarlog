#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    DbOpen(#[from] hypr_db_core::DbOpenError),
    #[error(transparent)]
    Migrate(#[from] hypr_db_migrate::MigrateError),
    #[error(transparent)]
    Execute(#[from] hypr_db_execute::Error),
    #[error(transparent)]
    Reactive(#[from] hypr_db_reactive::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
