use anlg_desktop_db_runtime::QueryEvent;
use tauri::ipc::Channel;

pub use anlg_desktop_db_runtime::runtime::{
    CloudsyncTokenConfiguration, E2eeWorkspaceKeyConfiguration, open_app_db, open_app_db_unmigrated,
};
pub use anlg_desktop_db_runtime::{DesktopDbRuntime, QueryEventSink};

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

pub type PluginDbRuntime = DesktopDbRuntime<QueryEventChannel>;
