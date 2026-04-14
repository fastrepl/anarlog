use std::sync::Arc;

use crate::QueryEventListener;
use hypr_db_live_query::QueryEventSink;

#[derive(Clone)]
pub(crate) struct ListenerSink {
    listener: Arc<dyn QueryEventListener>,
}

impl ListenerSink {
    pub(crate) fn new(listener: Arc<dyn QueryEventListener>) -> Self {
        Self { listener }
    }
}

impl QueryEventSink for ListenerSink {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        let rows_json = serde_json::to_string(&rows).map_err(|error| error.to_string())?;
        self.listener.on_result(rows_json);
        Ok(())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.listener.on_error(error);
        Ok(())
    }
}
