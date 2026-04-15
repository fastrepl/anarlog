use std::path::Path;

use hypr_db_core2::{Db3, DbOpenOptions, DbStorage};
use hypr_db_live_query::QueryEventSink;
use tauri::ipc::Channel;

use crate::{QueryEvent, Result};

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

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db3> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };

    let db = Db3::open(DbOpenOptions {
        storage,
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_migrate::migrate(&db, hypr_db_app::schema()).await?;

    Ok(db)
}
