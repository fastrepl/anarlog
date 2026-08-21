use std::{sync::Arc, time::Duration};

use anlg_session_ingest::{AcknowledgeRequest, AcknowledgeResponse, DeliveryPage, SessionRead};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

use crate::{
    auth::{AuthenticationError, WorkspaceAuthenticator},
    capture::{
        AppendCaptureEventRequest, CaptureJob, CaptureJobCheckpoint, CaptureJobLease,
        CaptureJobStatus, ClaimCaptureJobRequest, CreateCaptureJobRequest, ProjectionPublication,
        RenewCaptureJobLeaseRequest,
    },
    license::License,
    schedule::{CalendarEventInput, CapturePolicy, ScheduledCapture},
    store::{ControlPlaneStore, StoreError},
    zoom::{ZoomDispatchError, ZoomWebhookError, ZoomWebhookOutcome, ZoomWebhookService},
};

const MAX_REQUEST_BYTES: usize = 256 * 1024;
const DEFAULT_PAGE_LIMIT: u16 = 10;
const MAX_PAGE_LIMIT: u16 = 100;
const CAPTURE_LEASE_DURATION: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct AppState {
    store: Arc<dyn ControlPlaneStore>,
    authenticator: Arc<dyn WorkspaceAuthenticator>,
    zoom: Option<Arc<ZoomWebhookService>>,
    license: Option<License>,
}

impl AppState {
    pub fn new(
        store: Arc<dyn ControlPlaneStore>,
        authenticator: Arc<dyn WorkspaceAuthenticator>,
    ) -> Self {
        Self {
            store,
            authenticator,
            zoom: None,
            license: None,
        }
    }

    pub fn with_zoom(mut self, zoom: Arc<ZoomWebhookService>) -> Self {
        self.zoom = Some(zoom);
        self
    }

    pub fn with_license(mut self, license: License) -> Self {
        self.license = Some(license);
        self
    }
}

pub fn router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .route(
            "/v1/workspaces/{workspace_id}/capture-jobs/{job_id}",
            post(create_capture_job).get(read_capture_checkpoint),
        )
        .route(
            "/v1/workspaces/{workspace_id}/capture-jobs/{job_id}/events",
            post(append_capture_event),
        )
        .route(
            "/v1/workspaces/{workspace_id}/capture-jobs/{job_id}/claim",
            post(claim_capture_job),
        )
        .route(
            "/v1/workspaces/{workspace_id}/capture-jobs/{job_id}/lease",
            post(renew_capture_job_lease),
        )
        .route(
            "/v1/workspaces/{workspace_id}/session-envelopes",
            get(list_deliveries),
        )
        .route(
            "/v1/workspaces/{workspace_id}/session-envelopes/{job_id}/ack",
            post(acknowledge),
        )
        .route(
            "/v1/workspaces/{workspace_id}/sessions/{job_id}",
            get(read_session),
        )
        .route(
            "/v1/workspaces/{workspace_id}/capture-policy",
            get(read_capture_policy).put(write_capture_policy),
        )
        .route(
            "/v1/workspaces/{workspace_id}/calendar-events",
            put(upsert_calendar_events),
        )
        .route(
            "/v1/workspaces/{workspace_id}/scheduled-captures",
            get(list_scheduled_captures).post(dispatch_scheduled_captures),
        )
        .route(
            "/v1/workspaces/{workspace_id}/scheduled-captures/{calendar_event_id}",
            delete(cancel_scheduled_capture),
        );
    if state.zoom.is_some() {
        router = router.route("/webhooks/zoom", post(zoom_webhook));
    }
    router
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn zoom_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let service = state.zoom.as_ref().ok_or_else(ApiError::not_found)?;
    let timestamp = headers
        .get("x-zm-request-timestamp")
        .and_then(|value| value.to_str().ok());
    let signature = headers
        .get("x-zm-signature")
        .and_then(|value| value.to_str().ok());
    match service
        .handle(timestamp, signature, body.as_ref())
        .await
        .map_err(ApiError::from_zoom)?
    {
        ZoomWebhookOutcome::Validation(response) => Ok(Json(response).into_response()),
        ZoomWebhookOutcome::Accepted(outcome) => Ok((
            StatusCode::OK,
            Json(ZoomWebhookAccepted {
                status: if outcome.started {
                    "started"
                } else {
                    "already_running"
                },
                job_id: outcome.job_id,
            }),
        )
            .into_response()),
        ZoomWebhookOutcome::Ignored => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

async fn read_capture_checkpoint(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<CaptureJobCheckpoint>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    let checkpoint = state
        .store
        .read_capture_checkpoint(&workspace_id, &job_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(checkpoint))
}

async fn create_capture_job(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<CreateCaptureJobRequest>,
) -> Result<(StatusCode, Json<CaptureJobStatus>), ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    validate_identifier(&request.bot_id, "bot_id")?;
    validate_identifier(&request.owner_user_id, "owner_user_id")?;
    validate_identifier(&request.requesting_actor_id, "requesting_actor_id")?;
    validate_identifier(&request.session_id, "session_id")?;
    if request.session_title.trim().is_empty() || request.session_title.len() > 1024 {
        return Err(ApiError::bad_request(
            "invalid_session_title",
            "sessionTitle must be between 1 and 1024 bytes",
        ));
    }
    let status = state
        .store
        .create_capture_job(&CaptureJob {
            workspace_id,
            job_id,
            bot_id: request.bot_id,
            owner_user_id: request.owner_user_id,
            requesting_actor_id: request.requesting_actor_id,
            session_id: request.session_id,
            session_title: request.session_title,
            provider: request.provider,
            meeting: request.meeting,
            created_at: request.created_at,
        })
        .await
        .map_err(ApiError::from_store)?;
    let response_status = if status.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((response_status, Json(status)))
}

async fn claim_capture_job(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<ClaimCaptureJobRequest>,
) -> Result<Json<CaptureJobLease>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    validate_identifier(&request.worker_id, "worker_id")?;
    validate_identifier(&request.lease_id, "lease_id")?;
    let lease = state
        .store
        .claim_capture_job(
            &workspace_id,
            &job_id,
            &request.worker_id,
            &request.lease_id,
            CAPTURE_LEASE_DURATION,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(lease))
}

async fn renew_capture_job_lease(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RenewCaptureJobLeaseRequest>,
) -> Result<Json<CaptureJobLease>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    validate_identifier(&request.lease.worker_id, "worker_id")?;
    validate_identifier(&request.lease.lease_id, "lease_id")?;
    if request.lease.epoch == 0 {
        return Err(ApiError::bad_request(
            "invalid_lease_epoch",
            "lease epoch must be greater than zero",
        ));
    }
    let lease = state
        .store
        .renew_capture_job_lease(
            &workspace_id,
            &job_id,
            &request.lease,
            CAPTURE_LEASE_DURATION,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(lease))
}

async fn append_capture_event(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AppendCaptureEventRequest>,
) -> Result<Json<ProjectionPublication>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    validate_identifier(&request.lease.worker_id, "worker_id")?;
    validate_identifier(&request.lease.lease_id, "lease_id")?;
    if request.lease.epoch == 0 {
        return Err(ApiError::bad_request(
            "invalid_lease_epoch",
            "lease epoch must be greater than zero",
        ));
    }
    validate_identifier(&request.event.id, "event_id")?;
    validate_identifier(&request.event.bot_id, "bot_id")?;
    let publication = state
        .store
        .append_capture_event(&workspace_id, &job_id, &request.lease, &request.event)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(publication))
}

async fn liveness() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn readiness(State(state): State<AppState>) -> Response {
    match state.store.readiness().await {
        Ok(()) => Json(HealthResponse { status: "ready" }).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "database readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "not_ready",
                }),
            )
                .into_response()
        }
    }
}

async fn list_deliveries(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Result<Json<DeliveryPage>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&query.consumer_id, "consumerId")?;
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(ApiError::bad_request(
            "invalid_limit",
            "limit must be between 1 and 100",
        ));
    }
    let page = state
        .store
        .list_deliveries(
            &workspace_id,
            &query.consumer_id,
            query.after.unwrap_or(0),
            limit,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(page))
}

async fn acknowledge(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AcknowledgeRequest>,
) -> Result<Json<AcknowledgeResponse>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    validate_identifier(&request.consumer_id, "consumerId")?;
    if request.revision == 0 {
        return Err(ApiError::bad_request(
            "invalid_revision",
            "revision must be greater than zero",
        ));
    }
    if request.content_hash.len() != 64
        || !request
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ApiError::bad_request(
            "invalid_content_hash",
            "contentHash must be a lowercase SHA-256 digest",
        ));
    }
    state
        .store
        .acknowledge(&workspace_id, &job_id, &request)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(AcknowledgeResponse { acknowledged: true }))
}

async fn read_session(
    State(state): State<AppState>,
    Path((workspace_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SessionRead>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    validate_identifier(&job_id, "job_id")?;
    let session = state
        .store
        .read_session(&workspace_id, &job_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(session))
}

async fn read_capture_policy(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CapturePolicy>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    let policy = state
        .store
        .get_capture_policy(&workspace_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(policy))
}

async fn write_capture_policy(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    Json(mut policy): Json<CapturePolicy>,
) -> Result<Json<CapturePolicy>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    policy.workspace_id = workspace_id;
    let policy = state
        .store
        .upsert_capture_policy(&policy)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(policy))
}

async fn upsert_calendar_events(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    Json(events): Json<Vec<CalendarEventInput>>,
) -> Result<Json<Vec<ScheduledCapture>>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    if events.len() > 500 {
        return Err(ApiError::bad_request(
            "invalid_calendar_events",
            "calendar event batches are limited to 500 events",
        ));
    }
    let scheduled = state
        .store
        .upsert_calendar_events(&workspace_id, &events)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(scheduled))
}

async fn list_scheduled_captures(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ScheduledCapture>>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    let scheduled = state
        .store
        .list_scheduled_captures(&workspace_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(scheduled))
}

async fn cancel_scheduled_capture(
    State(state): State<AppState>,
    Path((workspace_id, calendar_event_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ScheduledCapture>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    if calendar_event_id.is_empty() || calendar_event_id.len() > 512 {
        return Err(ApiError::bad_request(
            "invalid_calendar_event_id",
            "calendarEventId must contain 1-512 bytes",
        ));
    }
    let scheduled = state
        .store
        .cancel_scheduled_capture(&workspace_id, &calendar_event_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(scheduled))
}

async fn dispatch_scheduled_captures(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<CaptureJobStatus>>, ApiError> {
    authorize(&state, &headers, &workspace_id)?;
    validate_identifier(&workspace_id, "workspace_id")?;
    let dispatched = state
        .store
        .dispatch_due_scheduled_captures(chrono::Utc::now())
        .await
        .map_err(ApiError::from_store)?;
    Ok(Json(
        dispatched
            .into_iter()
            .filter(|job| job.job_id.starts_with("cal-"))
            .collect(),
    ))
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    requested_workspace_id: &str,
) -> Result<(), ApiError> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let authenticated =
        state
            .authenticator
            .authenticate(authorization)
            .map_err(|error| match error {
                AuthenticationError::InvalidCredentials => ApiError::unauthorized(),
                AuthenticationError::NoCredentials | AuthenticationError::DuplicateToken => {
                    tracing::error!(error = %error, "workspace authenticator is invalid");
                    ApiError::internal()
                }
            })?;
    if authenticated.workspace_id.as_ref() != requested_workspace_id {
        return Err(ApiError::forbidden());
    }
    if state
        .license
        .as_ref()
        .is_some_and(|license| !license.authorizes_workspace(requested_workspace_id))
    {
        return Err(ApiError::forbidden());
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), ApiError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(ApiError::bad_request(
            "invalid_identifier",
            match field {
                "consumerId" => "consumerId contains unsupported characters",
                "job_id" => "job_id contains unsupported characters",
                "worker_id" => "workerId contains unsupported characters",
                "lease_id" => "leaseId contains unsupported characters",
                _ => "workspace_id contains unsupported characters",
            },
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListQuery {
    consumer_id: String,
    after: Option<u64>,
    limit: Option<u16>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ZoomWebhookAccepted {
    status: &'static str,
    job_id: String,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    authenticate: bool,
}

impl ApiError {
    fn bad_request(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message,
            authenticate: false,
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "valid bearer credentials are required",
            authenticate: true,
        }
    }

    fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "workspace_forbidden",
            message: "credentials do not authorize this workspace",
            authenticate: false,
        }
    }

    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "the capture service could not complete the request",
            authenticate: false,
        }
    }

    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "the requested route was not found",
            authenticate: false,
        }
    }

    fn from_zoom(error: ZoomWebhookError) -> Self {
        match error {
            ZoomWebhookError::MissingHeaders | ZoomWebhookError::Verification(_) => Self {
                status: StatusCode::UNAUTHORIZED,
                code: "invalid_zoom_signature",
                message: "the Zoom webhook signature is invalid",
                authenticate: false,
            },
            ZoomWebhookError::Protocol(_) => Self::bad_request(
                "invalid_zoom_webhook",
                "the Zoom webhook payload is invalid",
            ),
            ZoomWebhookError::Dispatch(ZoomDispatchError::NotFound) => Self::not_found(),
            ZoomWebhookError::Dispatch(ZoomDispatchError::Unavailable(error)) => {
                tracing::error!(error = %error, "Zoom webhook dispatch failed");
                Self {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "zoom_dispatch_unavailable",
                    message: "the Zoom capture worker could not accept the event",
                    authenticate: false,
                }
            }
            ZoomWebhookError::InvalidSystemClock => {
                tracing::error!("system clock is before Unix epoch");
                Self::internal()
            }
        }
    }

    fn from_store(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: "the requested delivery was not found",
                authenticate: false,
            },
            StoreError::RevisionConflict => Self {
                status: StatusCode::CONFLICT,
                code: "revision_conflict",
                message: "revision or contentHash does not match the delivery",
                authenticate: false,
            },
            StoreError::CaptureJobConflict => Self {
                status: StatusCode::CONFLICT,
                code: "capture_job_conflict",
                message: "capture job already exists with different immutable fields",
                authenticate: false,
            },
            StoreError::CaptureBotConflict => Self {
                status: StatusCode::CONFLICT,
                code: "capture_bot_conflict",
                message: "capture bot is already assigned to a different job",
                authenticate: false,
            },
            StoreError::CaptureExternalIdConflict => Self {
                status: StatusCode::CONFLICT,
                code: "capture_external_id_conflict",
                message: "capture provider identity matches more than one queued job",
                authenticate: false,
            },
            StoreError::CaptureEventConflict | StoreError::CaptureSequenceConflict { .. } => Self {
                status: StatusCode::CONFLICT,
                code: "capture_event_conflict",
                message: "capture event id, sequence, or content conflicts with persisted data",
                authenticate: false,
            },
            StoreError::InvalidCaptureEvent(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_capture_event",
                message: "capture event violates the normalized capture contract",
                authenticate: false,
            },
            StoreError::CaptureJobTerminal => Self {
                status: StatusCode::CONFLICT,
                code: "capture_job_terminal",
                message: "capture job is already terminal",
                authenticate: false,
            },
            StoreError::CaptureLeaseUnavailable => Self {
                status: StatusCode::CONFLICT,
                code: "capture_lease_unavailable",
                message: "capture job already has an active worker lease",
                authenticate: false,
            },
            StoreError::CaptureLeaseLost => Self {
                status: StatusCode::CONFLICT,
                code: "capture_lease_lost",
                message: "capture worker lease is expired or no longer current",
                authenticate: false,
            },
            error => {
                tracing::error!(error = %error, "delivery store request failed");
                Self::internal()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        if self.authenticate {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                http::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}
