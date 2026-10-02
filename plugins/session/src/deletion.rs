use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

const SESSION_CHILD_TABLES: &[&str] = &[
    "session_documents",
    "transcripts",
    "session_participants",
    "session_tags",
    "action_items",
    "session_attachments",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TombstoneSessionRequest {
    pub session_id: String,
    pub tombstone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DeletedSessionRow {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RestoreDeletedSessionOutcome {
    Restored,
    Alive,
    NotDeleted,
}

async fn run_tombstone_statements(
    conn: &mut sqlx::SqliteConnection,
    session_id: &str,
    tombstone: &str,
    restore: bool,
) -> Result<u64, String> {
    for table in SESSION_CHILD_TABLES {
        anlg_db_app::update_session_child_tombstone(conn, table, session_id, tombstone, restore)
            .await
            .map_err(|error| error.to_string())?;
    }
    anlg_db_app::update_entity_mentions_tombstone(conn, session_id, tombstone, restore)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::update_sessions_tombstone(conn, session_id, tombstone, restore)
        .await
        .map_err(|error| error.to_string())
}

pub async fn soft_delete_session(
    pool: &SqlitePool,
    request: TombstoneSessionRequest,
) -> Result<Option<DeletedSessionRow>, String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let Some(session) = anlg_db_app::load_session_delete_row(&mut transaction, &request.session_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        transaction
            .rollback()
            .await
            .map_err(|error| error.to_string())?;
        return Ok(None);
    };

    let tombstoned = run_tombstone_statements(
        &mut transaction,
        &request.session_id,
        &request.tombstone,
        false,
    )
    .await?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if tombstoned != 1 {
        return Ok(None);
    }

    Ok(Some(DeletedSessionRow {
        id: session.id,
        title: session.title,
    }))
}

pub async fn restore_deleted_session(
    pool: &SqlitePool,
    request: TombstoneSessionRequest,
) -> Result<RestoreDeletedSessionOutcome, String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let restored = run_tombstone_statements(
        &mut transaction,
        &request.session_id,
        &request.tombstone,
        true,
    )
    .await?;
    let outcome = if restored == 1 {
        RestoreDeletedSessionOutcome::Restored
    } else if anlg_db_app::session_is_alive(&mut transaction, &request.session_id)
        .await
        .map_err(|error| error.to_string())?
    {
        RestoreDeletedSessionOutcome::Alive
    } else {
        RestoreDeletedSessionOutcome::NotDeleted
    };

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    Ok(outcome)
}
