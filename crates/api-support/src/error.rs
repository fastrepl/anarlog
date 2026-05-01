use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SupportError>;

#[derive(Debug, Error)]
pub enum SupportError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<octocrab::Error> for SupportError {
    fn from(err: octocrab::Error) -> Self {
        Self::GitHub(err.to_string())
    }
}

impl IntoResponse for SupportError {
    fn into_response(self) -> Response {
        let internal_message = "Internal server error".to_string();

        match self {
            Self::InvalidRequest(message) => (
                StatusCode::BAD_REQUEST,
                Json(crate::routes::FeedbackResponse {
                    success: false,
                    issue_url: None,
                    error: Some(message),
                }),
            )
                .into_response(),
            Self::GitHub(message) => {
                tracing::error!(error = %message, "github_error");
                sentry::capture_message(&message, sentry::Level::Error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(crate::routes::FeedbackResponse {
                        success: false,
                        issue_url: None,
                        error: Some(internal_message),
                    }),
                )
                    .into_response()
            }
            Self::Internal(message) => {
                tracing::error!(error = %message, "internal_error");
                sentry::capture_message(&message, sentry::Level::Error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(crate::routes::FeedbackResponse {
                        success: false,
                        issue_url: None,
                        error: Some(internal_message),
                    }),
                )
                    .into_response()
            }
        }
    }
}
