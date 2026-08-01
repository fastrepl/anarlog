use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, Sqlite, SqlitePool};

use super::binding::{require_claimed_binding, validated_account_user_id};
use super::{
    CLOUDSYNC_FULL_RESYNC_PENDING_ID, CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
    CLOUDSYNC_FULL_RESYNC_RECOVERY_ID, CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
    CLOUDSYNC_RECOVERY_BARRIER_FIELD, CLOUDSYNC_RECOVERY_BARRIER_TABLE,
    CLOUDSYNC_RECOVERY_PROTOCOL_VERSION, CloudsyncWorkspaceError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudsyncFullResyncResetState {
    ResetRequired,
    PoisonRecoveryRequired,
    ReceiveOnlyResetApplied,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudsyncRecoveryPhase {
    NeedFirstLogout,
    NeedBarrierInsert,
    NeedBarrierConfirm,
    NeedCleanReceive,
    NeedWitnessRepair,
    NeedBarrierCleanup,
    NeedTransportResume,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudsyncRecoveryState {
    pub protocol_version: u32,
    pub generation: String,
    pub account_user_id: String,
    pub workspace_id: String,
    pub key_id: String,
    pub phase: CloudsyncRecoveryPhase,
    pub barrier_id: String,
    pub barrier_payload: String,
    pub barrier_payload_hash: String,
}

pub async fn cloudsync_full_resync_generation(
    pool: &SqlitePool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .fetch_optional(pool)
            .await?;
    value_json
        .map(|value_json| {
            serde_json::from_str(&value_json)
                .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)
        })
        .transpose()
}

pub async fn stage_cloudsync_poison_recovery(
    pool: &SqlitePool,
    account_user_id: &str,
    workspace_id: &str,
) -> Result<String, CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;
    if workspace_id.trim().is_empty() {
        return Err(CloudsyncWorkspaceError::InvalidRecoveryState);
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    require_claimed_binding(&mut transaction, account_user_id).await?;

    let recovery_state: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(&mut *transaction)
            .await?;
    let pending_generation: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .fetch_optional(&mut *transaction)
            .await?;
    let generation = match (recovery_state, pending_generation) {
        (Some(recovery_json), pending_json) => {
            let state = parse_cloudsync_recovery_state(&recovery_json)?;
            if state.account_user_id != account_user_id || state.workspace_id != workspace_id {
                return Err(CloudsyncWorkspaceError::RecoveryConflict);
            }
            if let Some(pending_json) = pending_json {
                let pending_generation = serde_json::from_str::<String>(&pending_json)
                    .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
                if pending_generation != state.generation {
                    return Err(CloudsyncWorkspaceError::RecoveryConflict);
                }
            }
            state.generation
        }
        (None, Some(value_json)) => serde_json::from_str::<String>(&value_json)
            .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?,
        (None, None) => {
            for id in [
                CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
                CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
            ] {
                sqlx::query("DELETE FROM app_settings WHERE id = ?")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
            }
            uuid::Uuid::new_v4().to_string()
        }
    };

    let value_json = serde_json::to_string(&generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
         VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(value_json)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(generation)
}

pub async fn ensure_cloudsync_recovery_state(
    pool: &SqlitePool,
    generation: &str,
    account_user_id: &str,
    workspace_id: &str,
    key: &anlg_e2ee::WorkspaceKey,
) -> Result<CloudsyncRecoveryState, CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;
    if generation.trim().is_empty() || workspace_id.trim().is_empty() {
        return Err(CloudsyncWorkspaceError::InvalidRecoveryState);
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let pending_value = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    let pending_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM app_settings WHERE id = ? AND value_json = ?
         )",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(&pending_value)
    .fetch_one(&mut *transaction)
    .await?;
    if !pending_matches {
        return Err(CloudsyncWorkspaceError::RecoveryConflict);
    }

    if let Some(value_json) =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(&mut *transaction)
            .await?
    {
        let state = parse_cloudsync_recovery_state(&value_json)?;
        if state.generation != generation
            || state.account_user_id != account_user_id
            || state.workspace_id != workspace_id
            || state.key_id != key.key_id()
        {
            return Err(CloudsyncWorkspaceError::RecoveryConflict);
        }
        transaction.commit().await?;
        return Ok(state);
    }

    let writer_id = uuid::Uuid::new_v4().simple().to_string();
    let barrier = key
        .seal_field(
            workspace_id,
            CLOUDSYNC_RECOVERY_BARRIER_TABLE,
            generation,
            CLOUDSYNC_RECOVERY_BARRIER_FIELD,
            &writer_id,
            1,
            false,
            json!({
                "protocol_version": CLOUDSYNC_RECOVERY_PROTOCOL_VERSION,
                "generation": generation,
                "nonce": uuid::Uuid::new_v4().to_string(),
            }),
        )
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    let state = CloudsyncRecoveryState {
        protocol_version: CLOUDSYNC_RECOVERY_PROTOCOL_VERSION,
        generation: generation.to_string(),
        account_user_id: account_user_id.to_string(),
        workspace_id: workspace_id.to_string(),
        key_id: key.key_id().to_string(),
        phase: CloudsyncRecoveryPhase::NeedFirstLogout,
        barrier_id: barrier.record_id,
        barrier_payload_hash: anlg_e2ee::payload_hash(&barrier.payload),
        barrier_payload: barrier.payload,
    };
    let value_json =
        serde_json::to_string(&state).map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
        .bind(value_json)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(state)
}

pub async fn cloudsync_recovery_state<'e, E>(
    executor: E,
) -> Result<Option<CloudsyncRecoveryState>, CloudsyncWorkspaceError>
where
    E: Executor<'e, Database = Sqlite>,
{
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(executor)
            .await?;
    value_json
        .map(|value_json| parse_cloudsync_recovery_state(&value_json))
        .transpose()
}

pub async fn advance_cloudsync_recovery_phase(
    pool: &SqlitePool,
    generation: &str,
    expected: CloudsyncRecoveryPhase,
    next: CloudsyncRecoveryPhase,
) -> Result<bool, CloudsyncWorkspaceError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let Some(value_json) =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(&mut *transaction)
            .await?
    else {
        transaction.commit().await?;
        return Ok(false);
    };
    let mut state = parse_cloudsync_recovery_state(&value_json)?;
    if state.generation != generation || state.phase != expected {
        transaction.commit().await?;
        return Ok(false);
    }
    let generation_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    let pending_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM app_settings WHERE id = ? AND value_json = ?
         )",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(generation_json)
    .fetch_one(&mut *transaction)
    .await?;
    if !pending_matches {
        transaction.commit().await?;
        return Ok(false);
    }
    state.phase = next;
    let next_value_json =
        serde_json::to_string(&state).map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    let updated = sqlx::query(
        "UPDATE app_settings
         SET value_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND value_json = ?",
    )
    .bind(next_value_json)
    .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
    .bind(value_json)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        == 1;
    transaction.commit().await?;
    Ok(updated)
}

pub async fn insert_cloudsync_recovery_barrier(
    pool: &SqlitePool,
    state: &CloudsyncRecoveryState,
    key: &anlg_e2ee::WorkspaceKey,
) -> Result<bool, CloudsyncWorkspaceError> {
    validate_cloudsync_recovery_barrier(state, key)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let existing: Option<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, payload
         FROM e2ee_records
         WHERE id = ?",
    )
    .bind(&state.barrier_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((workspace_id, payload)) = existing {
        if workspace_id != state.workspace_id || payload != state.barrier_payload {
            return Err(CloudsyncWorkspaceError::RecoveryConflict);
        }
        transaction.commit().await?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
         VALUES (?, ?, ?)",
    )
    .bind(&state.barrier_id)
    .bind(&state.workspace_id)
    .bind(&state.barrier_payload)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

pub async fn cloudsync_recovery_barrier_is_exact(
    pool: &SqlitePool,
    state: &CloudsyncRecoveryState,
    key: &anlg_e2ee::WorkspaceKey,
) -> Result<bool, CloudsyncWorkspaceError> {
    validate_cloudsync_recovery_barrier(state, key)?;
    let record: Option<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, payload
         FROM e2ee_records
         WHERE id = ?",
    )
    .bind(&state.barrier_id)
    .fetch_optional(pool)
    .await?;
    Ok(record.is_some_and(|(workspace_id, payload)| {
        workspace_id == state.workspace_id && payload == state.barrier_payload
    }))
}

pub async fn delete_cloudsync_recovery_barrier(
    pool: &SqlitePool,
    state: &CloudsyncRecoveryState,
    key: &anlg_e2ee::WorkspaceKey,
) -> Result<bool, CloudsyncWorkspaceError> {
    validate_cloudsync_recovery_barrier(state, key)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let record: Option<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, payload
         FROM e2ee_records
         WHERE id = ?",
    )
    .bind(&state.barrier_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((workspace_id, payload)) = record else {
        transaction.commit().await?;
        return Ok(false);
    };
    if workspace_id != state.workspace_id || payload != state.barrier_payload {
        return Err(CloudsyncWorkspaceError::RecoveryConflict);
    }
    sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
        .bind(&state.barrier_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(true)
}

pub async fn complete_cloudsync_recovery(
    pool: &SqlitePool,
    generation: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let Some(value_json) =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(&mut *transaction)
            .await?
    else {
        transaction.commit().await?;
        return Ok(false);
    };
    let state = parse_cloudsync_recovery_state(&value_json)?;
    if state.generation != generation || state.phase != CloudsyncRecoveryPhase::NeedTransportResume
    {
        transaction.commit().await?;
        return Ok(false);
    }
    let generation_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    let pending_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM app_settings WHERE id = ? AND value_json = ?
         )",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(&generation_json)
    .fetch_one(&mut *transaction)
    .await?;
    if !pending_matches {
        transaction.commit().await?;
        return Ok(false);
    }
    for id in [
        CLOUDSYNC_FULL_RESYNC_PENDING_ID,
        CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
        CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
    ] {
        sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
            .bind(id)
            .bind(&generation_json)
            .execute(&mut *transaction)
            .await?;
    }
    let deleted = sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
        .bind(value_json)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
        == 1;
    transaction.commit().await?;
    Ok(deleted)
}

pub async fn clear_cloudsync_full_resync_pending(
    pool: &SqlitePool,
    generation: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(&value_json)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
        .bind(&value_json)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
        .bind(value_json)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn cloudsync_full_resync_reset_state(
    pool: &SqlitePool,
    generation: &str,
) -> Result<CloudsyncFullResyncResetState, CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    let (legacy_marker, receive_only_marker): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT
           (SELECT value_json FROM app_settings WHERE id = ?),
           (SELECT value_json FROM app_settings WHERE id = ?)",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
    .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
    .fetch_one(pool)
    .await?;

    Ok(
        if receive_only_marker.as_deref() == Some(value_json.as_str()) {
            CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
        } else if legacy_marker.is_some() && legacy_marker != receive_only_marker {
            CloudsyncFullResyncResetState::PoisonRecoveryRequired
        } else {
            CloudsyncFullResyncResetState::ResetRequired
        },
    )
}

pub async fn cloudsync_full_resync_requires_reset(
    pool: &SqlitePool,
    generation: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
            .fetch_optional(pool)
            .await?;
    let applied_generation = value_json
        .map(|value_json| {
            serde_json::from_str::<String>(&value_json)
                .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)
        })
        .transpose()?;
    Ok(applied_generation.as_deref() != Some(generation))
}

pub async fn cloudsync_encrypted_replica_is_empty(
    pool: &SqlitePool,
) -> Result<bool, CloudsyncWorkspaceError> {
    let has_records: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_records LIMIT 1)")
        .fetch_one(pool)
        .await?;
    Ok(!has_records)
}

pub async fn mark_cloudsync_full_resync_reset_applied(
    pool: &SqlitePool,
    generation: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
         VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
    .bind(value_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_cloudsync_full_resync_receive_only_reset_applied(
    pool: &SqlitePool,
    generation: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let pending_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM app_settings WHERE id = ? AND value_json = ?
         )",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(&value_json)
    .fetch_one(&mut *transaction)
    .await?;
    if !pending_matches {
        transaction.commit().await?;
        return Ok(false);
    }

    for id in [
        CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
        CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
    ] {
        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET
               value_json = excluded.value_json,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        )
        .bind(id)
        .bind(&value_json)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(true)
}

pub(super) fn parse_cloudsync_recovery_state(
    value_json: &str,
) -> Result<CloudsyncRecoveryState, CloudsyncWorkspaceError> {
    let state: CloudsyncRecoveryState = serde_json::from_str(value_json)
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    if state.protocol_version != CLOUDSYNC_RECOVERY_PROTOCOL_VERSION
        || state.generation.trim().is_empty()
        || state.account_user_id.trim().is_empty()
        || state.workspace_id.trim().is_empty()
        || state.key_id.trim().is_empty()
        || state.barrier_id.trim().is_empty()
        || state.barrier_payload.trim().is_empty()
        || anlg_e2ee::payload_hash(&state.barrier_payload) != state.barrier_payload_hash
    {
        return Err(CloudsyncWorkspaceError::InvalidRecoveryState);
    }
    Ok(state)
}

fn validate_cloudsync_recovery_barrier(
    state: &CloudsyncRecoveryState,
    key: &anlg_e2ee::WorkspaceKey,
) -> Result<(), CloudsyncWorkspaceError> {
    if state.protocol_version != CLOUDSYNC_RECOVERY_PROTOCOL_VERSION
        || state.key_id != key.key_id()
        || anlg_e2ee::payload_hash(&state.barrier_payload) != state.barrier_payload_hash
    {
        return Err(CloudsyncWorkspaceError::InvalidRecoveryState);
    }
    let field = key
        .open_field(
            &state.workspace_id,
            &state.barrier_id,
            &state.barrier_payload,
        )
        .map_err(|_| CloudsyncWorkspaceError::InvalidRecoveryState)?;
    if field.table != CLOUDSYNC_RECOVERY_BARRIER_TABLE
        || field.row_id != state.generation
        || field.field != CLOUDSYNC_RECOVERY_BARRIER_FIELD
        || field.revision != 1
        || field.deleted
        || field
            .value
            .get("protocol_version")
            .and_then(|value| value.as_u64())
            != Some(u64::from(CLOUDSYNC_RECOVERY_PROTOCOL_VERSION))
        || field
            .value
            .get("generation")
            .and_then(|value| value.as_str())
            != Some(state.generation.as_str())
        || field
            .value
            .get("nonce")
            .and_then(|value| value.as_str())
            .is_none()
    {
        return Err(CloudsyncWorkspaceError::InvalidRecoveryState);
    }
    Ok(())
}
