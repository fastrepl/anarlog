mod accepted;
mod snapshot;

use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderValue, header},
    routing::{get, post},
};
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::error::{Result, SyncError};
use crate::state::ReplicaState;

const MAX_EVENTS_PER_BATCH: usize = 64;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_WITNESS_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_WITNESS_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
// Keeps each page short enough to download on slow mobile networks. The first
// event is always returned, so a maximum-size event still fits.
const MAX_WITNESS_PAGE_BYTES: i32 = 16 * 1024 * 1024;
const WITNESS_PAGE_SIZE: i32 = 1024;
const LEGACY_WITNESS_PAGE_SIZE: i32 = 3;
const WITNESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const WITNESS_WAIT_HOLD: std::time::Duration = std::time::Duration::from_secs(25);
const WITNESS_WAIT_RECHECK: std::time::Duration = std::time::Duration::from_secs(15);
const WITNESS_WAIT_HEAD_LIMIT: i32 = 1;
const WITNESS_WAIT_HEAD_BYTES: i32 = 1024;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitE2eeWitnessQuery {
    #[serde(default)]
    after_sequence: u64,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct E2eeWitnessWaitResponse {
    initialized: bool,
    head_sequence: u64,
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

#[derive(Serialize)]
struct HeadRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_workspace_id: &'a str,
}

#[derive(Deserialize)]
struct HeadRpcRow {
    initialized_at: Option<String>,
    head_sequence: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalWakeRequest {
    workspace_id: String,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        read_e2ee_witness,
        publish_e2ee_witness,
        wait_e2ee_witness,
        accepted::read,
        accepted::accept,
        snapshot::read,
        snapshot::prepare_upload,
        snapshot::commit
    ),
    components(schemas(
        accepted::AcceptRequest,
        accepted::AcceptResponse,
        accepted::Receipt,
        accepted::AcceptedPage,
        snapshot::SnapshotDescriptor,
        snapshot::SnapshotResponse,
        snapshot::PrepareSnapshotUploadRequest,
        snapshot::PrepareSnapshotUploadResponse,
        snapshot::CommitSnapshotRequest,
        snapshot::CommitSnapshotResponse,
        E2eeWitnessEvent,
        PublishE2eeWitnessRequest,
        PublishE2eeWitnessResponse,
        E2eeWitnessPageEvent,
        E2eeWitnessPage,
        E2eeWitnessWaitResponse
    ))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router() -> Router<ReplicaState> {
    Router::new()
        .merge(accepted::router())
        .merge(snapshot::router())
        .route(
            "/e2ee/witness/{workspace_id}",
            get(read_e2ee_witness).post(publish_e2ee_witness),
        )
        .route("/e2ee/witness/{workspace_id}/wait", get(wait_e2ee_witness))
        .layer(DefaultBodyLimit::max(MAX_WITNESS_REQUEST_BYTES))
}

pub(super) fn internal_wake_router() -> Router<ReplicaState> {
    Router::new().route(
        "/internal/e2ee/witness/wake",
        post(receive_internal_witness_wake),
    )
}

async fn receive_internal_witness_wake(
    State(state): State<ReplicaState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<InternalWakeRequest>,
) -> axum::http::StatusCode {
    let Ok(workspace) = Uuid::parse_str(&body.workspace_id) else {
        return axum::http::StatusCode::BAD_REQUEST;
    };
    if workspace.to_string() != body.workspace_id {
        return axum::http::StatusCode::BAD_REQUEST;
    }
    let Some(ts) = headers.get("x-anlg-wake-ts").and_then(|v| v.to_str().ok()) else {
        return axum::http::StatusCode::UNAUTHORIZED;
    };
    let Some(sig) = headers.get("x-anlg-wake-sig").and_then(|v| v.to_str().ok()) else {
        return axum::http::StatusCode::UNAUTHORIZED;
    };
    if !state.witness_wakes.verify_wake(&body.workspace_id, ts, sig) {
        return axum::http::StatusCode::UNAUTHORIZED;
    }
    state.witness_wakes.notify_local(&body.workspace_id);
    axum::http::StatusCode::NO_CONTENT
}

#[utoipa::path(
    get,
    path = "/e2ee/witness/{workspace_id}",
    tag = "sync",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
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
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ReadE2eeWitnessQuery>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<E2eeWitnessPage>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
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

#[utoipa::path(
    get,
    path = "/e2ee/witness/{workspace_id}/wait",
    tag = "sync",
    params(
        ("workspace_id" = String, Path, description = "Workspace ID"),
        ("afterSequence" = Option<u64>, Query, description = "Last witness sequence known to the caller")
    ),
    responses(
        (status = 200, description = "Witness head state, held until it advances past the cursor or the hold expires", body = E2eeWitnessWaitResponse),
        (status = 400, description = "Invalid witness cursor"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Witness workspace access denied"),
        (status = 502, description = "Witness service unavailable")
    )
)]
async fn wait_e2ee_witness(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WaitE2eeWitnessQuery>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<E2eeWitnessWaitResponse>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
    i64::try_from(query.after_sequence)
        .map_err(|_| SyncError::BadRequest("E2EE witness cursor is invalid".to_string()))?;
    let wake = state.witness_wakes.subscribe(&workspace_id);
    let has_fanout = state.witness_wakes.has_peer_fanout();
    let deadline = tokio::time::Instant::now() + WITNESS_WAIT_HOLD;
    loop {
        let notified = wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let head = read_witness_head(&state, &auth.claims.sub, &workspace_id).await?;
        let now = tokio::time::Instant::now();
        if head.head_sequence > query.after_sequence || !head.initialized || now >= deadline {
            return Ok((
                [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
                Json(head),
            ));
        }
        let wait_for = if has_fanout {
            deadline - now
        } else {
            WITNESS_WAIT_RECHECK.min(deadline - now)
        };
        let woke = tokio::select! {
            () = &mut notified => true,
            () = tokio::time::sleep(wait_for) => false,
        };
        if !woke && tokio::time::Instant::now() >= deadline {
            return Ok((
                [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
                Json(head),
            ));
        }
    }
}

async fn read_witness_head(
    state: &ReplicaState,
    actor_user_id: &str,
    workspace_id: &str,
) -> Result<E2eeWitnessWaitResponse> {
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_e2ee_witness_head",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&HeadRpcRequest {
            p_actor_user_id: actor_user_id,
            p_workspace_id: workspace_id,
        })
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "wait"))?;
    let (status, bytes) = read_bounded_response(response, "wait").await?;
    if !is_missing_v2_rpc(status, &bytes) {
        if !status.is_success() {
            return Err(map_postgrest_error(status, &bytes));
        }
        let rows = serde_json::from_slice::<Vec<HeadRpcRow>>(&bytes).map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE witness head response was invalid");
            SyncError::E2eeWitnessServiceUnavailable
        })?;
        let Some(first) = rows.first() else {
            return Err(SyncError::E2eeWitnessServiceUnavailable);
        };
        let head_sequence = u64::try_from(first.head_sequence)
            .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
        return Ok(E2eeWitnessWaitResponse {
            initialized: first.initialized_at.is_some(),
            head_sequence,
        });
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_e2ee_freshness_page_v2",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        // through=0 keeps the event window empty, so the page RPC returns the
        // head-only singleton row without materializing any event ciphertext.
        .json(&ReadRpcRequest {
            p_actor_user_id: actor_user_id,
            p_workspace_id: workspace_id,
            p_after_sequence: 0,
            p_through_sequence: Some(0),
            p_limit: WITNESS_WAIT_HEAD_LIMIT,
            p_max_bytes: WITNESS_WAIT_HEAD_BYTES,
        })
        .send()
        .await
        .map_err(|error| witness_transport_error(error, "wait"))?;
    let (status, bytes) = read_bounded_response(response, "wait").await?;
    let (status, bytes) = if is_missing_v2_rpc(status, &bytes) {
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
                p_after_sequence: 0,
                p_through_sequence: Some(0),
                p_limit: WITNESS_WAIT_HEAD_LIMIT,
            })
            .send()
            .await
            .map_err(|error| witness_transport_error(error, "wait"))?;
        read_bounded_response(response, "wait").await?
    } else {
        (status, bytes)
    };
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let rows = serde_json::from_slice::<Vec<ReadRpcRow>>(&bytes).map_err(|error| {
        tracing::warn!(%error, "Supabase E2EE witness head response was invalid");
        SyncError::E2eeWitnessServiceUnavailable
    })?;
    let Some(first) = rows.first() else {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    };
    let head_sequence =
        u64::try_from(first.head_sequence).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    Ok(E2eeWitnessWaitResponse {
        initialized: first.initialized_at.is_some(),
        head_sequence,
    })
}

async fn read_witness_page(
    state: &ReplicaState,
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
    params(("workspace_id" = String, Path, description = "Workspace ID")),
    request_body = PublishE2eeWitnessRequest,
    responses(
        (status = 200, description = "Ciphertext events appended", body = PublishE2eeWitnessResponse),
        (status = 400, description = "Invalid witness event"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Witness workspace access denied"),
        (status = 409, description = "Legacy witness requires an established device"),
        (status = 503, description = "Publication busy; retry after the Retry-After delay"),
        (status = 502, description = "Witness service unavailable")
    )
)]
async fn publish_e2ee_witness(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<PublishE2eeWitnessRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<PublishE2eeWitnessResponse>,
)> {
    require_witness_workspace(&auth, &workspace_id)?;
    validate_publish_request(&request)?;
    let _permit = state
        .witness_writes
        .try_acquire(&workspace_id)
        .ok_or(SyncError::E2eeWitnessBusy)?;
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
    state.witness_wakes.notify(&workspace_id);
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(PublishE2eeWitnessResponse {
            initialized_at,
            head_sequence,
        }),
    ))
}

fn require_witness_workspace(auth: &AuthContext, workspace_id: &str) -> Result<()> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    let workspace = Uuid::parse_str(workspace_id)
        .map_err(|_| SyncError::BadRequest("E2EE witness workspace is invalid".to_string()))?;
    if workspace.to_string() != workspace_id {
        return Err(SyncError::BadRequest(
            "E2EE witness workspace is invalid".to_string(),
        ));
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
        (_, Some("A0002")) => SyncError::CloudsyncUpgradeRequired,
        (_, Some("A0003")) => SyncError::E2eeHistoryCompacted,
        (_, Some("55000")) => SyncError::E2eeWitnessUninitialized,
        (_, Some("55P03")) => SyncError::E2eeWitnessBusy,
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
    use anlg_api_auth::Claims;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header as request_header, method, path},
    };

    use super::*;
    use crate::ReplicaConfig;

    const OWNER: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER: &str = "22222222-2222-4222-8222-222222222222";
    const RECORD_ID: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const PAYLOAD_HASH: &str = "bSKYhMEmi7CrMtjaMV0P5S-RRyKL2DCje8n7KKlUlA0";

    fn test_router(server: &MockServer) -> Router {
        test_router_with_state(
            ReplicaState::new(
                ReplicaConfig::new(server.uri(), "anon-key", "service-role-key").unwrap(),
            ),
            OWNER,
        )
    }

    fn test_router_with_state(state: ReplicaState, workspace: &str) -> Router {
        router().with_state(state).layer(Extension(AuthContext {
            token: "user-token".to_string(),
            claims: Claims {
                sub: workspace.to_string(),
                email: None,
                entitlements: ["hyprnote_pro".to_string()].into_iter().collect(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
                referral_extension: None,
            },
        }))
    }

    #[tokio::test]
    async fn witness_writes_cannot_exhaust_the_database_pool_across_workspaces() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_e2ee_freshness_events"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(500))
                    .set_body_json(
                        json!([{"initialized_at":"2026-10-05T00:00:00Z","head_sequence":1}]),
                    ),
            )
            .mount(&server)
            .await;
        let state = ReplicaState::new(
            ReplicaConfig::new(server.uri(), "anon-key", "service-role-key").unwrap(),
        );
        let mut pending = Vec::new();
        for index in 1..=4 {
            let workspace = format!("00000000-0000-4000-8000-{index:012}");
            pending.push(tokio::spawn(
                test_router_with_state(state.clone(), &workspace).oneshot(request(
                    Method::POST,
                    &format!("/e2ee/witness/{workspace}"),
                    Some(publish_body()),
                )),
            ));
        }
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while server.received_requests().await.unwrap().len() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let router = test_router_with_state(state, OWNER);
        let overloaded = router
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}"),
                Some(publish_body()),
            ))
            .await
            .unwrap();
        assert_eq!(overloaded.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(overloaded.headers()[header::RETRY_AFTER], "1");
        for request in pending {
            assert_eq!(request.await.unwrap().unwrap().status(), StatusCode::OK);
        }
        assert_eq!(
            router
                .oneshot(request(
                    Method::POST,
                    &format!("/e2ee/witness/{OWNER}"),
                    Some(publish_body())
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
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

    fn witness_row(initialized_at: Option<&str>, head: i64) -> Value {
        json!({
            "initialized_at": initialized_at,
            "head_sequence": head,
            "through_sequence": head,
            "event_sequence": null,
            "record_id": null,
            "payload_hash": null,
            "payload": null
        })
    }

    fn publish_body() -> Value {
        json!({
            "initialize": false,
            "events": [{
                "recordId": RECORD_ID,
                "payloadHash": PAYLOAD_HASH,
                "payload": "opaque"
            }]
        })
    }

    #[tokio::test]
    async fn acceptance_validates_receipts_and_exposes_a_retryable_base_conflict() {
        for case in ["accepted", "wrong_receipt", "stale_base", "busy"] {
            let server = MockServer::start().await;
            let mutation = "11111111-1111-4111-8111-111111111111";
            let upstream = if case == "busy" {
                ResponseTemplate::new(500).set_body_json(json!({"code":"55P03"}))
            } else if case == "stale_base" {
                ResponseTemplate::new(409).set_body_json(json!({"code":"40001"}))
            } else {
                ResponseTemplate::new(200).set_body_json(json!([{
                    "initialized_at":"2026-10-05T00:00:00Z", "head_sequence":1, "cloud_authority_after":0,
                    "mutation_id": mutation, "receipts":[{"sequence":1,"recordId": RECORD_ID,
                        "payloadHash": if case == "wrong_receipt" { RECORD_ID } else { PAYLOAD_HASH }}]
                }]))
            };
            Mock::given(method("POST")).and(path("/rest/v1/rpc/accept_e2ee_replica_batch"))
                .and(body_partial_json(json!({"p_actor_user_id":OWNER,"p_workspace_id":OWNER,"p_mutation_id":mutation,"p_base_sequence":0})))
                .respond_with(upstream).mount(&server).await;
            let mut body = publish_body();
            body["mutationId"] = json!(mutation);
            body["baseSequence"] = json!(0);
            let response = test_router(&server)
                .oneshot(request(
                    Method::POST,
                    &format!("/e2ee/witness/{OWNER}/accepted"),
                    Some(body),
                ))
                .await
                .unwrap();
            let expected = match case {
                "accepted" => StatusCode::OK,
                "stale_base" => StatusCode::CONFLICT,
                "busy" => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::BAD_GATEWAY,
            };
            assert_eq!(response.status(), expected, "{case}");
            if case == "busy" {
                assert_eq!(response.headers()[header::RETRY_AFTER], "1");
            }
            let body = response_json(response).await;
            if case == "accepted" {
                assert_eq!(body["receipts"][0]["sequence"], 1);
            }
            if case == "stale_base" {
                assert_eq!(body["error"]["code"], "e2ee_replica_base_changed");
            }
        }
    }

    #[tokio::test]
    async fn busy_publication_does_not_queue_another_database_write_or_block_reads() {
        let server = MockServer::start().await;
        let mutation = "11111111-1111-4111-8111-111111111111";
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/accept_e2ee_replica_batch"))
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(200)).set_body_json(json!([{
                "initialized_at":"2026-10-05T00:00:00Z", "head_sequence":1, "cloud_authority_after":0,
                "mutation_id":mutation, "receipts":[{"sequence":1,"recordId":RECORD_ID,"payloadHash":PAYLOAD_HASH}]
            }])))
            .mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page_v2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([witness_row(Some("2026-10-05T00:00:00Z"), 0)])),
            )
            .mount(&server)
            .await;
        let router = test_router(&server);
        let mut body = publish_body();
        body["mutationId"] = json!(mutation);
        body["baseSequence"] = json!(0);
        let pending = tokio::spawn(router.clone().oneshot(request(
            Method::POST,
            &format!("/e2ee/witness/{OWNER}/accepted"),
            Some(body.clone()),
        )));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while server.received_requests().await.unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for path in [
            format!("/e2ee/witness/{OWNER}/accepted"),
            format!("/e2ee/witness/{OWNER}"),
        ] {
            let request_body = if path.ends_with("accepted") {
                body.clone()
            } else {
                publish_body()
            };
            let response = router
                .clone()
                .oneshot(request(Method::POST, &path, Some(request_body)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        }
        let read = router
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(read.status(), StatusCode::OK);
        assert_eq!(pending.await.unwrap().unwrap().status(), StatusCode::OK);
        let throttled = router
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}/accepted"),
                Some(body.clone()),
            ))
            .await
            .unwrap();
        assert_eq!(throttled.status(), StatusCode::SERVICE_UNAVAILABLE);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_eq!(
            router
                .oneshot(request(
                    Method::POST,
                    &format!("/e2ee/witness/{OWNER}/accepted"),
                    Some(body)
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
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
                "p_max_bytes": 16777216
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
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([witness_row(Some("2026-07-17T00:00:00Z"), 0)])),
            )
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
                Some(publish_body()),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["headSequence"], 1);
    }

    #[tokio::test]
    async fn wait_resolves_immediately_when_head_is_ahead_or_uninitialized() {
        for (case, initialized_at, head, after_sequence, expected_initialized) in [
            (
                "head ahead of the requested sequence",
                Some("2026-07-17T00:00:00Z"),
                5,
                3,
                true,
            ),
            ("uninitialized witness", None, 0, 0, false),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/rest/v1/rpc/read_e2ee_witness_head"))
                .and(body_partial_json(json!({
                    "p_actor_user_id": OWNER,
                    "p_workspace_id": OWNER
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                    "initialized_at": initialized_at,
                    "head_sequence": head
                }])))
                .mount(&server)
                .await;

            let response = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                test_router(&server).oneshot(request(
                    Method::GET,
                    &format!("/e2ee/witness/{OWNER}/wait?afterSequence={after_sequence}"),
                    None,
                )),
            )
            .await
            .expect("witness waits should resolve immediately")
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{case}");
            let body = response_json(response).await;
            assert_eq!(body["initialized"], expected_initialized, "{case}");
            assert_eq!(body["headSequence"], head, "{case}");
        }
    }

    #[derive(Clone)]
    struct HeadFromCounter {
        head: std::sync::Arc<std::sync::atomic::AtomicI64>,
    }

    impl wiremock::Respond for HeadFromCounter {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            let head = self.head.load(std::sync::atomic::Ordering::SeqCst);
            ResponseTemplate::new(200)
                .set_body_json(json!([witness_row(Some("2026-07-17T00:00:00Z"), head)]))
        }
    }

    #[derive(Clone)]
    struct PublishBumpsCounter {
        head: std::sync::Arc<std::sync::atomic::AtomicI64>,
    }

    impl wiremock::Respond for PublishBumpsCounter {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            let head = self
                .head
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                .saturating_add(1);
            ResponseTemplate::new(200).set_body_json(json!([{
                "initialized_at": "2026-07-17T00:00:00Z",
                "head_sequence": head
            }]))
        }
    }

    #[tokio::test]
    async fn wait_wakes_when_a_publish_lands_on_the_same_instance() {
        let server = MockServer::start().await;
        let head = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(3));
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_witness_head"))
            .respond_with(HeadFromCounter {
                head: std::sync::Arc::clone(&head),
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_e2ee_freshness_events"))
            .respond_with(PublishBumpsCounter {
                head: std::sync::Arc::clone(&head),
            })
            .mount(&server)
            .await;

        let router = test_router(&server);
        let waiter = router.clone();
        let wait = tokio::spawn(async move {
            waiter
                .oneshot(request(
                    Method::GET,
                    &format!("/e2ee/witness/{OWNER}/wait?afterSequence=3"),
                    None,
                ))
                .await
                .unwrap()
        });
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let publish = router
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}"),
                Some(publish_body()),
            ))
            .await
            .unwrap();
        assert_eq!(publish.status(), StatusCode::OK);

        // Well under the 10s periodic recheck, so only the publish wake can
        // explain a prompt response.
        let response = tokio::time::timeout(std::time::Duration::from_secs(5), wait)
            .await
            .expect("a same-instance publish must wake the held wait")
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["headSequence"], 4);
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
    async fn proxies_shared_workspace_access_to_supabase() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page_v2"))
            .and(body_partial_json(json!({
                "p_actor_user_id": OWNER,
                "p_workspace_id": OTHER
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([witness_row(Some("2026-08-15T00:00:00Z"), 0)])),
            )
            .expect(1)
            .mount(&server)
            .await;
        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OTHER}"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        server.verify().await;
    }

    #[tokio::test]
    async fn maps_database_errors_to_client_statuses() {
        for (case, request_method, request_path, rpc_name, pg_code, expected_status) in [
            (
                "revoked shared workspace access",
                Method::GET,
                format!("/e2ee/witness/{OTHER}"),
                "read_e2ee_freshness_page_v2",
                "42501",
                StatusCode::FORBIDDEN,
            ),
            (
                "invalid witness events",
                Method::POST,
                format!("/e2ee/witness/{OWNER}"),
                "publish_e2ee_freshness_events",
                "22023",
                StatusCode::BAD_REQUEST,
            ),
            (
                "uninitialized legacy witness",
                Method::POST,
                format!("/e2ee/witness/{OWNER}"),
                "publish_e2ee_freshness_events",
                "55000",
                StatusCode::CONFLICT,
            ),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(format!("/rest/v1/rpc/{rpc_name}")))
                .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                    "code": pg_code,
                    "message": "database error"
                })))
                .expect(1)
                .mount(&server)
                .await;

            let body = (request_method == Method::POST).then(publish_body);
            let response = test_router(&server)
                .oneshot(request(request_method, &request_path, body))
                .await
                .unwrap();

            assert_eq!(response.status(), expected_status, "{case}");
            server.verify().await;
        }
    }

    #[tokio::test]
    async fn read_witness_head_falls_back_to_page_rpc_during_rollout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_witness_head"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "code": "PGRST202",
                "message": "function is not in the schema cache"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_freshness_page_v2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([witness_row(Some("2026-07-17T00:00:00Z"), 7)])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}/wait?afterSequence=5"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["headSequence"], 7);
        server.verify().await;
    }

    #[tokio::test]
    async fn wait_wakes_when_a_publish_lands_on_a_peer_instance() {
        let mock_supabase = MockServer::start().await;
        let head = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(3));
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_witness_head"))
            .respond_with(HeadFromCounter {
                head: std::sync::Arc::clone(&head),
            })
            .mount(&mock_supabase)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_e2ee_freshness_events"))
            .respond_with(PublishBumpsCounter {
                head: std::sync::Arc::clone(&head),
            })
            .mount(&mock_supabase)
            .await;

        let state_b = ReplicaState::new(
            ReplicaConfig::new(mock_supabase.uri(), "anon-key", "service-role-key").unwrap(),
        );
        let app_b = test_router_with_state(state_b.clone(), OWNER)
            .merge(internal_wake_router().with_state(state_b));
        let listener_b = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr_b = listener_b.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener_b, app_b).await.unwrap();
        });

        let state_a = ReplicaState::new(
            ReplicaConfig::new(mock_supabase.uri(), "anon-key", "service-role-key").unwrap(),
        )
        .with_wake_peers(vec![format!("http://{addr_b}")]);
        let router_a = test_router_with_state(state_a, OWNER);

        let wait_handle = tokio::spawn(async move {
            reqwest::Client::new()
                .get(format!(
                    "http://{addr_b}/e2ee/witness/{OWNER}/wait?afterSequence=3"
                ))
                .send()
                .await
                .unwrap()
        });
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let publish = router_a
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}"),
                Some(publish_body()),
            ))
            .await
            .unwrap();
        assert_eq!(publish.status(), StatusCode::OK);

        let waited = tokio::time::timeout(std::time::Duration::from_secs(2), wait_handle)
            .await
            .expect("peer publish must wake held wait immediately")
            .unwrap();
        assert_eq!(waited.status(), StatusCode::OK);
        let body: Value = waited.json().await.unwrap();
        assert_eq!(body["headSequence"], 4);
    }

    #[tokio::test]
    async fn internal_wake_rejects_missing_or_invalid_hmac() {
        let server = MockServer::start().await;
        let state = ReplicaState::new(
            ReplicaConfig::new(server.uri(), "anon-key", "service-role-key").unwrap(),
        );
        let router = internal_wake_router().with_state(state);

        let missing = router
            .clone()
            .oneshot(request(
                Method::POST,
                "/internal/e2ee/witness/wake",
                Some(json!({ "workspaceId": OWNER })),
            ))
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let invalid = axum::http::Request::builder()
            .method(Method::POST)
            .uri("/internal/e2ee/witness/wake")
            .header("content-type", "application/json")
            .header("x-anlg-wake-ts", "9999999999")
            .header("x-anlg-wake-sig", "invalid")
            .body(Body::from(
                serde_json::to_vec(&json!({ "workspaceId": OWNER })).unwrap(),
            ))
            .unwrap();
        let response = router.oneshot(invalid).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn maps_compacted_history_error_to_410() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_replica_page"))
            .respond_with(ResponseTemplate::new(410).set_body_json(json!({
                "code": "A0003",
                "message": "E2EE witness history compacted"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}/accepted?afterSequence=0"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "e2ee_history_compacted");
        server.verify().await;
    }

    #[tokio::test]
    async fn snapshot_routes_read_prepare_and_commit_with_verification() {
        let server = MockServer::start().await;
        let sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let key_id = "abcdefghijklmnopqrstuv";
        let snapshot_bytes = b"ANLGSNP1mockpayload";

        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_latest_e2ee_snapshot"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let empty = test_router(&server)
            .oneshot(request(
                Method::GET,
                &format!("/e2ee/witness/{OWNER}/snapshot"),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(empty.status(), StatusCode::OK);
        assert_eq!(response_json(empty).await["snapshot"], Value::Null);

        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_e2ee_witness_head"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "initialized_at": "2026-10-05T00:00:00Z",
                "head_sequence": 12
            }])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(wiremock::matchers::path_regex(
                r"^/storage/v1/object/upload/sign/e2ee-snapshots/11111111-1111-4111-8111-111111111111/10-[0-9a-f-]+\.ans1$",
            ))
            .respond_with(|req: &wiremock::Request| {
                let signed_path = req
                    .url
                    .path()
                    .strip_prefix("/storage/v1")
                    .unwrap_or(req.url.path());
                ResponseTemplate::new(200).set_body_json(json!({
                    "url": format!("{signed_path}?token=tok123")
                }))
            })
            .mount(&server)
            .await;

        let prepared = test_router(&server)
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}/snapshot/upload"),
                Some(json!({
                    "snapshotSequence": 10,
                    "payloadBytes": snapshot_bytes.len(),
                    "payloadSha256": sha256,
                    "recordCount": 5,
                    "keyId": key_id
                })),
            ))
            .await
            .unwrap();
        assert_eq!(prepared.status(), StatusCode::OK);
        let prepared_body = response_json(prepared).await;
        let object_key = prepared_body["objectKey"].as_str().unwrap().to_string();
        assert!(object_key.starts_with(&format!("{OWNER}/10-")));
        assert!(object_key.ends_with(".ans1"));

        // Re-test commit route cleanly using exact object_key mock for Storage download + RPC commit.
        let commit_server = MockServer::start().await;
        let commit_key = format!("{OWNER}/10-11111111-1111-4111-8111-111111111111.ans1");
        let real_bytes = b"snapshot-body-bytes".to_vec();
        let real_sha = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(&real_bytes);
            let mut s = String::new();
            for b in digest {
                s.push(char::from(b"0123456789abcdef"[(b >> 4) as usize]));
                s.push(char::from(b"0123456789abcdef"[(b & 0x0f) as usize]));
            }
            s
        };
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/e2ee-snapshots/{commit_key}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(real_bytes.clone()))
            .mount(&commit_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/commit_e2ee_snapshot"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "committed": true,
                "retired_object_keys": [format!("{OWNER}/1-00000000-0000-4000-8000-000000000001.ans1")]
            }])))
            .mount(&commit_server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/storage/v1/object/e2ee-snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&commit_server)
            .await;

        let committed = test_router(&commit_server)
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}/snapshot/commit"),
                Some(json!({
                    "snapshotSequence": 10,
                    "objectKey": commit_key,
                    "formatVersion": 1,
                    "payloadBytes": real_bytes.len(),
                    "payloadSha256": real_sha,
                    "recordCount": 5,
                    "keyId": key_id
                })),
            ))
            .await
            .unwrap();
        assert_eq!(committed.status(), StatusCode::OK);
        let commit_body = response_json(committed).await;
        assert_eq!(commit_body["committed"], true);
        assert_eq!(commit_body["snapshotSequence"], 10);
        commit_server.verify().await;

        // Checksum mismatch on commit is rejected with 400 before RPC commit.
        let bad = test_router(&commit_server)
            .oneshot(request(
                Method::POST,
                &format!("/e2ee/witness/{OWNER}/snapshot/commit"),
                Some(json!({
                    "snapshotSequence": 10,
                    "objectKey": format!("{OWNER}/10-11111111-1111-4111-8111-111111111111.ans1"),
                    "formatVersion": 1,
                    "payloadBytes": real_bytes.len(),
                    "payloadSha256": sha256,
                    "recordCount": 5,
                    "keyId": key_id
                })),
            ))
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    }
}
