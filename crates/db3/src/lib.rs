#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

pub use hypr_cloudsync::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub struct Db3 {
    cloudsync_path: Option<PathBuf>,
    pool: SqlitePool,
}

impl Db3 {
    pub async fn connect_local(path: impl AsRef<Path>) -> Result<Self, Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_memory() -> Result<Self, Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_local_plain(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&Path> {
        self.cloudsync_path.as_deref()
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn cloudsync_version(&self) -> Result<String, Error> {
        hypr_cloudsync::version(&self.pool).await
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        force: Option<bool>,
    ) -> Result<(), Error> {
        hypr_cloudsync::init(&self.pool, table_name, crdt_algo, force).await
    }

    pub async fn cloudsync_network_init(&self, connection_string: &str) -> Result<(), Error> {
        hypr_cloudsync::network_init(&self.pool, connection_string).await
    }

    pub async fn cloudsync_network_set_apikey(&self, api_key: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_apikey(&self.pool, api_key).await
    }

    pub async fn cloudsync_network_set_token(&self, token: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_token(&self.pool, token).await
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<(), Error> {
        hypr_cloudsync::network_sync(&self.pool, wait_ms, max_retries).await
    }
}
