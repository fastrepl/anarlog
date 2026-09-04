use anlg_agent_access as access;
use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, put},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    CloudApiError,
    mcp::mcp_service,
    state::{AppState, StoreError},
};

const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SNAPSHOT_REQUEST_BYTES: usize = MAX_SNAPSHOT_BYTES + 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CloudApiSettings {
    pub enabled: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSettingsBody {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedApiKey {
    #[serde(flatten)]
    pub info: ApiKeyInfo,
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyBody {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SnapshotReceipt {
    pub session_id: String,
    pub revision: i64,
    pub published_at: String,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ListMeetingsQuery {
    pub(crate) query: Option<String>,
    pub(crate) series_id: Option<String>,
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct TranscriptQuery {
    pub(crate) offset: Option<u32>,
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct HistoryQuery {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ExportQuery {
    format: Option<String>,
}

pub fn management_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/v1/cloud-api/settings",
            get(get_settings).put(update_settings),
        )
        .route("/v1/cloud-api/keys", get(list_keys).post(create_key))
        .route("/v1/cloud-api/keys/{key_id}", delete(revoke_key))
        .route(
            "/v1/sync-snapshots/{session_id}",
            put(publish_snapshot).delete(delete_snapshot),
        )
        .layer(DefaultBodyLimit::max(MAX_SNAPSHOT_REQUEST_BYTES))
        .with_state(state)
}

pub fn connector_router(state: AppState) -> Router {
    let mcp = mcp_service(state.clone());
    let v1 = Router::new()
        .route("/meetings", get(list_meetings))
        .route("/meetings/{meeting_id}", get(get_meeting))
        .route("/meetings/{meeting_id}/transcript", get(get_transcript))
        .route("/meetings/{meeting_id}/history", get(get_history))
        .route("/meetings/{meeting_id}/export", get(export_meeting));

    Router::new()
        .nest("/v1", v1)
        .nest_service("/mcp", mcp)
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/v1/cloud-api/settings",
    tag = "cloud-api",
    responses(
        (status = 200, body = CloudApiSettings),
        (status = 403, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn get_settings(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<CloudApiSettings>, CloudApiError> {
    state
        .get_settings(&auth.claims.sub)
        .await
        .map(Json)
        .map_err(map_store_error)
}

#[utoipa::path(
    put,
    path = "/v1/cloud-api/settings",
    tag = "cloud-api",
    request_body = UpdateSettingsBody,
    responses(
        (status = 200, body = CloudApiSettings),
        (status = 403, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn update_settings(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<UpdateSettingsBody>,
) -> Result<Json<CloudApiSettings>, CloudApiError> {
    state
        .set_enabled(&auth.claims.sub, body.enabled)
        .await
        .map(Json)
        .map_err(map_store_error)
}

#[utoipa::path(
    get,
    path = "/v1/cloud-api/keys",
    tag = "cloud-api",
    responses(
        (status = 200, body = [ApiKeyInfo]),
        (status = 403, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn list_keys(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKeyInfo>>, CloudApiError> {
    state
        .list_keys(&auth.claims.sub)
        .await
        .map(Json)
        .map_err(map_store_error)
}

#[utoipa::path(
    post,
    path = "/v1/cloud-api/keys",
    tag = "cloud-api",
    request_body = CreateApiKeyBody,
    responses(
        (status = 201, body = CreatedApiKey),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn create_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateApiKeyBody>,
) -> Result<(StatusCode, Json<CreatedApiKey>), CloudApiError> {
    let name = body.name.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(CloudApiError::InvalidRequest(
            "key name must be between 1 and 128 bytes".to_string(),
        ));
    }
    let key = format!("anl_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let key_hash = Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let key_prefix = &key[..12];
    let info = state
        .create_key(
            &auth.claims.sub,
            &Uuid::new_v4().to_string(),
            name,
            &key_hash,
            key_prefix,
        )
        .await
        .map_err(map_store_error)?;

    Ok((StatusCode::CREATED, Json(CreatedApiKey { info, key })))
}

#[utoipa::path(
    delete,
    path = "/v1/cloud-api/keys/{key_id}",
    tag = "cloud-api",
    params(("key_id" = String, Path, description = "Cloud API key id")),
    responses(
        (status = 204),
        (status = 404, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn revoke_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, CloudApiError> {
    if Uuid::parse_str(&key_id).is_err() {
        return Err(CloudApiError::NotFound(format!(
            "API key '{key_id}' not found"
        )));
    }
    if state
        .revoke_key(&auth.claims.sub, &key_id)
        .await
        .map_err(map_store_error)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(CloudApiError::NotFound(format!(
            "API key '{key_id}' not found"
        )))
    }
}

#[utoipa::path(
    put,
    path = "/v1/sync-snapshots/{session_id}",
    tag = "cloud-api",
    params(("session_id" = String, Path, description = "Anarlog meeting id")),
    request_body = access::MeetingExport,
    responses(
        (status = 200, body = SnapshotReceipt),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)
    )
)]
pub(crate) async fn publish_snapshot(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
    Json(export): Json<access::MeetingExport>,
) -> Result<Json<SnapshotReceipt>, CloudApiError> {
    let export = sanitize_export(export)?;
    if export.meeting.id != session_id {
        return Err(CloudApiError::InvalidRequest(
            "snapshot meeting id must match the path".to_string(),
        ));
    }
    state
        .publish_snapshot(&auth.claims.sub, &session_id, &export)
        .await
        .map(Json)
        .map_err(map_store_error)
}

#[utoipa::path(
    delete,
    path = "/v1/sync-snapshots/{session_id}",
    tag = "cloud-api",
    params(("session_id" = String, Path, description = "Anarlog meeting id")),
    responses((status = 204))
)]
pub(crate) async fn delete_snapshot(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, CloudApiError> {
    state
        .delete_snapshot(&auth.claims.sub, &session_id)
        .await
        .map_err(map_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/v1/meetings",
    tag = "cloud-api",
    params(
        ("query" = Option<String>, Query),
        ("series_id" = Option<String>, Query),
        ("limit" = Option<u32>, Query),
        ("offset" = Option<u32>, Query)
    ),
    responses(
        (status = 200, body = access::MeetingPage),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 429, body = String, description = "Cloud API rate limit exceeded")
    )
)]
pub(crate) async fn list_meetings(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListMeetingsQuery>,
) -> Result<Json<access::MeetingPage>, CloudApiError> {
    list_meetings_for_user(&state, &auth.claims.sub, params)
        .await
        .map(Json)
}

pub(crate) async fn list_meetings_for_user(
    state: &AppState,
    user_id: &str,
    params: ListMeetingsQuery,
) -> Result<access::MeetingPage, CloudApiError> {
    let limit = params
        .limit
        .unwrap_or(access::DEFAULT_LIST_LIMIT)
        .clamp(1, access::MAX_LIST_LIMIT);
    let offset = params.offset.unwrap_or(0);
    let mut exports = state
        .list_snapshots(
            user_id,
            params.query.as_deref(),
            params.series_id.as_deref(),
            limit + 1,
            offset,
        )
        .await
        .map_err(map_store_error)?;
    let has_more = exports.len() > limit as usize;
    exports.truncate(limit as usize);
    let meetings = exports
        .iter()
        .map(|export| access::MeetingListItem::from(&export.meeting))
        .collect::<Vec<_>>();

    Ok(access::MeetingPage {
        pagination: access::Pagination {
            offset,
            limit,
            returned: meetings.len(),
            total: None,
            next_offset: has_more.then_some(offset.saturating_add(limit)),
        },
        meetings,
    })
}

#[utoipa::path(
    get,
    path = "/v1/meetings/{meeting_id}",
    tag = "cloud-api",
    params(("meeting_id" = String, Path)),
    responses(
        (status = 200, body = access::Meeting),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = String, description = "Cloud API rate limit exceeded")
    )
)]
pub(crate) async fn get_meeting(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(meeting_id): Path<String>,
) -> Result<Json<access::Meeting>, CloudApiError> {
    read_export(&state, &auth.claims.sub, &meeting_id)
        .await
        .map(|export| Json(export.meeting))
}

#[utoipa::path(
    get,
    path = "/v1/meetings/{meeting_id}/transcript",
    tag = "cloud-api",
    params(
        ("meeting_id" = String, Path),
        ("offset" = Option<u32>, Query),
        ("limit" = Option<u32>, Query)
    ),
    responses(
        (status = 200, body = access::TranscriptPage),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = String, description = "Cloud API rate limit exceeded")
    )
)]
pub(crate) async fn get_transcript(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(meeting_id): Path<String>,
    Query(params): Query<TranscriptQuery>,
) -> Result<Json<access::TranscriptPage>, CloudApiError> {
    let export = read_export(&state, &auth.claims.sub, &meeting_id).await?;
    Ok(Json(access::paginate_transcripts(
        &meeting_id,
        &export.transcripts,
        params.offset.unwrap_or(0),
        params.limit.unwrap_or(access::DEFAULT_TRANSCRIPT_LIMIT),
    )))
}

#[utoipa::path(
    get,
    path = "/v1/meetings/{meeting_id}/history",
    tag = "cloud-api",
    params(
        ("meeting_id" = String, Path),
        ("limit" = Option<u32>, Query),
        ("offset" = Option<u32>, Query)
    ),
    responses(
        (status = 200, body = access::MeetingPage),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = String, description = "Cloud API rate limit exceeded")
    )
)]
pub(crate) async fn get_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(meeting_id): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<access::MeetingPage>, CloudApiError> {
    history_for_user(&state, &auth.claims.sub, meeting_id, params)
        .await
        .map(Json)
}

pub(crate) async fn history_for_user(
    state: &AppState,
    user_id: &str,
    meeting_id: String,
    params: HistoryQuery,
) -> Result<access::MeetingPage, CloudApiError> {
    let export = read_export(state, user_id, &meeting_id).await?;
    let limit = params
        .limit
        .unwrap_or(access::DEFAULT_LIST_LIMIT)
        .clamp(1, access::MAX_LIST_LIMIT);
    let offset = params.offset.unwrap_or(0);
    if export.meeting.series_id.trim().is_empty() {
        return Ok(access::MeetingPage {
            meetings: Vec::new(),
            pagination: access::Pagination {
                offset,
                limit,
                returned: 0,
                total: Some(0),
                next_offset: None,
            },
        });
    }
    list_meetings_for_user(
        state,
        user_id,
        ListMeetingsQuery {
            query: None,
            series_id: Some(export.meeting.series_id),
            limit: Some(limit),
            offset: Some(offset),
        },
    )
    .await
}

#[utoipa::path(
    get,
    path = "/v1/meetings/{meeting_id}/export",
    tag = "cloud-api",
    params(
        ("meeting_id" = String, Path),
        ("format" = Option<String>, Query, description = "json or markdown")
    ),
    responses(
        (status = 200, description = "Meeting export"),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 429, body = String, description = "Cloud API rate limit exceeded")
    )
)]
pub(crate) async fn export_meeting(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(meeting_id): Path<String>,
    Query(params): Query<ExportQuery>,
) -> Result<Response, CloudApiError> {
    let export = read_export(&state, &auth.claims.sub, &meeting_id).await?;
    match params.format.as_deref().unwrap_or("json") {
        "json" => Ok(Json(export).into_response()),
        "markdown" => Ok((
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            export.to_markdown(),
        )
            .into_response()),
        other => Err(CloudApiError::InvalidRequest(format!(
            "unknown format '{other}'; use 'json' or 'markdown'"
        ))),
    }
}

pub(crate) async fn read_export(
    state: &AppState,
    user_id: &str,
    meeting_id: &str,
) -> Result<access::MeetingExport, CloudApiError> {
    state
        .read_snapshot(user_id, meeting_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| CloudApiError::NotFound(format!("meeting '{meeting_id}' not found")))
}

fn sanitize_export(export: access::MeetingExport) -> Result<access::MeetingExport, CloudApiError> {
    let mut value = serde_json::to_value(export)
        .map_err(|error| CloudApiError::InvalidRequest(error.to_string()))?;
    sanitize_value(&mut value);
    let mut export: access::MeetingExport = serde_json::from_value(value)
        .map_err(|error| CloudApiError::InvalidRequest(error.to_string()))?;
    export.meeting.title = export.meeting.title.trim().to_string();
    let bytes = serde_json::to_vec(&export)
        .map_err(|error| CloudApiError::InvalidRequest(error.to_string()))?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(CloudApiError::InvalidRequest(format!(
            "snapshot exceeds the {MAX_SNAPSHOT_BYTES}-byte limit"
        )));
    }
    Ok(export)
}

fn sanitize_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => {
            text.retain(|character| {
                !character.is_control() || matches!(character, '\n' | '\r' | '\t')
            });
        }
        serde_json::Value::Array(values) => {
            for value in values {
                sanitize_value(value);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["path", "file_path", "audio_path"] {
                object.remove(key);
            }
            for value in object.values_mut() {
                sanitize_value(value);
            }
        }
        _ => {}
    }
}

fn map_store_error(error: StoreError) -> CloudApiError {
    match &error {
        StoreError::Response { body, .. } if body.contains("hyprnote pro entitlement required") => {
            CloudApiError::SubscriptionRequired
        }
        StoreError::Response { body, .. } if body.contains("cloud API not enabled") => {
            CloudApiError::NotEnabled
        }
        StoreError::Response { body, .. }
            if body.contains("invalid cloud API") || body.contains("violates check constraint") =>
        {
            CloudApiError::InvalidRequest("invalid cloud API request".to_string())
        }
        _ => CloudApiError::Internal(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
        middleware,
    };
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, method, path},
    };

    const TEST_KEY_ID: &str = "cloud-oauth-test";
    const TEST_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt\n\
kHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L\n\
950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+\n\
-----END PRIVATE KEY-----";

    fn oauth_token(issuer: &str, scope: &str) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(TEST_KEY_ID.to_string());
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3_600;
        encode(
            &header,
            &serde_json::json!({
                "sub": "00000000-0000-0000-0000-000000000001",
                "iss": issuer,
                "aud": ["https://api.anarlog.so/mcp"],
                "exp": expires_at,
                "client_id": "chatgpt-client",
                "scope": scope,
            }),
            &EncodingKey::from_ec_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    fn session_token(issuer: &str) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(TEST_KEY_ID.to_string());
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3_600;
        encode(
            &header,
            &serde_json::json!({
                "sub": "00000000-0000-0000-0000-000000000001",
                "iss": issuer,
                "aud": "authenticated",
                "exp": expires_at,
            }),
            &EncodingKey::from_ec_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    fn export() -> access::MeetingExport {
        access::MeetingExport {
            meeting: access::Meeting {
                id: "meeting-1".to_string(),
                title: " Planning \0".to_string(),
                kind: "meeting".to_string(),
                status: "completed".to_string(),
                created_at: "2026-07-28".to_string(),
                updated_at: "2026-07-28".to_string(),
                started_at: "2026-07-28".to_string(),
                ended_at: String::new(),
                timezone: "Asia/Seoul".to_string(),
                language: "en".to_string(),
                series_id: String::new(),
                note: None,
                summaries: Vec::new(),
                participants: Vec::new(),
                action_items: Vec::new(),
            },
            transcripts: vec![access::Transcript {
                id: "transcript-1".to_string(),
                source: "mic".to_string(),
                provider: String::new(),
                model: String::new(),
                language: "en".to_string(),
                started_at_ms: 0,
                ended_at_ms: None,
                memo: String::new(),
                text: "hello".to_string(),
                words: vec![serde_json::json!({
                    "text": "hello",
                    "audio_path": "/Users/example/meeting.wav",
                })],
                speaker_hints: Vec::new(),
            }],
        }
    }

    #[test]
    fn snapshot_sanitizer_removes_control_characters_and_local_paths() {
        let export = sanitize_export(export()).unwrap();

        assert_eq!(export.meeting.title, "Planning");
        assert!(export.transcripts[0].words[0].get("audio_path").is_none());
    }

    #[tokio::test]
    async fn mcp_supports_current_stateless_tool_discovery_on_the_public_api_host() {
        let server = MockServer::start().await;
        let key = format!("anl_{}", "a".repeat(64));
        let key_hash = Sha256::digest(key.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/verify_cloud_api_key"))
            .and(body_json(serde_json::json!({ "p_key_hash": key_hash })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "user_id": "00000000-0000-0000-0000-000000000001",
                    "status": "ok",
                }])),
            )
            .expect(2)
            .mount(&server)
            .await;
        let state =
            AppState::new(crate::CloudApiConfig::new(server.uri(), "service-role-key").unwrap());
        let app = connector_router(state.clone()).route_layer(middleware::from_fn_with_state(
            state,
            crate::require_cloud_connector_auth,
        ));

        let response = app
            .clone()
            .oneshot(
                Request::post("/mcp")
                    .header("host", "api.anarlog.so")
                    .header("authorization", format!("Bearer {key}"))
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .header("mcp-protocol-version", "2026-07-28")
                    .header("mcp-method", "server/discover")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "server/discover",
                            "params": {
                                "_meta": current_mcp_meta(),
                            },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
        assert!(
            body["result"]["supportedVersions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|version| version == "2026-07-28")
        );

        let response = app
            .oneshot(
                Request::post("/mcp")
                    .header("host", "api.anarlog.so")
                    .header("authorization", format!("Bearer {key}"))
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .header("mcp-protocol-version", "2026-07-28")
                    .header("mcp-method", "tools/list")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "tools/list",
                            "params": {
                                "_meta": current_mcp_meta(),
                            },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
        assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 5);
        assert!(
            body["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == "list_meetings")
        );
        assert!(
            body["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == "export_meeting")
        );
    }

    fn current_mcp_meta() -> serde_json::Value {
        serde_json::json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {
                "name": "anarlog-test",
                "version": "1.0.0",
            },
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    }

    #[tokio::test]
    async fn mcp_export_tool_returns_the_complete_snapshot() {
        let server = MockServer::start().await;
        let key = format!("anl_{}", "d".repeat(64));
        let key_hash = Sha256::digest(key.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/verify_cloud_api_key"))
            .and(body_json(serde_json::json!({ "p_key_hash": key_hash })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "user_id": "00000000-0000-0000-0000-000000000001",
                    "status": "ok",
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_cloud_api_snapshot"))
            .and(body_json(serde_json::json!({
                "p_user_id": "00000000-0000-0000-0000-000000000001",
                "p_session_id": "meeting-1",
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "content_json": export(),
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;
        let state =
            AppState::new(crate::CloudApiConfig::new(server.uri(), "service-role-key").unwrap());
        let app = connector_router(state.clone()).route_layer(middleware::from_fn_with_state(
            state,
            crate::require_cloud_connector_auth,
        ));

        let response = app
            .oneshot(
                Request::post("/mcp")
                    .header("host", "api.anarlog.so")
                    .header("authorization", format!("Bearer {key}"))
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .header("mcp-protocol-version", "2025-11-25")
                    .body(Body::from(
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "tools/call",
                            "params": {
                                "name": "export_meeting",
                                "arguments": { "meeting_id": "meeting-1" },
                            },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
        assert_eq!(body["result"]["structuredContent"]["id"], "meeting-1");
        assert_eq!(
            body["result"]["structuredContent"]["transcripts"][0]["text"],
            "hello"
        );
    }

    #[tokio::test]
    async fn connector_routes_require_and_verify_cloud_api_keys() {
        let server = MockServer::start().await;
        let state =
            AppState::new(crate::CloudApiConfig::new(server.uri(), "service-role-key").unwrap());
        let app = connector_router(state.clone()).route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::require_cloud_connector_auth,
        ));

        let unauthorized = app
            .clone()
            .oneshot(Request::builder().uri("/mcp").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthorized
                .headers()
                .get("www-authenticate")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer resource_metadata=\"https://api.anarlog.so/.well-known/oauth-protected-resource/mcp\""
        );
        let body = to_bytes(unauthorized.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"]["code"],
            "unauthorized"
        );

        for (key_character, verification_status, error_code) in [
            ("b", "subscription_required", "subscription_required"),
            ("c", "cloud_api_not_enabled", "cloud_api_not_enabled"),
        ] {
            let key = format!("anl_{}", key_character.repeat(64));
            let key_hash = Sha256::digest(key.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            Mock::given(method("POST"))
                .and(path("/rest/v1/rpc/verify_cloud_api_key"))
                .and(body_json(serde_json::json!({ "p_key_hash": key_hash })))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                        "user_id": "00000000-0000-0000-0000-000000000001",
                        "status": verification_status,
                    }])),
                )
                .expect(1)
                .mount(&server)
                .await;

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/v1/meetings")
                        .header("authorization", format!("Bearer {key}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"]["code"],
                error_code
            );
        }

        let key = format!("anl_{}", "a".repeat(64));
        let key_hash = Sha256::digest(key.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/verify_cloud_api_key"))
            .and(body_json(serde_json::json!({ "p_key_hash": key_hash })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "user_id": "00000000-0000-0000-0000-000000000001",
                    "status": "ok",
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/list_cloud_api_snapshots"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "content_json": export(),
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/meetings")
                    .header("authorization", format!("Bearer {key}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["meetings"][0]["id"], "meeting-1");
        assert_eq!(body["pagination"]["returned"], 1);
    }

    #[tokio::test]
    async fn connector_routes_validate_oauth_tokens_and_live_account_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/auth/v1/.well-known/jwks.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "keys": [{
                    "kty": "EC",
                    "use": "sig",
                    "crv": "P-256",
                    "kid": TEST_KEY_ID,
                    "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                    "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
                    "alg": "ES256"
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/verify_cloud_api_user"))
            .and(body_json(serde_json::json!({
                "p_user_id": "00000000-0000-0000-0000-000000000001"
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "status": "ok" }])),
            )
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/list_cloud_api_snapshots"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "content_json": export(),
                }])),
            )
            .expect(2)
            .mount(&server)
            .await;
        let state =
            AppState::new(crate::CloudApiConfig::new(server.uri(), "service-role-key").unwrap());
        let app = connector_router(state.clone()).route_layer(middleware::from_fn_with_state(
            state,
            crate::require_cloud_connector_auth,
        ));
        let issuer = format!("{}/auth/v1", server.uri());

        let invalid = app
            .clone()
            .oneshot(
                Request::get("/v1/meetings")
                    .header("authorization", "Bearer invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
        assert!(
            invalid
                .headers()
                .get("www-authenticate")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("error=\"invalid_token\"")
        );

        let session = session_token(&issuer);
        let response = app
            .clone()
            .oneshot(
                Request::get("/v1/meetings")
                    .header("authorization", format!("Bearer {session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::post("/mcp")
                    .header("authorization", format!("Bearer {session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::get("/v1/meetings")
                    .header(
                        "authorization",
                        format!("Bearer {}", oauth_token(&issuer, "email")),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
