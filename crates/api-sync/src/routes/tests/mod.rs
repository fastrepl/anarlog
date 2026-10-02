use anlg_api_auth::{AuthContext, Claims};
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{HeaderValue, Method, Request, StatusCode, header as http_header},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, header, method, path, query_param},
};

use super::*;
use crate::{
    SyncConfig, SyncError,
    config::{CloudsyncProtocolMode, CloudsyncTransport},
};

const TEST_KEY_ID: &str = "abcdefghijklmnopqrstuv";

fn test_router(server: &MockServer, api_key: &str, entitlements: &[&str]) -> Router {
    test_router_with_protocol(
        server,
        api_key,
        entitlements,
        CloudsyncProtocolMode::E2eeEnforced,
        None,
    )
}

fn test_router_with_protocol(
    server: &MockServer,
    api_key: &str,
    entitlements: &[&str],
    protocol_mode: CloudsyncProtocolMode,
    legacy_database_id: Option<&str>,
) -> Router {
    test_router_with_transport(
        server,
        api_key,
        entitlements,
        protocol_mode,
        legacy_database_id,
        CloudsyncTransport::SqliteSync,
    )
}

fn test_router_with_transport(
    server: &MockServer,
    api_key: &str,
    entitlements: &[&str],
    protocol_mode: CloudsyncProtocolMode,
    legacy_database_id: Option<&str>,
    desktop_transport: CloudsyncTransport,
) -> Router {
    let state = AppState::new(SyncConfig {
        project_url: server.uri(),
        token_issuer_api_key: api_key.to_string(),
        database_id: "database-id".to_string(),
        legacy_database_id: legacy_database_id.map(ToString::to_string),
        protocol_mode,
        desktop_transport,
        token_ttl_seconds: 60,
        supabase_url: server.uri(),
        supabase_anon_key: "anon-key".to_string(),
        supabase_service_role_key: "service-role-key".to_string(),
    });
    cloudsync_router(state.clone())
        .merge(replica_router(state.replica.clone()))
        .merge(device_router(state.replica.clone()))
        .merge(session_share_router(state.clone()))
        .merge(web_edit_router(state))
        .layer(Extension(AuthContext {
            token: "supabase-token".to_string(),
            claims: Claims {
                sub: "user-123".to_string(),
                email: None,
                entitlements: entitlements
                    .iter()
                    .map(|entitlement| (*entitlement).to_string())
                    .collect(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        }))
}

fn personal_workspace(id: &str) -> Value {
    json!({
        "id": id,
        "user_id": "user-123",
        "role": "owner",
        "created_at": "2026-07-16T08:01:00Z",
        "updated_at": "2026-07-16T08:02:00Z",
        "workspace": {
            "id": id,
            "owner_user_id": id,
            "kind": "personal",
            "name": "Personal",
            "created_at": "2026-07-16T08:00:00Z",
            "updated_at": "2026-07-16T08:00:00Z"
        }
    })
}

fn team_membership() -> Value {
    json!({
        "id": "membership-team",
        "user_id": "user-123",
        "role": "member",
        "created_at": "2026-07-16T09:01:00Z",
        "updated_at": "2026-07-16T10:01:00Z",
        "workspace": {
            "id": "workspace-team",
            "owner_user_id": "user-456",
            "kind": "shared",
            "name": "Acme",
            "created_at": "2026-07-16T09:00:00Z",
            "updated_at": "2026-07-16T10:00:00Z"
        }
    })
}

fn team_key_grant() -> Value {
    json!({
        "workspace_id": "workspace-team",
        "key_id": "AAAAAAAAAAAAAAAAAAAAAA",
        "ephemeral_public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "nonce": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        "ciphertext": "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
        "is_active": true
    })
}

fn put_json(path: &str, payload: Value) -> Request<Body> {
    Request::builder()
        .method(Method::PUT)
        .uri(path)
        .header(http_header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap()
}

async fn mock_workspace_projection(server: &MockServer, body: Value) {
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer supabase-token"))
        .and(query_param("select", WORKSPACE_PROJECTION_SELECT))
        .and(query_param("user_id", "eq.user-123"))
        .and(query_param("deleted_at", "is.null"))
        .and(query_param("workspace.deleted_at", "is.null"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn mock_e2ee_key_claim(server: &MockServer, returned_key_id: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/claim_personal_workspace_e2ee_key"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_key_id": TEST_KEY_ID,
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{ "key_id": returned_key_id }])),
        )
        .mount(server)
        .await;
}

async fn mock_workspace_key_grants(server: &MockServer, body: Value) {
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_all_my_workspace_e2ee_grants"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer supabase-token"))
        .and(body_partial_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn token_request() -> Request<Body> {
    Request::post("/token")
        .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
        .body(Body::empty())
        .unwrap()
}

fn replica_capable_token_request() -> Request<Body> {
    Request::post("/token")
        .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
        .header(CLOUDSYNC_TRANSPORTS_HEADER, "replica")
        .body(Body::empty())
        .unwrap()
}

async fn mock_sync_transport_override(server: &MockServer, transport: Option<&str>) {
    let body = match transport {
        Some(transport) => json!([{ "transport": transport }]),
        None => json!([]),
    };
    Mock::given(method("GET"))
        .and(path("/rest/v1/sync_transport_overrides"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(query_param("user_id", "eq.user-123"))
        .and(query_param("select", "transport"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn mock_sqlitecloud_token(server: &MockServer, token: &str) {
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .and(header("authorization", "Bearer issuer-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "token": token }
        })))
        .mount(server)
        .await;
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

mod credentials;
mod enrollment;
mod live_docs;
mod meeting_presence;
mod publication;
mod web_edits;
