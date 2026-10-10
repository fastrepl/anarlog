use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptRequest {
    mutation_id: String,
    base_sequence: u64,
    initialize: bool,
    events: Vec<E2eeWitnessEvent>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    sequence: u64,
    record_id: String,
    payload_hash: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptResponse {
    initialized_at: String,
    head_sequence: u64,
    cloud_authority_after: u64,
    mutation_id: String,
    receipts: Vec<Receipt>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedPage {
    #[serde(flatten)]
    page: E2eeWitnessPage,
    cloud_authority_after: Option<u64>,
}

#[derive(Deserialize)]
struct PageRow {
    #[serde(flatten)]
    page: ReadRpcRow,
    cloud_authority_after: Option<u64>,
}

#[derive(Deserialize)]
struct AcceptRow {
    initialized_at: String,
    head_sequence: u64,
    cloud_authority_after: u64,
    mutation_id: String,
    receipts: Vec<Receipt>,
}

pub(super) fn router() -> Router<ReplicaState> {
    Router::new().route(
        "/e2ee/witness/{workspace_id}/accepted",
        get(read).post(accept),
    )
}

#[utoipa::path(get, path = "/e2ee/witness/{workspace_id}/accepted", tag = "sync", operation_id = "read_accepted_e2ee_replica",
    params(("workspace_id" = String, Path), ("afterSequence" = Option<u64>, Query), ("throughSequence" = Option<u64>, Query)),
    responses((status = 200, description = "Accepted cloud state and legacy cutover boundary", body = AcceptedPage)))]
pub(super) async fn read(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ReadE2eeWitnessQuery>,
) -> Result<([(header::HeaderName, HeaderValue); 1], Json<AcceptedPage>)> {
    require_witness_workspace(&auth, &workspace_id)?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/read_e2ee_replica_page",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&ReadRpcRequest {
            p_actor_user_id: &auth.claims.sub,
            p_workspace_id: &workspace_id,
            p_after_sequence: i64::try_from(query.after_sequence)
                .map_err(|_| SyncError::BadRequest("Invalid cursor".into()))?,
            p_through_sequence: query
                .through_sequence
                .map(i64::try_from)
                .transpose()
                .map_err(|_| SyncError::BadRequest("Invalid cursor".into()))?,
            p_limit: WITNESS_PAGE_SIZE,
            p_max_bytes: MAX_WITNESS_PAGE_BYTES,
        })
        .send()
        .await
        .map_err(|e| witness_transport_error(e, "accepted read"))?;
    let (status, bytes) = read_bounded_response(response, "accepted read").await?;
    if !status.is_success() {
        return Err(map_postgrest_error(status, &bytes));
    }
    let rows: Vec<PageRow> =
        serde_json::from_slice(&bytes).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    let boundary = rows
        .first()
        .ok_or(SyncError::E2eeWitnessServiceUnavailable)?
        .cloud_authority_after;
    if rows.iter().any(|r| {
        r.cloud_authority_after != boundary
            || boundary.is_some_and(|b| b > r.page.head_sequence as u64)
    }) {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }
    let page = validate_read_rows(
        rows.into_iter().map(|r| r.page).collect(),
        query.after_sequence,
        query.through_sequence,
    )?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(AcceptedPage {
            page,
            cloud_authority_after: boundary,
        }),
    ))
}

#[utoipa::path(post, path = "/e2ee/witness/{workspace_id}/accepted", tag = "sync", operation_id = "accept_e2ee_replica_batch",
    params(("workspace_id" = String, Path)), request_body = AcceptRequest,
    responses((status = 200, description = "Atomic acceptance receipt", body = AcceptResponse),
              (status = 409, description = "Cloud base changed; pull and rebase"),
              (status = 503, description = "Acceptance busy; retry after the Retry-After delay")))]
pub(super) async fn accept(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AcceptRequest>,
) -> Result<([(header::HeaderName, HeaderValue); 1], Json<AcceptResponse>)> {
    require_witness_workspace(&auth, &workspace_id)?;
    if Uuid::parse_str(&request.mutation_id).is_err() {
        return Err(SyncError::BadRequest("Invalid mutation identity".into()));
    }
    let _permit = state
        .witness_writes
        .try_acquire(&workspace_id)
        .ok_or(SyncError::E2eeWitnessBusy)?;
    let legacy = PublishE2eeWitnessRequest {
        initialize: request.initialize,
        events: request.events,
    };
    if legacy.events.is_empty() || legacy.events.len() > 1024 {
        return Err(SyncError::BadRequest("Invalid replica batch size".into()));
    }
    for chunk in legacy.events.chunks(MAX_EVENTS_PER_BATCH) {
        validate_publish_request(&PublishE2eeWitnessRequest {
            initialize: legacy.initialize,
            events: chunk.to_vec(),
        })?;
    }
    let events = legacy
        .events
        .iter()
        .map(|event| PublishRpcEvent {
            record_id: &event.record_id,
            payload_hash: &event.payload_hash,
            payload: &event.payload,
        })
        .collect::<Vec<_>>();
    let base = i64::try_from(request.base_sequence)
        .map_err(|_| SyncError::BadRequest("Invalid base".into()))?;
    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/accept_e2ee_replica_batch",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WITNESS_TIMEOUT)
        .json(&serde_json::json!({
            "p_actor_user_id": auth.claims.sub, "p_workspace_id": workspace_id,
            "p_mutation_id": request.mutation_id, "p_base_sequence": base,
            "p_initialize": request.initialize, "p_events": events,
        }))
        .send()
        .await
        .map_err(|e| witness_transport_error(e, "acceptance"))?;
    let (status, bytes) = read_bounded_response(response, "acceptance").await?;
    if !status.is_success() {
        if serde_json::from_slice::<PostgrestError>(&bytes).is_ok_and(|e| e.code == "40001") {
            return Err(SyncError::E2eeReplicaBaseChanged);
        }
        return Err(map_postgrest_error(status, &bytes));
    }
    let mut rows: Vec<AcceptRow> =
        serde_json::from_slice(&bytes).map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?;
    if rows.len() != 1 {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }
    let row = rows.pop().unwrap();
    if row.mutation_id != request.mutation_id
        || row.head_sequence <= request.base_sequence
        || row.cloud_authority_after >= row.head_sequence
        || row.receipts.len() != events.len()
        || row
            .receipts
            .windows(2)
            .any(|p| p[0].sequence >= p[1].sequence)
        || row
            .receipts
            .last()
            .is_none_or(|r| r.sequence != row.head_sequence)
        || events.iter().any(|e| {
            row.receipts
                .iter()
                .filter(|r| {
                    r.record_id == e.record_id
                        && r.payload_hash == e.payload_hash
                        && r.sequence > request.base_sequence
                })
                .count()
                != 1
        })
    {
        return Err(SyncError::E2eeWitnessServiceUnavailable);
    }
    let initialized_at = chrono::DateTime::parse_from_rfc3339(&row.initialized_at)
        .map_err(|_| SyncError::E2eeWitnessServiceUnavailable)?
        .to_rfc3339();
    state.witness_wakes.notify(&workspace_id);
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(AcceptResponse {
            initialized_at,
            head_sequence: row.head_sequence,
            cloud_authority_after: row.cloud_authority_after,
            mutation_id: row.mutation_id,
            receipts: row.receipts,
        }),
    ))
}
