use axum::{
    Json, Router,
    extract::{Path, Query, Request, State, rejection::QueryRejection},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{CreatedWebhook, WebhookInfo, dispatch};

static CREATE_ENDPOINT_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();

pub struct ServerHandle {
    pub port: u16,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl ServerHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}

pub async fn start(pool: SqlitePool, port: u16) -> Result<ServerHandle, String> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|error| format!("could not bind 127.0.0.1:{port}: {error}"));
    let listener = listener?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let router = router(pool);
    let task = tokio::spawn(async move {
        let serve = axum::serve(listener, router).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });
        if let Err(error) = serve.await {
            tracing::error!("[local-api] server failed: {error}");
        }
    });
    tracing::info!("[local-api] listening on http://127.0.0.1:{port}");

    Ok(ServerHandle {
        port,
        shutdown: shutdown_tx,
        task,
    })
}

pub fn router(pool: SqlitePool) -> Router {
    let v1 = Router::new()
        .route("/meetings", get(list_meetings))
        .route("/meetings/{meeting_id}", get(get_meeting))
        .route("/meetings/{meeting_id}/transcript", get(get_transcript))
        .route("/meetings/{meeting_id}/history", get(get_history))
        .route("/meetings/{meeting_id}/export", get(export_meeting))
        .route("/webhooks", get(list_webhooks).post(create_webhook))
        .route("/webhooks/{webhook_id}", delete(delete_webhook))
        .route("/webhooks/{webhook_id}/test", post(test_webhook))
        .route_layer(middleware::from_fn_with_state(
            pool.clone(),
            require_api_key,
        ))
        .with_state(pool.clone());

    Router::new().route("/health", get(health)).nest("/v1", v1)
}

enum ApiError {
    Unauthorized,
    NotFound(String),
    BadRequest(String),
    Busy(String),
    Internal(String),
}

impl From<anlg_agent_access::Error> for ApiError {
    fn from(error: anlg_agent_access::Error) -> Self {
        match error {
            anlg_agent_access::Error::NotFound(what) => Self::NotFound(format!("{what} not found")),
            anlg_agent_access::Error::Database { .. } => Self::Internal(error.to_string()),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Provide a valid API key as `Authorization: Bearer <key>`.".to_string(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "invalid_request", message),
            Self::Busy(message) => (StatusCode::TOO_MANY_REQUESTS, "busy", message),
            Self::Internal(message) => {
                tracing::error!("[local-api] internal error: {message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Something went wrong while handling the request.".to_string(),
                )
            }
        };
        (
            status,
            Json(serde_json::json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

fn parse_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    query
        .map(|Query(value)| value)
        .map_err(|error| ApiError::BadRequest(error.body_text()))
}

async fn require_api_key(
    State(pool): State<SqlitePool>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthorized)?;

    let key_hash = anlg_db_app::hash_api_key(token);
    let key = anlg_db_app::find_active_api_key_by_hash(&pool, &key_hash)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if let Err(error) = anlg_db_app::touch_api_key(&pool, &key.id).await {
        tracing::warn!("[local-api] failed to update key usage: {error}");
    }

    Ok(next.run(request).await)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Debug, Default, Deserialize)]
struct ListMeetingsQuery {
    query: Option<String>,
    series_id: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn list_meetings(
    State(pool): State<SqlitePool>,
    params: Result<Query<ListMeetingsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let params = parse_query(params)?;
    let page = anlg_agent_access::list_meetings(
        &pool,
        anlg_agent_access::ListMeetingsInput {
            query: params.query,
            series_id: params.series_id,
            limit: params.limit,
            offset: params.offset,
        },
    )
    .await?;
    Ok(Json(page).into_response())
}

async fn get_meeting(
    State(pool): State<SqlitePool>,
    Path(meeting_id): Path<String>,
) -> Result<Response, ApiError> {
    let meeting =
        anlg_agent_access::get_meeting(&pool, anlg_agent_access::GetMeetingInput { meeting_id })
            .await?;
    Ok(Json(meeting).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct TranscriptQuery {
    offset: Option<u32>,
    limit: Option<u32>,
}

async fn get_transcript(
    State(pool): State<SqlitePool>,
    Path(meeting_id): Path<String>,
    params: Result<Query<TranscriptQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let params = parse_query(params)?;
    let page = anlg_agent_access::get_meeting_transcript(
        &pool,
        anlg_agent_access::GetMeetingTranscriptInput {
            meeting_id,
            offset: params.offset,
            limit: params.limit,
        },
    )
    .await?;
    Ok(Json(page).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct HistoryQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn get_history(
    State(pool): State<SqlitePool>,
    Path(meeting_id): Path<String>,
    params: Result<Query<HistoryQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let params = parse_query(params)?;
    let page = anlg_agent_access::get_recurring_meeting_history(
        &pool,
        anlg_agent_access::GetRecurringMeetingHistoryInput {
            meeting_id,
            limit: params.limit,
            offset: params.offset,
        },
    )
    .await?;
    Ok(Json(page).into_response())
}

#[derive(Debug, Default, Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

async fn export_meeting(
    State(pool): State<SqlitePool>,
    Path(meeting_id): Path<String>,
    params: Result<Query<ExportQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let params = parse_query(params)?;
    let format = params.format.as_deref().unwrap_or("json");
    let export = anlg_agent_access::get_meeting_export(&pool, meeting_id).await?;
    match format {
        "json" => Ok(Json(export).into_response()),
        "markdown" => Ok((
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            export.to_markdown(),
        )
            .into_response()),
        other => Err(ApiError::BadRequest(format!(
            "unknown format '{other}'; use 'json' or 'markdown'"
        ))),
    }
}

async fn list_webhooks(State(pool): State<SqlitePool>) -> Result<Response, ApiError> {
    let webhooks = anlg_db_app::list_webhook_endpoints(&pool)
        .await?
        .into_iter()
        .map(WebhookInfo::from)
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "webhooks": webhooks })).into_response())
}

#[derive(Debug, Deserialize)]
struct CreateWebhookBody {
    url: String,
    #[serde(default)]
    events: Vec<String>,
}

pub(crate) async fn create_endpoint(
    pool: &SqlitePool,
    url: &str,
    events: &[String],
) -> Result<CreatedWebhook, String> {
    let _guard = CREATE_ENDPOINT_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("url must start with http:// or https://".to_string());
    }
    if let Some(unknown) = events
        .iter()
        .find(|event| !dispatch::KNOWN_EVENTS.contains(&event.as_str()))
    {
        return Err(format!(
            "unknown event '{unknown}'; known events: {}",
            dispatch::KNOWN_EVENTS.join(", ")
        ));
    }
    let endpoint_count = anlg_db_app::list_webhook_endpoints(pool)
        .await
        .map_err(|error| error.to_string())?
        .len();
    if endpoint_count >= dispatch::MAX_WEBHOOK_ENDPOINTS {
        return Err(format!(
            "at most {} webhook endpoints can be configured",
            dispatch::MAX_WEBHOOK_ENDPOINTS
        ));
    }

    let events_json = serde_json::to_string(events).map_err(|error| error.to_string())?;
    let secret = anlg_db_app::generate_webhook_secret();
    let row = anlg_db_app::insert_webhook_endpoint(
        pool,
        &uuid::Uuid::new_v4().to_string(),
        url,
        &secret,
        &events_json,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(CreatedWebhook {
        info: WebhookInfo::from(row),
        secret,
    })
}

async fn create_webhook(
    State(pool): State<SqlitePool>,
    Json(body): Json<CreateWebhookBody>,
) -> Result<Response, ApiError> {
    let created = create_endpoint(&pool, &body.url, &body.events)
        .await
        .map_err(ApiError::BadRequest)?;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

async fn delete_webhook(
    State(pool): State<SqlitePool>,
    Path(webhook_id): Path<String>,
) -> Result<Response, ApiError> {
    if anlg_db_app::delete_webhook_endpoint(&pool, &webhook_id).await? {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(ApiError::NotFound(format!(
            "webhook '{webhook_id}' not found"
        )))
    }
}

async fn test_webhook(
    State(pool): State<SqlitePool>,
    Path(webhook_id): Path<String>,
) -> Result<Response, ApiError> {
    let endpoint = anlg_db_app::get_webhook_endpoint(&pool, &webhook_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("webhook '{webhook_id}' not found")))?;
    let delivery = dispatch::send_test(&pool, &endpoint)
        .await
        .map_err(ApiError::Busy)?;
    Ok(Json(delivery).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_creation_stops_at_the_fanout_limit() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();

        for index in 0..dispatch::MAX_WEBHOOK_ENDPOINTS {
            anlg_db_app::insert_webhook_endpoint(
                db.pool(),
                &format!("webhook-{index}"),
                "https://example.com",
                "whsec_test",
                "[]",
            )
            .await
            .unwrap();
        }

        let error = create_endpoint(db.pool(), "https://example.com", &[])
            .await
            .unwrap_err();

        assert_eq!(
            error,
            format!(
                "at most {} webhook endpoints can be configured",
                dispatch::MAX_WEBHOOK_ENDPOINTS
            )
        );
        assert_eq!(
            anlg_db_app::list_webhook_endpoints(db.pool())
                .await
                .unwrap()
                .len(),
            dispatch::MAX_WEBHOOK_ENDPOINTS
        );
    }
}
