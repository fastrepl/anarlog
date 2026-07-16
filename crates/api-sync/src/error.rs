use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Anarlog Pro is required for CloudSync")]
    ProPlanRequired,

    #[error("This account is protected by a different E2EE recovery key")]
    E2eeKeyMismatch,

    #[error("Shared note publication is not permitted")]
    SnapshotPublicationForbidden,

    #[error("Shared note service is unavailable")]
    SnapshotServiceUnavailable,

    #[error("Shared note is unavailable")]
    SharedNoteNotFound,

    #[error("CloudSync credential service is unavailable")]
    Upstream,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::ProPlanRequired => (
                StatusCode::FORBIDDEN,
                "subscription_required",
                "Anarlog Pro is required for CloudSync".to_string(),
            ),
            Self::E2eeKeyMismatch => (
                StatusCode::CONFLICT,
                "e2ee_key_mismatch",
                "This account is protected by a different E2EE recovery key".to_string(),
            ),
            Self::SnapshotPublicationForbidden => (
                StatusCode::FORBIDDEN,
                "shared_note_publication_forbidden",
                "Shared note publication is not permitted".to_string(),
            ),
            Self::SnapshotServiceUnavailable => (
                StatusCode::BAD_GATEWAY,
                "shared_note_service_unavailable",
                "Shared note service is unavailable".to_string(),
            ),
            Self::SharedNoteNotFound => (
                StatusCode::NOT_FOUND,
                "shared_note_not_found",
                "Shared note is unavailable".to_string(),
            ),
            Self::Upstream => (
                StatusCode::BAD_GATEWAY,
                "cloudsync_credential_service_unavailable",
                "CloudSync credential service is unavailable".to_string(),
            ),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                message,
            ),
        };

        hypr_api_error::error_response(status, code, &message)
    }
}
