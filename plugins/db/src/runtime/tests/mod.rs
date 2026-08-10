use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

#[derive(Clone, Default)]
struct InitiallyUninitializedWitness {
    initialized: std::sync::Arc<AtomicBool>,
}

impl Respond for InitiallyUninitializedWitness {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.method == wiremock::http::Method::POST {
            let body: serde_json::Value = request.body_json().unwrap();
            if !self.initialized.load(Ordering::SeqCst)
                && body["initialize"].as_bool() != Some(true)
            {
                return ResponseTemplate::new(409);
            }
            if body["events"].as_array().is_none_or(Vec::is_empty) {
                return ResponseTemplate::new(400);
            }
            self.initialized.store(true, Ordering::SeqCst);
            return ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "initializedAt": "2026-07-23T00:00:00Z",
                "headSequence": 0,
            }));
        }

        let initialized = self.initialized.load(Ordering::SeqCst);
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "initialized": initialized,
            "initializedAt": initialized.then_some("2026-07-23T00:00:00Z"),
            "headSequence": 0,
            "throughSequence": 0,
            "nextAfterSequence": 0,
            "events": [],
        }))
    }
}

async fn configured_test_hook() -> (E2eeSyncHook, MockServer) {
    let hook = E2eeSyncHook::default();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    hook.set_personal_workspace("workspace-1", &recovery_key)
        .unwrap();
    let witness_state = InitiallyUninitializedWitness::default();
    witness_state.initialized.store(true, Ordering::SeqCst);
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(witness_state)
        .mount(&witness_server)
        .await;
    hook.set_witness(
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap(),
    );
    (hook, witness_server)
}

fn receive_result(chunks: u64, complete: bool) -> anlg_db_core::CloudsyncNetworkResult {
    serde_json::from_value(serde_json::json!({
        "receive": {
            "rows": chunks,
            "tables": [],
            "chunks": chunks,
            "bytes": chunks,
            "complete": complete
        }
    }))
    .unwrap()
}

mod activity;
mod configuration;
mod open_and_status;
mod recovery_protocol;
mod recovery_schedule;
mod sync_hook;
mod witness_watch;
