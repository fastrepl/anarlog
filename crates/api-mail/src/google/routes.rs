use axum::{Extension, Json};
use hypr_api_auth::AuthContext;
use hypr_api_nango::{GoogleMail, NangoConnectionState, NangoIntegrationId};
use hypr_google_mail::{
    Attachment, GoogleMailClient, ListHistoryResponse, ListLabelsResponse, ListMessagesResponse,
    ListThreadsResponse, Message, Profile, Thread,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{MailError, Result};

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListLabelsRequest {
    pub connection_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListMessagesRequest {
    pub connection_id: String,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub label_ids: Option<Vec<String>>,
    #[serde(default)]
    pub include_spam_trash: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleGetMessageRequest {
    pub connection_id: String,
    pub id: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub metadata_headers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleGetAttachmentRequest {
    pub connection_id: String,
    pub message_id: String,
    pub attachment_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleGetProfileRequest {
    pub connection_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListThreadsRequest {
    pub connection_id: String,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub label_ids: Option<Vec<String>>,
    #[serde(default)]
    pub include_spam_trash: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleGetThreadRequest {
    pub connection_id: String,
    pub id: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub metadata_headers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListHistoryRequest {
    pub connection_id: String,
    pub start_history_id: String,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub label_id: Option<String>,
    #[serde(default)]
    pub history_types: Option<Vec<String>>,
}

#[utoipa::path(
    post,
    path = "/google/list-labels",
    operation_id = "google_list_labels",
    request_body = GoogleListLabelsRequest,
    responses(
        (status = 200, description = "Google mail labels fetched", body = ListLabelsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn list_labels(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListLabelsRequest>,
) -> Result<Json<ListLabelsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let response = client
        .list_labels()
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/list-messages",
    operation_id = "google_list_messages",
    request_body = GoogleListMessagesRequest,
    responses(
        (status = 200, description = "Google mail messages fetched", body = ListMessagesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn list_messages(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListMessagesRequest>,
) -> Result<Json<ListMessagesResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let google_req = hypr_google_mail::ListMessagesRequest {
        q: req.q,
        max_results: req.max_results,
        page_token: req.page_token,
        label_ids: req.label_ids,
        include_spam_trash: req.include_spam_trash,
    };

    let response = client
        .list_messages(google_req)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/get-message",
    operation_id = "google_get_message",
    request_body = GoogleGetMessageRequest,
    responses(
        (status = 200, description = "Google mail message fetched", body = Message),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn get_message(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleGetMessageRequest>,
) -> Result<Json<Message>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let format = req
        .format
        .as_deref()
        .map(|s| match s {
            "full" => Ok(hypr_google_mail::MessageFormat::Full),
            "metadata" => Ok(hypr_google_mail::MessageFormat::Metadata),
            "minimal" => Ok(hypr_google_mail::MessageFormat::Minimal),
            "raw" => Ok(hypr_google_mail::MessageFormat::Raw),
            other => Err(MailError::BadRequest(format!("Invalid format: {other}"))),
        })
        .transpose()?;

    let google_req = hypr_google_mail::GetMessageRequest {
        id: req.id,
        format,
        metadata_headers: req.metadata_headers,
    };

    let response = client
        .get_message(google_req)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/get-attachment",
    operation_id = "google_get_attachment",
    request_body = GoogleGetAttachmentRequest,
    responses(
        (status = 200, description = "Google mail attachment fetched", body = Attachment),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn get_attachment(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleGetAttachmentRequest>,
) -> Result<Json<Attachment>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let response = client
        .get_attachment(&req.message_id, &req.attachment_id)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/get-profile",
    operation_id = "google_get_profile",
    request_body = GoogleGetProfileRequest,
    responses(
        (status = 200, description = "Google mail profile fetched", body = Profile),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn get_profile(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleGetProfileRequest>,
) -> Result<Json<Profile>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let response = client
        .get_profile()
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/list-threads",
    operation_id = "google_list_threads",
    request_body = GoogleListThreadsRequest,
    responses(
        (status = 200, description = "Google mail threads fetched", body = ListThreadsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn list_threads(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListThreadsRequest>,
) -> Result<Json<ListThreadsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let google_req = hypr_google_mail::ListThreadsRequest {
        q: req.q,
        max_results: req.max_results,
        page_token: req.page_token,
        label_ids: req.label_ids,
        include_spam_trash: req.include_spam_trash,
    };

    let response = client
        .list_threads(google_req)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/get-thread",
    operation_id = "google_get_thread",
    request_body = GoogleGetThreadRequest,
    responses(
        (status = 200, description = "Google mail thread fetched", body = Thread),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn get_thread(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleGetThreadRequest>,
) -> Result<Json<Thread>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let format = req
        .format
        .as_deref()
        .map(|s| match s {
            "full" => Ok(hypr_google_mail::MessageFormat::Full),
            "metadata" => Ok(hypr_google_mail::MessageFormat::Metadata),
            "minimal" => Ok(hypr_google_mail::MessageFormat::Minimal),
            "raw" => Ok(hypr_google_mail::MessageFormat::Raw),
            other => Err(MailError::BadRequest(format!("Invalid format: {other}"))),
        })
        .transpose()?;

    let google_req = hypr_google_mail::GetThreadRequest {
        id: req.id,
        format,
        metadata_headers: req.metadata_headers,
    };

    let response = client
        .get_thread(google_req)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/list-history",
    operation_id = "google_list_history",
    request_body = GoogleListHistoryRequest,
    responses(
        (status = 200, description = "Google mail history fetched", body = ListHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "mail",
)]
pub async fn list_history(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListHistoryRequest>,
) -> Result<Json<ListHistoryResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleMail::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleMailClient::new(http);

    let history_types = req
        .history_types
        .map(|types| {
            types
                .iter()
                .map(|s| match s.as_str() {
                    "messageAdded" => Ok(hypr_google_mail::HistoryType::MessageAdded),
                    "messageDeleted" => Ok(hypr_google_mail::HistoryType::MessageDeleted),
                    "labelAdded" => Ok(hypr_google_mail::HistoryType::LabelAdded),
                    "labelRemoved" => Ok(hypr_google_mail::HistoryType::LabelRemoved),
                    other => Err(MailError::BadRequest(format!(
                        "Invalid history type: {other}"
                    ))),
                })
                .collect::<std::result::Result<Vec<_>, _>>()
        })
        .transpose()?;

    let google_req = hypr_google_mail::ListHistoryRequest {
        start_history_id: req.start_history_id,
        max_results: req.max_results,
        page_token: req.page_token,
        label_id: req.label_id,
        history_types,
    };

    let response = client
        .list_history(google_req)
        .await
        .map_err(|e| MailError::Internal(e.to_string()))?;

    Ok(Json(response))
}
