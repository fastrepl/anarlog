use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderValue, header},
    routing::get,
};
use hypr_api_auth::AuthContext;
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::error::{Result, SyncError};
use crate::state::AppState;

const MAX_EVENTS_PER_BATCH: usize = 64;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_WITNESS_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_WITNESS_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_WITNESS_PAGE_BYTES: i32 = 48 * 1024 * 1024;
const WITNESS_PAGE_SIZE: i32 = 1024;
const LEGACY_WITNESS_PAGE_SIZE: i32 = 3;
const WITNESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct E2eeWitnessEvent {
    record_id: String,
    payload_hash: String,
    payload: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishE2eeWitnessRequest {
    initialize: bool,
    events: Vec<E2eeWitnessEvent>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishE2eeWitnessResponse {
    initialized_at: String,
    head_sequence: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadE2eeWitnessQuery {
    #[serde(default)]
    after_sequence: u64,
    through_sequence: Option<u64>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct E2eeWitnessPageEvent {
    sequence: u64,
    record_id: String,
    payload_hash: String,
    payload: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct E2eeWitnessPage {
    initialized: bool,
    initialized_at: Option<String>,
    head_sequence: u64,
    through_sequence: u64,
    next_after_sequence: u64,
    events: Vec<E2eeWitnessPageEvent>,
}

#[derive(Serialize)]
struct PublishRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_workspace_id: &'a str,
    p_initialize: bool,
    p_events: &'a [PublishRpcEvent<'a>],
}

#[derive(Serialize)]
struct PublishRpcEvent<'a> {
    record_id: &'a str,
    payload_hash: &'a str,
    payload: &'a str,
}

#[derive(Serialize)]
struct ReadRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_workspace_id: &'a str,
    p_after_sequence: i64,
    p_through_sequence: Option<i64>,
    p_limit: i32,
    p_max_bytes: i32,
}

#[derive(Serialize)]
struct LegacyReadRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_workspace_id: &'a str,
    p_after_sequence: i64,
    p_through_sequence: Option<i64>,
    p_limit: i32,
}

#[derive(Deserialize)]
struct PublishRpcRow {
    initialized_at: String,
    head_sequence: i64,
}

#[derive(Deserialize)]
struct ReadRpcRow {
    initialized_at: Option<String>,
    head_sequence: i64,
    through_sequence: i64,
    event_sequence: Option<i64>,
    record_id: Option<String>,
    payload_hash: Option<String>,
    payload: Option<String>,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(read_e2ee_witness, publish_e2ee_witness),
    components(schemas(
        E2eeWitnessEvent,
        PublishE2eeWitnessRequest,
        PublishE2eeWitnessResponse,
        E2eeWitnessPageEvent,
        E2eeWitnessPage
    ))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/e2ee/witness/{workspace_id}",
            get(read_e2ee_witness).post(publish_e2ee_witness),
        )
        .layer(DefaultBodyLimit::max(MAX_WITNESS_REQUEST_BYTES))
}

#[utoipa::path(
    get,
    path = "/e2ee/witness/{workspace_id}",
    tag = "sync",
    params(
        ("workspace_id" = String, Path, description = "Personal workspace ID"),
        ("afterSequence" = Option<u64>, Query, description = "Last applied witness sequence"),
        ("throughSequence" = Option<u64>, Query, description = "Stable witness page boundary")
    ),
    responses(
        (status = 200, description = "Append-only E2EE witness page", body = E2eeWitnessPage),
        (status = 400, description = "Invalid witness cursor"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Witness workspace access denied"),
        (status = 502, description = "Witness service unavailable")
    )
)]
async fn read_e2ee_witness(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ReadE2eeWitnessQuery>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<E2eeWitnessPage>,
)> {
    require_personal_workspace(&auth, &workspace_id)?;
    let after_sequence = i64::try_from(query.after_sequence)
        .map_err(|_| SyncError::BadRequest("E2EE witness cursor is invalid".to_string()))?;
    let through_sequence = query
        .through_sequence
        .map(i64::try_from)
        .transpose()
        .map_err(|_| SyncError::BadRequest("E2EE witness cursor is invalid".to_string()))?;
    let (status, bytes) = read_witness_page(
        &state,
        &auth.claims.sub,
        &workspace_id,
        after_sequence,
        through_sequence,
    )
    .await?;
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let rows = serde_json::from_slice::<Vec<ReadRpcRow>>(&bytes).map_err(|error| {
        tracing::warn!(%error, "Supabase E2EE witness read response was invalid");
        SyncError::E2eeWitnessServiceUnavailable
    })?;
    let page = validate_read_rows(rows, query.after_sequence, query.through_sequence)?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(page),
    ))
}

async fn read_witness_page(
    state: &AppState,
    actor_user_id: &str,
    workspace_id: &str,
    after_sequence: i64,
    through_sequence: Option<i64>,
) -> Result<(HttpStatusCode, Vec<u8>)> {
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_e2ee_freshness_page_v2",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&ReadRpcRequest {
            p_actor_user_id: actor_user_id,
            p_workspace_id: workspace_id,
            p_after_sequence: after_sequence,
            p_through_sequence: through_sequence,
            p_limit: WITNESS_PAGE_SIZE,
            p_max_bytes: MAX_WITNESS_PAGE_BYTES,
        })
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "read"))?;
    let result = read_bounded_response(response, "read").await?;
    if !is_missing_v2_rpc(result.0, &result.1) {
        return Ok(result);
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_e2ee_freshness_page",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&LegacyReadRpcRequest {
            p_actor_user_id: actor_user_id,
            p_workspace_id: workspace_id,
            p_after_sequence: after_sequence,
            p_through_sequence: through_sequence,
            p_limit: LEGACY_WITNESS_PAGE_SIZE,
        })
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "read"))?;
    read_bounded_response(response, "read").await
}

#[utoipa::path(
    post,
    path = "/e2ee/witness/{workspace_id}",
    tag = "sync",
    params(("workspace_id" = String, Path, description = "Personal workspace ID")),
    request_body = PublishE2eeWitnessRequest,
    responses(
        (status = 200, description = "Ciphertext events appended", body = PublishE2eeWitnessResponse),
        (status = 400, description = "Invalid witness event"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Witness workspace access denied"),
        (status = 409, description = "Legacy witness requires an established device"),
        (status = 502, description = "Witness service unavailable")
    )
)]
async fn publish_e2ee_witness(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<PublishE2eeWitnessRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<PublishE2eeWitnessResponse>,
)> {
    require_personal_workspace(&auth, &workspace_id)?;
    validate_publish_request(&request)?;
    let events = request
        .events
        .iter()
        .map(|event| PublishRpcEvent {
            record_id: &event.record_id,
            payload_hash: &event.payload_hash,
            payload: &event.payload,
        })
        .collect::<Vec<_>>();
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/publish_e2ee_freshness_events",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&PublishRpcRequest {
            p_actor_user_id: &auth.claims.sub,
            p_workspace_id: &workspace_id,
            p_initialize: request.initialize,
            p_events: &events,
        })
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "publication"))?;
    let (status, bytes) = read_bounded_response(response, "publication").await?;
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let mut rows = serde_json::from_slice::<Vec<PublishRpcRow>>(&bytes).map_err(|error| {
        tracing::warn!(%error, "Supabase E2EE witness publication response was invalid");
        SyncError::E2eeWitnessServiceUnavailable
    })?;
    if rows.len() != 1 {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase E2EE witness publication returned an invalid row count"
        );
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }
    let row = rows.pop().expect("row count was checked");
    let initialized_at = chrono::DateTime::parse_from_rfc3339(&row.initialized_at)
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?
        .to_rfc3339();
    let head_sequence =
        u64::try_from(row.head_sequence).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(PublishE2eeWitnessResponse {
            initialized_at,
            head_sequence,
        }),
    ))
}

fn require_personal_workspace(auth: &AuthContext, workspace_id: &str) -> Result<()> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    let workspace = Uuid::parse_str(workspace_id)
        .map_err(|_| SyncError::BadRequest("E2EE witness workspace is invalid".to_string()))?;
    if workspace.to_string() != workspace_id || auth.claims.sub != workspace_id {
        return Err(SyncError::E2eeWitnessForbidden);
    }
    Ok(())
}

fn validate_publish_request(request: &PublishE2eeWitnessRequest) -> Result<()> {
    if request.events.is_empty() || request.events.len() > MAX_EVENTS_PER_BATCH {
        return Err(SyncError::BadRequest(
            "E2EE witness event batch is invalid".to_string(),
        ));
    }
    for event in &request.events {
        if !is_blinded_id(&event.record_id)
            || !is_blinded_id(&event.payload_hash)
            || event.payload.is_empty()
            || event.payload.len() > MAX_EVENT_BYTES
        {
            return Err(SyncError::BadRequest(
                "E2EE witness event is invalid".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_read_rows(
    rows: Vec<ReadRpcRow>,
    requested_after: u64,
    requested_through: Option<u64>,
) -> Result<E2eeWitnessPage> {
    let Some(first) = rows.first() else {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    };
    let raw_head_sequence = first.head_sequence;
    let raw_through_sequence = first.through_sequence;
    let raw_initialized_at = first.initialized_at.clone();
    let head_sequence =
        u64::try_from(raw_head_sequence).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let through_sequence = u64::try_from(raw_through_sequence)
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let initialized_at = raw_initialized_at
        .as_deref()
        .map(chrono::DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?
        .map(|value| value.to_rfc3339());
    if requested_after > through_sequence
        || through_sequence > head_sequence
        || requested_through.is_some_and(|value| value != through_sequence)
        || (initialized_at.is_none() && (head_sequence != 0 || rows.len() != 1))
    {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }

    let mut events = Vec::with_capacity(rows.len());
    let mut previous = requested_after;
    for row in rows {
        if row.head_sequence != raw_head_sequence
            || row.through_sequence != raw_through_sequence
            || row.initialized_at != raw_initialized_at
        {
            return Err(SyncError::E2eeWitnessServiceUnavailable);
        }
        let Some(sequence) = row.event_sequence else {
            if row.record_id.is_some() || row.payload_hash.is_some() || row.payload.is_some() {
                return Err(SyncError::E2eeWitnessServiceUnavailable);
            }
            continue;
        };
        let sequence =
            u64::try_from(sequence).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
        let (Some(record_id), Some(payload_digest), Some(payload)) =
            (row.record_id, row.payload_hash, row.payload)
        else {
            return Err(SyncError::E2eeWitnessServiceUnavailable);
        };
        if sequence <= previous
            || sequence > through_sequence
            || !is_blinded_id(&record_id)
            || !is_blinded_id(&payload_digest)
            || payload.is_empty()
            || payload.len() > MAX_EVENT_BYTES
        {
            return Err(SyncError::E2eeWitnessServiceUnavailable);
        }
        previous = sequence;
        events.push(E2eeWitnessPageEvent {
            sequence,
            record_id,
            payload_hash: payload_digest,
            payload,
        });
    }

    Ok(E2eeWitnessPage {
        initialized: initialized_at.is_some(),
        initialized_at,
        head_sequence,
        through_sequence,
        next_after_sequence: previous,
        events,
    })
}

async fn read_bounded_response(
    mut response: reqwest::Response,
    operation: &'static str,
) -> Result<(HttpStatusCode, Vec<u8>)> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WITNESS_RESPONSE_BYTES as u64)
    {
        tracing::warn!(%status, operation, "Supabase E2EE witness response was too large");
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, operation, "Supabase E2EE witness response could not be read");
        SyncError::E2eeWitnessServiceUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > MAX_WITNESS_RESPONSE_BYTES {
            tracing::warn!(%status, operation, "Supabase E2EE witness response was too large");
            return Err(SyncError::E2eeWitnessServiceUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}

fn witness_transport_error(error: reqwest::Error, operation: &'static str) -> SyncError {
    tracing::warn!(%error, operation, "Supabase E2EE witness request failed");
    SyncError::E2eeWitnessServiceUnavailable
}

fn map_postgrest_error(status: HttpStatusCode, bytes: &[u8]) -> SyncError {
    let code = serde_json::from_slice::<PostgrestError>(bytes)
        .ok()
        .map(|error| error.code);
    tracing::warn!(%status, ?code, "Supabase E2EE witness request was rejected");
    match (status, code.as_deref()) {
        (HttpStatusCode::UNAUTHORIZED | HttpStatusCode::FORBIDDEN, _) | (_, Some("42501")) => {
            SyncError::E2eeWitnessForbidden
        }
        (_, Some("22023")) => SyncError::BadRequest("E2EE witness request is invalid".to_string()),
        (_, Some("55000")) => SyncError::E2eeWitnessUninitialized,
        _ => SyncError::E2eeWitnessServiceUnavailable,
    }
}

fn is_missing_v2_rpc(status: HttpStatusCode, bytes: &[u8]) -> bool {
    status == HttpStatusCode::NOT_FOUND
        && serde_json::from_slice::<PostgrestError>(bytes)
            .is_ok_and(|error| error.code == "PGRST202")
}

fn is_blinded_id(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use hypr_api_auth::Claims;
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header as request_header, method, path},
    };

    use super::*;
    use crate::SyncConfig;

    const OWNER: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER: &str = "22222222-2222-4222-8222-222222222222";
    const RECORD_ID: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const PAYLOAD_HASH: &str = "bSKYhMEmi7CrMtjaMV0P5S-RRyKL2DCje8n7KKlUlA0";

    fn test_router(server: &MockServer) -> Router {
        router()
            .with_state(AppState::new(SyncConfig {
                project_url: server.uri(),
                token_issuer_api_key: "issuer-key".to_string(),
                database_id: "database-id".to_string(),
                legacy_database_id: None,
                protocol_mode: crate::config::CloudsyncProtocolMode::E2eeEnforced,
                token_ttl_seconds: 60,
                supabase_url: server.uri(),
                supabase_anon_key: "anon-key".to_string(),
                supabase_service_role_key: "service-role-key".to_string(),
            }))
            .layer(Extension(AuthContext {
                token: "user-token".to_string(),
                claims: Claims {
                    sub: OWNER.to_string(),
                    email: None,
                    entitlements: ["hyprnote_pro".to_string()].into_iter().collect(),
                    subscription_status: None,
                    trial_end: None,
                    has_payment_method: None,
                },
            }))
    }

    fn request(method: Method, path: &str, body: Option<Value>) -> Request<Body> {
        let mut request = Request::builder().method(method).uri(path);
        if body.is_some() {
            request = request.header(header::CONTENT_TYPE, "application/json");
        }
        request
            .body(body.map_or_else(Body::empty, |body| {
                Body::from(serde_json::to_vec(&body).unwrap())
            }))
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn reads_a_stable_witness_page_through_the_service_role() {
        let server = MockServer::start().await;
        let payload = "opaque";
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page_v2"))
            .and(request_header("apikey", "service-role-key"))
            .and(request_header("authorization", "Bearer service-role-key"))
            .and(body_partial_json(json!({
                "p_actor_user_id": OWNER,
                "p_workspace_id": OWNER,
                "p_after_sequence": 0,
                "p_limit": 1024,
                "p_max_bytes": 50331648
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "initialized_at": "2026-07-17T00:00:00Z",
                "head_sequence": 1,
                "through_sequence": 1,
                "event_sequence": 1,
                "record_id": RECORD_ID,
                "payload_hash": PAYLOAD_HASH,
                "payload": payload
            }])))
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}?afterSequence=0"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["headSequence"], 1);
        assert_eq!(body["events"][0]["recordId"], RECORD_ID);
    }

    #[tokio::test]
    async fn falls_back_to_legacy_witness_pages_during_database_rollout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page_v2"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "code": "PGRST202",
                "message": "function is not in the schema cache"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page"))
            .and(body_partial_json(json!({
                "p_actor_user_id": OWNER,
                "p_workspace_id": OWNER,
                "p_after_sequence": 0,
                "p_limit": 3
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "initialized_at": "2026-07-17T00:00:00Z",
                "head_sequence": 0,
                "through_sequence": 0,
                "event_sequence": null,
                "record_id": null,
                "payload_hash": null,
                "payload": null
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}?afterSequence=0"),
                None,
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["events"], json!([]));
        server.verify().await;
    }

    #[tokio::test]
    async fn publishes_snake_case_rpc_events() {
        let server = MockServer::start().await;
        let payload = "opaque";
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_e2ee_freshness_events"))
            .and(body_partial_json(json!({
                "p_actor_user_id": OWNER,
                "p_workspace_id": OWNER,
                "p_initialize": false,
                "p_events": [{
                    "record_id": RECORD_ID,
                    "payload_hash": PAYLOAD_HASH,
                    "payload": payload
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "initialized_at": "2026-07-17T00:00:00Z",
                "head_sequence": 1
            }])))
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}"),
                Some(json!({
                    "initialize": false,
                    "events": [{
                        "recordId": RECORD_ID,
                        "payloadHash": PAYLOAD_HASH,
                        "payload": payload
                    }]
                })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["headSequence"], 1);
    }

    #[test]
    fn accepts_the_database_publish_batch_limit() {
        let event = E2eeWitnessEvent {
            record_id: RECORD_ID.to_string(),
            payload_hash: PAYLOAD_HASH.to_string(),
            payload: "opaque".to_string(),
        };
        assert!(
            validate_publish_request(&PublishE2eeWitnessRequest {
                initialize: false,
                events: vec![event.clone(); 64],
            })
            .is_ok()
        );
        assert!(
            validate_publish_request(&PublishE2eeWitnessRequest {
                initialize: false,
                events: vec![event; 65],
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_cross_workspace_access_before_calling_supabase() {
        let server = MockServer::start().await;
        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OTHER}"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_uninitialized_legacy_witnesses_to_conflict() {
        let server = MockServer::start().await;
        let payload = "opaque";
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_e2ee_freshness_events"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(json!({ "code": "55000", "message": "uninitialized" })),
            )
            .mount(&server)
            .await;
        let response = test_router(&server)
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}"),
                Some(json!({
                    "initialize": false,
                    "events": [{
                        "recordId": RECORD_ID,
                        "payloadHash": PAYLOAD_HASH,
                        "payload": payload
                    }]
                })),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
