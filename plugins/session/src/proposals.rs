use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PersistChatSessionProposalRequest {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub target_id: String,
    pub base_updated_at: String,
    pub current_markdown: String,
    pub proposed_markdown: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SetSessionProposalStatusRequest {
    pub proposal_id: String,
    pub status: String,
}

pub async fn persist_chat_session_proposal(
    pool: &SqlitePool,
    request: PersistChatSessionProposalRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::insert_pending_session_proposal(
        &mut transaction,
        &request.id,
        &request.session_id,
        &request.kind,
        &request.target_id,
        &request.base_updated_at,
        &request.current_markdown,
        &request.proposed_markdown,
        &request.source,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn set_session_proposal_status(
    pool: &SqlitePool,
    request: SetSessionProposalStatusRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::set_pending_session_proposal_status(
        &mut transaction,
        &request.proposal_id,
        &request.status,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
