use anlg_api_auth::AuthContext;
use axum::{
    Extension, Json, Router,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::Serialize;
use utoipa::OpenApi;
use uuid::Uuid;

use crate::{
    error::{Result, SyncError},
    routes::attachment_backups::require_backup_access,
    state::AppState,
};

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct SignedDatabaseBackupUrl {
    signed_url: String,
    expires_in_seconds: u32,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct DatabaseBackupObjectKey {
    key: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(upload_url, download_url, worker_object_key),
    components(schemas(SignedDatabaseBackupUrl, DatabaseBackupObjectKey))
)]
struct ApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/database-backups/upload-url", post(upload_url))
        .route("/database-backups/download-url", get(download_url))
        .route(
            "/database-backups/worker-object-key",
            get(worker_object_key),
        )
        .layer(middleware::from_fn(add_no_store))
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
    path = "/database-backups/worker-object-key",
    tag = "sync",
    responses(
        (status = 200, description = "User-scoped private R2 object key for the backup Worker", body = DatabaseBackupObjectKey),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Backup access required")
    )
)]
async fn worker_object_key(
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<DatabaseBackupObjectKey>> {
    require_backup_access(&auth)?;
    Ok(Json(DatabaseBackupObjectKey {
        key: backup_key(&auth)?,
    }))
}

fn backup_key(auth: &AuthContext) -> Result<String> {
    let user_id = Uuid::parse_str(&auth.claims.sub)
        .ok()
        .filter(|id| id.to_string() == auth.claims.sub)
        .ok_or(SyncError::AttachmentBackupForbidden)?;
    Ok(format!("database-backups/{user_id}/latest.anarlogdb.enc"))
}

#[utoipa::path(
    post,
    path = "/database-backups/upload-url",
    tag = "sync",
    responses(
        (status = 200, description = "Short-lived private R2 upload URL", body = SignedDatabaseBackupUrl),
        (status = 401, description = "Authentication required"),
        (status = 502, description = "R2 backup storage is unavailable")
    )
)]
async fn upload_url(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> Result<Json<SignedDatabaseBackupUrl>> {
    require_backup_access(&auth)?;
    let key = backup_key(&auth)?;
    let storage = state
        .r2_backup
        .as_ref()
        .ok_or(SyncError::AttachmentBackupServiceUnavailable)?;
    let signed_url = storage.presigned_put_object(&key).await.map_err(|_| {
        tracing::warn!("R2 database backup upload signing failed");
        SyncError::AttachmentBackupServiceUnavailable
    })?;
    Ok(Json(SignedDatabaseBackupUrl {
        signed_url,
        expires_in_seconds: 60 * 60,
    }))
}

#[utoipa::path(
    get,
    path = "/database-backups/download-url",
    tag = "sync",
    responses(
        (status = 200, description = "Short-lived private R2 download URL", body = SignedDatabaseBackupUrl),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "No database backup exists"),
        (status = 502, description = "R2 backup storage is unavailable")
    )
)]
async fn download_url(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> Result<Json<SignedDatabaseBackupUrl>> {
    require_backup_access(&auth)?;
    let key = backup_key(&auth)?;
    let storage = state
        .r2_backup
        .as_ref()
        .ok_or(SyncError::AttachmentBackupServiceUnavailable)?;
    let exists = storage.head_object(&key).await.map_err(|_| {
        tracing::warn!("R2 database backup lookup failed");
        SyncError::AttachmentBackupServiceUnavailable
    })?;
    if !exists {
        return Err(SyncError::AttachmentBackupNotFound);
    }
    let signed_url = storage.presigned_get_object(&key).await.map_err(|_| {
        tracing::warn!("R2 database backup download signing failed");
        SyncError::AttachmentBackupServiceUnavailable
    })?;
    Ok(Json(SignedDatabaseBackupUrl {
        signed_url,
        expires_in_seconds: 15 * 60,
    }))
}
