use hypr_db_core2::{Db3, DbOpenOptions, DbStorage, MigrationFailurePolicy};
use hypr_db_live_query::QueryEventSink;
use tauri::Manager;
use tauri::ipc::Channel;

use crate::{Error, QueryEvent, Result};

#[derive(Clone)]
pub struct QueryEventChannel(Channel<QueryEvent>);

impl QueryEventChannel {
    pub fn new(channel: Channel<QueryEvent>) -> Self {
        Self(channel)
    }
}

impl QueryEventSink for QueryEventChannel {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Result(rows))
            .map_err(|error| error.to_string())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Error(error))
            .map_err(|error| error.to_string())
    }
}

pub type PluginDbRuntime = hypr_db_live_query::DbRuntime<QueryEventChannel>;

pub fn open_app_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Db3> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))?
        .join("app.db");

    let db = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create db init runtime");
        runtime.block_on(Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync: false,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |pool| Box::pin(hypr_db_app::migrate(pool)),
        ))
    })
    .join()
    .expect("db init thread panicked")?;

    Ok(db)
}
