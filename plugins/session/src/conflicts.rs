use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ResolveSessionConflictsRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ResolveSessionConflictRequest {
    pub conflict_id: String,
}

pub async fn resolve_session_conflicts(
    pool: &SqlitePool,
    request: ResolveSessionConflictsRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::resolve_session_note_conflicts(&mut transaction, &request.session_id)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn resolve_session_conflict(
    pool: &SqlitePool,
    request: ResolveSessionConflictRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::resolve_session_conflict(&mut transaction, &request.conflict_id)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
