use std::sync::Arc;

use hypr_db_reactive::{
    DependencyAnalysis, LiveQueryRuntime, QueryEventSink, SubscriptionRegistration,
};
use tokio::sync::mpsc;

use crate::Result;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct LiveQueryResult {
    pub rows: Vec<serde_json::Value>,
    pub reactive: bool,
}

#[derive(Clone)]
pub struct JsonRowsSink {
    tx: mpsc::UnboundedSender<std::result::Result<Vec<serde_json::Value>, String>>,
}

impl JsonRowsSink {
    pub fn new(
        tx: mpsc::UnboundedSender<std::result::Result<Vec<serde_json::Value>, String>>,
    ) -> Self {
        Self { tx }
    }
}

impl QueryEventSink for JsonRowsSink {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.tx.send(Ok(rows)).map_err(|error| error.to_string())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.tx
            .send(Err(error.clone()))
            .map_err(|send_error| send_error.to_string())
    }
}

pub struct LiveQueryWatch {
    runtime: Arc<LiveQueryRuntime<JsonRowsSink>>,
    registration: SubscriptionRegistration,
    reactive: bool,
    rx: mpsc::UnboundedReceiver<std::result::Result<Vec<serde_json::Value>, String>>,
}

impl LiveQueryWatch {
    pub fn id(&self) -> &str {
        &self.registration.id
    }

    pub fn reactive(&self) -> bool {
        self.reactive
    }

    pub async fn next(&mut self) -> Option<std::result::Result<LiveQueryResult, String>> {
        self.rx.recv().await.map(|next| {
            next.map(|rows| LiveQueryResult {
                rows,
                reactive: self.reactive,
            })
        })
    }

    pub async fn close(&self) -> Result<()> {
        self.runtime.unsubscribe(self.id()).await?;
        Ok(())
    }
}

impl Drop for LiveQueryWatch {
    fn drop(&mut self) {
        let runtime = Arc::clone(&self.runtime);
        let id = self.registration.id.clone();
        tokio::spawn(async move {
            let _ = runtime.unsubscribe(&id).await;
        });
    }
}

pub async fn subscribe(
    state: &AppState,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<LiveQueryWatch> {
    let (tx, rx) = mpsc::unbounded_channel();
    let registration = state
        .live_query_runtime()
        .subscribe(sql, params, JsonRowsSink::new(tx))
        .await?;
    let reactive = matches!(registration.analysis, DependencyAnalysis::Reactive { .. });

    Ok(LiveQueryWatch {
        runtime: Arc::clone(state.live_query_runtime()),
        registration,
        reactive,
        rx,
    })
}
