use anlg_api_auth::AuthContext;
use anlg_api_nango::{NangoConnectionState, NangoIntegrationId, Zoom};
use anlg_zoom::ZoomClient;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{Result, ZoomError};
use crate::import::{self, ZoomImportFile};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ZoomImportMeetingsRequest {
    pub connection_id: String,
    #[serde(default)]
    pub known_meeting_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ZoomImportTextFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ZoomImportMeetingsResponse {
    pub files: Vec<ZoomImportTextFile>,
    pub warnings: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/import-meetings",
    operation_id = "zoom_import_meetings",
    request_body = ZoomImportMeetingsRequest,
    responses(
        (status = 200, description = "Zoom meetings fetched for import", body = ZoomImportMeetingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "zoom",
)]
pub async fn import_meetings(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<ZoomImportMeetingsRequest>,
) -> Result<Json<ZoomImportMeetingsResponse>> {
    if req.connection_id.trim().is_empty() {
        return Err(ZoomError::BadRequest(
            "connection_id is required".to_string(),
        ));
    }

    let http = nango_state
        .build_http_client(&auth.token, &auth.claims.sub, Zoom::ID, &req.connection_id)
        .await?;
    let proxy = http.clone().into_proxy();
    let client = ZoomClient::new(http);
    let imported = import::import_meetings(&client, &proxy, &req.known_meeting_ids)
        .await
        .map_err(ZoomError::Internal)?;

    Ok(Json(ZoomImportMeetingsResponse {
        files: imported.files.into_iter().map(into_file).collect(),
        warnings: imported.warnings,
    }))
}

fn into_file(file: ZoomImportFile) -> ZoomImportTextFile {
    ZoomImportTextFile {
        path: file.path,
        name: file.name,
        content: file.content,
    }
}
