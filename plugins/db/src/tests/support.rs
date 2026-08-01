use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tauri::ipc::{Channel, InvokeResponseBody};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

use crate::{CloudsyncE2eeWitness, QueryEvent, runtime};

pub(super) fn capture_channel() -> (Channel<QueryEvent>, Arc<Mutex<Vec<QueryEvent>>>) {
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

pub(super) async fn next_event(
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

async fn setup_runtime_with_cloudsync(
    cloudsync_enabled: bool,
) -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(&db_path),
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "UPDATE storage_migration_state
         SET importer_version = ?, parity_verified = 1
         WHERE id = 'legacy_v1'",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .execute(db.pool())
    .await
    .unwrap();

    (dir, Arc::new(runtime::PluginDbRuntime::new(Arc::new(db))))
}

pub(super) async fn setup_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
    setup_runtime_with_cloudsync(false).await
}

pub(super) async fn setup_enabled_cloudsync_runtime()
-> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
    let (dir, runtime) = setup_runtime_with_cloudsync(true).await;
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("user-a", &recovery_key)
        .unwrap();
    (dir, runtime)
}

#[derive(Clone, Default)]
struct WitnessResponder {
    events: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl Respond for WitnessResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.method == wiremock::http::Method::POST {
            let body: serde_json::Value = request.body_json().unwrap();
            let mut events = self.events.lock().unwrap();
            for event in body["events"].as_array().unwrap() {
                let duplicate = events.iter().any(|existing| {
                    existing["recordId"] == event["recordId"]
                        && existing["payloadHash"] == event["payloadHash"]
                });
                if !duplicate {
                    let mut event = event.clone();
                    event["sequence"] = json!(events.len() + 1);
                    events.push(event);
                }
            }
            return ResponseTemplate::new(200).set_body_json(json!({
                "initializedAt": "2026-07-17T00:00:00Z",
                "headSequence": events.len(),
            }));
        }

        let query = request
            .url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        let after = query
            .get("afterSequence")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let events = self.events.lock().unwrap();
        let head = u64::try_from(events.len()).unwrap();
        let through = query
            .get("throughSequence")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(head);
        let page = events
            .iter()
            .filter(|event| {
                event["sequence"].as_u64().unwrap() > after
                    && event["sequence"].as_u64().unwrap() <= through
            })
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        let next = page
            .last()
            .and_then(|event| event["sequence"].as_u64())
            .unwrap_or(after);
        ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-07-17T00:00:00Z",
            "headSequence": head,
            "throughSequence": through,
            "nextAfterSequence": next,
            "events": page,
        }))
    }
}

pub(super) async fn setup_witness(workspace_id: &str) -> (MockServer, CloudsyncE2eeWitness) {
    let server = MockServer::start().await;
    Mock::given(path(format!("/sync/e2ee/witness/{workspace_id}")))
        .respond_with(WitnessResponder::default())
        .mount(&server)
        .await;
    let config = CloudsyncE2eeWitness {
        endpoint: format!("{}/sync/e2ee/witness/{workspace_id}", server.uri()),
        access_token: "access-token".to_string(),
    };
    (server, config)
}

pub(super) fn unreachable_witness(workspace_id: &str) -> CloudsyncE2eeWitness {
    CloudsyncE2eeWitness {
        endpoint: format!("http://127.0.0.1:9/sync/e2ee/witness/{workspace_id}"),
        access_token: "access-token".to_string(),
    }
}

pub(super) async fn setup_unmigrated_runtime_with_max_connections(
    max_connections: u32,
) -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(&db_path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(max_connections),
    })
    .await
    .unwrap();

    (dir, Arc::new(runtime::PluginDbRuntime::new(Arc::new(db))))
}

pub(super) async fn setup_unmigrated_runtime() -> (tempfile::TempDir, Arc<runtime::PluginDbRuntime>)
{
    setup_unmigrated_runtime_with_max_connections(4).await
}
