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
    async fn subscribe_sends_initial_result() {
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
    async fn dependent_writes_trigger_refresh() {
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

        let _ = next_event(&events, 0).await.unwrap();

        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind("note-1")
            .bind("2026-04-13")
            .bind("{}")
            .bind("user-1")
            .execute(runtime.pool())
            .await
            .unwrap();

        let event = next_event(&events, 1).await.unwrap();
        let QueryEvent::Result(rows) = event else {
            panic!("expected result event");
        };
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn unrelated_writes_do_not_trigger_refresh() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind("note-seed")
            .bind("2026-04-12")
            .bind("{}")
            .bind("user-1")
            .execute(runtime.pool())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        runtime
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let _ = next_event(&events, 0).await.unwrap();

        sqlx::query(
            "INSERT INTO daily_summaries (id, daily_note_id, date, content, timeline_json, topics_json, status, source_cursor_ms, source_fingerprint, generation_error, generated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("summary-1")
        .bind("note-seed")
        .bind("2026-04-12")
        .bind("{}")
        .bind("[]")
        .bind("[]")
        .bind("ready")
        .bind(0_i64)
        .bind("")
        .bind("")
        .bind("2026-04-12T00:00:00Z")
        .execute(runtime.pool())
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unsubscribe_stops_future_events() {
        let (_dir, runtime) = setup_runtime().await;
        let (channel, events) = capture_channel();

        let registration = runtime
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                vec![],
                runtime::QueryEventChannel::new(channel),
            )
            .await
            .unwrap();

        let _ = next_event(&events, 0).await.unwrap();
        runtime.unsubscribe(&registration.id).await.unwrap();

        sqlx::query("INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)")
            .bind("note-2")
            .bind("2026-04-14")
            .bind("{}")
            .bind("user-1")
            .execute(runtime.pool())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn invalid_sql_sends_error_event() {
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
