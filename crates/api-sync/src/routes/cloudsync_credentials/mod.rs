use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    routing::{get, patch, post, put},
};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::{
    config::CloudsyncProtocolMode,
    error::{Result, SyncError},
    state::{AppState, ReplicaState},
};

mod enrollment;
mod grants;
mod identity;
mod projection;
mod token;

use enrollment::{
    __path_consume_e2ee_device_enrollment, __path_register_e2ee_device_enrollment,
    __path_seal_e2ee_device_enrollment, ConsumeE2eeDeviceEnrollmentRequest,
    E2eeDeviceEnrollmentPackage, E2eeDeviceEnrollmentSummary, RegisterE2eeDeviceEnrollmentRequest,
    RegisterE2eeDeviceEnrollmentResponse, consume_e2ee_device_enrollment,
    list_e2ee_device_enrollments, register_e2ee_device_enrollment, seal_e2ee_device_enrollment,
};
use grants::{
    SetWorkspaceE2eeKeyRequest, SetWorkspaceE2eeKeyResult, WorkspaceE2eeKeyRecipient,
    fetch_workspace_key_recipients, publish_workspace_e2ee_key,
};
use grants::{WorkspaceE2eeKeyGrant, fetch_workspace_key_grants};
use identity::{
    SyncDeviceRow, claim_personal_e2ee_key, claim_sync_device, is_valid_e2ee_key_id,
    list_sync_devices, publish_e2ee_member_identity, remove_sync_device, rename_sync_device,
};
pub use projection::CloudsyncWorkspace;
pub(super) use projection::encode_workspace_token_attributes;
use projection::{fetch_workspace_projection, validate_workspace_projection};
use token::{E2eeCreateTokenRequest, LegacyCreateTokenRequest, mint_cloudsync_token, token_expiry};

#[cfg(test)]
pub(super) use identity::{DEVICE_FINGERPRINT_HEADER, DEVICE_NAME_HEADER};
#[cfg(test)]
pub(super) use projection::{
    MAX_TOKEN_ATTRIBUTES_BYTES, MAX_TOKEN_WORKSPACES, WORKSPACE_PROJECTION_SELECT,
};

const SUPABASE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const CLOUDSYNC_ENCRYPTION_VERSION: u8 = 2;
const REPLICA_CREDENTIAL_TTL_SECONDS: u64 = 15 * 60;
pub(super) const E2EE_KEY_ID_HEADER: &str = "x-anarlog-e2ee-key-id";
pub(super) use identity::E2EE_MEMBER_PUBLIC_KEY_HEADER;

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
    workspace_key_grants: Vec<WorkspaceE2eeKeyGrant>,
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
#[serde(rename_all = "camelCase")]
pub struct ReplicaCredentials {
    transport: &'static str,
    encryption_version: u8,
    encryption_key_id: String,
    expires_at: String,
    workspace_id: String,
    account_user_id: String,
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

#[derive(OpenApi)]
#[openapi(
    paths(
        create_credentials,
        create_replica_credentials,
        claim_e2ee_identity,
        get_devices,
        patch_device,
        delete_device,
        register_e2ee_device_enrollment,
        seal_e2ee_device_enrollment,
        consume_e2ee_device_enrollment,
        get_workspace_e2ee_key_recipients,
        set_workspace_e2ee_key
    ),
    components(schemas(
        CloudsyncCredentialResponse,
        CloudsyncCredentials,
        CloudsyncWorkspace,
        WorkspaceE2eeKeyGrant,
        WorkspaceE2eeKeyRecipient,
        SetWorkspaceE2eeKeyRequest,
        SetWorkspaceE2eeKeyResult,
        ClaimE2eeIdentityRequest,
        E2eeIdentity,
        RenameSyncDeviceRequest,
        SyncDevicesResponse,
        SyncDeviceRow,
        E2eeDeviceEnrollmentSummary,
        E2eeDeviceEnrollmentPackage,
        RegisterE2eeDeviceEnrollmentRequest,
        RegisterE2eeDeviceEnrollmentResponse,
        ConsumeE2eeDeviceEnrollmentRequest,
        LegacyCloudsyncCredentials,
        ReplicaCredentials
    ))
)]
pub struct ApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/token", post(create_credentials))
}

pub(super) fn replica_router() -> Router<ReplicaState> {
    Router::new()
        .route("/replica/credentials", post(create_replica_credentials))
        .route("/e2ee/identity", put(claim_e2ee_identity))
        .route("/devices", get(get_devices))
        .route(
            "/devices/{fingerprint}",
            patch(patch_device).delete(delete_device),
        )
        .route(
            "/e2ee/device-enrollments",
            post(register_e2ee_device_enrollment),
        )
        .route(
            "/e2ee/device-enrollments/{request_id}/seal",
            post(seal_e2ee_device_enrollment),
        )
        .route(
            "/e2ee/device-enrollments/{request_id}/consume",
            post(consume_e2ee_device_enrollment),
        )
        .route(
            "/e2ee/workspaces/{workspace_id}/recipients",
            get(get_workspace_e2ee_key_recipients),
        )
        .route(
            "/e2ee/workspaces/{workspace_id}/key",
            put(set_workspace_e2ee_key),
        )
}

#[utoipa::path(
    get,
    path = "/e2ee/workspaces/{workspace_id}/recipients",
    tag = "sync",
    params(("workspace_id" = String, Path, description = "Shared workspace ID")),
    responses(
        (status = 200, description = "Active workspace key recipients", body = [WorkspaceE2eeKeyRecipient]),
        (status = 400, description = "Invalid workspace ID"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Workspace manager access required"),
        (status = 502, description = "Workspace key service unavailable")
    )
)]
async fn get_workspace_e2ee_key_recipients(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceE2eeKeyRecipient>>> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    Ok(Json(
        fetch_workspace_key_recipients(&state, &auth.token, &workspace_id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/e2ee/workspaces/{workspace_id}/key",
    tag = "sync",
    params(("workspace_id" = String, Path, description = "Shared workspace ID")),
    request_body = SetWorkspaceE2eeKeyRequest,
    responses(
        (status = 200, description = "Wrapped workspace key published", body = SetWorkspaceE2eeKeyResult),
        (status = 400, description = "Invalid workspace key grants"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Workspace manager access required"),
        (status = 502, description = "Workspace key service unavailable")
    )
)]
async fn set_workspace_e2ee_key(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SetWorkspaceE2eeKeyRequest>,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<SetWorkspaceE2eeKeyResult>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(publish_workspace_e2ee_key(&state, &auth.token, &workspace_id, request).await?),
    ))
}

#[utoipa::path(
    post,
    path = "/replica/credentials",
    tag = "sync",
    params(
        ("x-anarlog-e2ee-key-id" = String, Header, description = "Local recovery-key identity"),
        ("x-anarlog-e2ee-member-public-key" = Option<String>, Header, description = "Account-level member identity public key")
    ),
    responses(
        (status = 200, description = "Encrypted replica credentials", body = ReplicaCredentials),
        (status = 400, description = "Invalid E2EE key identity"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 409, description = "Account already uses a different recovery key"),
        (status = 502, description = "Replica credential service unavailable")
    )
)]
async fn create_replica_credentials(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    headers: HeaderMap,
) -> Result<(
    [(header::HeaderName, HeaderValue); 1],
    Json<ReplicaCredentials>,
)> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }

    publish_requested_member_identity(&state, &auth, &headers).await?;
    let requested_key_id = headers
        .get(E2EE_KEY_ID_HEADER)
        .ok_or_else(|| SyncError::BadRequest("E2EE key identity is required".to_string()))?
        .to_str()
        .map_err(|_| SyncError::BadRequest("E2EE key identity is invalid".to_string()))?;
    let encryption_key_id =
        claim_personal_e2ee_key(&state, &auth.claims.sub, requested_key_id).await?;
    claim_sync_device(&state, &auth.claims.sub, &headers).await?;
    let expires_at = token_expiry(REPLICA_CREDENTIAL_TTL_SECONDS)?;

    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(ReplicaCredentials {
            transport: "replica",
            encryption_version: CLOUDSYNC_ENCRYPTION_VERSION,
            encryption_key_id,
            expires_at,
            workspace_id: auth.claims.sub.clone(),
            account_user_id: auth.claims.sub,
        }),
    ))
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncDevicesResponse {
    devices: Vec<SyncDeviceRow>,
    pending_devices: Vec<E2eeDeviceEnrollmentSummary>,
    max_devices: u8,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameSyncDeviceRequest {
    device_name: String,
}

#[utoipa::path(
    get,
    path = "/devices",
    tag = "sync",
    responses(
        (status = 200, description = "Approved and pending sync devices", body = SyncDevicesResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 502, description = "Device service unavailable")
    )
)]
async fn get_devices(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
) -> Result<Json<SyncDevicesResponse>> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    let (devices, pending_devices) = tokio::try_join!(
        list_sync_devices(&state, &auth.claims.sub),
        list_e2ee_device_enrollments(&state, &auth.claims.sub)
    )?;
    Ok(Json(SyncDevicesResponse {
        devices,
        pending_devices,
        max_devices: 5,
    }))
}

#[utoipa::path(
    patch,
    path = "/devices/{fingerprint}",
    tag = "sync",
    params(("fingerprint" = String, Path, description = "Device fingerprint")),
    request_body = RenameSyncDeviceRequest,
    responses(
        (status = 204, description = "Device renamed"),
        (status = 400, description = "Invalid device fingerprint or name"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 502, description = "Device service unavailable")
    )
)]
async fn patch_device(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(fingerprint): Path<String>,
    Json(request): Json<RenameSyncDeviceRequest>,
) -> Result<axum::http::StatusCode> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    rename_sync_device(&state, &auth.claims.sub, &fingerprint, &request.device_name).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/devices/{fingerprint}",
    tag = "sync",
    params(("fingerprint" = String, Path, description = "Device fingerprint")),
    responses(
        (status = 204, description = "Device removed"),
        (status = 400, description = "Invalid device fingerprint"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Anarlog Pro subscription required"),
        (status = 502, description = "Device service unavailable")
    )
)]
async fn delete_device(
    Extension(auth): Extension<AuthContext>,
    State(state): State<ReplicaState>,
    Path(fingerprint): Path<String>,
) -> Result<axum::http::StatusCode> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    remove_sync_device(&state, &auth.claims.sub, &fingerprint).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
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
    State(state): State<ReplicaState>,
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
    post,
    path = "/token",
    tag = "sync",
    params(
        ("x-anarlog-e2ee-key-id" = Option<String>, Header, description = "Local recovery-key identity"),
        ("x-anarlog-e2ee-member-public-key" = Option<String>, Header, description = "Account-level member identity public key")
    ),
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

    publish_requested_member_identity(&state.replica, &auth, &headers).await?;

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

        claim_sync_device(&state.replica, &auth.claims.sub, &headers).await?;
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

    let workspace_rows = fetch_workspace_projection(&state.replica, &auth).await?;
    let (personal_workspace_id, workspaces) =
        validate_workspace_projection(workspace_rows, &auth.claims.sub)?;
    let workspace_key_grants =
        fetch_workspace_key_grants(&state.replica, &auth.token, &workspaces).await?;
    let encryption_key_id =
        claim_personal_e2ee_key(&state.replica, &auth.claims.sub, requested_key_id).await?;
    claim_sync_device(&state.replica, &auth.claims.sub, &headers).await?;
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
            workspace_key_grants,
        })),
    ))
}

async fn publish_requested_member_identity(
    state: &ReplicaState,
    auth: &AuthContext,
    headers: &HeaderMap,
) -> Result<()> {
    let Some(public_key) = headers.get(E2EE_MEMBER_PUBLIC_KEY_HEADER) else {
        return Ok(());
    };
    let public_key = public_key
        .to_str()
        .map_err(|_| SyncError::BadRequest("E2EE member identity is invalid".to_string()))?;
    publish_e2ee_member_identity(state, &auth.token, public_key).await
}
