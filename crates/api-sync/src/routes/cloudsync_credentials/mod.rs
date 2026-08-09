use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

use crate::{
    config::CloudsyncProtocolMode,
    error::{Result, SyncError},
    state::AppState,
};

mod identity;
mod projection;
mod token;

use identity::{
    SyncDeviceRow, claim_personal_e2ee_key, claim_sync_device, is_valid_e2ee_key_id,
    list_sync_devices, remove_sync_device,
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
pub(super) const E2EE_KEY_ID_HEADER: &str = "x-anarlog-e2ee-key-id";

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

#[derive(OpenApi)]
#[openapi(
    paths(create_credentials, claim_e2ee_identity),
    components(schemas(
        CloudsyncCredentialResponse,
        CloudsyncCredentials,
        CloudsyncWorkspace,
        ClaimE2eeIdentityRequest,
        E2eeIdentity,
        LegacyCloudsyncCredentials
    ))
)]
pub struct ApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/token", post(create_credentials))
        .route("/e2ee/identity", put(claim_e2ee_identity))
        .route("/devices", get(get_devices))
        .route("/devices/{fingerprint}", delete(delete_device))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncDevicesResponse {
    devices: Vec<SyncDeviceRow>,
}

async fn get_devices(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> Result<Json<SyncDevicesResponse>> {
    if !auth.claims.is_pro() {
        return Err(SyncError::ProPlanRequired);
    }
    Ok(Json(SyncDevicesResponse {
        devices: list_sync_devices(&state, &auth.claims.sub).await?,
    }))
}

async fn delete_device(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
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

    claim_sync_device(&state, &auth.claims.sub, &headers).await?;

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
