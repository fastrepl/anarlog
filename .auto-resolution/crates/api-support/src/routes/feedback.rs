use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

pub use hypr_template_support::DeviceInfo;

use crate::error::SupportError;
use crate::github::{self, BugReportInput, FeatureRequestInput};
use crate::state::AppState;

#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackType {
    #[default]
    Bug,
    Feature,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackRequest {
    #[serde(default)]
    pub r#type: FeedbackType,
    pub description: String,
    pub logs: Option<String>,
    pub device_info: DeviceInfo,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const SOURCE: &str = "from the Char Desktop app";

#[utoipa::path(
    post,
    path = "/feedback/submit",
    request_body = FeedbackRequest,
    responses(
        (status = 200, description = "Feedback submitted successfully", body = FeedbackResponse),
        (status = 400, description = "Invalid request", body = FeedbackResponse),
        (status = 500, description = "Server error", body = FeedbackResponse),
    ),
    tag = "feedback",
)]
pub async fn submit(
    State(state): State<AppState>,
    Json(payload): Json<FeedbackRequest>,
) -> std::result::Result<Json<FeedbackResponse>, SupportError> {
    if payload.description.trim().len() < 10 {
        return Err(SupportError::InvalidRequest(
            "description must be at least 10 characters".into(),
        ));
    }
    let di = &payload.device_info;

    let url = match payload.r#type {
        FeedbackType::Bug => {
            github::submit_bug_report(
                &state,
                BugReportInput {
                    description: &payload.description,
                    platform: &di.platform,
                    arch: &di.arch,
                    os_version: &di.os_version,
                    app_version: &di.app_version,
                    source: SOURCE,
                    logs: payload.logs.as_deref(),
                },
            )
            .await?
        }
        FeedbackType::Feature => {
            github::submit_feature_request(
                &state,
                FeatureRequestInput {
                    description: &payload.description,
                    platform: &di.platform,
                    arch: &di.arch,
                    os_version: &di.os_version,
                    app_version: &di.app_version,
                    source: SOURCE,
                },
            )
            .await?
        }
    };

    Ok(Json(FeedbackResponse {
        success: true,
        issue_url: Some(url),
        error: None,
    }))
}
