use anlg_api_auth::AuthContext;
use anlg_api_nango::{
    Fathom, GoogleMeet, MicrosoftTeams, NangoConnectionState, NangoIntegrationId, Webex,
};
use anlg_meeting_import::{
    fathom::FathomClient, google_meet::GoogleMeetClient, teams::TeamsClient, webex::WebexClient,
};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{MeetingImportError, Result};
use crate::import::{self, ImportFile};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportMeetingsRequest {
    pub connection_id: String,
    #[serde(default)]
    pub known_meeting_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportTextFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportMeetingsResponse {
    pub files: Vec<ImportTextFile>,
    pub warnings: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/fathom/import-meetings",
    operation_id = "fathom_import_meetings",
    request_body = ImportMeetingsRequest,
    responses(
        (status = 200, description = "Fathom meetings fetched for import", body = ImportMeetingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "fathom",
)]
pub async fn fathom_import_meetings(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<ImportMeetingsRequest>,
) -> Result<Json<ImportMeetingsResponse>> {
    let http = connection_http(&auth, &nango_state, Fathom::ID, &req).await?;
    let client = FathomClient::new(http);
    let imported = import::fathom::import_meetings(&client, &req.known_meeting_ids)
        .await
        .map_err(MeetingImportError::Internal)?;
    Ok(Json(into_response(imported.files, imported.warnings)))
}

#[utoipa::path(
    post,
    path = "/webex/import-meetings",
    operation_id = "webex_import_meetings",
    request_body = ImportMeetingsRequest,
    responses(
        (status = 200, description = "Webex meetings fetched for import", body = ImportMeetingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "webex",
)]
pub async fn webex_import_meetings(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<ImportMeetingsRequest>,
) -> Result<Json<ImportMeetingsResponse>> {
    let http = connection_http(&auth, &nango_state, Webex::ID, &req).await?;
    let proxy = http.clone().into_proxy();
    let client = WebexClient::new(http);
    let imported = import::webex::import_meetings(&client, &proxy, &req.known_meeting_ids)
        .await
        .map_err(MeetingImportError::Internal)?;
    Ok(Json(into_response(imported.files, imported.warnings)))
}

#[utoipa::path(
    post,
    path = "/google-meet/import-meetings",
    operation_id = "google_meet_import_meetings",
    request_body = ImportMeetingsRequest,
    responses(
        (status = 200, description = "Google Meet meetings fetched for import", body = ImportMeetingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "google-meet",
)]
pub async fn google_meet_import_meetings(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<ImportMeetingsRequest>,
) -> Result<Json<ImportMeetingsResponse>> {
    let http = connection_http(&auth, &nango_state, GoogleMeet::ID, &req).await?;
    let client = GoogleMeetClient::new(http);
    let imported = import::google_meet::import_meetings(&client, &req.known_meeting_ids)
        .await
        .map_err(MeetingImportError::Internal)?;
    Ok(Json(into_response(imported.files, imported.warnings)))
}

#[utoipa::path(
    post,
    path = "/microsoft-teams/import-meetings",
    operation_id = "microsoft_teams_import_meetings",
    request_body = ImportMeetingsRequest,
    responses(
        (status = 200, description = "Microsoft Teams meetings fetched for import", body = ImportMeetingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "microsoft-teams",
)]
pub async fn teams_import_meetings(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<ImportMeetingsRequest>,
) -> Result<Json<ImportMeetingsResponse>> {
    let http = connection_http(&auth, &nango_state, MicrosoftTeams::ID, &req).await?;
    let proxy = http.clone().into_proxy();
    let client = TeamsClient::new(http);
    let imported = import::teams::import_meetings(&client, &proxy, &req.known_meeting_ids)
        .await
        .map_err(MeetingImportError::Internal)?;
    Ok(Json(into_response(imported.files, imported.warnings)))
}

async fn connection_http(
    auth: &AuthContext,
    nango_state: &NangoConnectionState,
    integration_id: &str,
    req: &ImportMeetingsRequest,
) -> Result<anlg_nango::OwnedNangoHttpClient> {
    if req.connection_id.trim().is_empty() {
        return Err(MeetingImportError::BadRequest(
            "connection_id is required".to_string(),
        ));
    }
    nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            integration_id,
            &req.connection_id,
        )
        .await
        .map_err(MeetingImportError::from)
}

fn into_response(files: Vec<ImportFile>, warnings: Vec<String>) -> ImportMeetingsResponse {
    ImportMeetingsResponse {
        files: files.into_iter().map(into_file).collect(),
        warnings,
    }
}

fn into_file(file: ImportFile) -> ImportTextFile {
    ImportTextFile {
        path: file.path,
        name: file.name,
        content: file.content,
    }
}
