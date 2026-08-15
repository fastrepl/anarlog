use axum::Router;

use crate::state::{AppState, ReplicaState};

mod attachment_backups;
mod cloudsync_credentials;
mod e2ee_witness;
mod session_shares;
mod shared_attachments;

#[cfg(test)]
use cloudsync_credentials::{
    CloudsyncWorkspace, DEVICE_FINGERPRINT_HEADER, DEVICE_NAME_HEADER, E2EE_KEY_ID_HEADER,
    MAX_TOKEN_ATTRIBUTES_BYTES, MAX_TOKEN_WORKSPACES, WORKSPACE_PROJECTION_SELECT,
    encode_workspace_token_attributes,
};
#[cfg(test)]
use session_shares::MAX_SNAPSHOT_RESPONSE_BYTES;
pub(crate) use session_shares::{SharedNoteAttachment, validate_shared_attachments};

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut openapi = cloudsync_credentials::openapi();
    openapi.merge(session_shares::openapi());
    openapi.merge(attachment_backups::openapi());
    openapi.merge(e2ee_witness::openapi());
    openapi.merge(shared_attachments::openapi());
    openapi
}

pub fn cloudsync_router(state: AppState) -> Router {
    cloudsync_credentials::router()
        .merge(attachment_backups::router())
        .with_state(state)
}

pub fn replica_router(state: ReplicaState) -> Router {
    cloudsync_credentials::replica_router().with_state(state)
}

pub fn session_share_router(state: AppState) -> Router {
    session_shares::router()
        .merge(shared_attachments::router())
        .with_state(state)
}

pub fn e2ee_witness_router(state: ReplicaState) -> Router {
    e2ee_witness::router().with_state(state)
}

pub fn web_edit_router(state: AppState) -> Router {
    session_shares::web_edit_router().with_state(state)
}

#[cfg(test)]
mod tests;
