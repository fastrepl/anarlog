use anlg_api_auth::{AuthContext, Claims};
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, header as request_header, method, path},
};

use super::*;
use crate::SyncConfig;

const OWNER: &str = "11111111-1111-4111-8111-111111111111";
const OBJECT_ID: &str = "22222222-2222-4222-8222-222222222222";
const OBJECT_UUID: &str = "33333333-3333-4333-8333-333333333333";
const DELETE_REQUEST_ID: &str = "44444444-4444-4444-8444-444444444444";
const DISPLACED_UUID: &str = "55555555-5555-4555-8555-555555555555";
const DELETE_FENCE_ID: &str = "66666666-6666-4666-8666-666666666666";
const ATTACHMENT_REF: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const VERSION_REF: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
const CIPHERTEXT_SHA256: &str = "ad47fd9e87159d651a53b3dfba3ef200684a9ed88c2528b62e18f3881fe203b0";

fn object_key() -> String {
    format!("{OWNER}/{OBJECT_UUID}.anb1")
}

fn displaced_key() -> String {
    format!("{OWNER}/{DISPLACED_UUID}.anb1")
}

fn timestamp_after(seconds: i64) -> String {
    (Utc::now() + TimeDelta::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn test_state(server: &MockServer) -> AppState {
    AppState::new(SyncConfig {
        project_url: server.uri(),
        token_issuer_api_key: "issuer-key".to_string(),
        database_id: "database-id".to_string(),
        legacy_database_id: None,
        protocol_mode: crate::config::CloudsyncProtocolMode::E2eeEnforced,
        token_ttl_seconds: 60,
        supabase_url: server.uri(),
        supabase_anon_key: "anon-key".to_string(),
        supabase_service_role_key: "service-role-key".to_string(),
    })
}

fn test_router_with_state(state: AppState, is_pro: bool) -> axum::Router {
    router().with_state(state).layer(Extension(AuthContext {
        token: "user-token".to_string(),
        claims: Claims {
            sub: OWNER.to_string(),
            email: None,
            entitlements: is_pro
                .then(|| "hyprnote_pro".to_string())
                .into_iter()
                .collect(),
            subscription_status: None,
            trial_end: None,
            has_payment_method: None,
        },
    }))
}

fn test_router(server: &MockServer, is_pro: bool) -> axum::Router {
    test_router_with_state(test_state(server), is_pro)
}

fn json_request(method: Method, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn reserved_row(state: &str, hash: Option<&str>) -> Value {
    json!({
        "object_id": OBJECT_ID,
        "object_key": object_key(),
        "object_state": state,
        "ciphertext_sha256": hash,
        "ciphertext_size_bytes": 1234,
        "format_version": 1,
        "reservation_expires_at": timestamp_after(15 * 60),
        "cleanup_not_before": timestamp_after(30 * 60),
        "was_created": true
    })
}

fn object_row(state: &str, hash: Option<&str>) -> Value {
    json!({
        "object_id": OBJECT_ID,
        "attachment_ref": ATTACHMENT_REF,
        "version_ref": VERSION_REF,
        "object_key": object_key(),
        "object_state": state,
        "ciphertext_sha256": hash,
        "ciphertext_size_bytes": 1234,
        "format_version": 1,
        "reservation_expires_at": timestamp_after(15 * 60),
        "upload_expires_at": hash.map(|_| timestamp_after(2 * 60 * 60)),
        "cleanup_not_before": timestamp_after(26 * 60 * 60)
    })
}

fn delete_request_body() -> Value {
    json!({
        "objectKey": object_key(),
        "attachmentRef": ATTACHMENT_REF,
        "versionRef": VERSION_REF,
        "deleteRequestId": DELETE_REQUEST_ID
    })
}

fn scheduled_deletion_row(delete_not_before: &str, was_created: bool) -> Value {
    json!({
        "outcome": "scheduled",
        "object_id": OBJECT_ID,
        "object_key": object_key(),
        "delete_request_id": DELETE_REQUEST_ID,
        "delete_fence_id": DELETE_FENCE_ID,
        "delete_generation": 7,
        "delete_not_before": delete_not_before,
        "was_created": was_created
    })
}

async fn mount_rpc(server: &MockServer, function: &str, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(format!("/rest/v1/rpc/{function}")))
        .and(request_header("apikey", "service-role-key"))
        .and(request_header("authorization", "Bearer service-role-key"))
        .respond_with(response)
        .mount(server)
        .await;
}

mod deletion;
mod finalize;
mod head_download;
mod protocol;
mod reservation;
mod upload;
