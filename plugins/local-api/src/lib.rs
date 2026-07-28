mod commands;
mod dispatch;
mod server;
mod settings;
mod types;

pub use dispatch::{EVENT_MEETING_COMPLETED, EVENT_NOTE_ENHANCED, KNOWN_EVENTS};
pub use server::ServerHandle;
pub use settings::{DEFAULT_PORT, LocalApiSettings};
pub use types::*;

use tauri::Manager;

const PLUGIN_NAME: &str = "local-api";

#[derive(Default)]
pub struct PluginState {
    server: tokio::sync::Mutex<Option<ServerHandle>>,
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![])
        .commands(tauri_specta::collect_commands![
            commands::get_status::<tauri::Wry>,
            commands::set_enabled::<tauri::Wry>,
            commands::list_api_keys::<tauri::Wry>,
            commands::create_api_key::<tauri::Wry>,
            commands::revoke_api_key::<tauri::Wry>,
            commands::list_webhooks::<tauri::Wry>,
            commands::create_webhook::<tauri::Wry>,
            commands::delete_webhook::<tauri::Wry>,
            commands::test_webhook::<tauri::Wry>,
            commands::dispatch_event::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            app.manage(PluginState::default());

            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                autostart(app).await;
            });

            Ok(())
        })
        .build()
}

/// Waits for the db plugin to come up, then starts the server if the user
/// enabled it in a previous run.
async fn autostart(app: tauri::AppHandle<tauri::Wry>) {
    let mut db = None;
    for _ in 0..150 {
        if let Some(state) = app.try_state::<tauri_plugin_db::ManagedState>() {
            db = Some(state.inner().clone());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let Some(db) = db else {
        tracing::warn!("[local-api] database never became ready; server not started");
        return;
    };

    let state = app.state::<PluginState>();
    let mut server = state.server.lock().await;
    let pool = db.pool().clone();
    let loaded = match settings::load(&pool).await {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!("[local-api] could not load settings: {error}");
            return;
        }
    };
    if !loaded.enabled || server.is_some() {
        return;
    }

    match server::start(pool.clone(), loaded.port).await {
        Ok(handle) => {
            *server = Some(handle);
        }
        Err(error) => {
            tracing::warn!("[local-api] autostart failed: {error}");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
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

    async fn seeded_pool() -> sqlx::SqlitePool {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at, series_id) \
             VALUES ('meeting-1', 'Planning', '2026-07-13', 'series-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, body_format, body, title) \
             VALUES ('note-1', 'meeting-1', 'note', 'markdown', 'Launch decision', 'Notes')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, started_at_ms, words_json) \
             VALUES ('transcript-1', 'meeting-1', 0, '[{\"text\":\"hello\"},{\"text\":\"world\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().clone()
    }

    async fn request(
        port: u16,
        method: reqwest::Method,
        path: &str,
        key: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> reqwest::Response {
        let client = reqwest::Client::new();
        let mut request = client.request(method, format!("http://127.0.0.1:{port}{path}"));
        if let Some(key) = key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        request.send().await.unwrap()
    }

    #[tokio::test]
    async fn server_serves_meetings_and_webhooks_behind_api_key_auth() {
        let pool = seeded_pool().await;
        let generated = hypr_db_app::generate_api_key();
        hypr_db_app::insert_api_key(
            &pool,
            "key-1",
            "test",
            &generated.key_hash,
            &generated.key_prefix,
        )
        .await
        .unwrap();

        let handle = server::start(pool.clone(), 0).await.unwrap();
        let port = handle.port;

        let health = request(port, reqwest::Method::GET, "/health", None, None).await;
        assert_eq!(health.status(), 200);

        let unauthorized = request(port, reqwest::Method::GET, "/v1/meetings", None, None).await;
        assert_eq!(unauthorized.status(), 401);

        let wrong_key = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings",
            Some("anl_wrong"),
            None,
        )
        .await;
        assert_eq!(wrong_key.status(), 401);

        let meetings = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings",
            Some(&generated.key),
            None,
        )
        .await;
        assert_eq!(meetings.status(), 200);
        let meetings: serde_json::Value = meetings.json().await.unwrap();
        assert_eq!(meetings["meetings"][0]["id"], "meeting-1");

        let meeting = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings/meeting-1",
            Some(&generated.key),
            None,
        )
        .await;
        let meeting: serde_json::Value = meeting.json().await.unwrap();
        assert_eq!(meeting["note"]["markdown"], "Launch decision");

        let transcript = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings/meeting-1/transcript?limit=1",
            Some(&generated.key),
            None,
        )
        .await;
        let transcript: serde_json::Value = transcript.json().await.unwrap();
        assert_eq!(transcript["text"], "hello");
        assert_eq!(transcript["pagination"]["total"], 2);

        let markdown = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings/meeting-1/export?format=markdown",
            Some(&generated.key),
            None,
        )
        .await;
        assert_eq!(markdown.status(), 200);
        let markdown = markdown.text().await.unwrap();
        assert!(markdown.contains("# Planning"));
        assert!(markdown.contains("hello world"));

        let missing = request(
            port,
            reqwest::Method::GET,
            "/v1/meetings/nope",
            Some(&generated.key),
            None,
        )
        .await;
        assert_eq!(missing.status(), 404);

        let created = request(
            port,
            reqwest::Method::POST,
            "/v1/webhooks",
            Some(&generated.key),
            Some(serde_json::json!({
                "url": "https://example.com/hook",
                "events": ["note.enhanced"],
            })),
        )
        .await;
        assert_eq!(created.status(), 201);
        let created: serde_json::Value = created.json().await.unwrap();
        assert!(created["secret"].as_str().unwrap().starts_with("whsec_"));
        let webhook_id = created["id"].as_str().unwrap().to_string();

        let bad_event = request(
            port,
            reqwest::Method::POST,
            "/v1/webhooks",
            Some(&generated.key),
            Some(serde_json::json!({
                "url": "https://example.com/hook",
                "events": ["nope"],
            })),
        )
        .await;
        assert_eq!(bad_event.status(), 400);

        let listed = request(
            port,
            reqwest::Method::GET,
            "/v1/webhooks",
            Some(&generated.key),
            None,
        )
        .await;
        let listed: serde_json::Value = listed.json().await.unwrap();
        assert_eq!(listed["webhooks"][0]["id"], webhook_id);
        assert!(listed["webhooks"][0].get("secret").is_none());

        let deleted = request(
            port,
            reqwest::Method::DELETE,
            &format!("/v1/webhooks/{webhook_id}"),
            Some(&generated.key),
            None,
        )
        .await;
        assert_eq!(deleted.status(), 204);

        let key_rows = hypr_db_app::list_api_keys(&pool).await.unwrap();
        assert!(key_rows[0].last_used_at.is_some());

        handle.shutdown().await;

        let rebound = server::start(pool, port).await.unwrap();
        rebound.shutdown().await;
    }
}
