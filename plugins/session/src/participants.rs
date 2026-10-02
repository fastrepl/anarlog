use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AddSessionParticipantRequest {
    pub session_id: String,
    pub human_id: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RemoveSessionParticipantRequest {
    pub mapping_id: String,
}

pub async fn add_session_participant(
    pool: &SqlitePool,
    request: AddSessionParticipantRequest,
) -> Result<(), String> {
    let participant_id = uuid::Uuid::new_v4().to_string();
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::revive_excluded_session_participant(
        &mut transaction,
        &request.session_id,
        &request.human_id,
        &request.source,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::insert_manual_session_participant(
        &mut transaction,
        &participant_id,
        &request.session_id,
        &request.human_id,
        &request.source,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn remove_session_participant(
    pool: &SqlitePool,
    request: RemoveSessionParticipantRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::remove_session_participant_mapping(&mut transaction, &request.mapping_id, &now)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
