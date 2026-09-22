use anlg_api_auth::AuthContext;
use anlg_nango::OwnedNangoProxy;
use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    GoogleDrive, NangoConnectionError, NangoConnectionState, NangoIntegrationId, state::AppState,
};

#[derive(Debug)]
pub enum DriveError {
    Connection(NangoConnectionError),
    Request(StatusCode, &'static str),
}
impl IntoResponse for DriveError {
    fn into_response(self) -> Response {
        match self {
            Self::Connection(error) => error.into_response(),
            Self::Request(status, message) => {
                anlg_api_error::error_response(status, "google_drive_error", message)
            }
        }
    }
}
impl From<NangoConnectionError> for DriveError {
    fn from(error: NangoConnectionError) -> Self {
        Self::Connection(error)
    }
}
impl From<anlg_nango::Error> for DriveError {
    fn from(_: anlg_nango::Error) -> Self {
        provider_error()
    }
}
impl From<reqwest::Error> for DriveError {
    fn from(_: reqwest::Error) -> Self {
        provider_error()
    }
}
fn provider_error() -> DriveError {
    DriveError::Request(
        StatusCode::BAD_GATEWAY,
        "Google Drive request failed. Try again.",
    )
}
fn invalid(message: &'static str) -> DriveError {
    DriveError::Request(StatusCode::BAD_REQUEST, message)
}
fn validate_id(id: &str) -> Result<(), DriveError> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(invalid("Invalid Google Drive or meeting ID"));
    }
    Ok(())
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveConnectionRequest {
    pub connection_id: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveFolderRequest {
    pub connection_id: String,
    pub folder_id: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct DriveFolder {
    pub id: String,
    pub name: String,
    pub drive_id: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DrivePrepareExportRequest {
    pub connection_id: String,
    pub folder_id: String,
    pub meeting_id: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct DriveExportFile {
    pub file_id: String,
    pub url: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveExportRequest {
    pub connection_id: String,
    pub folder_id: String,
    pub meeting_id: String,
    pub file_id: String,
    pub filename: String,
    pub markdown: String,
}

pub(super) async fn proxy(
    state: &AppState,
    auth: &AuthContext,
    connection_id: &str,
) -> Result<OwnedNangoProxy, DriveError> {
    validate_id(connection_id)?;
    Ok(NangoConnectionState::from_config(&state.config)
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleDrive::ID,
            connection_id,
        )
        .await?
        .into_proxy()
        .base_url_override("https://www.googleapis.com")
        .retries(0))
}

async fn response(response: reqwest::Response) -> Result<Value, DriveError> {
    let status = response.status();
    if !status.is_success() {
        return Err(match status {
            StatusCode::UNAUTHORIZED => DriveError::Request(
                StatusCode::FAILED_DEPENDENCY,
                "Reconnect Google Drive, then try again.",
            ),
            StatusCode::FORBIDDEN => DriveError::Request(
                status,
                "Google Drive denied access. Check the folder permissions or reconnect Google Drive.",
            ),
            StatusCode::NOT_FOUND => DriveError::Request(
                status,
                "The Google Drive folder or file is no longer available. Choose a folder again.",
            ),
            StatusCode::TOO_MANY_REQUESTS => {
                DriveError::Request(status, "Google Drive is busy. Try again later.")
            }
            _ => provider_error(),
        });
    }
    Ok(response.json().await?)
}

pub(super) async fn folder(
    proxy: &OwnedNangoProxy,
    folder_id: &str,
) -> Result<DriveFolder, DriveError> {
    validate_id(folder_id)?;
    let value = response(proxy.get(format!("/drive/v3/files/{folder_id}?supportsAllDrives=true&fields=id,name,driveId,mimeType,trashed,capabilities(canAddChildren)"))?.timeout(std::time::Duration::from_secs(30)).send().await?).await?;
    if value["mimeType"] != "application/vnd.google-apps.folder"
        || value["trashed"] == true
        || value["capabilities"]["canAddChildren"] != true
    {
        return Err(invalid(
            "Choose a Google Drive folder where you can add files.",
        ));
    }
    Ok(DriveFolder {
        id: folder_id.to_owned(),
        name: value["name"]
            .as_str()
            .ok_or_else(provider_error)?
            .to_owned(),
        drive_id: value["driveId"].as_str().map(str::to_owned),
    })
}

#[utoipa::path(post, path = "/google-drive/folder", operation_id = "google_drive_validate_folder", request_body = DriveFolderRequest, responses((status = 200, body = DriveFolder)), tag = "nango")]
pub async fn validate_folder(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<DriveFolderRequest>,
) -> Result<Json<DriveFolder>, DriveError> {
    Ok(Json(
        folder(
            &proxy(&state, &auth, &req.connection_id).await?,
            &req.folder_id,
        )
        .await?,
    ))
}

#[utoipa::path(post, path = "/google-drive/prepare-export", operation_id = "google_drive_prepare_export", request_body = DrivePrepareExportRequest, responses((status = 200, body = DriveExportFile)), tag = "nango")]
pub async fn prepare_export(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<DrivePrepareExportRequest>,
) -> Result<Json<DriveExportFile>, DriveError> {
    validate_id(&req.meeting_id)?;
    let proxy = proxy(&state, &auth, &req.connection_id).await?;
    let folder = folder(&proxy, &req.folder_id).await?;
    let corpus = folder
        .drive_id
        .map(|id| format!("corpora=drive&driveId={}", urlencoding::encode(&id)))
        .unwrap_or_else(|| "corpora=user".to_string());
    let query = format!(
        "'{}' in parents and trashed = false and appProperties has {{ key='anarlog_meeting_id' and value='{}' }}",
        req.folder_id, req.meeting_id
    );
    let found = response(proxy.get(format!("/drive/v3/files?{corpus}&supportsAllDrives=true&includeItemsFromAllDrives=true&pageSize=1&fields=files(id)&q={}", urlencoding::encode(&query)))?.timeout(std::time::Duration::from_secs(30)).send().await?).await?;
    let file_id = match found["files"][0]["id"].as_str() {
        Some(id) => id.to_owned(),
        None => {
            let ids = response(
                proxy
                    .get("/drive/v3/files/generateIds?count=1&space=drive&type=files")?
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await?,
            )
            .await?;
            ids["ids"][0]
                .as_str()
                .ok_or_else(provider_error)?
                .to_owned()
        }
    };
    Ok(Json(export_file(file_id)))
}
fn export_file(file_id: String) -> DriveExportFile {
    DriveExportFile {
        url: format!("https://drive.google.com/file/d/{file_id}/view"),
        file_id,
    }
}

fn verify_export_file(value: &Value, req: &DriveExportRequest) -> Result<(), DriveError> {
    if value["trashed"] == true
        || value["appProperties"]["anarlog_meeting_id"] != req.meeting_id
        || !value["parents"]
            .as_array()
            .is_some_and(|parents| parents.iter().any(|p| p == &req.folder_id))
        || value["mimeType"] != "text/markdown"
    {
        return Err(invalid(
            "The export file no longer matches this meeting and folder.",
        ));
    }
    Ok(())
}

#[utoipa::path(post, path = "/google-drive/export", operation_id = "google_drive_export_markdown", request_body = DriveExportRequest, responses((status = 200, body = DriveExportFile)), tag = "nango")]
pub async fn export_markdown(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<DriveExportRequest>,
) -> Result<Json<DriveExportFile>, DriveError> {
    validate_id(&req.meeting_id)?;
    validate_id(&req.file_id)?;
    if req.markdown.trim().is_empty()
        || req.markdown.len() > 1_500_000
        || req.filename.len() > 512
        || !req.filename.ends_with(".md")
        || req.filename.chars().any(char::is_control)
    {
        return Err(invalid("Invalid Markdown export or export exceeds 1.5 MB."));
    }
    let proxy = proxy(&state, &auth, &req.connection_id).await?;
    folder(&proxy, &req.folder_id).await?;
    let existing = proxy.get(format!("/drive/v3/files/{}?supportsAllDrives=true&fields=id,parents,appProperties,mimeType,trashed", req.file_id))?.timeout(std::time::Duration::from_secs(30)).send().await?;
    if existing.status() == StatusCode::NOT_FOUND {
        // The client persists this generated ID before creating the file. A lost
        // response can therefore be retried without creating a second file.
        let created = proxy.post("/drive/v3/files?supportsAllDrives=true", serde_json::to_vec(&json!({
            "id": req.file_id, "name": req.filename, "mimeType": "text/markdown", "parents": [req.folder_id],
            "appProperties": { "anarlog_meeting_id": req.meeting_id }
        })).map_err(|_| provider_error())?, "application/json")?.timeout(std::time::Duration::from_secs(30)).send().await?;
        if created.status() != StatusCode::CONFLICT {
            response(created).await?;
        }
        let file = response(proxy.get(format!("/drive/v3/files/{}?supportsAllDrives=true&fields=parents,appProperties,mimeType,trashed", req.file_id))?.timeout(std::time::Duration::from_secs(30)).send().await?).await?;
        verify_export_file(&file, &req)?;
    } else {
        verify_export_file(&response(existing).await?, &req)?;
    }
    response(
        proxy
            .patch(
                format!("/drive/v3/files/{}?supportsAllDrives=true", req.file_id),
                &json!({"name": req.filename}),
            )?
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?,
    )
    .await?;
    response(
        proxy
            .patch(
                format!(
                    "/upload/drive/v3/files/{}?uploadType=media&supportsAllDrives=true",
                    req.file_id
                ),
                &json!(null),
            )?
            .headers(reqwest::header::HeaderMap::from_iter([(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("text/markdown; charset=utf-8"),
            )]))
            .body(req.markdown)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?,
    )
    .await?;
    Ok(Json(export_file(req.file_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::Request};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method, path, query_param},
    };

    async fn setup(owned: bool) -> (MockServer, Router) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/nango_connections"))
            .and(query_param("user_id", "eq.user-1"))
            .and(query_param("connection_id", "eq.connection"))
            .and(query_param("integration_id", "eq.google-drive"))
            .respond_with(ResponseTemplate::new(200).set_body_json(if owned {
                json!([{"status":"connected"}])
            } else {
                json!([])
            }))
            .mount(&server)
            .await;
        let config = crate::NangoConfig::for_test(&server.uri(), &server.uri());
        let auth = AuthContext {
            token: "user-token".into(),
            claims: serde_json::from_value(json!({"sub":"user-1"})).unwrap(),
        };
        (server, crate::session_router(config).layer(Extension(auth)))
    }
    async fn post(app: Router, path: &str, body: Value) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 2_000_000)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    async fn mock_folder(server: &MockServer, writable: bool, shared: bool) {
        Mock::given(method("GET")).and(path("/proxy/drive/v3/files/folder"))
            .and(query_param("supportsAllDrives", "true"))
            .and(header("Connection-Id", "connection"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"folder", "name":"회의 노트", "mimeType":"application/vnd.google-apps.folder", "trashed":false,
                "capabilities":{"canAddChildren":writable}, "driveId": if shared { Some("shared-drive") } else { None }
            }))).mount(server).await;
    }

    #[tokio::test]
    async fn refuses_unowned_connections_before_obtaining_credentials() {
        let (server, app) = setup(false).await;
        let (status, _) = post(
            app,
            "/google-drive/picker-start",
            json!({"connection_id":"connection"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn validates_shared_folder_and_rejects_readonly_folder() {
        for writable in [true, false] {
            let (server, app) = setup(true).await;
            mock_folder(&server, writable, true).await;
            let (status, value) = post(
                app,
                "/google-drive/folder",
                json!({"connection_id":"connection", "folder_id":"folder"}),
            )
            .await;
            assert_eq!(
                status,
                if writable {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_REQUEST
                }
            );
            if writable {
                assert_eq!(value["name"], "회의 노트");
            }
        }
    }

    #[tokio::test]
    async fn prepare_export_searches_shared_drive_and_generates_a_stable_id() {
        let (server, app) = setup(true).await;
        mock_folder(&server, true, true).await;
        Mock::given(method("GET"))
            .and(path("/proxy/drive/v3/files"))
            .and(query_param("corpora", "drive"))
            .and(query_param("driveId", "shared-drive"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"files":[]})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proxy/drive/v3/files/generateIds"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"ids":["generated-file"]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let (status, value) = post(
            app,
            "/google-drive/prepare-export",
            json!({"connection_id":"connection", "folder_id":"folder", "meeting_id":"meeting-1"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["file_id"], "generated-file");
    }

    #[tokio::test]
    async fn repeated_uploads_create_once_and_update_the_same_file() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let (server, app) = setup(true).await;
        mock_folder(&server, true, false).await;
        let reads = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET")).and(path("/proxy/drive/v3/files/file"))
            .respond_with(move |_: &wiremock::Request| {
                if reads.fetch_add(1, Ordering::SeqCst) == 0 { ResponseTemplate::new(404) } else {
                    ResponseTemplate::new(200).set_body_json(json!({"id":"file", "parents":["folder"],"mimeType":"text/markdown","appProperties":{"anarlog_meeting_id":"meeting-1"}}))
                }
            }).mount(&server).await;
        Mock::given(method("POST")).and(path("/proxy/drive/v3/files"))
            .and(body_json(json!({"id":"file","name":"회의.md","mimeType":"text/markdown","parents":["folder"],"appProperties":{"anarlog_meeting_id":"meeting-1"}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"file"}))).expect(1).mount(&server).await;
        Mock::given(method("PATCH"))
            .and(path("/proxy/drive/v3/files/file"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"file"})))
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/proxy/upload/drive/v3/files/file"))
            .and(header("Content-Type", "text/markdown; charset=utf-8"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"file"})))
            .expect(2)
            .mount(&server)
            .await;
        for _ in 0..2 {
            let (status, value) = post(app.clone(), "/google-drive/export", json!({"connection_id":"connection","folder_id":"folder","meeting_id":"meeting-1","file_id":"file","filename":"회의.md","markdown":"# 요약\n\n전사문"})).await;
            assert_eq!(status, StatusCode::OK, "{value}");
        }
    }

    #[test]
    fn export_never_overwrites_an_unrelated_or_moved_file() {
        let req = DriveExportRequest {
            connection_id: "connection".into(),
            folder_id: "folder".into(),
            meeting_id: "meeting-1".into(),
            file_id: "file".into(),
            filename: "note.md".into(),
            markdown: "summary".into(),
        };
        for file in [
            json!({"parents":["other"],"appProperties":{"anarlog_meeting_id":"meeting-1"},"mimeType":"text/markdown"}),
            json!({"parents":["folder"],"appProperties":{"anarlog_meeting_id":"other"},"mimeType":"text/markdown"}),
        ] {
            assert!(verify_export_file(&file, &req).is_err());
        }
        assert!(validate_id("folder' or trashed=true").is_err());
    }
}
