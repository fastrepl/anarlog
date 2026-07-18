use std::collections::HashSet;

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, header},
    routing::{post, put},
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use hypr_api_auth::AuthContext;
use reqwest::StatusCode as HttpStatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::config::CloudsyncProtocolMode;
use crate::error::{Result, SyncError};
use crate::snapshot::{
    MAX_SNAPSHOT_BODY_BYTES, sanitize_document_with_attachments, sanitize_title,
};
use crate::state::AppState;

mod attachment_backups;
mod e2ee_witness;
mod shared_attachments;

const WORKSPACE_PROJECTION_SELECT: &str = "id,user_id,role,created_at,updated_at,workspace:workspaces!inner(id,owner_user_id,kind,name,created_at,updated_at)";
const WORKSPACE_PROJECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_TOKEN_WORKSPACES: usize = 128;
const MAX_TOKEN_ATTRIBUTES_BYTES: usize = 8 * 1024;
const SNAPSHOT_PUBLISH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_SNAPSHOT_REQUEST_BYTES: usize = MAX_SNAPSHOT_BODY_BYTES + 16 * 1024;
const MAX_SNAPSHOT_RESPONSE_BYTES: u64 = (MAX_SNAPSHOT_BODY_BYTES + 16 * 1024) as u64;
const CLOUDSYNC_ENCRYPTION_VERSION: u8 = 2;
const E2EE_KEY_ID_HEADER: &str = "x-anarlog-e2ee-key-id";

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncCredentials {
    encryption_version: u8,
    encryption_key_id: String,
    database_id: String,
    token: String,
    expires_at: String,
    workspace_id: String,
    account_user_id: String,
    personal_workspace_id: String,
    workspaces: Vec<CloudsyncWorkspace>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCloudsyncCredentials {
    database_id: String,
    token: String,
    expires_at: String,
    workspace_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum CloudsyncCredentialResponse {
    Legacy(LegacyCloudsyncCredentials),
    E2ee(CloudsyncCredentials),
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimE2eeIdentityRequest {
    key_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct E2eeIdentity {
    key_id: String,
}

#[derive(Serialize)]
struct ClaimE2eeKeyRpcRequest<'a> {
    p_actor_user_id: &'a str,
    p_key_id: &'a str,
}

#[derive(Deserialize)]
struct E2eeKeyIdRow {
    key_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudsyncWorkspace {
    id: String,
    owner_user_id: String,
    kind: String,
    name: String,
    membership_id: String,
    role: String,
    membership_created_at: String,
    membership_updated_at: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct E2eeCreateTokenRequest<'a> {
    name: &'static str,
    user_id: &'a str,
    expires_at: &'a str,
    attributes: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyCreateTokenRequest<'a> {
    name: &'static str,
    user_id: &'a str,
    expires_at: &'a str,
}

#[derive(Deserialize)]
struct CreateTokenEnvelope {
    data: CreateTokenResponse,
}

#[derive(Deserialize)]
struct CreateTokenResponse {
    token: String,
}

#[derive(Deserialize)]
struct WorkspaceMembershipRow {
    id: String,
    user_id: String,
    role: String,
    created_at: String,
    updated_at: String,
    workspace: WorkspaceRow,
}

#[derive(Deserialize)]
struct WorkspaceRow {
    id: String,
    owner_user_id: String,
    kind: String,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishSessionShareSnapshotRequest {
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
pub struct PublishedSessionShareSnapshot {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body: Value,
    attachments: Vec<SharedNoteAttachment>,
    published_at: String,
}

#[derive(Serialize)]
struct PublishSnapshotRpcRequest<'a> {
    p_share_id: &'a str,
    p_actor_user_id: &'a str,
    p_title: &'a str,
    p_body_json: &'a Value,
    p_attachment_ids: Option<&'a [String]>,
}

#[derive(Deserialize)]
struct PublishedSnapshotRow {
    share_id: String,
    schema_version: i16,
    content_revision: i64,
    title: String,
    body_json: Value,
    attachments_json: Vec<SharedNoteAttachment>,
    published_at: String,
}

#[derive(Deserialize)]
struct PostgrestError {
    code: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        create_credentials,
        claim_e2ee_identity,
        publish_session_share_snapshot
    ),
    components(schemas(
        CloudsyncCredentialResponse,
        CloudsyncCredentials,
        CloudsyncWorkspace,
        ClaimE2eeIdentityRequest,
        E2eeIdentity,
        LegacyCloudsyncCredentials,
        PublishSessionShareSnapshotRequest,
        PublishedSessionShareSnapshot,
        SharedNoteAttachment
    ))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut openapi = ApiDoc::openapi();
    openapi.merge(attachment_backups::openapi());
    openapi.merge(e2ee_witness::openapi());
    openapi.merge(shared_attachments::openapi());
    openapi
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/token", post(create_credentials))
        .route("/e2ee/identity", put(claim_e2ee_identity))
        .merge(e2ee_witness::router())
        .route(
            "/shares/{share_id}/snapshot",
            put(publish_session_share_snapshot)
                .layer(DefaultBodyLimit::max(MAX_SNAPSHOT_REQUEST_BYTES)),
        )
        .merge(attachment_backups::router())
        .merge(shared_attachments::router())
        .with_state(state)
}

#[utoipa::path(
    put,
    path = "/e2ee/identity",
    tag = "sync",
    request_body = ClaimE2eeIdentityRequest,
    responses(
        (status = 200, description = "E2EE recovery-key identity claimed", body = E2eeIdentity),
        (status = 400, description = "Invalid E2EE key identity"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 409, description = "Account already uses a different recovery key"),
        (status = 502, description = "E2EE identity service unavailable")
    )
)]
async fn claim_e2ee_identity(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(request): Json<ClaimE2eeIdentityRequest>,
) -> Result<([(header::HeaderName, HeaderValue); 1], Json<E2eeIdentity>)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let key_id = claim_personal_e2ee_key(&state, &auth.claims.sub, &request.key_id).await?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(E2eeIdentity { key_id }),
    ))
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
        (status = 413, description = "Shared-note snapshot is too large"),
        (status = 502, description = "Shared-note service unavailable")
    )
)]
async fn publish_session_share_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(share_id): Path<String>,
    Json(request): Json<PublishSessionShareSnapshotRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<PublishedSessionShareSnapshot>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let share_id = Uuid::parse_str(&share_id)
        .map_err(|_| SyncError::BadRequest("Shared note ID is invalid".to_string()))?
        .to_string();
    let title = sanitize_title(&request.title)?;
    let mut attachment_ids = HashSet::new();
    if let Some(requested_attachment_ids) = &request.attachment_ids {
        if requested_attachment_ids.len() > 64 {
            return Err(SyncError::BadRequest(
                "Shared note has too many attachments".to_string(),
            ));
        }
        for attachment_id in requested_attachment_ids {
            let uuid = Uuid::parse_str(attachment_id).map_err(|_| {
                SyncError::BadRequest("Shared attachment ID is invalid".to_string())
            })?;
            if uuid.to_string() != *attachment_id
                || uuid.get_version() != Some(uuid::Version::Random)
                || !attachment_ids.insert(attachment_id.clone())
            {
                return Err(SyncError::BadRequest(
                    "Shared attachment ID is invalid".to_string(),
                ));
            }
        }
    }
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
        .json(&PublishSnapshotRpcRequest {
            p_share_id: &share_id,
            p_actor_user_id: &auth.claims.sub,
            p_title: &title,
            p_body_json: &body,
            p_attachment_ids: request.attachment_ids.as_deref(),
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase shared-note publication request failed");
            SyncError::SnapshotServiceUnavailable
        })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPSHOT_RESPONSE_BYTES)
    {
        tracing::warn!(%status, "Supabase shared-note publication response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let bytes = response.bytes().await.map_err(|error| {
        tracing::warn!(%error, "Supabase shared-note publication response could not be read");
        SyncError::SnapshotServiceUnavailable
    })?;
    if bytes.len() as u64 > MAX_SNAPSHOT_RESPONSE_BYTES {
        tracing::warn!(%status, "Supabase shared-note publication response was too large");
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    if !status.is_success() {
        let code = serde_json::from_slice::<PostgrestError>(&bytes)
            .ok()
            .map(|error| error.code);
        tracing::warn!(%status, ?code, "Supabase shared-note publication was rejected");
        return match (status, code.as_deref()) {
            (HttpStatusCode::UNAUTHORIZED | HttpStatusCode::FORBIDDEN, _) | (_, Some("42501")) => {
                Err(SyncError::SnapshotPublicationForbidden)
            }
            (_, Some("22023")) => Err(SyncError::BadRequest(
                "Shared note snapshot is invalid".to_string(),
            )),
            _ => Err(SyncError::SnapshotServiceUnavailable),
        };
    }

    let mut rows =
        serde_json::from_slice::<Vec<PublishedSnapshotRow>>(&bytes).map_err(|error| {
            tracing::warn!(%error, "Supabase shared-note publication response was invalid");
            SyncError::SnapshotServiceUnavailable
        })?;
    if rows.len() != 1 {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase shared-note publication returned an invalid row count"
        );
        return Err(SyncError::SnapshotServiceUnavailable);
    }
    let row = rows.pop().expect("row count was checked");
    if row.share_id != share_id
        || row.schema_version != 1
        || row.content_revision < 1
        || row.title != title
        || row.body_json != body
        || validate_shared_attachments(&row.attachments_json, request.attachment_ids.as_deref())
            .is_err()
        || chrono::DateTime::parse_from_rfc3339(&row.published_at).is_err()
    {
        tracing::warn!("Supabase shared-note publication response failed validation");
        return Err(SyncError::SnapshotServiceUnavailable);
    }

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(PublishedSessionShareSnapshot {
            share_id: row.share_id,
            schema_version: row.schema_version,
            content_revision: row.content_revision,
            title: row.title,
            body: row.body_json,
            attachments: row.attachments_json,
            published_at: row.published_at,
        }),
    ))
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

#[utoipa::path(
    post,
    path = "/token",
    tag = "sync",
    params(("x-anarlog-e2ee-key-id" = Option<String>, Header, description = "Local recovery-key identity")),
    responses(
        (status = 200, description = "Short-lived CloudSync credentials", body = CloudsyncCredentialResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 426, description = "Desktop upgrade required"),
        (status = 502, description = "Credential issuer unavailable")
    )
)]
async fn create_credentials(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<CloudsyncCredentialResponse>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    let requested_key_id = headers
        .get(E2EE_KEY_ID_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| SyncError::BadRequest("E2EE key identity is invalid".to_string()))
        })
        .transpose()?;
    if requested_key_id.is_some_and(|key_id| !is_valid_e2ee_key_id(key_id)) {
        return Err(SyncError::BadRequest(
            "E2EE key identity is invalid".to_string(),
        ));
    }

    let expires_at = token_expiry(state.config.token_ttl_seconds)?;
    if requested_key_id.is_none() {
        if state.config.protocol_mode != CloudsyncProtocolMode::Dual {
            return Err(SyncError::CloudsyncUpgradeRequired);
        }

        let database_id = state.config.legacy_database_id.clone().ok_or_else(|| {
            SyncError::Internal("Legacy CloudSync database is missing".to_string())
        })?;
        let token = mint_cloudsync_token(
            &state,
            &LegacyCreateTokenRequest {
                name: "anarlog-cloudsync",
                user_id: &auth.claims.sub,
                expires_at: &expires_at,
            },
        )
        .await?;
        tracing::info!(protocol_version = 1, "issued legacy CloudSync credentials");

        return Ok((
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            Json(CloudsyncCredentialResponse::Legacy(
                LegacyCloudsyncCredentials {
                    database_id,
                    token,
                    expires_at,
                    workspace_id: auth.claims.sub,
                },
            )),
        ));
    }
    let requested_key_id = requested_key_id.expect("header presence was checked");

    let workspace_rows = fetch_workspace_projection(&state, &auth).await?;
    let (personal_workspace_id, workspaces) =
        validate_workspace_projection(workspace_rows, &auth.claims.sub)?;
    let encryption_key_id =
        claim_personal_e2ee_key(&state, &auth.claims.sub, requested_key_id).await?;
    let token_attributes = encode_workspace_token_attributes(&workspaces)?;

    let token = mint_cloudsync_token(
        &state,
        &E2eeCreateTokenRequest {
            name: "anarlog-cloudsync",
            user_id: &auth.claims.sub,
            expires_at: &expires_at,
            attributes: &token_attributes,
        },
    )
    .await?;

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(CloudsyncCredentialResponse::E2ee(CloudsyncCredentials {
            encryption_version: CLOUDSYNC_ENCRYPTION_VERSION,
            encryption_key_id,
            database_id: state.config.database_id,
            token,
            expires_at,
            workspace_id: personal_workspace_id.clone(),
            account_user_id: auth.claims.sub,
            personal_workspace_id,
            workspaces,
        })),
    ))
}

fn token_expiry(token_ttl_seconds: u64) -> Result<String> {
    let ttl = i64::try_from(token_ttl_seconds)
        .map_err(|_| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    let ttl = TimeDelta::try_seconds(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token TTL is too large".to_string()))?;
    Utc::now()
        .checked_add_signed(ttl)
        .ok_or_else(|| SyncError::Internal("CloudSync token expiry is invalid".to_string()))
        .map(|expiry| expiry.to_rfc3339_opts(SecondsFormat::Secs, true))
}

async fn mint_cloudsync_token(state: &AppState, request: &impl Serialize) -> Result<String> {
    let response = state
        .client
        .post(format!("{}/v2/tokens", state.config.project_url))
        .bearer_auth(&state.config.token_issuer_api_key)
        .json(request)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "SQLite Cloud token request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "SQLite Cloud token request was rejected");
        return Err(SyncError::Upstream);
    }
    let response: CreateTokenEnvelope = response.json().await.map_err(|error| {
        tracing::warn!(%error, "SQLite Cloud token response was invalid");
        SyncError::Upstream
    })?;
    if response.data.token.trim().is_empty() {
        return Err(SyncError::Upstream);
    }
    Ok(response.data.token)
}

async fn claim_personal_e2ee_key(
    state: &AppState,
    account_user_id: &str,
    requested_key_id: &str,
) -> Result<String> {
    if !is_valid_e2ee_key_id(requested_key_id) {
        return Err(SyncError::BadRequest(
            "E2EE key identity is invalid".to_string(),
        ));
    }

    let response = state
        .client
        .post(format!(
            "{}/rest/v1/rpc/claim_personal_workspace_e2ee_key",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .timeout(WORKSPACE_PROJECTION_TIMEOUT)
        .json(&ClaimE2eeKeyRpcRequest {
            p_actor_user_id: account_user_id,
            p_key_id: requested_key_id,
        })
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE identity request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "Supabase E2EE identity request was rejected");
        return Err(SyncError::Upstream);
    }
    let mut rows = response
        .json::<Vec<E2eeKeyIdRow>>()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase E2EE identity response was invalid");
            SyncError::Upstream
        })?;
    if rows.len() != 1 || !is_valid_e2ee_key_id(&rows[0].key_id) {
        tracing::warn!(
            row_count = rows.len(),
            "Supabase E2EE identity response failed validation"
        );
        return Err(SyncError::Upstream);
    }
    let key_id = rows.pop().expect("row count was checked").key_id;
    if key_id != requested_key_id {
        return Err(SyncError::E2eeKeyMismatch);
    }
    Ok(key_id)
}

fn is_valid_e2ee_key_id(key_id: &str) -> bool {
    key_id.len() == 22
        && key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

async fn fetch_workspace_projection(
    state: &AppState,
    auth: &AuthContext,
) -> Result<Vec<WorkspaceMembershipRow>> {
    let user_filter = format!("eq.{}", auth.claims.sub);
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/workspace_memberships",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_anon_key)
        .bearer_auth(&auth.token)
        .timeout(WORKSPACE_PROJECTION_TIMEOUT)
        .query(&[
            ("select", WORKSPACE_PROJECTION_SELECT),
            ("user_id", user_filter.as_str()),
            ("deleted_at", "is.null"),
            ("workspace.deleted_at", "is.null"),
        ])
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase workspace projection request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase workspace projection request was rejected"
        );
        return Err(SyncError::Upstream);
    }

    response.json().await.map_err(|error| {
        tracing::warn!(%error, "Supabase workspace projection response was invalid");
        SyncError::Upstream
    })
}

fn validate_workspace_projection(
    mut rows: Vec<WorkspaceMembershipRow>,
    account_user_id: &str,
) -> Result<(String, Vec<CloudsyncWorkspace>)> {
    let mut membership_ids = HashSet::with_capacity(rows.len());
    let mut workspace_ids = HashSet::with_capacity(rows.len());

    for row in &rows {
        if row.user_id != account_user_id {
            return invalid_workspace_projection("membership user does not match account");
        }
        if row.id.trim().is_empty()
            || row.workspace.id.trim().is_empty()
            || row.workspace.owner_user_id.trim().is_empty()
        {
            return invalid_workspace_projection("workspace projection contains a blank identity");
        }
        if !matches!(row.role.as_str(), "owner" | "admin" | "member") {
            return invalid_workspace_projection("workspace membership has an invalid role");
        }
        if !matches!(row.workspace.kind.as_str(), "personal" | "shared") {
            return invalid_workspace_projection("workspace has an invalid kind");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.workspace.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.workspace.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace has an invalid timestamp");
        }
        if chrono::DateTime::parse_from_rfc3339(&row.created_at).is_err()
            || chrono::DateTime::parse_from_rfc3339(&row.updated_at).is_err()
        {
            return invalid_workspace_projection("workspace membership has an invalid timestamp");
        }
        if !membership_ids.insert(&row.id) || !workspace_ids.insert(&row.workspace.id) {
            return invalid_workspace_projection("workspace projection contains duplicate rows");
        }
    }

    let personal_workspaces = rows
        .iter()
        .filter(|row| row.workspace.kind == "personal")
        .collect::<Vec<_>>();
    if personal_workspaces.len() != 1 {
        return invalid_workspace_projection(
            "account must have exactly one personal owner workspace",
        );
    }

    let personal_workspace = personal_workspaces[0];
    if personal_workspace.role != "owner"
        || personal_workspace.workspace.id != account_user_id
        || personal_workspace.workspace.owner_user_id != account_user_id
    {
        return invalid_workspace_projection("personal workspace identity does not match account");
    }

    let personal_workspace_id = personal_workspace.workspace.id.clone();
    rows.sort_by(|left, right| {
        let left_is_personal = left.workspace.id == personal_workspace_id;
        let right_is_personal = right.workspace.id == personal_workspace_id;
        right_is_personal
            .cmp(&left_is_personal)
            .then_with(|| left.workspace.created_at.cmp(&right.workspace.created_at))
            .then_with(|| left.workspace.id.cmp(&right.workspace.id))
    });

    Ok((
        personal_workspace_id,
        rows.into_iter()
            .map(|row| CloudsyncWorkspace {
                id: row.workspace.id,
                owner_user_id: row.workspace.owner_user_id,
                kind: row.workspace.kind,
                name: row.workspace.name,
                membership_id: row.id,
                role: row.role,
                membership_created_at: row.created_at,
                membership_updated_at: row.updated_at,
                created_at: row.workspace.created_at,
                updated_at: row.workspace.updated_at,
            })
            .collect(),
    ))
}

fn invalid_workspace_projection<T>(reason: &'static str) -> Result<T> {
    tracing::warn!(reason, "Supabase workspace projection failed validation");
    Err(SyncError::Upstream)
}

fn encode_workspace_token_attributes(workspaces: &[CloudsyncWorkspace]) -> Result<String> {
    if workspaces.len() > MAX_TOKEN_WORKSPACES {
        tracing::warn!(
            workspace_count = workspaces.len(),
            "CloudSync workspace projection exceeds token limit"
        );
        return Err(SyncError::Upstream);
    }

    let attributes = serde_json::to_string(&serde_json::json!({
        "workspace_ids": workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| {
        SyncError::Internal(format!("CloudSync token attributes are invalid: {error}"))
    })?;
    if attributes.len() > MAX_TOKEN_ATTRIBUTES_BYTES {
        tracing::warn!(
            attribute_bytes = attributes.len(),
            "CloudSync workspace projection exceeds token size limit"
        );
        return Err(SyncError::Upstream);
    }

    Ok(attributes)
}

#[cfg(test)]
mod tests {
    use axum::{Extension, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use hypr_api_auth::{AuthContext, Claims};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    use super::*;
    use crate::SyncConfig;

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
        router(AppState::new(SyncConfig {
            project_url: server.uri(),
            token_issuer_api_key: api_key.to_string(),
            database_id: "database-id".to_string(),
            legacy_database_id: legacy_database_id.map(ToString::to_string),
            protocol_mode,
            token_ttl_seconds: 60,
            supabase_url: server.uri(),
            supabase_anon_key: "anon-key".to_string(),
            supabase_service_role_key: "service-role-key".to_string(),
        }))
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

    fn token_request() -> Request<Body> {
        Request::post("/token")
            .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
            .body(Body::empty())
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn mints_token_for_verified_supabase_subject() {
        let server = MockServer::start().await;
        mock_workspace_projection(
            &server,
            json!([
                {
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
                },
                personal_workspace("user-123")
            ]),
        )
        .await;
        mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .and(header("authorization", "Bearer issuer-key"))
            .and(body_partial_json(json!({
                "name": "anarlog-cloudsync",
                "userId": "user-123"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "token": "sqlite-token" }
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(token_request())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["databaseId"], "database-id");
        assert_eq!(body["encryptionVersion"], 2);
        assert_eq!(body["encryptionKeyId"], TEST_KEY_ID);
        assert_eq!(body["token"], "sqlite-token");
        assert_eq!(body["workspaceId"], "user-123");
        assert_eq!(body["accountUserId"], "user-123");
        assert_eq!(body["personalWorkspaceId"], "user-123");
        assert_eq!(body["workspaces"][0]["id"], "user-123");
        assert_eq!(body["workspaces"][0]["membershipId"], "user-123");
        assert_eq!(body["workspaces"][0]["role"], "owner");
        assert_eq!(body["workspaces"][1]["id"], "workspace-team");
        assert_eq!(body["workspaces"][1]["ownerUserId"], "user-456");
        assert_eq!(body["workspaces"][1]["kind"], "shared");
        assert_eq!(body["workspaces"][1]["name"], "Acme");
        assert_eq!(
            body["workspaces"][1]["membershipCreatedAt"],
            "2026-07-16T09:01:00Z"
        );
        assert_eq!(
            body["workspaces"][1]["membershipUpdatedAt"],
            "2026-07-16T10:01:00Z"
        );
        assert_eq!(body["workspaces"][1]["createdAt"], "2026-07-16T09:00:00Z");
        assert_eq!(body["workspaces"][1]["updatedAt"], "2026-07-16T10:00:00Z");
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
        assert_eq!(
            requests[1].url.path(),
            "/rest/v1/rpc/claim_personal_workspace_e2ee_key"
        );
        assert_eq!(requests[2].url.path(), "/v2/tokens");
        let token_request: Value = serde_json::from_slice(&requests[2].body).unwrap();
        assert_eq!(token_request.as_object().unwrap().len(), 4);
        assert_eq!(token_request["userId"], "user-123");
        let attributes: Value =
            serde_json::from_str(token_request["attributes"].as_str().unwrap()).unwrap();
        assert_eq!(
            attributes,
            json!({ "workspace_ids": ["user-123", "workspace-team"] })
        );
        assert!(token_request.get("workspaceId").is_none());
        assert!(token_request.get("workspaceIds").is_none());
    }

    #[tokio::test]
    async fn rejects_a_different_recovery_key_before_minting_a_token() {
        let server = MockServer::start().await;
        mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
        mock_e2ee_key_claim(&server, "zyxwvutsrqponmlkjihgfe").await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(token_request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "e2ee_key_mismatch"
        );
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|request| request.url.path() != "/v2/tokens")
        );
    }

    #[tokio::test]
    async fn claims_the_recovery_key_identity_without_receiving_the_key() {
        let server = MockServer::start().await;
        mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(
                Request::put("/e2ee/identity")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "keyId": TEST_KEY_ID })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response_json(response).await["keyId"], TEST_KEY_ID);
        let request = server.received_requests().await.unwrap().pop().unwrap();
        let body = String::from_utf8(request.body).unwrap();
        assert!(body.contains(TEST_KEY_ID));
        assert!(!body.contains("anarlog-e2ee-v1"));
    }

    #[tokio::test]
    async fn requires_an_upgrade_for_legacy_clients_after_dual_mode() {
        for protocol_mode in [
            CloudsyncProtocolMode::E2eeOnly,
            CloudsyncProtocolMode::E2eeEnforced,
        ] {
            let server = MockServer::start().await;
            let response = test_router_with_protocol(
                &server,
                "issuer-key",
                &["hyprnote_pro"],
                protocol_mode,
                Some("legacy-database-id"),
            )
            .oneshot(Request::post("/token").body(Body::empty()).unwrap())
            .await
            .unwrap();

            assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
            assert_eq!(
                response_json(response).await["error"]["code"],
                "cloudsync_upgrade_required"
            );
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn preserves_the_desktop_1_2_2_credential_and_issuer_contract_in_dual_mode() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .and(header("authorization", "Bearer issuer-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "token": "legacy-sqlite-token" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router_with_protocol(
            &server,
            "issuer-key",
            &["hyprnote_pro"],
            CloudsyncProtocolMode::Dual,
            Some("legacy-database-id"),
        )
        .oneshot(Request::post("/token").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body.as_object().unwrap().len(), 4);
        assert_eq!(body["databaseId"], "legacy-database-id");
        assert_eq!(body["token"], "legacy-sqlite-token");
        assert_eq!(body["workspaceId"], "user-123");
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let token_request: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(token_request.as_object().unwrap().len(), 3);
        assert_eq!(token_request["name"], "anarlog-cloudsync");
        assert_eq!(token_request["userId"], "user-123");
        assert!(token_request["expiresAt"].as_str().unwrap().ends_with('Z'));
        assert!(token_request.get("attributes").is_none());
    }

    #[tokio::test]
    async fn rejects_a_malformed_e2ee_header_instead_of_downgrading_to_legacy() {
        let server = MockServer::start().await;
        let response = test_router_with_protocol(
            &server,
            "issuer-key",
            &["hyprnote_pro"],
            CloudsyncProtocolMode::Dual,
            Some("legacy-database-id"),
        )
        .oneshot(
            Request::post("/token")
                .header(
                    E2EE_KEY_ID_HEADER,
                    HeaderValue::from_bytes(&[0xff]).unwrap(),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn bounds_workspace_token_attributes() {
        let workspace = |id: String| CloudsyncWorkspace {
            id,
            owner_user_id: "user-123".to_string(),
            kind: "shared".to_string(),
            name: "Shared".to_string(),
            membership_id: "membership".to_string(),
            role: "member".to_string(),
            membership_created_at: "2026-07-16T08:00:00Z".to_string(),
            membership_updated_at: "2026-07-16T08:00:00Z".to_string(),
            created_at: "2026-07-16T08:00:00Z".to_string(),
            updated_at: "2026-07-16T08:00:00Z".to_string(),
        };

        let too_many = (0..=MAX_TOKEN_WORKSPACES)
            .map(|index| workspace(format!("workspace-{index}")))
            .collect::<Vec<_>>();
        assert!(matches!(
            encode_workspace_token_attributes(&too_many),
            Err(SyncError::Upstream)
        ));

        let oversized = [workspace("x".repeat(MAX_TOKEN_ATTRIBUTES_BYTES))];
        assert!(matches!(
            encode_workspace_token_attributes(&oversized),
            Err(SyncError::Upstream)
        ));
    }

    #[tokio::test]
    async fn rejects_users_without_pro_entitlement() {
        let cases: &[&[&str]] = &[&[], &["hyprnote_lite"]];

        for entitlements in cases {
            let server = MockServer::start().await;
            let response = test_router(&server, "issuer-key", entitlements)
                .oneshot(token_request())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let body = response_json(response).await;
            assert_eq!(body["error"]["code"], "subscription_required");
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn refuses_token_when_workspace_projection_is_invalid() {
        let invalid_projections = [
            json!([]),
            json!([personal_workspace("different-user")]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "other-owner-membership",
                    "user_id": "user-123",
                    "role": "owner",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "other-personal",
                        "owner_user_id": "other-personal",
                        "kind": "personal",
                        "name": "Other",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([{
                "id": "user-123",
                "user_id": "user-123",
                "role": "admin",
                "created_at": "2026-07-16T08:00:00Z",
                "updated_at": "2026-07-16T08:00:00Z",
                "workspace": {
                    "id": "user-123",
                    "owner_user_id": "user-123",
                    "kind": "personal",
                    "name": "Personal",
                    "created_at": "2026-07-16T08:00:00Z",
                    "updated_at": "2026-07-16T08:00:00Z"
                }
            }]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "editor",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "team",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "not-a-timestamp",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
            json!([
                personal_workspace("user-123"),
                {
                    "id": "membership-team",
                    "user_id": "user-123",
                    "role": "member",
                    "created_at": "not-a-timestamp",
                    "updated_at": "2026-07-16T09:00:00Z",
                    "workspace": {
                        "id": "workspace-team",
                        "owner_user_id": "user-456",
                        "kind": "shared",
                        "name": "Acme",
                        "created_at": "2026-07-16T09:00:00Z",
                        "updated_at": "2026-07-16T09:00:00Z"
                    }
                }
            ]),
        ];

        for projection in invalid_projections {
            let server = MockServer::start().await;
            mock_workspace_projection(&server, projection).await;

            let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
                .oneshot(token_request())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let requests = server.received_requests().await.unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
        }
    }

    #[tokio::test]
    async fn workspace_projection_failure_is_redacted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/workspace_memberships"))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret supabase detail"))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(token_request())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await.to_string();
        assert!(!body.contains("anon-key"));
        assert!(!body.contains("supabase-token"));
        assert!(!body.contains("supabase detail"));
    }

    #[tokio::test]
    async fn sqlite_cloud_failure_is_redacted() {
        let server = MockServer::start().await;
        mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
        mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
        Mock::given(method("POST"))
            .and(path("/v2/tokens"))
            .respond_with(ResponseTemplate::new(403).set_body_string("secret upstream detail"))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-secret", &["hyprnote_pro"])
            .oneshot(token_request())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await.to_string();
        assert!(!body.contains("issuer-secret"));
        assert!(!body.contains("upstream detail"));
    }

    #[tokio::test]
    async fn publishes_only_the_sanitized_snapshot_as_the_authenticated_actor() {
        let server = MockServer::start().await;
        let share_id = "11111111-1111-4111-8111-111111111111";
        let sanitized_body = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "Shared note" },
                        { "type": "text", "text": "Planning" }
                    ]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Attachment omitted" }]
                }
            ]
        });
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
            ))
            .and(header("apikey", "service-role-key"))
            .and(header("authorization", "Bearer service-role-key"))
            .and(body_partial_json(json!({
                "p_share_id": share_id,
                "p_actor_user_id": "user-123",
                "p_title": "Quarterly plan",
                "p_body_json": sanitized_body,
                "p_attachment_ids": null
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "share_id": share_id,
                "schema_version": 1,
                "content_revision": 2,
                "title": "Quarterly plan",
                "body_json": sanitized_body,
                "attachments_json": [],
                "published_at": "2026-07-16T10:00:00Z"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
            ))
            .and(header("apikey", "service-role-key"))
            .and(header("authorization", "Bearer service-role-key"))
            .and(body_partial_json(json!({
                "p_share_id": share_id,
                "p_actor_user_id": "user-123",
                "p_title": "Quarterly plan",
                "p_body_json": sanitized_body,
                "p_attachment_ids": []
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "share_id": share_id,
                "schema_version": 1,
                "content_revision": 3,
                "title": "Quarterly plan",
                "body_json": sanitized_body,
                "attachments_json": [],
                "published_at": "2026-07-16T10:01:00Z"
            }])))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(
                Request::put(format!("/shares/{share_id}/snapshot"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "title": "  Quarterly plan  ",
                            "body": {
                                "type": "doc",
                                "attrs": { "workspaceId": "private-workspace" },
                                "content": [
                                    {
                                        "type": "paragraph",
                                        "content": [
                                            { "type": "text", "text": "Shared note" },
                                            {
                                                "type": "mention-@",
                                                "attrs": {
                                                    "id": "private-mention-id",
                                                    "type": "session",
                                                    "label": "Planning"
                                                }
                                            }
                                        ]
                                    },
                                    {
                                        "type": "image",
                                        "attrs": {
                                            "src": "asset://localhost/Users/alice/secret.png",
                                            "attachmentId": "private-attachment-id"
                                        }
                                    }
                                ]
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(body["shareId"], share_id);
        assert_eq!(body["schemaVersion"], 1);
        assert_eq!(body["contentRevision"], 2);
        assert_eq!(body["title"], "Quarterly plan");
        assert_eq!(body["body"], sanitized_body);

        let requests = server.received_requests().await.unwrap();
        let published = String::from_utf8(requests[0].body.clone()).unwrap();
        assert!(!published.contains("private-workspace"));
        assert!(!published.contains("private-mention-id"));
        assert!(!published.contains("/Users/alice"));
        assert!(!published.contains("private-attachment-id"));
        assert!(!published.contains("supabase-token"));

        let explicit_empty_response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(
                Request::put(format!("/shares/{share_id}/snapshot"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Quarterly plan",
                            "body": sanitized_body,
                            "attachmentIds": []
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(explicit_empty_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_invalid_snapshot_requests_before_calling_supabase() {
        let cases = [
            (
                "/shares/not-a-uuid/snapshot".to_string(),
                json!({ "title": "Title", "body": { "type": "doc" } }),
            ),
            (
                "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
                json!({ "title": "Title", "body": { "type": "paragraph" } }),
            ),
        ];

        for (path, payload) in cases {
            let server = MockServer::start().await;
            let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
                .oneshot(
                    Request::put(path)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn rejects_snapshot_publication_without_pro_entitlement() {
        let server = MockServer::start().await;
        let response = test_router(&server, "issuer-key", &[])
            .oneshot(
                Request::put("/shares/11111111-1111-4111-8111-111111111111/snapshot")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "title": "Title", "body": { "type": "doc" } }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "subscription_required"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_manager_denial_without_leaking_supabase_details() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
            ))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "code": "42501",
                "message": "secret database detail"
            })))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(
                Request::put("/shares/11111111-1111-4111-8111-111111111111/snapshot")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "title": "Title", "body": { "type": "doc" } }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await.to_string();
        assert!(body.contains("shared_note_publication_forbidden"));
        assert!(!body.contains("secret database detail"));
        assert!(!body.contains("service-role-key"));
    }
}
