mod commands;
mod error;
mod runtime;

pub use error::{Error, Result};
use tauri::Manager;

const PLUGIN_NAME: &str = "db";

pub type ManagedState = std::sync::Arc<runtime::PluginDbRuntime>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq)]
#[serde(tag = "event", content = "data")]
pub enum QueryEvent {
    #[serde(rename = "result")]
    Result(Vec<serde_json::Value>),
    #[serde(rename = "error")]
    Error(String),
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::execute,
            commands::subscribe,
            commands::unsubscribe,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _| {
            let db = runtime::open_app_db(app)?;
            app.manage(std::sync::Arc::new(runtime::PluginDbRuntime::new(db)));
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use hypr_db_live_query::QueryEventSink;
    use serde_json::json;
    use tauri::ipc::{Channel, InvokeResponseBody};

    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn capture_channel() -> (Channel<QueryEvent>, Arc<Mutex<Vec<QueryEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(payload) = body else {
                return Ok(());
            };
            let event: QueryEvent =
                serde_json::from_str(&payload).expect("channel payload should parse");
            captured.lock().unwrap().push(event);
            Ok(())
        });
        (channel, events)
    }

    async fn next_event(
        events: &Arc<Mutex<Vec<QueryEvent>>>,
        index: usize,
    ) -> anyhow::Result<QueryEvent> {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(event) = events.lock().unwrap().get(index).cloned() {
                    return event;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(anyhow::Error::from)
    }

    async fn setup_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = hypr_db_core2::Db3::open_with_migrate(
            hypr_db_core2::DbOpenOptions {
                storage: hypr_db_core2::DbStorage::Local(&db_path),
                cloudsync: false,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(4),
                migration_failure_policy: hypr_db_core2::MigrationFailurePolicy::Fail,
            },
            |pool| Box::pin(hypr_db_app::migrate(pool)),
        )
        .await
        .unwrap();

        (dir, Arc::new(runtime::PluginDbRuntime::new(db)))
    }

    #[tokio::test]
    async fn query_event_channel_sends_result_payload() {
        let (channel, events) = capture_channel();
        let sink = runtime::QueryEventChannel::new(channel);

        sink.send_result(vec![json!({ "id": "note-1" })]).unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Result(vec![json!({ "id": "note-1" })]));
    }

    #[tokio::test]
    async fn query_event_channel_sends_error_payload() {
        let (channel, events) = capture_channel();
        let sink = runtime::QueryEventChannel::new(channel);

        sink.send_error("boom".to_string()).unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Error("boom".to_string()));
    }

    #[test]
    fn query_event_serializes_with_tagged_shape() {
        let result =
            serde_json::to_value(QueryEvent::Result(vec![json!({ "id": "note-1" })])).unwrap();
        let error = serde_json::to_value(QueryEvent::Error("boom".to_string())).unwrap();

        assert_eq!(
            result,
            json!({ "event": "result", "data": [{ "id": "note-1" }] })
        );
        assert_eq!(error, json!({ "event": "error", "data": "boom" }));
    }

    #[tokio::test]
    async fn subscribe_sends_initial_result_through_channel() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        runtime
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert_eq!(event, QueryEvent::Result(Vec::new()));
    }

    #[tokio::test]
    async fn invalid_sql_sends_error_through_channel() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        runtime
            .subscribe(
                "SELECT * FROM missing_table".to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let event = next_event(&events, 0).await.unwrap();
        assert!(matches!(event, QueryEvent::Error(_)));
    }
}
