use std::collections::HashSet;

use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::put,
};
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::error::{Result, SyncError};
use crate::snapshot::{
    MAX_SNAPSHOT_BODY_BYTES, sanitize_document_with_attachments, sanitize_title,
};
use crate::state::AppState;

const SNAPSHOT_PUBLISH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_SNAPSHOT_REQUEST_BYTES: usize = MAX_SNAPSHOT_BODY_BYTES + 16 * 1024;
pub(super) const MAX_SNAPSHOT_RESPONSE_BYTES: usize = MAX_SNAPSHOT_BODY_BYTES + 256 * 1024;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub(super) enum PublishSessionShareSnapshotRequest {
    Cas(CasSessionShareSnapshotRequest),
    Legacy(LegacySessionShareSnapshotRequest),
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CasSessionShareSnapshotRequest {
    base_revision: i64,
    mutation_id: String,
    title: String,
    body: Value,
    attachment_ids: Vec<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LegacySessionShareSnapshotRequest {
    title: String,
    body: Value,
    attachment_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SharedNoteAttachment {
    pub(crate) id: String,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublishedSessionShareSnapshot {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body: Value,
    attachments: Vec<SharedNoteAttachment>,
    web_editable: bool,
    access_version: i64,
    published_at: String,
}

#[derive(Serialize)]
struct PublishSnapshotCasRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_expected_content_revision: i64,
    p_mutation_id: &'a str,
    p_title: &'a str,
    p_body_json: &'a Value,
    p_attachment_ids: &'a [String],
    p_web_editable: bool,
}

#[derive(Serialize)]
struct EditSnapshotCasRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_expected_content_revision: i64,
    p_mutation_id: &'a str,
    p_title: &'a str,
    p_body_json: &'a Value,
    p_attachment_ids: &'a [String],
}

#[derive(Serialize)]
struct LegacyPublishSnapshotRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_title: &'a str,
    p_body_json: &'a Value,
    p_attachment_ids: Option<&'a [String]>,
}

#[derive(Deserialize)]
struct LegacyPublishedSnapshotRow {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body_json: Value,
    attachments_json: Vec<SharedNoteAttachment>,
    published_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPublishedSessionShareSnapshot {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body: Value,
    attachments: Vec<SharedNoteAttachment>,
    published_at: String,
}

#[derive(Deserialize)]
struct PublishedSnapshotRow {
    outcome: String,
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body_json: Value,
    attachments_json: Vec<SharedNoteAttachment>,
    web_editable: bool,
    access_version: i64,
    published_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotConflictResponse {
    code: &'static str,
    snapshot: PublishedSessionShareSnapshot,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}
#[derive(OpenApi)]
#[openapi(
    paths(publish_session_share_snapshot, edit_session_share_snapshot),
    components(schemas(
        PublishSessionShareSnapshotRequest,
        CasSessionShareSnapshotRequest,
        LegacySessionShareSnapshotRequest,
        PublishedSessionShareSnapshot,
        SharedNoteAttachment
    ))
)]
struct ApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/shares/{share_id}/snapshot",
        put(publish_session_share_snapshot)
            .layer(DefaultBodyLimit::max(MAX_SNAPSHOT_REQUEST_BYTES)),
    )
}

pub(super) fn web_edit_router() -> Router<AppState> {
    Router::new().route(
        "/shares/{share_id}/web-edit",
        put(edit_session_share_snapshot).layer(DefaultBodyLimit::max(MAX_SNAPSHOT_REQUEST_BYTES)),
    )
}

#[utoipa::path(
    put,
    path = "/shares/{share_id}/snapshot",
    tag = "sync",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = PublishSessionShareSnapshotRequest,
    responses(
        (status = 200, description = "Sanitized shared-note snapshot published", body = PublishedSessionShareSnapshot),
        (status = 400, description = "Invalid shared-note snapshot"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro or share-manager access required"),
        (status = 409, description = "Shared note changed since the supplied base revision"),
        (status = 413, description = "Shared-note snapshot is too large"),
        (status = 502, description = "Shared-note service unavailable")
    )
)]
async fn publish_session_share_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Response> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let request = serde_json::from_value::<PublishSessionShareSnapshotRequest>(payload)
        .map_err(|_| SyncError::BadRequest("Shared note snapshot is invalid".to_string()))?;
    match request {
        PublishSessionShareSnapshotRequest::Legacy(request) => {
            publish_legacy_session_share_snapshot(&state, &auth.claims.sub, &share_id, &request)
                .await
        }
        PublishSessionShareSnapshotRequest::Cas(request) => {
            mutate_session_share_snapshot(
                &state,
                &auth.claims.sub,
                &share_id,
                request,
                SnapshotMutationKind::DesktopPublish,
            )
            .await
        }
    }
}

async fn publish_legacy_session_share_snapshot(
    state: &AppState,
    actor_user_id: &str,
    share_id: &str,
    request: &LegacySessionShareSnapshotRequest,
) -> Result<Response> {
    let share_id = canonical_random_uuid(share_id, "Shared note ID")?;
    let title = sanitize_title(&request.title)?;
    let requested_attachment_ids = request.attachment_ids.as_deref();
    let attachment_ids = validate_attachment_ids(requested_attachment_ids.unwrap_or_default())?;
    let body = sanitize_document_with_attachments(&request.body, &attachment_ids)?;

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SNAPSHOT_PUBLISH_TIMEOUT)
        .json(&LegacyPublishSnapshotRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: actor_user_id,
            p_title: &title,
            p_body_json: &body,
            p_attachment_ids: requested_attachment_ids,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase legacy shared-note publication request failed");
            SyncError::SnapshotServiceUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPSHOT_RESPONSE_BYTES as u64)
    {
        tracing::warn!(%status, "Supabase legacy shared-note publication response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let bytes = response.bytes().await.map_err(|error| {
        tracing::warn!(%error, "Supabase legacy shared-note publication response could not be read");
        SyncError::SnapshotServiceUnavailable
    })?;
    if bytes.len() > MAX_SNAPSHOT_RESPONSE_BYTES {
        tracing::warn!(%status, "Supabase legacy shared-note publication response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    if !status.is_success() {
        let code = serde_json::from_slice::<PostgrestError>(&bytes)
            .ok()
            .map(|error| error.code);
        tracing::warn!(%status, ?code, "Supabase legacy shared-note publication was rejected");
        return match (status, code.as_deref()) {
            (HttpStatusCode::UNAUTHORIZED | HttpStatusCode::FORBIDDEN, _) | (_, Some("42501")) => {
                Err(SyncError::SnapshotPublicationForbidden)
            }
            (_, Some("23514")) => Err(SyncError::SnapshotChanged),
            (_, Some("22023")) => Err(SyncError::BadRequest(
                "Shared note snapshot is invalid".to_string(),
            )),
            _ => Err(SyncError::SnapshotServiceUnavailable),
        };
    }

    let mut rows =
        serde_json::from_slice::<Vec<LegacyPublishedSnapshotRow>>(&bytes).map_err(|error| {
            tracing::warn!(%error, "Supabase legacy shared-note publication response was invalid");
            SyncError::SnapshotServiceUnavailable
        })?;
    if rows.len() != 1 {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase legacy shared-note publication returned an invalid row count"
        );
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let row = rows.pop().expect("row count was checked");
    if row.share_id != share_id
        || row.schema_version != 1
        || row.content_revision < 1
        || row.title != title
        || row.body_json != body
        || validate_shared_attachments(&row.attachments_json, requested_attachment_ids).is_err()
        || chrono::DateTime::parse_from_rfc3339(&row.published_at).is_err()
    {
        tracing::warn!("Supabase legacy shared-note publication response failed validation");
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(LegacyPublishedSessionShareSnapshot {
            share_id: row.share_id,
            schema_version: row.schema_version,
            content_revision: row.content_revision,
            title: row.title,
            body: row.body_json,
            attachments: row.attachments_json,
            published_at: row.published_at,
        }),
    )
        .into_response())
}

#[utoipa::path(
    put,
    path = "/shares/{share_id}/web-edit",
    tag = "sync",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = PublishSessionShareSnapshotRequest,
    responses(
        (status = 200, description = "Shared-note edit saved", body = PublishedSessionShareSnapshot),
        (status = 400, description = "Invalid or unsupported shared-note edit"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Explicit Editor access required"),
        (status = 409, description = "Shared note changed since the supplied base revision"),
        (status = 413, description = "Shared-note edit is too large"),
        (status = 502, description = "Shared-note service unavailable")
    )
)]
async fn edit_session_share_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Response> {
    let request = serde_json::from_value::<CasSessionShareSnapshotRequest>(payload)
        .map_err(|_| SyncError::BadRequest("Shared note edit is invalid".to_string()))?;
    mutate_session_share_snapshot(
        &state,
        &auth.claims.sub,
        &share_id,
        request,
        SnapshotMutationKind::WebEdit,
    )
    .await
}

#[derive(Clone, Copy)]
enum SnapshotMutationKind {
    DesktopPublish,
    WebEdit,
}

async fn mutate_session_share_snapshot(
    state: &AppState,
    actor_user_id: &str,
    share_id: &str,
    request: CasSessionShareSnapshotRequest,
    kind: SnapshotMutationKind,
) -> Result<Response> {
    let share_id = canonical_random_uuid(share_id, "Shared note ID")?;
    let base_revision = request.base_revision;
    let mutation_id = canonical_random_uuid(&request.mutation_id, "Mutation ID")?;
    let minimum_revision = match kind {
        SnapshotMutationKind::DesktopPublish => 0,
        SnapshotMutationKind::WebEdit => 1,
    };
    if base_revision < minimum_revision || base_revision == i64::MAX {
        return Err(SyncError::BadRequest(
            "Shared note base revision is invalid".to_string(),
        ));
    }
    let title = sanitize_title(&request.title)?;
    let requested_attachment_ids = request.attachment_ids.as_slice();
    let attachment_ids = validate_attachment_ids(requested_attachment_ids)?;
    let body = sanitize_document_with_attachments(&request.body, &attachment_ids)?;
    let web_editable = body == request.body;
    if matches!(kind, SnapshotMutationKind::WebEdit) && !web_editable {
        return Err(SyncError::BadRequest(
            "Shared note edit contains unsupported content".to_string(),
        ));
    }

    let rpc_name = match kind {
        SnapshotMutationKind::DesktopPublish => "publish_session_share_snapshot_cas",
        SnapshotMutationKind::WebEdit => "edit_session_share_snapshot_cas",
    };
    let builder = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/{rpc_name}",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SNAPSHOT_PUBLISH_TIMEOUT);
    let builder = match kind {
        SnapshotMutationKind::DesktopPublish => builder.json(&PublishSnapshotCasRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: actor_user_id,
            p_expected_content_revision: base_revision,
            p_mutation_id: &mutation_id,
            p_title: &title,
            p_body_json: &body,
            p_attachment_ids: requested_attachment_ids,
            p_web_editable: web_editable,
        }),
        SnapshotMutationKind::WebEdit => builder.json(&EditSnapshotCasRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: actor_user_id,
            p_expected_content_revision: base_revision,
            p_mutation_id: &mutation_id,
            p_title: &title,
            p_body_json: &body,
            p_attachment_ids: requested_attachment_ids,
        }),
    };
    let mut response = builder.send().await.map_err(|error| {
        tracing::warn!(%error, rpc_name, "Supabase shared-note mutation request failed");
        SyncError::SnapshotServiceUnavailable
    })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPSHOT_RESPONSE_BYTES as u64)
    {
        tracing::warn!(%status, rpc_name, "Supabase shared-note mutation response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, rpc_name, "Supabase shared-note mutation response could not be read");
        SyncError::SnapshotServiceUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > MAX_SNAPSHOT_RESPONSE_BYTES {
            tracing::warn!(%status, rpc_name, "Supabase shared-note mutation response was too large");
            return Err(SyncError::SnapshotServiceUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let code = serde_json::from_slice::<PostgrestError>(&bytes)
            .ok()
            .map(|error| error.code);
        tracing::warn!(%status, ?code, rpc_name, "Supabase shared-note mutation was rejected");
        return match (status, code.as_deref()) {
            (HttpStatusCode::UNAUTHORIZED | HttpStatusCode::FORBIDDEN, _) | (_, Some("42501")) => {
                Err(SyncError::SnapshotPublicationForbidden)
            }
            (_, Some("40001")) => Err(SyncError::SnapshotChanged),
            (_, Some("22023" | "55000")) => Err(SyncError::BadRequest(
                "Shared note snapshot is invalid".to_string(),
            )),
            _ => Err(SyncError::SnapshotServiceUnavailable),
        };
    }

    let mut rows =
        serde_json::from_slice::<Vec<PublishedSnapshotRow>>(&bytes).map_err(|error| {
            tracing::warn!(%error, rpc_name, "Supabase shared-note mutation response was invalid");
            SyncError::SnapshotServiceUnavailable
        })?;
    if rows.len() != 1 {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase shared-note mutation returned an invalid row count"
        );
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let row = rows.pop().expect("row count was checked");
    if !matches!(row.outcome.as_str(), "applied" | "replayed" | "conflict")
        || row.share_id != share_id
        || row.schema_version != 1
        || row.content_revision < 1
        || (row.outcome == "conflict" && row.content_revision == base_revision)
        || (row.outcome != "conflict" && row.content_revision != base_revision + 1)
        || row.access_version < 1
        || chrono::DateTime::parse_from_rfc3339(&row.published_at).is_err()
    {
        tracing::warn!(
            rpc_name,
            "Supabase shared-note mutation response failed validation"
        );
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let expected_ids = (row.outcome != "conflict").then_some(requested_attachment_ids);
    if validate_shared_attachments(&row.attachments_json, expected_ids).is_err()
        || validate_snapshot_document(&row.body_json, &row.attachments_json).is_err()
        || !sanitize_title(&row.title).is_ok_and(|title| title == row.title)
        || (row.outcome != "conflict" && (row.title != title || row.body_json != body))
        || (matches!(kind, SnapshotMutationKind::WebEdit)
            && row.outcome != "conflict"
            && !row.web_editable)
    {
        tracing::warn!(
            rpc_name,
            "Supabase shared-note mutation payload failed validation"
        );
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let outcome = row.outcome;
    let snapshot = PublishedSessionShareSnapshot {
        share_id: row.share_id,
        schema_version: row.schema_version,
        content_revision: row.content_revision,
        title: row.title,
        body: row.body_json,
        attachments: row.attachments_json,
        web_editable: row.web_editable,
        access_version: row.access_version,
        published_at: row.published_at,
    };
    let headers = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    if outcome == "conflict" {
        return Ok((
            StatusCode::CONFLICT,
            headers,
            Json(SnapshotConflictResponse {
                code: "snapshot_conflict",
                snapshot,
            }),
        )
            .into_response());
    }
    Ok((headers, Json(snapshot)).into_response())
}

fn canonical_random_uuid(value: &str, label: &str) -> Result<String> {
    let uuid =
        Uuid::parse_str(value).map_err(|_| SyncError::BadRequest(format!("{label} is invalid")))?;
    if uuid.to_string() != value || uuid.get_version() != Some(uuid::Version::Random) {
        return Err(SyncError::BadRequest(format!("{label} is invalid")));
    }
    Ok(value.to_string())
}

fn validate_attachment_ids(values: &[String]) -> Result<HashSet<String>> {
    if values.len() > 64 {
        return Err(SyncError::BadRequest(
            "Shared note has too many attachments".to_string(),
        ));
    }
    let mut ids = HashSet::new();
    for value in values {
        canonical_random_uuid(value, "Shared attachment ID")?;
        if !ids.insert(value.clone()) {
            return Err(SyncError::BadRequest(
                "Shared attachment ID is invalid".to_string(),
            ));
        }
    }
    Ok(ids)
}

fn validate_snapshot_document(body: &Value, attachments: &[SharedNoteAttachment]) -> Result<()> {
    let ids = attachments
        .iter()
        .map(|attachment| attachment.id.clone())
        .collect::<HashSet<_>>();
    let sanitized = sanitize_document_with_attachments(body, &ids)?;
    if sanitized != *body {
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    Ok(())
}

pub(crate) fn validate_shared_attachments(
    attachments: &[SharedNoteAttachment],
    expected_ids: Option<&[String]>,
) -> std::result::Result<(), ()> {
    if expected_ids.is_some_and(|expected| attachments.len() != expected.len()) {
        return Err(());
    }
    let mut seen = HashSet::new();
    for (index, attachment) in attachments.iter().enumerate() {
        let id = Uuid::parse_str(&attachment.id).map_err(|_| ())?;
        let valid_content_type = attachment
            .content_type
            .split_once('/')
            .is_some_and(|(kind, subtype)| !kind.is_empty() && !subtype.is_empty());
        if expected_ids.is_some_and(|expected| attachment.id != expected[index])
            || id.to_string() != attachment.id
            || id.get_version() != Some(uuid::Version::Random)
            || !seen.insert(attachment.id.clone())
            || attachment.filename.is_empty()
            || attachment.filename.len() > 1024
            || attachment.filename.trim() != attachment.filename
            || attachment.filename.contains(['/', '\\'])
            || attachment.filename.chars().any(char::is_control)
            || !valid_content_type
            || attachment.size_bytes == 0
            || attachment.size_bytes > 512 * 1024 * 1024
            || attachment.sha256.len() != 64
            || !attachment
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(());
        }
    }
    Ok(())
}
