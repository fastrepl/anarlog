use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MessengerError>;

#[derive(Debug, Error)]
pub enum MessengerError {
    #[error("Slack error: {0}")]
    Slack(#[from] hypr_slack_web::Error),

    #[error("Teams error: {0}")]
    Teams(#[from] hypr_teems::Error),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for MessengerError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            Self::Slack(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "slack_error",
                err.to_string(),
            ),
            Self::Teams(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "teams_error",
                err.to_string(),
            ),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                msg,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
