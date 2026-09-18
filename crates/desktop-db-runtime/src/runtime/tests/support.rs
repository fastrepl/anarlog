use std::sync::{Arc, Mutex};

use serde_json::json;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

use crate::CloudsyncE2eeWitness;

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

pub(crate) async fn setup_witness(workspace_id: &str) -> (MockServer, CloudsyncE2eeWitness) {
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

pub(crate) async fn setup_witnesses(workspace_ids: &[&str]) -> (MockServer, CloudsyncE2eeWitness) {
    let server = MockServer::start().await;
    for workspace_id in workspace_ids {
        Mock::given(path(format!("/sync/e2ee/witness/{workspace_id}")))
            .respond_with(WitnessResponder::default())
            .mount(&server)
            .await;
    }
    let config = CloudsyncE2eeWitness {
        endpoint: format!("{}/sync/e2ee/witness/{}", server.uri(), workspace_ids[0]),
        access_token: "access-token".to_string(),
    };
    (server, config)
}

pub(crate) fn unreachable_witness(workspace_id: &str) -> CloudsyncE2eeWitness {
    CloudsyncE2eeWitness {
        endpoint: format!("http://127.0.0.1:9/sync/e2ee/witness/{workspace_id}"),
        access_token: "access-token".to_string(),
    }
}
