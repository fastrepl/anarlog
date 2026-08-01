use sqlx::{Sqlite, SqlitePool, Transaction};

use super::binding::{require_claimed_binding, validated_account_user_id};
use super::recovery::parse_cloudsync_recovery_state;
use super::{
    CLOUDSYNC_FULL_RESYNC_PENDING_ID, CLOUDSYNC_FULL_RESYNC_RECOVERY_ID, CloudsyncWorkspaceError,
};

const CLOUDSYNC_WRITE_FILTER_VERSION_ID: &str = "cloudsync_write_filter_version";
const CLOUDSYNC_WRITE_FILTER_VERSION: &str = "writable-workspaces-v1";
pub(super) const CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE: i64 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudsyncWorkspaceProjection {
    pub account_user_id: String,
    pub personal_workspace_id: String,
    pub workspaces: Vec<CloudsyncWorkspaceProjectionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudsyncWorkspaceProjectionEntry {
    pub id: String,
    pub owner_user_id: String,
    pub kind: String,
    pub name: String,
    pub membership_id: String,
    pub role: String,
    pub membership_created_at: String,
    pub membership_updated_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudsyncWorkspaceReconciliationPlan {
    pub granted_workspace_ids: Vec<String>,
    pub revoked_workspace_ids: Vec<String>,
}

impl CloudsyncWorkspaceReconciliationPlan {
    pub fn requires_replica_reset(&self) -> bool {
        !self.revoked_workspace_ids.is_empty()
    }

    pub fn requires_full_resync(&self) -> bool {
        !self.granted_workspace_ids.is_empty() || !self.revoked_workspace_ids.is_empty()
    }
}

fn check_workspace_projection_cancellation(
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    if is_cancelled() {
        Err(CloudsyncWorkspaceError::ProjectionCancelled)
    } else {
        Ok(())
    }
}

pub async fn replace_cloudsync_workspace_projection(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
) -> Result<(), CloudsyncWorkspaceError> {
    write_cloudsync_workspace_projection(pool, projection, false, false)
        .await
        .map(|_| ())
}

pub async fn stage_cloudsync_workspace_reconciliation(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
) -> Result<CloudsyncWorkspaceReconciliationPlan, CloudsyncWorkspaceError> {
    stage_cloudsync_workspace_reconciliation_cancellable(pool, projection, || false).await
}

pub async fn stage_cloudsync_workspace_reconciliation_cancellable(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<CloudsyncWorkspaceReconciliationPlan, CloudsyncWorkspaceError> {
    validate_cloudsync_workspace_projection(projection)?;
    check_workspace_projection_cancellation(&mut is_cancelled)?;

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = stage_cloudsync_workspace_reconciliation_in_transaction(
        &mut transaction,
        projection,
        &mut is_cancelled,
    )
    .await;
    let result = result.and_then(|plan| {
        check_workspace_projection_cancellation(&mut is_cancelled)?;
        Ok(plan)
    });
    match result {
        Ok(plan) => {
            transaction.commit().await?;
            Ok(plan)
        }
        Err(CloudsyncWorkspaceError::ProjectionCancelled) => {
            transaction.rollback().await?;
            Err(CloudsyncWorkspaceError::ProjectionCancelled)
        }
        Err(error) => Err(error),
    }
}

async fn stage_cloudsync_workspace_reconciliation_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    projection: &CloudsyncWorkspaceProjection,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<CloudsyncWorkspaceReconciliationPlan, CloudsyncWorkspaceError> {
    require_claimed_binding(transaction, &projection.account_user_id).await?;
    check_workspace_projection_cancellation(is_cancelled)?;

    let existing_workspace_ids = load_active_workspace_ids_in_batches(
        transaction,
        &projection.account_user_id,
        is_cancelled,
    )
    .await?;
    let projected_workspace_ids = projection
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut granted_workspace_ids = projected_workspace_ids
        .difference(&existing_workspace_ids)
        .cloned()
        .collect::<Vec<_>>();
    let mut revoked_workspace_ids = existing_workspace_ids
        .difference(&projected_workspace_ids)
        .cloned()
        .collect::<Vec<_>>();
    granted_workspace_ids.sort();
    revoked_workspace_ids.sort();

    for workspace_id in &revoked_workspace_ids {
        stage_workspace_session_evictions_in_batches(transaction, workspace_id, is_cancelled)
            .await?;
        check_workspace_projection_cancellation(is_cancelled)?;
    }

    Ok(CloudsyncWorkspaceReconciliationPlan {
        granted_workspace_ids,
        revoked_workspace_ids,
    })
}

async fn load_active_workspace_ids_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<std::collections::HashSet<String>, CloudsyncWorkspaceError> {
    let mut workspace_ids = std::collections::HashSet::new();
    let mut after_id = None::<String>;
    loop {
        check_workspace_projection_cancellation(is_cancelled)?;
        let rows: Vec<(String, String)> = if let Some(after_id) = after_id.as_deref() {
            sqlx::query_as(
                "SELECT id, workspace_id
                 FROM workspace_memberships
                 WHERE user_id = ? AND deleted_at IS NULL AND id > ?
                 ORDER BY id
                 LIMIT ?",
            )
            .bind(account_user_id)
            .bind(after_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id, workspace_id
                 FROM workspace_memberships
                 WHERE user_id = ? AND deleted_at IS NULL
                 ORDER BY id
                 LIMIT ?",
            )
            .bind(account_user_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        };
        check_workspace_projection_cancellation(is_cancelled)?;

        let batch_len = rows.len();
        if let Some((last_id, _)) = rows.last() {
            after_id = Some(last_id.clone());
        }
        workspace_ids.extend(rows.into_iter().map(|(_, workspace_id)| workspace_id));
        if batch_len < CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize {
            return Ok(workspace_ids);
        }
    }
}

async fn stage_workspace_session_evictions_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut last_session_id = None::<String>;
    loop {
        check_workspace_projection_cancellation(is_cancelled)?;
        let session_ids: Vec<String> = if let Some(last_session_id) = last_session_id.as_deref() {
            sqlx::query_scalar(
                "INSERT INTO cloudsync_session_evictions (session_id, workspace_id)
                 SELECT id, workspace_id
                 FROM sessions
                 WHERE workspace_id = ? AND id > ?
                 ORDER BY id
                 LIMIT ?
                 ON CONFLICT(session_id) DO UPDATE SET
                   workspace_id = excluded.workspace_id,
                   queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   attempt_count = 0,
                   last_attempt_at = NULL,
                   last_error = ''
                 RETURNING session_id",
            )
            .bind(workspace_id)
            .bind(last_session_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        } else {
            sqlx::query_scalar(
                "INSERT INTO cloudsync_session_evictions (session_id, workspace_id)
                 SELECT id, workspace_id
                 FROM sessions
                 WHERE workspace_id = ?
                 ORDER BY id
                 LIMIT ?
                 ON CONFLICT(session_id) DO UPDATE SET
                   workspace_id = excluded.workspace_id,
                   queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   attempt_count = 0,
                   last_attempt_at = NULL,
                   last_error = ''
                 RETURNING session_id",
            )
            .bind(workspace_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        };
        check_workspace_projection_cancellation(is_cancelled)?;

        let batch_len = session_ids.len();
        if batch_len < CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize {
            break;
        }
        last_session_id = session_ids.into_iter().max();
    }
    Ok(())
}

pub async fn commit_cloudsync_workspace_projection(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    write_cloudsync_workspace_projection(pool, projection, require_full_resync, true).await
}

pub async fn commit_cloudsync_workspace_projection_cancellable(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
    is_cancelled: impl FnMut() -> bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    write_cloudsync_workspace_projection_cancellable(
        pool,
        projection,
        require_full_resync,
        true,
        is_cancelled,
    )
    .await
}

pub async fn cloudsync_write_filter_installed(
    pool: &SqlitePool,
    personal_workspace_id: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let personal_workspace_id = validated_account_user_id(personal_workspace_id)?;
    if !cloudsync_write_filter_version_current(pool).await? {
        return Ok(false);
    }
    let writable_scope_matches: bool = sqlx::query_scalar(
        "SELECT COUNT(*) = 1 AND MAX(allowed_workspace_id) = ?
         FROM cloudsync_writable_workspaces",
    )
    .bind(personal_workspace_id)
    .fetch_one(pool)
    .await?;
    Ok(writable_scope_matches)
}

pub async fn cloudsync_write_filter_version_current(
    pool: &SqlitePool,
) -> Result<bool, CloudsyncWorkspaceError> {
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_WRITE_FILTER_VERSION_ID)
            .fetch_optional(pool)
            .await?;
    let current_version = value_json
        .and_then(|value_json| serde_json::from_str::<String>(&value_json).ok())
        .is_some_and(|version| version == CLOUDSYNC_WRITE_FILTER_VERSION);
    Ok(current_version)
}

pub async fn set_cloudsync_personal_write_scope(
    pool: &SqlitePool,
    personal_workspace_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let personal_workspace_id = validated_account_user_id(personal_workspace_id)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query("DELETE FROM cloudsync_writable_workspaces")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("INSERT INTO cloudsync_writable_workspaces (allowed_workspace_id) VALUES (?)")
        .bind(personal_workspace_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn mark_cloudsync_write_filter_installed(
    pool: &SqlitePool,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(CLOUDSYNC_WRITE_FILTER_VERSION)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    sqlx::query(
        "INSERT INTO app_settings (id, value_json)
         VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(CLOUDSYNC_WRITE_FILTER_VERSION_ID)
    .bind(value_json)
    .execute(pool)
    .await?;
    Ok(())
}

async fn write_cloudsync_workspace_projection(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
    require_claimed_account: bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    write_cloudsync_workspace_projection_cancellable(
        pool,
        projection,
        require_full_resync,
        require_claimed_account,
        || false,
    )
    .await
}

async fn write_cloudsync_workspace_projection_cancellable(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
    require_claimed_account: bool,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    validate_cloudsync_workspace_projection(projection)?;
    check_workspace_projection_cancellation(&mut is_cancelled)?;

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = write_cloudsync_workspace_projection_in_transaction(
        &mut transaction,
        projection,
        require_full_resync,
        require_claimed_account,
        &mut is_cancelled,
    )
    .await;
    let result = result.and_then(|generation| {
        check_workspace_projection_cancellation(&mut is_cancelled)?;
        Ok(generation)
    });
    match result {
        Ok(generation) => {
            transaction.commit().await?;
            Ok(generation)
        }
        Err(CloudsyncWorkspaceError::ProjectionCancelled) => {
            transaction.rollback().await?;
            Err(CloudsyncWorkspaceError::ProjectionCancelled)
        }
        Err(error) => Err(error),
    }
}

async fn write_cloudsync_workspace_projection_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
    require_claimed_account: bool,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    if require_claimed_account {
        require_claimed_binding(transaction, &projection.account_user_id).await?;
        check_workspace_projection_cancellation(is_cancelled)?;
    }
    let recovery_state: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_RECOVERY_ID)
            .fetch_optional(&mut **transaction)
            .await?;
    check_workspace_projection_cancellation(is_cancelled)?;
    let full_resync_generation = if let Some(value_json) = recovery_state {
        let state = parse_cloudsync_recovery_state(&value_json)?;
        if state.account_user_id != projection.account_user_id
            || state.workspace_id != projection.personal_workspace_id
        {
            return Err(CloudsyncWorkspaceError::RecoveryConflict);
        }
        Some(state.generation)
    } else if require_full_resync {
        Some(uuid::Uuid::new_v4().to_string())
    } else {
        None
    };
    delete_workspace_projection_rows_in_batches(transaction, "workspace_memberships", is_cancelled)
        .await?;
    delete_workspace_projection_rows_in_batches(transaction, "workspaces", is_cancelled).await?;

    for workspace in &projection.workspaces {
        sqlx::query(
            "INSERT INTO workspaces (
               id, owner_user_id, kind, name, created_at, updated_at, deleted_at
             ) VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&workspace.id)
        .bind(&workspace.owner_user_id)
        .bind(&workspace.kind)
        .bind(&workspace.name)
        .bind(&workspace.created_at)
        .bind(&workspace.updated_at)
        .execute(&mut **transaction)
        .await?;
        check_workspace_projection_cancellation(is_cancelled)?;
        sqlx::query(
            "INSERT INTO workspace_memberships (
               id, workspace_id, user_id, role, created_at, updated_at, deleted_at
             ) VALUES (?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(&workspace.membership_id)
        .bind(&workspace.id)
        .bind(&projection.account_user_id)
        .bind(&workspace.role)
        .bind(&workspace.membership_created_at)
        .bind(&workspace.membership_updated_at)
        .execute(&mut **transaction)
        .await?;
        check_workspace_projection_cancellation(is_cancelled)?;

        delete_workspace_session_evictions_in_batches(transaction, &workspace.id, is_cancelled)
            .await?;
        check_workspace_projection_cancellation(is_cancelled)?;
    }

    if let Some(generation) = full_resync_generation.as_ref() {
        let value_json = serde_json::to_string(generation)
            .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET
               value_json = excluded.value_json,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        )
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(value_json)
        .execute(&mut **transaction)
        .await?;
        check_workspace_projection_cancellation(is_cancelled)?;
    }

    Ok(full_resync_generation)
}

async fn delete_workspace_projection_rows_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let delete_sql = match table_name {
        "workspace_memberships" => {
            "DELETE FROM workspace_memberships
             WHERE id IN (
               SELECT id
               FROM workspace_memberships
               ORDER BY id
               LIMIT ?
             )
             RETURNING id"
        }
        "workspaces" => {
            "DELETE FROM workspaces
             WHERE id IN (
               SELECT id
               FROM workspaces
               ORDER BY id
               LIMIT ?
             )
             RETURNING id"
        }
        _ => unreachable!("workspace projection table names are fixed"),
    };

    loop {
        check_workspace_projection_cancellation(is_cancelled)?;
        let deleted_ids: Vec<String> = sqlx::query_scalar(delete_sql)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?;
        check_workspace_projection_cancellation(is_cancelled)?;
        if deleted_ids.len() < CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize {
            return Ok(());
        }
    }
}

async fn delete_workspace_session_evictions_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut last_session_id = None::<String>;
    loop {
        check_workspace_projection_cancellation(is_cancelled)?;
        let session_ids: Vec<String> = if let Some(last_session_id) = last_session_id.as_deref() {
            sqlx::query_scalar(
                "DELETE FROM cloudsync_session_evictions
                 WHERE session_id IN (
                   SELECT session_id
                   FROM cloudsync_session_evictions
                   WHERE workspace_id = ? AND session_id > ?
                   ORDER BY session_id
                   LIMIT ?
                 )
                 RETURNING session_id",
            )
            .bind(workspace_id)
            .bind(last_session_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        } else {
            sqlx::query_scalar(
                "DELETE FROM cloudsync_session_evictions
                 WHERE session_id IN (
                   SELECT session_id
                   FROM cloudsync_session_evictions
                   WHERE workspace_id = ?
                   ORDER BY session_id
                   LIMIT ?
                 )
                 RETURNING session_id",
            )
            .bind(workspace_id)
            .bind(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE)
            .fetch_all(&mut **transaction)
            .await?
        };
        check_workspace_projection_cancellation(is_cancelled)?;

        let batch_len = session_ids.len();
        if batch_len < CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize {
            break;
        }
        last_session_id = session_ids.into_iter().max();
    }
    Ok(())
}

pub fn validate_cloudsync_workspace_projection(
    projection: &CloudsyncWorkspaceProjection,
) -> Result<(), CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(&projection.account_user_id)?;
    if projection.personal_workspace_id != account_user_id || projection.workspaces.is_empty() {
        return Err(CloudsyncWorkspaceError::InvalidWorkspaceProjection);
    }

    let mut workspace_ids = std::collections::HashSet::new();
    let mut membership_ids = std::collections::HashSet::new();
    for workspace in &projection.workspaces {
        if workspace.id.trim().is_empty()
            || workspace.owner_user_id.trim().is_empty()
            || !matches!(workspace.kind.as_str(), "personal" | "shared")
            || workspace.membership_id.trim().is_empty()
            || !matches!(workspace.role.as_str(), "owner" | "admin" | "member")
            || workspace.membership_created_at.trim().is_empty()
            || workspace.membership_updated_at.trim().is_empty()
            || workspace.created_at.trim().is_empty()
            || workspace.updated_at.trim().is_empty()
            || !workspace_ids.insert(workspace.id.as_str())
            || !membership_ids.insert(workspace.membership_id.as_str())
        {
            return Err(CloudsyncWorkspaceError::InvalidWorkspaceProjection);
        }
    }

    let mut personal_workspaces = projection
        .workspaces
        .iter()
        .filter(|workspace| workspace.kind == "personal");
    let Some(personal_workspace) = personal_workspaces.next() else {
        return Err(CloudsyncWorkspaceError::InvalidWorkspaceProjection);
    };
    if personal_workspaces.next().is_some()
        || personal_workspace.id != projection.personal_workspace_id
        || personal_workspace.owner_user_id != account_user_id
        || personal_workspace.role != "owner"
    {
        return Err(CloudsyncWorkspaceError::InvalidWorkspaceProjection);
    }

    Ok(())
}
