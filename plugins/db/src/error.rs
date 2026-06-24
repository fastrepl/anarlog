use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] hypr_db_core::DbOpenError),
    #[error(transparent)]
    Migrate(#[from] hypr_db_migrate::MigrateError),
    #[error(transparent)]
    AppSchema(#[from] hypr_db_app::AppSchemaError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Execute(#[from] hypr_db_execute::Error),
    #[error(transparent)]
    Reactive(#[from] hypr_db_reactive::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
