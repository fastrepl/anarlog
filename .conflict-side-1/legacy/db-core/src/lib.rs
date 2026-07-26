use std::sync::Arc;

mod errors;
pub use errors::*;

pub use libsql;

pub const MIGRATION_TABLE_SQL: &str = include_str!("./migration.sql");

#[derive(Clone)]
pub enum Database {
    StaticConnection(libsql::Connection),
    DynamicConnection(Arc<libsql::Database>),
}

impl Database {
    pub fn conn(&self) -> Result<libsql::Connection, crate::Error> {
        match self {
            Database::StaticConnection(conn) => Ok(conn.clone()),
            Database::DynamicConnection(db) => db.connect().map_err(Into::into),
        }
    }

    pub async fn sync(&self) -> Result<(), crate::Error> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct DatabaseConfig {
    memory: Option<bool>,
    local_path: Option<std::path::PathBuf>,
    remote_config: Option<(String, String)>,
}

#[derive(Default)]
pub struct DatabaseBuilder {
    config: DatabaseConfig,
}

impl DatabaseBuilder {
    pub fn memory(mut self) -> Self {
        self.config.memory = Some(true);
        self
    }

    pub fn local(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.config.local_path = Some(path.as_ref().to_owned());
        self
    }

    pub fn remote(mut self, url: impl Into<String>, token: impl Into<String>) -> Self {
        self.config.remote_config = Some((url.into(), token.into()));
        self
    }

    pub async fn build(self) -> Result<Database, crate::Error> {
        let db = match (
            self.config.memory,
            self.config.local_path,
            self.config.remote_config,
        ) {
            (Some(true), _, _) => {
                let db = libsql::Builder::new_local(":memory:").build().await?;
                let conn = db.connect()?;
                Database::StaticConnection(conn)
            }
            (_, Some(path), None) => {
                let db = libsql::Builder::new_local(path).build().await?;
                let conn = db.connect()?;
                Database::StaticConnection(conn)
            }
            (_, None, Some((url, token))) => {
                let db = libsql::Builder::new_remote(url, token).build().await?;
                Database::DynamicConnection(Arc::new(db))
            }
            (_, Some(path), Some((url, token))) => {
                let db = libsql::Builder::new_remote_replica(path, url, token)
                    .read_your_writes(true)
                    .sync_interval(std::time::Duration::from_secs(300))
                    .build()
                    .await?;
                Database::DynamicConnection(Arc::new(db))
            }
            (_, None, None) => Err(crate::Error::InvalidDatabaseConfig(
                "either '.memory()' or '.local()' or '.remote()' must be called".to_string(),
            ))?,
        };

        Ok(db)
    }
}

pub enum TrackingSource {
    Pragma,
    Table,
}

// TODO:
// Turso on AWS does not support user_version PRAGMA. So this solution works for local-only (current status of desktop app) & cloud-only (admin db),
// but will not work once we start syncing local & cloud. At that point, we should do table-based tracking even though pragma is supported.
impl TrackingSource {
    pub async fn new(conn: &libsql::Connection) -> Result<Self, crate::Error> {
        let result = match conn.query("PRAGMA user_version", ()).await {
            Ok(v) => Ok::<Option<libsql::Rows>, crate::Error>(Some(v)),
            Err(libsql::Error::Hrana(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }?;

        match result {
            None => Ok(Self::Table),
            Some(_) => Ok(Self::Pragma),
        }
    }

    pub async fn get(&self, conn: &libsql::Connection) -> Result<i32, crate::Error> {
        match self {
            TrackingSource::Pragma => {
                let version: i32 = conn
                    .query("PRAGMA user_version", ())
                    .await?
                    .next()
                    .await?
                    .unwrap()
                    .get(0)
                    .unwrap_or(0);

                Ok(version)
            }
            TrackingSource::Table => {
                let mut result = conn
                    .query("SELECT MAX(version) FROM _migrations", ())
                    .await?;

                let row = result.next().await?;
                let version: Option<i32> = if let Some(row) = row {
                    row.get(0)?
                } else {
                    None
                };

                Ok(version.unwrap_or(0))
            }
        }
    }

    pub async fn set(&self, tx: &libsql::Transaction, version: i32) -> Result<(), crate::Error> {
        match self {
            TrackingSource::Pragma => {
                tx.execute(&format!("PRAGMA user_version = {}", version), ())
                    .await?;

                Ok(())
            }
            TrackingSource::Table => {
                tx.execute(
                    "INSERT INTO _migrations (version) VALUES (?)",
                    vec![version],
                )
                .await?;

                Ok(())
            }
        }
    }
}

pub async fn migrate(
    conn: &libsql::Connection,
    migrations: Vec<impl AsRef<str>>,
) -> Result<(), crate::Error> {
    let tracking = TrackingSource::new(conn).await?;

    if matches!(tracking, TrackingSource::Table) {
        conn.execute(MIGRATION_TABLE_SQL, ()).await?;
    }

    let current_version: i32 = tracking.get(conn).await?;
    let latest_version = migrations.len() as i32;

    if current_version < latest_version {
        let tx = conn.transaction().await?;

        for migration in migrations.iter().skip(current_version as usize) {
            tx.execute(migration.as_ref(), ()).await?;
        }

        tracking.set(&tx, latest_version).await?;
        tx.commit().await?;
    }

    Ok(())
}

pub trait SqlTable {
    fn sql_table() -> &'static str;
}
