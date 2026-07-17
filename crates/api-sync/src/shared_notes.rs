use std::{net::IpAddr, time::Duration};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hmac::{Hmac, KeyInit, Mac};
use hypr_api_auth::AuthContext;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::Sha256;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::{
    SharedNotesConfig,
    error::{Result, SyncError},
    routes::{SharedNoteAttachment, validate_shared_attachments},
    snapshot::MAX_SNAPSHOT_BODY_BYTES,
};

const SHARED_NOTE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_LINK_REQUEST_BYTES: usize = 1024;
const MAX_HANDOFF_CLAIM_REQUEST_BYTES: usize = 256;
const MAX_SNAPSHOT_RESPONSE_BYTES: usize = MAX_SNAPSHOT_BODY_BYTES + 256 * 1024;
const MAX_HANDOFF_RESPONSE_BYTES: usize = 16 * 1024;
const SHARED_ATTACHMENT_BUCKET: &str = "shared-note-attachments";
const ATTACHMENT_DOWNLOAD_TTL_SECONDS: i64 = 60;
const FLY_CLIENT_IP_HEADER: &str = "fly-client-ip";
const HANDOFF_SOURCE_DOMAIN: &[u8] = b"anarlog:shared-note-handoff-source:v1\0";

#[derive(Clone)]
pub struct SharedNotesState {
    config: SharedNotesConfig,
    client: reqwest::Client,
    storage: hypr_supabase_storage::SupabaseStorage,
}

impl SharedNotesState {
    pub fn new(config: SharedNotesConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(SHARED_NOTE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("shared-note HTTP client must build");
        let storage = hypr_supabase_storage::SupabaseStorage::new(
            client.clone(),
            &config.supabase_url,
            &config.supabase_service_role_key,
        );
        Self {
            config,
            client,
            storage,
        }
    }

    fn handoff_source_hash(&self, headers: &HeaderMap) -> String {
        let source = headers
            .get(FLY_CLIENT_IP_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<IpAddr>().ok())
            .map(|address| address.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.config.supabase_service_role_key.as_bytes())
                .expect("HMAC accepts service-role keys of any size");
        mac.update(HANDOFF_SOURCE_DOMAIN);
        mac.update(source.as_bytes());
        let bytes = mac.finalize().into_bytes();
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            encoded.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
            encoded.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
        }
        encoded
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SharedNoteLinkRequest {
    token: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedNoteHandoffClaimRequest {
    request_id: String,
    lease_id: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedNoteHandoffAttachmentRequest {
    lease_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharedNoteSnapshot {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body: Value,
    attachments: Vec<SharedNoteAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lease_expires_at: Option<String>,
    published_at: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharedAttachmentDownload {
    id: String,
    filename: String,
    content_type: String,
    size_bytes: u64,
    sha256: String,
    signed_url: String,
    expires_at: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharedNoteHandoff {
    request_id: String,
    expires_at: String,
}

#[derive(Serialize)]
struct PublicSlugRpcRequest<'a> {
    p_public_slug: &'a str,
}

#[derive(Serialize)]
struct LinkRpcRequest<'a> {
    p_share_id: &'a str,
    p_link_token: &'a str,
}

#[derive(Serialize)]
struct PublicHandoffRpcRequest<'a> {
    p_public_slug: &'a str,
    p_source_hash: &'a str,
}

#[derive(Serialize)]
struct LinkHandoffRpcRequest<'a> {
    p_share_id: &'a str,
    p_link_token: &'a str,
    p_source_hash: &'a str,
}

#[derive(Serialize)]
struct ClaimHandoffRpcRequest<'a> {
    p_request_id: &'a str,
    p_lease_id: &'a str,
}

#[derive(Serialize)]
struct HandoffAttachmentRpcRequest<'a> {
    p_lease_id: &'a str,
    p_attachment_id: &'a str,
    p_download_expires_at: &'a str,
}

#[derive(Serialize)]
struct PublicAttachmentRpcRequest<'a> {
    p_public_slug: &'a str,
    p_attachment_id: &'a str,
    p_download_expires_at: &'a str,
}

#[derive(Serialize)]
struct LinkAttachmentRpcRequest<'a> {
    p_share_id: &'a str,
    p_attachment_id: &'a str,
    p_link_token: &'a str,
    p_download_expires_at: &'a str,
}

#[derive(Serialize)]
struct MyAttachmentRpcRequest<'a> {
    p_share_id: &'a str,
    p_attachment_id: &'a str,
    p_actor_user_id: &'a str,
    p_download_expires_at: &'a str,
}

#[derive(Deserialize)]
struct GatewaySnapshotRow {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body_json: Value,
    attachments_json: Vec<SharedNoteAttachment>,
    published_at: String,
}

#[derive(Deserialize)]
struct GatewayClaimSnapshotRow {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body_json: Value,
    attachments_json: Vec<SharedNoteAttachment>,
    lease_expires_at: String,
    published_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedAttachmentRow {
    share_id: String,
    attachment_id: String,
    object_key: String,
    filename: String,
    content_type: String,
    size_bytes: i64,
    sha256: String,
    access_version: i64,
    cleanup_not_before: String,
}

#[derive(Deserialize)]
struct GatewayHandoffRow {
    request_id: String,
    expires_at: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        read_public_shared_note,
        read_link_shared_note,
        create_public_shared_note_handoff,
        create_link_shared_note_handoff,
        claim_shared_note_handoff,
        download_handoff_shared_attachment,
        download_public_shared_attachment,
        download_link_shared_attachment,
        download_access_shared_attachment
    ),
    components(schemas(
        SharedNoteLinkRequest,
        SharedNoteHandoffClaimRequest,
        SharedNoteHandoffAttachmentRequest,
        SharedNoteSnapshot,
        SharedNoteHandoff,
        SharedAttachmentDownload
    ))
)]
pub struct SharedNotesApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    SharedNotesApiDoc::openapi()
}

pub fn router(state: SharedNotesState) -> Router {
    Router::new()
        .route("/shared-notes/public/{slug}", get(read_public_shared_note))
        .route(
            "/shared-notes/link/{share_id}",
            post(read_link_shared_note).layer(DefaultBodyLimit::max(MAX_LINK_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/public/{slug}/handoff",
            post(create_public_shared_note_handoff),
        )
        .route(
            "/shared-notes/link/{share_id}/handoff",
            post(create_link_shared_note_handoff)
                .layer(DefaultBodyLimit::max(MAX_LINK_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/handoffs/claim",
            post(claim_shared_note_handoff)
                .layer(DefaultBodyLimit::max(MAX_HANDOFF_CLAIM_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/handoffs/attachments/{attachment_id}/download",
            post(download_handoff_shared_attachment)
                .layer(DefaultBodyLimit::max(MAX_HANDOFF_CLAIM_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/public/{slug}/attachments/{attachment_id}/download",
            post(download_public_shared_attachment),
        )
        .route(
            "/shared-notes/link/{share_id}/attachments/{attachment_id}/download",
            post(download_link_shared_attachment)
                .layer(DefaultBodyLimit::max(MAX_LINK_REQUEST_BYTES)),
        )
        .layer(middleware::from_fn(add_no_store))
        .with_state(state)
}

pub fn authenticated_router(state: SharedNotesState) -> Router {
    Router::new()
        .route(
            "/shared-notes/access/{share_id}/attachments/{attachment_id}/download",
            post(download_access_shared_attachment),
        )
        .layer(middleware::from_fn(add_no_store))
        .with_state(state)
}

async fn add_no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[utoipa::path(
    get,
    path = "/shared-notes/public/{slug}",
    tag = "shared-notes",
    params(("slug" = String, Path, description = "Public share slug")),
    responses(
        (status = 200, description = "Public shared note", body = SharedNoteSnapshot),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn read_public_shared_note(
    State(state): State<SharedNotesState>,
    Path(slug): Path<String>,
) -> Result<Json<SharedNoteSnapshot>> {
    if !is_valid_public_slug(&slug) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let row = rpc_single(
        &state,
        "gateway_read_public_session_share_snapshot_v2",
        &PublicSlugRpcRequest {
            p_public_slug: &slug,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_snapshot(row, None)?))
}

#[utoipa::path(
    post,
    path = "/shared-notes/link/{share_id}",
    tag = "shared-notes",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = SharedNoteLinkRequest,
    responses(
        (status = 200, description = "Bearer-link shared note", body = SharedNoteSnapshot),
        (status = 404, description = "Shared note unavailable"),
        (status = 413, description = "Request too large"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn read_link_shared_note(
    State(state): State<SharedNotesState>,
    Path(share_id): Path<String>,
    Json(request): Json<SharedNoteLinkRequest>,
) -> Result<Json<SharedNoteSnapshot>> {
    let share_id = canonical_uuid(&share_id).ok_or(SyncError::SharedNoteNotFound)?;
    if !is_valid_link_token(&request.token) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let row = rpc_single(
        &state,
        "gateway_read_session_share_link_snapshot_v2",
        &LinkRpcRequest {
            p_share_id: &share_id,
            p_link_token: &request.token,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_snapshot(row, Some(&share_id))?))
}

#[utoipa::path(
    post,
    path = "/shared-notes/public/{slug}/handoff",
    tag = "shared-notes",
    params(("slug" = String, Path, description = "Public share slug")),
    responses(
        (status = 200, description = "One-time desktop handoff", body = SharedNoteHandoff),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn create_public_shared_note_handoff(
    State(state): State<SharedNotesState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SharedNoteHandoff>> {
    if !is_valid_public_slug(&slug) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let source_hash = state.handoff_source_hash(&headers);
    let row = rpc_single(
        &state,
        "gateway_create_public_session_share_handoff",
        &PublicHandoffRpcRequest {
            p_public_slug: &slug,
            p_source_hash: &source_hash,
        },
        MAX_HANDOFF_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_handoff(row)?))
}

#[utoipa::path(
    post,
    path = "/shared-notes/link/{share_id}/handoff",
    tag = "shared-notes",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = SharedNoteLinkRequest,
    responses(
        (status = 200, description = "One-time desktop handoff", body = SharedNoteHandoff),
        (status = 404, description = "Shared note unavailable"),
        (status = 413, description = "Request too large"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn create_link_shared_note_handoff(
    State(state): State<SharedNotesState>,
    Path(share_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SharedNoteLinkRequest>,
) -> Result<Json<SharedNoteHandoff>> {
    let share_id = canonical_uuid(&share_id).ok_or(SyncError::SharedNoteNotFound)?;
    if !is_valid_link_token(&request.token) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let source_hash = state.handoff_source_hash(&headers);
    let row = rpc_single(
        &state,
        "gateway_create_session_share_link_handoff",
        &LinkHandoffRpcRequest {
            p_share_id: &share_id,
            p_link_token: &request.token,
            p_source_hash: &source_hash,
        },
        MAX_HANDOFF_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_handoff(row)?))
}

#[utoipa::path(
    post,
    path = "/shared-notes/handoffs/claim",
    tag = "shared-notes",
    request_body = SharedNoteHandoffClaimRequest,
    responses(
        (status = 200, description = "Claimed shared note with a retryable attachment lease", body = SharedNoteSnapshot),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn claim_shared_note_handoff(
    State(state): State<SharedNotesState>,
    Json(request): Json<SharedNoteHandoffClaimRequest>,
) -> Result<Json<SharedNoteSnapshot>> {
    let request_id = canonical_uuid_v4(&request.request_id).ok_or(SyncError::SharedNoteNotFound)?;
    let lease_id = canonical_uuid_v4(&request.lease_id).ok_or(SyncError::SharedNoteNotFound)?;
    let row: GatewayClaimSnapshotRow = rpc_single(
        &state,
        "gateway_lease_session_share_handoff",
        &ClaimHandoffRpcRequest {
            p_request_id: &request_id,
            p_lease_id: &lease_id,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    let lease_expires_at = validate_handoff_lease(&row.lease_expires_at)?;
    let mut snapshot = validate_snapshot(
        GatewaySnapshotRow {
            share_id: row.share_id,
            schema_version: row.schema_version,
            content_revision: row.content_revision,
            title: row.title,
            body_json: row.body_json,
            attachments_json: row.attachments_json,
            published_at: row.published_at,
        },
        None,
    )?;
    snapshot.lease_expires_at = Some(lease_expires_at);
    Ok(Json(snapshot))
}

#[utoipa::path(
    post,
    path = "/shared-notes/handoffs/attachments/{attachment_id}/download",
    tag = "shared-notes",
    params(("attachment_id" = String, Path, description = "Published attachment ID")),
    request_body = SharedNoteHandoffAttachmentRequest,
    responses(
        (status = 200, description = "Short-lived leased attachment download", body = SharedAttachmentDownload),
        (status = 404, description = "Shared attachment unavailable"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn download_handoff_shared_attachment(
    State(state): State<SharedNotesState>,
    Path(attachment_id): Path<String>,
    Json(request): Json<SharedNoteHandoffAttachmentRequest>,
) -> Result<Json<SharedAttachmentDownload>> {
    let lease_id =
        canonical_uuid_v4(&request.lease_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let attachment_id =
        canonical_uuid_v4(&attachment_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let expires_at = future_timestamp(ATTACHMENT_DOWNLOAD_TTL_SECONDS)?;
    let row = attachment_rpc_single(
        &state,
        "gateway_prepare_session_share_handoff_attachment_download",
        &HandoffAttachmentRpcRequest {
            p_lease_id: &lease_id,
            p_attachment_id: &attachment_id,
            p_download_expires_at: &expires_at,
        },
    )
    .await?;
    Ok(Json(
        sign_attachment_download(
            &state,
            row,
            &attachment_id,
            &expires_at,
            ATTACHMENT_DOWNLOAD_TTL_SECONDS,
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/shared-notes/public/{slug}/attachments/{attachment_id}/download",
    tag = "shared-notes",
    params(
        ("slug" = String, Path, description = "Public share slug"),
        ("attachment_id" = String, Path, description = "Published attachment ID")
    ),
    responses(
        (status = 200, description = "Short-lived public attachment download", body = SharedAttachmentDownload),
        (status = 404, description = "Shared attachment unavailable"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn download_public_shared_attachment(
    State(state): State<SharedNotesState>,
    Path((slug, attachment_id)): Path<(String, String)>,
) -> Result<Json<SharedAttachmentDownload>> {
    if !is_valid_public_slug(&slug) {
        return Err(SyncError::SharedAttachmentNotFound);
    }
    let attachment_id =
        canonical_uuid_v4(&attachment_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let expires_at = future_timestamp(ATTACHMENT_DOWNLOAD_TTL_SECONDS)?;
    let row = attachment_rpc_single(
        &state,
        "gateway_prepare_public_session_share_attachment_download",
        &PublicAttachmentRpcRequest {
            p_public_slug: &slug,
            p_attachment_id: &attachment_id,
            p_download_expires_at: &expires_at,
        },
    )
    .await?;
    Ok(Json(
        sign_attachment_download(
            &state,
            row,
            &attachment_id,
            &expires_at,
            ATTACHMENT_DOWNLOAD_TTL_SECONDS,
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/shared-notes/link/{share_id}/attachments/{attachment_id}/download",
    tag = "shared-notes",
    params(
        ("share_id" = String, Path, description = "Session share ID"),
        ("attachment_id" = String, Path, description = "Published attachment ID")
    ),
    request_body = SharedNoteLinkRequest,
    responses(
        (status = 200, description = "Short-lived bearer-link attachment download", body = SharedAttachmentDownload),
        (status = 404, description = "Shared attachment unavailable"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn download_link_shared_attachment(
    State(state): State<SharedNotesState>,
    Path((share_id, attachment_id)): Path<(String, String)>,
    Json(request): Json<SharedNoteLinkRequest>,
) -> Result<Json<SharedAttachmentDownload>> {
    let share_id = canonical_uuid(&share_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let attachment_id =
        canonical_uuid_v4(&attachment_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    if !is_valid_link_token(&request.token) {
        return Err(SyncError::SharedAttachmentNotFound);
    }
    let expires_at = future_timestamp(ATTACHMENT_DOWNLOAD_TTL_SECONDS)?;
    let row = attachment_rpc_single(
        &state,
        "gateway_prepare_session_share_link_attachment_download",
        &LinkAttachmentRpcRequest {
            p_share_id: &share_id,
            p_attachment_id: &attachment_id,
            p_link_token: &request.token,
            p_download_expires_at: &expires_at,
        },
    )
    .await?;
    Ok(Json(
        sign_attachment_download(
            &state,
            row,
            &attachment_id,
            &expires_at,
            ATTACHMENT_DOWNLOAD_TTL_SECONDS,
        )
        .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/shared-notes/access/{share_id}/attachments/{attachment_id}/download",
    tag = "shared-notes",
    params(
        ("share_id" = String, Path, description = "Session share ID"),
        ("attachment_id" = String, Path, description = "Published attachment ID")
    ),
    responses(
        (status = 200, description = "Short-lived authorized attachment download", body = SharedAttachmentDownload),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Shared attachment unavailable"),
        (status = 502, description = "Shared attachment service unavailable")
    )
)]
async fn download_access_shared_attachment(
    Extension(auth): Extension<AuthContext>,
    State(state): State<SharedNotesState>,
    Path((share_id, attachment_id)): Path<(String, String)>,
) -> Result<Json<SharedAttachmentDownload>> {
    let share_id = canonical_uuid(&share_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let attachment_id =
        canonical_uuid_v4(&attachment_id).ok_or(SyncError::SharedAttachmentNotFound)?;
    let actor_user_id =
        canonical_uuid(&auth.claims.sub).ok_or(SyncError::SharedAttachmentNotFound)?;
    let expires_at = future_timestamp(ATTACHMENT_DOWNLOAD_TTL_SECONDS)?;
    let row = attachment_rpc_single(
        &state,
        "prepare_my_session_share_attachment_download",
        &MyAttachmentRpcRequest {
            p_share_id: &share_id,
            p_attachment_id: &attachment_id,
            p_actor_user_id: &actor_user_id,
            p_download_expires_at: &expires_at,
        },
    )
    .await?;
    Ok(Json(
        sign_attachment_download(
            &state,
            row,
            &attachment_id,
            &expires_at,
            ATTACHMENT_DOWNLOAD_TTL_SECONDS,
        )
        .await?,
    ))
}

async fn attachment_rpc_single<RequestBody>(
    state: &SharedNotesState,
    function: &str,
    request: &RequestBody,
) -> Result<PreparedAttachmentRow>
where
    RequestBody: Serialize + ?Sized,
{
    rpc_single(state, function, request, MAX_HANDOFF_RESPONSE_BYTES)
        .await
        .map_err(|error| match error {
            SyncError::SharedNoteNotFound => SyncError::SharedAttachmentNotFound,
            SyncError::SnapshotServiceUnavailable => SyncError::SharedAttachmentServiceUnavailable,
            other => other,
        })
}

async fn sign_attachment_download(
    state: &SharedNotesState,
    row: PreparedAttachmentRow,
    expected_attachment_id: &str,
    expires_at: &str,
    expires_in_seconds: i64,
) -> Result<SharedAttachmentDownload> {
    validate_prepared_attachment(&row, expected_attachment_id, expires_at)?;
    let signed_url = state
        .storage
        .create_signed_url(
            SHARED_ATTACHMENT_BUCKET,
            &row.object_key,
            expires_in_seconds as u64,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared attachment download signing failed");
            SyncError::SharedAttachmentServiceUnavailable
        })?;
    Ok(SharedAttachmentDownload {
        id: row.attachment_id,
        filename: row.filename,
        content_type: row.content_type,
        size_bytes: row.size_bytes as u64,
        sha256: row.sha256,
        signed_url,
        expires_at: expires_at.to_string(),
    })
}

fn validate_prepared_attachment(
    row: &PreparedAttachmentRow,
    expected_attachment_id: &str,
    expires_at: &str,
) -> Result<()> {
    let share_id =
        canonical_uuid(&row.share_id).ok_or(SyncError::SharedAttachmentServiceUnavailable)?;
    if row.attachment_id != expected_attachment_id
        || canonical_uuid_v4(&row.attachment_id).is_none()
        || !is_valid_attachment_object_key(&row.object_key, &share_id, expected_attachment_id)
        || row.filename.is_empty()
        || row.filename.len() > 1024
        || row.filename.trim() != row.filename
        || row.filename.contains(['/', '\\'])
        || row.filename.chars().any(char::is_control)
        || !is_valid_content_type(&row.content_type)
        || !(1..=512 * 1024 * 1024).contains(&row.size_bytes)
        || row.sha256.len() != 64
        || !row
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || row.access_version < 1
        || chrono::DateTime::parse_from_rfc3339(&row.cleanup_not_before).is_err()
        || chrono::DateTime::parse_from_rfc3339(expires_at).is_err()
    {
        return Err(SyncError::SharedAttachmentServiceUnavailable);
    }
    Ok(())
}

fn is_valid_attachment_object_key(value: &str, share_id: &str, attachment_id: &str) -> bool {
    let mut parts = value.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(share) = parts.next() else {
        return false;
    };
    let Some(filename) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && canonical_uuid(owner).as_deref() == Some(owner)
        && share == share_id
        && filename == format!("{attachment_id}.sna1")
}

fn is_valid_content_type(value: &str) -> bool {
    value.len() <= 255
        && value == value.to_ascii_lowercase()
        && value
            .split_once('/')
            .is_some_and(|(kind, subtype)| !kind.is_empty() && !subtype.is_empty())
}

fn future_timestamp(seconds: i64) -> Result<String> {
    Utc::now()
        .checked_add_signed(TimeDelta::seconds(seconds))
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .ok_or_else(|| SyncError::Internal("shared attachment expiry overflow".to_string()))
}

fn validate_handoff_lease(value: &str) -> Result<String> {
    let expires_at = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| invalid_gateway_response("handoff lease expiry"))?;
    if expires_at <= Utc::now() {
        return Err(invalid_gateway_response("handoff lease expiry"));
    }
    Ok(value.to_string())
}

async fn rpc_single<RequestBody, Row>(
    state: &SharedNotesState,
    function: &str,
    request: &RequestBody,
    max_response_bytes: usize,
) -> Result<Row>
where
    RequestBody: Serialize + ?Sized,
    Row: DeserializeOwned,
{
    let mut response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/{function}",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(SHARED_NOTE_TIMEOUT)
        .json(request)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "shared-note gateway request failed");
            SyncError::SnapshotServiceUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > max_response_bytes as u64)
    {
        tracing::warn!(%status, "shared-note gateway response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        tracing::warn!(%error, "shared-note gateway response could not be read");
        SyncError::SnapshotServiceUnavailable
    })? {
        if bytes.len().saturating_add(chunk.len()) > max_response_bytes {
            tracing::warn!(%status, "shared-note gateway response was too large");
            return Err(SyncError::SnapshotServiceUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        tracing::warn!(%status, "shared-note gateway request was rejected");
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let mut rows = serde_json::from_slice::<Vec<Row>>(&bytes).map_err(|error| {
        tracing::warn!(%error, "shared-note gateway response was invalid");
        SyncError::SnapshotServiceUnavailable
    })?;
    match rows.len() {
        0 => Err(SyncError::SharedNoteNotFound),
        1 => Ok(rows.pop().expect("row count was checked")),
        row_count => {
            tracing::warn!(row_count, "shared-note gateway returned multiple rows");
            Err(SyncError::SnapshotServiceUnavailable)
        }
    }
}

fn validate_snapshot(
    row: GatewaySnapshotRow,
    expected_share_id: Option<&str>,
) -> Result<SharedNoteSnapshot> {
    let share_id =
        canonical_uuid(&row.share_id).ok_or_else(|| invalid_gateway_response("share"))?;
    let body_size = serde_json::to_vec(&row.body_json)
        .map_err(|_| invalid_gateway_response("body"))?
        .len();
    if expected_share_id.is_some_and(|expected| expected != share_id)
        || row.schema_version != 1
        || row.content_revision < 1
        || row.title.trim() != row.title
        || row.title.len() > 4096
        || row.body_json.get("type").and_then(Value::as_str) != Some("doc")
        || body_size > MAX_SNAPSHOT_BODY_BYTES
        || validate_shared_attachments(&row.attachments_json, None).is_err()
        || chrono::DateTime::parse_from_rfc3339(&row.published_at).is_err()
    {
        return Err(invalid_gateway_response("snapshot"));
    }

    Ok(SharedNoteSnapshot {
        share_id,
        schema_version: row.schema_version,
        content_revision: row.content_revision,
        title: row.title,
        body: row.body_json,
        attachments: row.attachments_json,
        lease_expires_at: None,
        published_at: row.published_at,
    })
}

fn validate_handoff(row: GatewayHandoffRow) -> Result<SharedNoteHandoff> {
    let request_id =
        canonical_uuid_v4(&row.request_id).ok_or_else(|| invalid_gateway_response("handoff"))?;
    if chrono::DateTime::parse_from_rfc3339(&row.expires_at).is_err() {
        return Err(invalid_gateway_response("handoff expiry"));
    }

    Ok(SharedNoteHandoff {
        request_id,
        expires_at: row.expires_at,
    })
}

fn invalid_gateway_response(field: &str) -> SyncError {
    tracing::warn!(field, "shared-note gateway response failed validation");
    SyncError::SnapshotServiceUnavailable
}

fn canonical_uuid(value: &str) -> Option<String> {
    let parsed = Uuid::parse_str(value).ok()?;
    let canonical = parsed.hyphenated().to_string();
    (canonical == value).then_some(canonical)
}

fn canonical_uuid_v4(value: &str) -> Option<String> {
    let parsed = Uuid::parse_str(value).ok()?;
    let canonical = parsed.hyphenated().to_string();
    (parsed.get_version_num() == 4 && canonical == value).then_some(canonical)
}

fn is_valid_link_token(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_valid_public_slug(value: &str) -> bool {
    value.len() == 34
        && value.starts_with("s_")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path},
    };

    use super::*;

    const SHARE_ID: &str = "11111111-1111-4111-8111-111111111111";
    const PUBLIC_SLUG: &str = "s_0123456789abcdef0123456789abcdef";
    const LINK_TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const HANDOFF_ID: &str = "22222222-2222-4222-8222-222222222222";
    const LEASE_ID: &str = "55555555-5555-4555-8555-555555555555";
    const ATTACHMENT_ID: &str = "33333333-3333-4333-8333-333333333333";
    const OWNER_ID: &str = "44444444-4444-4444-8444-444444444444";

    fn test_router(server: &MockServer) -> Router {
        router(SharedNotesState::new(
            SharedNotesConfig::new(server.uri(), "service-role-key").unwrap(),
        ))
    }

    fn test_source_hash(client_ip: Option<&str>) -> String {
        let state = SharedNotesState::new(
            SharedNotesConfig::new("https://project.supabase.co", "service-role-key").unwrap(),
        );
        let mut headers = HeaderMap::new();
        if let Some(client_ip) = client_ip {
            headers.insert(FLY_CLIENT_IP_HEADER, client_ip.parse().unwrap());
        }
        state.handoff_source_hash(&headers)
    }

    fn snapshot_row(title: &str) -> Value {
        json!({
            "share_id": SHARE_ID,
            "schema_version": 1,
            "content_revision": 2,
            "title": title,
            "body_json": {
                "type": "doc",
                "content": [{ "type": "paragraph" }]
            },
            "attachments_json": [],
            "published_at": "2026-07-16T10:00:00Z"
        })
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn mount_rpc(server: &MockServer, function: &str, request: Value, response: Value) {
        Mock::given(method("POST"))
            .and(path(format!("/rest/v1/rpc/{function}")))
            .and(header("apikey", "service-role-key"))
            .and(header("authorization", "Bearer service-role-key"))
            .and(body_partial_json(request))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn reads_public_snapshot_through_the_service_gateway() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "gateway_read_public_session_share_snapshot_v2",
            json!({ "p_public_slug": PUBLIC_SLUG }),
            json!([snapshot_row("Public note")]),
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["shareId"], SHARE_ID);
        assert_eq!(body["schemaVersion"], 1);
        assert_eq!(body["contentRevision"], 2);
        assert_eq!(body["title"], "Public note");
        assert!(body.get("accessVersion").is_none());
        assert!(body.get("workspaceId").is_none());
        assert!(body.get("sessionId").is_none());
        assert_eq!(body["attachments"], json!([]));
    }

    #[tokio::test]
    async fn authorizes_before_signing_a_public_attachment_download() {
        let server = MockServer::start().await;
        let object_key = format!("{OWNER_ID}/{SHARE_ID}/{ATTACHMENT_ID}.sna1");
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/gateway_prepare_public_session_share_attachment_download",
            ))
            .and(body_partial_json(json!({
                "p_public_slug": PUBLIC_SLUG,
                "p_attachment_id": ATTACHMENT_ID
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "share_id": SHARE_ID,
                "attachment_id": ATTACHMENT_ID,
                "object_key": object_key,
                "filename": "diagram.png",
                "content_type": "image/png",
                "size_bytes": 1024,
                "sha256": "a".repeat(64),
                "access_version": 3,
                "cleanup_not_before": "2026-07-16T10:10:00Z"
            }])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "signedURL": format!(
                    "/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}?token=download-token"
                )
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(
                Request::post(format!(
                    "/shared-notes/public/{PUBLIC_SLUG}/attachments/{ATTACHMENT_ID}/download"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["id"], ATTACHMENT_ID);
        assert_eq!(body["filename"], "diagram.png");
        assert_eq!(body["contentType"], "image/png");
        assert!(
            body["signedUrl"]
                .as_str()
                .unwrap()
                .contains("download-token")
        );
    }

    #[tokio::test]
    async fn accepts_a_maximum_size_document_with_bounded_envelope_overhead() {
        let server = MockServer::start().await;
        let mut body = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{ "type": "text", "text": "" }]
            }]
        });
        let empty_body_size = serde_json::to_vec(&body).unwrap().len();
        body["content"][0]["content"][0]["text"] =
            Value::String("x".repeat(MAX_SNAPSHOT_BODY_BYTES - empty_body_size));
        assert_eq!(
            serde_json::to_vec(&body).unwrap().len(),
            MAX_SNAPSHOT_BODY_BYTES
        );

        let mut row = snapshot_row("Maximum note");
        row["body_json"] = body;
        let gateway_response = json!([row]);
        let gateway_response_size = serde_json::to_vec(&gateway_response).unwrap().len();
        assert!(gateway_response_size > MAX_SNAPSHOT_BODY_BYTES);
        assert!(gateway_response_size <= MAX_SNAPSHOT_RESPONSE_BYTES);
        mount_rpc(
            &server,
            "gateway_read_public_session_share_snapshot_v2",
            json!({ "p_public_slug": PUBLIC_SLUG }),
            gateway_response,
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["title"], "Maximum note");
    }

    #[tokio::test]
    async fn reads_link_snapshot_without_forwarding_client_headers() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "gateway_read_session_share_link_snapshot_v2",
            json!({ "p_share_id": SHARE_ID, "p_link_token": LINK_TOKEN }),
            json!([snapshot_row("Link note")]),
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                Request::post(format!("/shared-notes/link/{SHARE_ID}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, "session=private")
                    .header(header::AUTHORIZATION, "Bearer user-token")
                    .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["title"], "Link note");
        let upstream = server.received_requests().await.unwrap().pop().unwrap();
        assert!(upstream.headers.get("cookie").is_none());
        assert_eq!(
            upstream.headers["authorization"].to_str().unwrap(),
            "Bearer service-role-key"
        );
        assert!(
            !String::from_utf8(upstream.body)
                .unwrap()
                .contains("user-token")
        );
    }

    #[tokio::test]
    async fn creates_and_leases_canonical_handoffs() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "gateway_create_public_session_share_handoff",
            json!({
                "p_public_slug": PUBLIC_SLUG,
                "p_source_hash": test_source_hash(Some("203.0.113.8"))
            }),
            json!([{
                "request_id": HANDOFF_ID,
                "expires_at": "2026-07-16T10:01:00Z"
            }]),
        )
        .await;
        let mut claimed_row = snapshot_row("Claimed note");
        claimed_row["lease_expires_at"] = json!("2099-07-16T10:20:00Z");
        mount_rpc(
            &server,
            "gateway_lease_session_share_handoff",
            json!({ "p_request_id": HANDOFF_ID, "p_lease_id": LEASE_ID }),
            json!([claimed_row]),
        )
        .await;

        let create_response = test_router(&server)
            .oneshot(
                Request::post(format!("/shared-notes/public/{PUBLIC_SLUG}/handoff"))
                    .header(FLY_CLIENT_IP_HEADER, "203.0.113.8")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let handoff = response_json(create_response).await;
        assert_eq!(handoff["requestId"], HANDOFF_ID);
        assert_eq!(handoff["expiresAt"], "2026-07-16T10:01:00Z");

        let claim_response = test_router(&server)
            .oneshot(
                Request::post("/shared-notes/handoffs/claim")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "requestId": HANDOFF_ID, "leaseId": LEASE_ID }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claim_response.status(), StatusCode::OK);
        let claimed = response_json(claim_response).await;
        assert_eq!(claimed["title"], "Claimed note");
        assert_eq!(claimed["leaseExpiresAt"], "2099-07-16T10:20:00Z");
    }

    #[tokio::test]
    async fn claims_manifest_without_signing_and_grants_one_attachment_just_in_time() {
        let server = MockServer::start().await;
        let object_key = format!("{OWNER_ID}/{SHARE_ID}/{ATTACHMENT_ID}.sna1");
        let attachment = json!({
            "id": ATTACHMENT_ID,
            "filename": "diagram.png",
            "contentType": "image/png",
            "sizeBytes": 1024,
            "sha256": "a".repeat(64)
        });
        let mut claimed_row = snapshot_row("Claimed attachment");
        claimed_row["attachments_json"] = json!([attachment]);
        claimed_row["lease_expires_at"] = json!("2099-07-16T10:20:00Z");
        mount_rpc(
            &server,
            "gateway_lease_session_share_handoff",
            json!({ "p_request_id": HANDOFF_ID, "p_lease_id": LEASE_ID }),
            json!([claimed_row]),
        )
        .await;
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/gateway_prepare_session_share_handoff_attachment_download",
            ))
            .and(body_partial_json(json!({
                "p_lease_id": LEASE_ID,
                "p_attachment_id": ATTACHMENT_ID
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "share_id": SHARE_ID,
                "attachment_id": ATTACHMENT_ID,
                "object_key": object_key,
                "filename": "diagram.png",
                "content_type": "image/png",
                "size_bytes": 1024,
                "sha256": "a".repeat(64),
                "access_version": 3,
                "cleanup_not_before": "2026-07-16T10:10:00Z"
            }])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "signedURL": format!(
                    "/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}?token=handoff-download"
                )
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(
                Request::post("/shared-notes/handoffs/claim")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "requestId": HANDOFF_ID, "leaseId": LEASE_ID }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["attachments"], json!([attachment]));
        assert!(body.get("attachmentDownloads").is_none());

        let response = test_router(&server)
            .oneshot(
                Request::post(format!(
                    "/shared-notes/handoffs/attachments/{ATTACHMENT_ID}/download"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "leaseId": LEASE_ID }).to_string()))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["id"], ATTACHMENT_ID);
        assert_eq!(body["filename"], "diagram.png");
        assert!(
            body["signedUrl"]
                .as_str()
                .unwrap()
                .contains("handoff-download")
        );
        assert!(body["expiresAt"].is_string());
        assert!(body.get("objectKey").is_none());
    }

    #[tokio::test]
    async fn creates_link_handoff_without_returning_the_bearer_token() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "gateway_create_session_share_link_handoff",
            json!({
                "p_share_id": SHARE_ID,
                "p_link_token": LINK_TOKEN,
                "p_source_hash": test_source_hash(None)
            }),
            json!([{
                "request_id": HANDOFF_ID,
                "expires_at": "2026-07-16T10:01:00Z"
            }]),
        )
        .await;

        let response = test_router(&server)
            .oneshot(
                Request::post(format!("/shared-notes/link/{SHARE_ID}/handoff"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["requestId"], HANDOFF_ID);
        assert!(!body.to_string().contains(LINK_TOKEN));
    }

    #[test]
    fn handoff_source_hashes_are_canonical_and_do_not_expose_client_ips() {
        let canonical = test_source_hash(Some("2001:db8::1"));
        let expanded = test_source_hash(Some("2001:0db8:0000:0000:0000:0000:0000:0001"));
        let other = test_source_hash(Some("2001:db8::2"));

        assert_eq!(canonical, expanded);
        assert_ne!(canonical, other);
        assert_eq!(canonical.len(), 64);
        assert!(canonical.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(!canonical.contains("2001"));
    }

    #[tokio::test]
    async fn rejects_invalid_capabilities_before_the_gateway() {
        let server = MockServer::start().await;
        let requests = [
            Request::get("/shared-notes/public/not-a-slug")
                .body(Body::empty())
                .unwrap(),
            Request::post("/shared-notes/link/not-a-uuid")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
                .unwrap(),
            Request::post(format!("/shared-notes/link/{SHARE_ID}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "token": "too-short" }).to_string()))
                .unwrap(),
            Request::post("/shared-notes/handoffs/claim")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "requestId": "22222222-2222-3222-8222-222222222222",
                        "leaseId": LEASE_ID
                    })
                    .to_string(),
                ))
                .unwrap(),
        ];

        for request in requests {
            let response = test_router(&server).oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response_json(response).await["error"]["code"],
                "shared_note_not_found"
            );
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn bounds_link_requests_and_redacts_gateway_failures() {
        let server = MockServer::start().await;
        let oversized = test_router(&server)
            .oneshot(
                Request::post(format!("/shared-notes/link/{SHARE_ID}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "token": "x".repeat(2000) }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(oversized.headers()[header::CACHE_CONTROL], "no-store");

        let oversized_claim = test_router(&server)
            .oneshot(
                Request::post("/shared-notes/handoffs/claim")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "requestId": "x".repeat(300) }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized_claim.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(oversized_claim.headers()[header::CACHE_CONTROL], "no-store");

        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/gateway_read_public_session_share_snapshot_v2",
            ))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret database detail"))
            .mount(&server)
            .await;
        let failed = test_router(&server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(failed).await.to_string();
        assert!(!body.contains("database detail"));
        assert!(!body.contains("service-role-key"));
    }

    #[tokio::test]
    async fn maps_empty_or_invalid_gateway_rows_without_leaking_details() {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "gateway_read_public_session_share_snapshot_v2",
            json!({ "p_public_slug": PUBLIC_SLUG }),
            json!([]),
        )
        .await;
        let missing = test_router(&server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let invalid_server = MockServer::start().await;
        mount_rpc(
            &invalid_server,
            "gateway_read_public_session_share_snapshot_v2",
            json!({ "p_public_slug": PUBLIC_SLUG }),
            json!([{
                "share_id": SHARE_ID,
                "schema_version": 1,
                "content_revision": 1,
                "title": "Private metadata",
                "body_json": { "type": "paragraph" },
                "attachments_json": [],
                "published_at": "2026-07-16T10:00:00Z"
            }]),
        )
        .await;
        let invalid = test_router(&invalid_server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response_json(invalid).await["error"]["code"],
            "shared_note_service_unavailable"
        );
    }

    #[tokio::test]
    async fn does_not_follow_gateway_redirects_with_service_credentials() {
        let redirect_target = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([snapshot_row("Leak")])))
            .mount(&redirect_target)
            .await;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/gateway_read_public_session_share_snapshot_v2",
            ))
            .respond_with(
                ResponseTemplate::new(307)
                    .insert_header("location", format!("{}/steal", redirect_target.uri())),
            )
            .mount(&server)
            .await;

        let response = test_router(&server)
            .oneshot(
                Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            redirect_target
                .received_requests()
                .await
                .unwrap()
                .is_empty()
        );
    }
}
