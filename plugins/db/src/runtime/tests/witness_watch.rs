use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

async fn wait_requests(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .expect("failed to inspect witness requests")
        .into_iter()
        .filter(|request| request.url.path().ends_with("/wait"))
        .collect()
}

#[tokio::test]
async fn watcher_polls_the_witness_and_stops_when_it_is_cleared() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/sync/e2ee/witness/user-a/wait"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "headSequence": 1,
        })))
        .mount(&server)
        .await;
    let witness = crate::e2ee_witness::E2eeWitnessClient::new(
        crate::CloudsyncE2eeWitness {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    assert!(
        wait_requests(&server).await.is_empty(),
        "the watcher polled before a witness was configured"
    );
    runtime.e2ee_sync_hook.set_witness(witness);

    let requests = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let requests = wait_requests(&server).await;
            if requests.len() >= 2 {
                return requests;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the watcher did not start long-polling the witness");
    assert_eq!(
        requests[0].url.query(),
        Some("afterSequence=0"),
        "the first poll must start from the local witness cursor"
    );
    assert_eq!(
        requests[1].url.query(),
        Some("afterSequence=1"),
        "an advanced head must move the poll cursor forward"
    );

    runtime.e2ee_sync_hook.clear();
    tokio::time::sleep(std::time::Duration::from_millis(1_500)).await;
    let settled = wait_requests(&server).await.len();
    tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;
    assert_eq!(
        wait_requests(&server).await.len(),
        settled,
        "the watcher kept polling after the witness was cleared"
    );
}

#[tokio::test]
async fn watcher_restarts_the_cursor_when_the_witness_workspace_changes() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/sync/e2ee/witness/user-a/wait"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "headSequence": 7,
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/sync/e2ee/witness/user-b/wait"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "headSequence": 0,
        })))
        .mount(&server)
        .await;
    let witness_for = |workspace: &str| {
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/{workspace}", server.uri()),
                access_token: "access-token".to_string(),
            },
            workspace,
        )
        .unwrap()
    };

    runtime.e2ee_sync_hook.set_witness(witness_for("user-a"));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if wait_requests(&server)
                .await
                .iter()
                .any(|request| request.url.query() == Some("afterSequence=7"))
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the watcher never advanced past the first workspace's head");

    runtime.e2ee_sync_hook.set_witness(witness_for("user-b"));
    let first_swap_query = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let request = wait_requests(&server)
                .await
                .into_iter()
                .find(|request| request.url.path().contains("user-b"));
            if let Some(request) = request {
                return request.url.query().map(str::to_string);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the watcher never polled the swapped witness");
    assert_eq!(
        first_swap_query.as_deref(),
        Some("afterSequence=0"),
        "a swapped witness must not inherit the previous workspace's trigger cursor"
    );
}

#[tokio::test]
async fn watcher_polls_every_configured_workspace_witness() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));

    let server = MockServer::start().await;
    for workspace in ["personal", "shared"] {
        Mock::given(method("GET"))
            .and(wiremock::matchers::path(format!(
                "/sync/e2ee/witness/{workspace}/wait"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "initialized": true,
                "headSequence": 0,
            })))
            .mount(&server)
            .await;
    }
    let personal = crate::e2ee_witness::E2eeWitnessClient::new(
        crate::CloudsyncE2eeWitness {
            endpoint: format!("{}/sync/e2ee/witness/personal", server.uri()),
            access_token: "access-token".to_string(),
        },
        "personal",
    )
    .unwrap();
    let shared = personal.for_workspace("shared").unwrap();

    runtime.e2ee_sync_hook.set_witnesses(HashMap::from([
        ("personal".to_string(), personal),
        ("shared".to_string(), shared),
    ]));

    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let paths = wait_requests(&server)
                .await
                .into_iter()
                .map(|request| request.url.path().to_string())
                .collect::<std::collections::HashSet<_>>();
            if paths.contains("/sync/e2ee/witness/personal/wait")
                && paths.contains("/sync/e2ee/witness/shared/wait")
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the watcher did not poll every configured workspace");
}
