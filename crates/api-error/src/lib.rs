use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetails {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorDetails,
}

pub fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let (code, message) = if status.is_server_error() {
        tracing::error!(error.type = %code, error.message = %message);
        sentry::capture_message(message, sentry::Level::Error);
        (code.to_string(), "Internal server error".to_string())
    } else {
        (code.to_string(), message.to_string())
    };

    let body = Json(ErrorResponse {
        error: ErrorDetails { code, message },
    });

    (status, body).into_response()
}
