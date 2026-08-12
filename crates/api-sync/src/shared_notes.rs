use std::{net::IpAddr, time::Duration};

use anlg_api_auth::AuthContext;
use anlg_loops::LoopClient;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use utoipa::OpenApi;

mod gateway;
mod invitation;

use gateway::{
    attachment_rpc_single, canonical_uuid, canonical_uuid_v4, future_timestamp,
    is_valid_link_preview_token, is_valid_link_token, is_valid_public_slug, rpc_single,
    sign_attachment_download, validate_handoff, validate_handoff_lease, validate_snapshot,
};
use invitation::__path_send_shared_note_invitation_email;
use invitation::send_shared_note_invitation_email;

use crate::{
    SharedNotesConfig,
    error::{Result, SyncError},
    routes::SharedNoteAttachment,
    snapshot::{MAX_SNAPSHOT_BODY_BYTES, sanitize_title},
};

const SHARED_NOTE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_LINK_REQUEST_BYTES: usize = 1024;
const MAX_PREVIEW_SUMMARY_CHARS: usize = 180;
const MAX_PREVIEW_DOCUMENT_DEPTH: usize = 64;
const MAX_PREVIEW_DOCUMENT_NODES: usize = 50_000;
const MAX_HANDOFF_CLAIM_REQUEST_BYTES: usize = 256;
const MAX_SNAPSHOT_RESPONSE_BYTES: usize = MAX_SNAPSHOT_BODY_BYTES + 256 * 1024;
const MAX_HANDOFF_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_INVITATION_EMAIL_REQUEST_BYTES: usize = 8 * 1024;
const MAX_RECAP_EMAIL_REQUEST_BYTES: usize = 128 * 1024;
const MAX_ACCESS_LIST_RESPONSE_BYTES: usize = 1024 * 1024;
const SHARED_ATTACHMENT_BUCKET: &str = "shared-note-attachments";
const ATTACHMENT_DOWNLOAD_TTL_SECONDS: i64 = 60;
const FLY_CLIENT_IP_HEADER: &str = "fly-client-ip";
const HANDOFF_SOURCE_DOMAIN: &[u8] = b"anarlog:shared-note-handoff-source:v1\0";
const INVITATION_TRANSACTIONAL_ID: &str = "cmrvkrh3c0k0t0jvh80zpkk93";

mod email_delivery;

pub fn recap_openapi() -> utoipa::openapi::OpenApi {
    invitation::recap_openapi()
}

#[derive(Clone)]
pub struct SharedNotesState {
    config: SharedNotesConfig,
    client: reqwest::Client,
    storage: anlg_supabase_storage::SupabaseStorage,
    email_delivery: Option<email_delivery::EmailDelivery>,
}

impl SharedNotesState {
    pub fn new(config: SharedNotesConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(SHARED_NOTE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("shared-note HTTP client must build");
        let storage = anlg_supabase_storage::SupabaseStorage::new(
            client.clone(),
            &config.supabase_url,
            &config.supabase_service_role_key,
        );
        let email_delivery = if let (Some(api_key), Some(from_email)) = (
            config.resend_api_key.clone(),
            config.resend_from_email.clone(),
        ) {
            Some(email_delivery::EmailDelivery::Resend(
                email_delivery::ResendClient::new(
                    client.clone(),
                    config.resend_api_base.clone(),
                    api_key,
                    from_email,
                ),
            ))
        } else {
            config.loops_api_key.as_ref().map(|api_key| {
                let mut builder = LoopClient::builder().api_key(api_key);
                if let Some(api_base) = config.loops_api_base.clone() {
                    builder = builder.api_base(api_base);
                }
                email_delivery::EmailDelivery::Loops(builder.build())
            })
        };
        Self {
            config,
            client,
            storage,
            email_delivery,
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
pub struct SharedNoteLinkPreviewRequest {
    preview_token: String,
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

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedNoteInvitationEmailRequest {
    share_id: String,
    invite_token: String,
    note_title: String,
    #[serde(default)]
    from_name: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingRecapEmailRequest {
    recipients: Vec<String>,
    sender_name: String,
    note_title: String,
    note_body: String,
    delivery_id: String,
}

#[derive(Deserialize)]
struct SessionShareAccessRow {
    entry_type: String,
    entry_id: String,
    user_email: Option<String>,
    status: String,
}

#[derive(Serialize)]
struct ListSessionShareAccessRequest<'a> {
    p_share_id: &'a str,
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
pub struct SharedNotePreview {
    title: String,
    summary: String,
    participants: Vec<String>,
    meeting_at: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SharedNoteLinkPreview {
    share_id: String,
    title: String,
    summary: String,
    participants: Vec<String>,
    meeting_at: String,
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
struct LinkPreviewRpcRequest<'a> {
    p_share_id: &'a str,
    p_preview_token: &'a str,
}

#[derive(Serialize)]
struct LinkIdRpcRequest<'a> {
    p_link_id: &'a str,
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
#[serde(deny_unknown_fields)]
struct GatewayPreviewRow {
    title: String,
    body_json: Value,
    participants: Vec<String>,
    meeting_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayLinkPreviewRow {
    share_id: String,
    title: String,
    body_json: Value,
    participants: Vec<String>,
    meeting_at: String,
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
        read_public_shared_note_preview,
        read_link_shared_note,
        read_link_shared_note_preview,
        read_short_link_shared_note_preview,
        create_public_shared_note_handoff,
        create_link_shared_note_handoff,
        claim_shared_note_handoff,
        download_handoff_shared_attachment,
        download_public_shared_attachment,
        download_link_shared_attachment,
        download_access_shared_attachment,
        send_shared_note_invitation_email
    ),
    components(schemas(
        SharedNoteLinkRequest,
        SharedNoteLinkPreviewRequest,
        SharedNoteHandoffClaimRequest,
        SharedNoteHandoffAttachmentRequest,
        SharedNoteInvitationEmailRequest,
        SharedNoteSnapshot,
        SharedNotePreview,
        SharedNoteLinkPreview,
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
            "/shared-notes/public/{slug}/preview",
            get(read_public_shared_note_preview),
        )
        .route(
            "/shared-notes/link/{share_id}",
            post(read_link_shared_note).layer(DefaultBodyLimit::max(MAX_LINK_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/link/{share_id}/preview",
            post(read_link_shared_note_preview)
                .layer(DefaultBodyLimit::max(MAX_LINK_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/links/{link_id}/preview",
            get(read_short_link_shared_note_preview),
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
            "/shared-notes/invitations/{invitation_id}/email",
            post(send_shared_note_invitation_email)
                .layer(DefaultBodyLimit::max(MAX_INVITATION_EMAIL_REQUEST_BYTES)),
        )
        .route(
            "/shared-notes/{share_id}/recap/email",
            post(invitation::send_shared_note_recap_email)
                .layer(DefaultBodyLimit::max(MAX_RECAP_EMAIL_REQUEST_BYTES)),
        )
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
    get,
    path = "/shared-notes/public/{slug}/preview",
    tag = "shared-notes",
    params(("slug" = String, Path, description = "Public share slug")),
    responses(
        (status = 200, description = "Public shared note preview", body = SharedNotePreview),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn read_public_shared_note_preview(
    State(state): State<SharedNotesState>,
    Path(slug): Path<String>,
) -> Result<Json<SharedNotePreview>> {
    if !is_valid_public_slug(&slug) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let row = rpc_single(
        &state,
        "gateway_read_public_session_share_preview",
        &PublicSlugRpcRequest {
            p_public_slug: &slug,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_preview(row)?))
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
    path = "/shared-notes/link/{share_id}/preview",
    tag = "shared-notes",
    params(("share_id" = String, Path, description = "Session share ID")),
    request_body = SharedNoteLinkPreviewRequest,
    responses(
        (status = 200, description = "Bearer-link shared note preview", body = SharedNotePreview),
        (status = 404, description = "Shared note unavailable"),
        (status = 413, description = "Request too large"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn read_link_shared_note_preview(
    State(state): State<SharedNotesState>,
    Path(share_id): Path<String>,
    Json(request): Json<SharedNoteLinkPreviewRequest>,
) -> Result<Json<SharedNotePreview>> {
    let share_id = canonical_uuid(&share_id).ok_or(SyncError::SharedNoteNotFound)?;
    if !is_valid_link_preview_token(&request.preview_token) {
        return Err(SyncError::SharedNoteNotFound);
    }

    let row = rpc_single(
        &state,
        "gateway_read_session_share_link_preview",
        &LinkPreviewRpcRequest {
            p_share_id: &share_id,
            p_preview_token: &request.preview_token,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_preview(row)?))
}

#[utoipa::path(
    get,
    path = "/shared-notes/links/{link_id}/preview",
    tag = "shared-notes",
    params(("link_id" = String, Path, description = "Session share link ID")),
    responses(
        (status = 200, description = "Short-link shared note preview", body = SharedNoteLinkPreview),
        (status = 404, description = "Shared note unavailable"),
        (status = 502, description = "Shared note service unavailable")
    )
)]
async fn read_short_link_shared_note_preview(
    State(state): State<SharedNotesState>,
    Path(link_id): Path<String>,
) -> Result<Json<SharedNoteLinkPreview>> {
    let link_id = canonical_uuid_v4(&link_id).ok_or(SyncError::SharedNoteNotFound)?;
    let row = rpc_single(
        &state,
        "gateway_read_session_share_link_preview_by_id",
        &LinkIdRpcRequest {
            p_link_id: &link_id,
        },
        MAX_SNAPSHOT_RESPONSE_BYTES,
    )
    .await?;
    Ok(Json(validate_link_preview(row)?))
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

fn validate_preview(row: GatewayPreviewRow) -> Result<SharedNotePreview> {
    let title = sanitize_title(&row.title).map_err(|_| SyncError::SnapshotServiceUnavailable)?;
    if title != row.title || row.participants.len() > 32 {
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let mut participants = Vec::with_capacity(row.participants.len());
    let mut seen = std::collections::HashSet::new();
    for participant in row.participants {
        if participant.is_empty()
            || participant.trim() != participant
            || participant.chars().count() > 100
            || !seen.insert(participant.clone())
        {
            return Err(SyncError::SnapshotServiceUnavailable);
        }
        participants.push(participant);
    }
    if chrono::DateTime::parse_from_rfc3339(&row.meeting_at).is_err() {
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let summary = extract_preview_summary(&row.body_json, &title)?;
    Ok(SharedNotePreview {
        title,
        summary,
        participants,
        meeting_at: row.meeting_at,
    })
}

fn validate_link_preview(row: GatewayLinkPreviewRow) -> Result<SharedNoteLinkPreview> {
    let share_id = canonical_uuid_v4(&row.share_id).ok_or(SyncError::SnapshotServiceUnavailable)?;
    let preview = validate_preview(GatewayPreviewRow {
        title: row.title,
        body_json: row.body_json,
        participants: row.participants,
        meeting_at: row.meeting_at,
    })?;
    Ok(SharedNoteLinkPreview {
        share_id,
        title: preview.title,
        summary: preview.summary,
        participants: preview.participants,
        meeting_at: preview.meeting_at,
    })
}

fn extract_preview_summary(body: &Value, title: &str) -> Result<String> {
    if body.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    let mut visited = 0;
    Ok(find_preview_paragraph(body, title, 0, &mut visited)?.unwrap_or_default())
}

fn find_preview_paragraph(
    node: &Value,
    title: &str,
    depth: usize,
    visited: &mut usize,
) -> Result<Option<String>> {
    if depth > MAX_PREVIEW_DOCUMENT_DEPTH || *visited >= MAX_PREVIEW_DOCUMENT_NODES {
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    *visited += 1;

    if node.get("type").and_then(Value::as_str) == Some("paragraph") {
        let mut text = String::new();
        collect_preview_text(node, depth, &mut text)?;
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalized.is_empty() && normalized != title {
            let truncated = normalized
                .chars()
                .take(MAX_PREVIEW_SUMMARY_CHARS - 1)
                .collect::<String>();
            if normalized.chars().count() > MAX_PREVIEW_SUMMARY_CHARS {
                return Ok(Some(format!("{}…", truncated.trim_end())));
            }
            return Ok(Some(normalized));
        }
    }

    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            if let Some(summary) = find_preview_paragraph(child, title, depth + 1, visited)? {
                return Ok(Some(summary));
            }
        }
    }
    Ok(None)
}

fn collect_preview_text(node: &Value, depth: usize, output: &mut String) -> Result<()> {
    if depth > MAX_PREVIEW_DOCUMENT_DEPTH {
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let node_type = node.get("type").and_then(Value::as_str);
    if node_type == Some("hardBreak") {
        output.push(' ');
    }
    if node_type == Some("text")
        && let Some(text) = node.get("text").and_then(Value::as_str)
    {
        output.push_str(text);
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            collect_preview_text(child, depth + 1, output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
