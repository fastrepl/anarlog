use axum::Json;
use hypr_api_nango::{GoogleDrive, NangoConnection};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{Result, StorageError};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListFilesRequest {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page_size: Option<u32>,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub order_by: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListFilesResponse {
    pub files: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetFileRequest {
    pub file_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetFileResponse {
    pub file: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadFileRequest {
    pub file_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadFileResponse {
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFolderRequest {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateFolderResponse {
    pub file: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteFileRequest {
    pub file_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadFileRequest {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadFileResponse {
    pub file: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMetadataRequest {
    pub file_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub starred: Option<bool>,
    #[serde(default)]
    pub trashed: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateMetadataResponse {
    pub file: serde_json::Value,
}

#[utoipa::path(
    post,
    path = "/files",
    request_body = ListFilesRequest,
    responses(
        (status = 200, description = "Files listed", body = ListFilesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn list_files(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<ListFilesRequest>,
) -> Result<Json<ListFilesResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let req = hypr_google_drive::ListFilesRequest {
        q: payload.q,
        page_size: payload.page_size,
        page_token: payload.page_token,
        order_by: payload.order_by,
        fields: None,
    };

    let response = client
        .list_files(req)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let files: Vec<serde_json::Value> = response
        .files
        .iter()
        .map(|f| serde_json::to_value(f).unwrap_or_default())
        .collect();

    Ok(Json(ListFilesResponse {
        files,
        next_page_token: response.next_page_token,
    }))
}

#[utoipa::path(
    post,
    path = "/files/get",
    request_body = GetFileRequest,
    responses(
        (status = 200, description = "File metadata fetched", body = GetFileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn get_file(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<GetFileRequest>,
) -> Result<Json<GetFileResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let req = hypr_google_drive::GetFileRequest {
        file_id: payload.file_id,
        fields: None,
    };

    let file = client
        .get_file(req)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let file = serde_json::to_value(file).unwrap_or_default();

    Ok(Json(GetFileResponse { file }))
}

#[utoipa::path(
    post,
    path = "/files/download",
    request_body = DownloadFileRequest,
    responses(
        (status = 200, description = "File content downloaded", body = DownloadFileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn download_file(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<DownloadFileRequest>,
) -> Result<Json<DownloadFileResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let data = client
        .download_file(&payload.file_id)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    Ok(Json(DownloadFileResponse { data }))
}

#[utoipa::path(
    post,
    path = "/files/create-folder",
    request_body = CreateFolderRequest,
    responses(
        (status = 200, description = "Folder created", body = CreateFolderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn create_folder(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<CreateFolderRequest>,
) -> Result<Json<CreateFolderResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let req = hypr_google_drive::CreateFolderRequest {
        name: payload.name,
        parent_id: payload.parent_id,
    };

    let file = client
        .create_folder(req)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let file = serde_json::to_value(file).unwrap_or_default();

    Ok(Json(CreateFolderResponse { file }))
}

#[utoipa::path(
    post,
    path = "/files/delete",
    request_body = DeleteFileRequest,
    responses(
        (status = 200, description = "File deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn delete_file(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<DeleteFileRequest>,
) -> Result<()> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    client
        .delete_file(&payload.file_id)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    Ok(())
}

#[utoipa::path(
    post,
    path = "/files/upload",
    request_body = UploadFileRequest,
    responses(
        (status = 200, description = "File uploaded", body = UploadFileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn upload_file(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<UploadFileRequest>,
) -> Result<Json<UploadFileResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let req = hypr_google_drive::UploadFileRequest {
        name: payload.name,
        parent_id: payload.parent_id,
        mime_type: payload.mime_type,
        data: payload.data,
    };

    let file = client
        .upload_file(req)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let file = serde_json::to_value(file).unwrap_or_default();

    Ok(Json(UploadFileResponse { file }))
}

#[utoipa::path(
    post,
    path = "/files/update",
    request_body = UpdateMetadataRequest,
    responses(
        (status = 200, description = "File metadata updated", body = UpdateMetadataResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "storage",
)]
pub async fn update_metadata(
    nango: NangoConnection<GoogleDrive>,
    Json(payload): Json<UpdateMetadataRequest>,
) -> Result<Json<UpdateMetadataResponse>> {
    let client = hypr_google_drive::GoogleDriveClient::new(nango.into_http());

    let req = hypr_google_drive::UpdateMetadataRequest {
        name: payload.name,
        description: payload.description,
        starred: payload.starred,
        trashed: payload.trashed,
    };

    let file = client
        .update_metadata(&payload.file_id, req)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

    let file = serde_json::to_value(file).unwrap_or_default();

    Ok(Json(UpdateMetadataResponse { file }))
}
