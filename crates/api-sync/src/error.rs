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

    #[error("A newer Anarlog version is required for encrypted CloudSync")]
    CloudsyncUpgradeRequired,

    #[error("This account is protected by a different E2EE recovery key")]
    E2eeKeyMismatch,

    #[error("CloudSync device limit reached")]
    SyncDeviceLimitReached,

    #[error("E2EE freshness witness access is not permitted")]
    E2eeWitnessForbidden,

    #[error("E2EE freshness witness is not initialized")]
    E2eeWitnessUninitialized,

    #[error("E2EE freshness witness is unavailable")]
    E2eeWitnessServiceUnavailable,

    #[error("Shared note publication is not permitted")]
    SnapshotPublicationForbidden,

    #[error("Shared note changed")]
    SnapshotChanged,

    #[error("Shared note service is unavailable")]
    SnapshotServiceUnavailable,

    #[error("Shared note is unavailable")]
    SharedNoteNotFound,

    #[error("Shared note invitation email is unavailable")]
    InvitationEmailUnavailable,

    #[error("CloudSync credential service is unavailable")]
    Upstream,

    #[error("Attachment backup access is not permitted")]
    AttachmentBackupForbidden,

    #[error("Attachment backup is unavailable")]
    AttachmentBackupNotFound,

    #[error("Attachment backup changed")]
    AttachmentBackupConflict,

    #[error("Attachment backup dependency appeared")]
    AttachmentBackupDependencyAppeared,

    #[error("Attachment backup deletion was canceled")]
    AttachmentBackupDeleteCancelled,

    #[error("Attachment backup deletion can no longer be canceled")]
    AttachmentBackupDeleteTooLate,

    #[error("Attachment backup quota is exhausted")]
    AttachmentBackupQuotaExceeded,

    #[error("Attachment backup service is unavailable")]
    AttachmentBackupServiceUnavailable,

    #[error("Attachment backup verification is busy")]
    AttachmentBackupVerificationBusy,

    #[error("Shared attachment access is not permitted")]
    SharedAttachmentForbidden,

    #[error("Shared attachment is unavailable")]
    SharedAttachmentNotFound,

    #[error("Shared attachment changed")]
    SharedAttachmentConflict,

    #[error("Shared attachment quota is exhausted")]
    SharedAttachmentQuotaExceeded,

    #[error("Shared attachment service is unavailable")]
    SharedAttachmentServiceUnavailable,

    #[error("Shared attachment verification is busy")]
    SharedAttachmentVerificationBusy,

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
            Self::CloudsyncUpgradeRequired => (
                StatusCode::UPGRADE_REQUIRED,
                "cloudsync_upgrade_required",
                "Update Anarlog to continue using encrypted CloudSync".to_string(),
            ),
            Self::E2eeKeyMismatch => (
                StatusCode::CONFLICT,
                "e2ee_key_mismatch",
                "This account is protected by a different E2EE recovery key".to_string(),
            ),
            Self::SyncDeviceLimitReached => (
                StatusCode::FORBIDDEN,
                "sync_device_limit_reached",
                "Cloud sync is limited to 5 devices per account; remove a device before syncing here"
                    .to_string(),
            ),
            Self::E2eeWitnessForbidden => (
                StatusCode::FORBIDDEN,
                "e2ee_witness_forbidden",
                "E2EE freshness witness access is not permitted".to_string(),
            ),
            Self::E2eeWitnessUninitialized => (
                StatusCode::CONFLICT,
                "e2ee_witness_uninitialized",
                "Open an existing trusted device before setting up encrypted sync on this device"
                    .to_string(),
            ),
            Self::E2eeWitnessServiceUnavailable => (
                StatusCode::BAD_GATEWAY,
                "e2ee_witness_unavailable",
                "E2EE freshness witness is unavailable".to_string(),
            ),
            Self::SnapshotPublicationForbidden => (
                StatusCode::FORBIDDEN,
                "shared_note_publication_forbidden",
                "Shared note publication is not permitted".to_string(),
            ),
            Self::SnapshotChanged => (
                StatusCode::CONFLICT,
                "snapshot_conflict",
                "Shared note changed; update Anarlog and reload before publishing again"
                    .to_string(),
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
            Self::InvitationEmailUnavailable => (
                StatusCode::BAD_GATEWAY,
                "shared_note_invitation_email_unavailable",
                "Shared note invitation email is unavailable".to_string(),
            ),
            Self::Upstream => (
                StatusCode::BAD_GATEWAY,
                "cloudsync_credential_service_unavailable",
                "CloudSync credential service is unavailable".to_string(),
            ),
            Self::AttachmentBackupForbidden => (
                StatusCode::FORBIDDEN,
                "attachment_backup_forbidden",
                "Attachment backup access is not permitted".to_string(),
            ),
            Self::AttachmentBackupNotFound => (
                StatusCode::NOT_FOUND,
                "attachment_backup_not_found",
                "Attachment backup is unavailable".to_string(),
            ),
            Self::AttachmentBackupConflict => (
                StatusCode::CONFLICT,
                "attachment_backup_conflict",
                "Attachment backup changed".to_string(),
            ),
            Self::AttachmentBackupDependencyAppeared => (
                StatusCode::CONFLICT,
                "attachment_backup_dependency_appeared",
                "Attachment backup dependency appeared".to_string(),
            ),
            Self::AttachmentBackupDeleteCancelled => (
                StatusCode::CONFLICT,
                "attachment_backup_delete_cancelled",
                "Attachment backup deletion was canceled".to_string(),
            ),
            Self::AttachmentBackupDeleteTooLate => (
                StatusCode::CONFLICT,
                "attachment_backup_delete_too_late",
                "Attachment backup deletion can no longer be canceled".to_string(),
            ),
            Self::AttachmentBackupQuotaExceeded => (
                StatusCode::INSUFFICIENT_STORAGE,
                "attachment_backup_quota_exceeded",
                "Attachment backup quota is exhausted".to_string(),
            ),
            Self::AttachmentBackupServiceUnavailable => (
                StatusCode::BAD_GATEWAY,
                "attachment_backup_service_unavailable",
                "Attachment backup service is unavailable".to_string(),
            ),
            Self::AttachmentBackupVerificationBusy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "attachment_backup_verification_busy",
                "Attachment backup verification is busy".to_string(),
            ),
            Self::SharedAttachmentForbidden => (
                StatusCode::FORBIDDEN,
                "shared_attachment_forbidden",
                "Shared attachment access is not permitted".to_string(),
            ),
            Self::SharedAttachmentNotFound => (
                StatusCode::NOT_FOUND,
                "shared_attachment_not_found",
                "Shared attachment is unavailable".to_string(),
            ),
            Self::SharedAttachmentConflict => (
                StatusCode::CONFLICT,
                "shared_attachment_conflict",
                "Shared attachment changed".to_string(),
            ),
            Self::SharedAttachmentQuotaExceeded => (
                StatusCode::INSUFFICIENT_STORAGE,
                "shared_attachment_quota_exceeded",
                "Shared attachment quota is exhausted".to_string(),
            ),
            Self::SharedAttachmentServiceUnavailable => (
                StatusCode::BAD_GATEWAY,
                "shared_attachment_service_unavailable",
                "Shared attachment service is unavailable".to_string(),
            ),
            Self::SharedAttachmentVerificationBusy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "shared_attachment_verification_busy",
                "Shared attachment verification is busy".to_string(),
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
