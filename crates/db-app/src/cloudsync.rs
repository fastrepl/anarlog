use std::sync::LazyLock;

use anlg_db_core::CloudsyncTableSpec;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, QueryBuilder, Sqlite, SqlitePool, Transaction};

pub const CLOUDSYNC_WORKSPACE_BINDING_ID: &str = "cloudsync_workspace_binding";
const CLOUDSYNC_FULL_RESYNC_PENDING_ID: &str = "cloudsync_full_resync_pending";
const CLOUDSYNC_FULL_RESYNC_RECOVERY_ID: &str = "cloudsync_full_resync_recovery_v1";
const CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID: &str = "cloudsync_full_resync_reset_applied";
const CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID: &str =
    "cloudsync_full_resync_receive_only_reset_applied";
const CLOUDSYNC_WRITE_FILTER_VERSION_ID: &str = "cloudsync_write_filter_version";
const CLOUDSYNC_WRITE_FILTER_VERSION: &str = "writable-workspaces-v1";
const LEGACY_DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";
const CLOUDSYNC_RECOVERY_PROTOCOL_VERSION: u32 = 1;
const CLOUDSYNC_RECOVERY_BARRIER_TABLE: &str = "__anarlog_cloudsync_control";
const CLOUDSYNC_RECOVERY_BARRIER_FIELD: &str = "$snapshot_barrier";
const CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE: usize = 128;
const CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE: i64 = 128;

const USER_ID_REFERENCES: &[(&str, &str)] = &[
    ("organizations", "owner_user_id"),
    ("humans", "owner_user_id"),
    ("sessions", "owner_user_id"),
    ("session_documents", "created_by"),
    ("session_documents", "updated_by"),
    ("transcripts", "owner_user_id"),
    ("session_participants", "owner_user_id"),
    ("session_participants", "human_id"),
    ("action_items", "assignee_human_id"),
    ("action_items", "created_by"),
    ("action_items", "updated_by"),
    ("tags", "owner_user_id"),
    ("session_tags", "owner_user_id"),
    ("entity_mentions", "owner_user_id"),
    ("chat_groups", "owner_user_id"),
    ("chat_messages", "owner_user_id"),
    ("daily_notes", "owner_user_id"),
];

#[derive(Debug)]
pub enum CloudsyncWorkspaceError {
    Sqlx(sqlx::Error),
    ClaimCancelled,
    ProjectionCancelled,
    InvalidWorkspaceId,
    InvalidBinding,
    InvalidWorkspaceProjection,
    InvalidRecoveryState,
    RecoveryConflict,
    AccountMismatch,
    ForeignWorkspace { table: String },
}

impl std::fmt::Display for CloudsyncWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::ClaimCancelled => write!(f, "CloudSync workspace claim was cancelled"),
            Self::ProjectionCancelled => {
                write!(f, "CloudSync workspace projection was cancelled")
            }
            Self::InvalidWorkspaceId => write!(f, "workspace ID is invalid"),
            Self::InvalidBinding => write!(f, "local CloudSync workspace binding is invalid"),
            Self::InvalidWorkspaceProjection => {
                write!(f, "CloudSync workspace projection is invalid")
            }
            Self::InvalidRecoveryState => write!(f, "CloudSync recovery state is invalid"),
            Self::RecoveryConflict => write!(f, "CloudSync recovery state changed unexpectedly"),
            Self::AccountMismatch => write!(
                f,
                "this local database is already bound to a different account"
            ),
            Self::ForeignWorkspace { table } => write!(
                f,
                "{table} contains rows owned by a different CloudSync workspace"
            ),
        }
    }
}

impl std::error::Error for CloudsyncWorkspaceError {}

impl From<sqlx::Error> for CloudsyncWorkspaceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

#[derive(Deserialize, Serialize)]
struct CloudsyncWorkspaceBinding {
    workspace_id: String,
    account_user_id: Option<String>,
}

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

impl CloudsyncWorkspaceReconciliationPlan {
    pub fn requires_replica_reset(&self) -> bool {
        !self.revoked_workspace_ids.is_empty()
    }

    pub fn requires_full_resync(&self) -> bool {
        !self.granted_workspace_ids.is_empty() || !self.revoked_workspace_ids.is_empty()
    }
}

static CLOUDSYNC_TABLE_REGISTRY: LazyLock<Vec<CloudsyncTableSpec>> = LazyLock::new(|| {
    [
        ("action_items", false),
        ("calendars", false),
        ("chat_groups", false),
        ("chat_messages", false),
        ("daily_notes", false),
        ("e2ee_records", true),
        ("entity_mentions", false),
        ("events", false),
        ("humans", false),
        ("organizations", false),
        ("session_attachments", false),
        ("session_documents", false),
        ("session_participants", false),
        ("session_tags", false),
        ("sessions", false),
        ("tags", false),
        ("templates", false),
        ("transcripts", false),
        ("workspace_memberships", false),
        ("workspaces", false),
    ]
    .into_iter()
    .map(|(table_name, enabled)| CloudsyncTableSpec {
        table_name: table_name.to_string(),
        crdt_algo: None,
        init_flags: None,
        enabled,
    })
    .collect()
});

pub fn cloudsync_table_registry() -> &'static [CloudsyncTableSpec] {
    CLOUDSYNC_TABLE_REGISTRY.as_slice()
}

pub fn cloudsync_alter_guard_required(table_name: &str) -> bool {
    table_name == "e2ee_records" || crate::E2EE_DOMAIN_TABLES.contains(&table_name)
}

pub async fn ensure_cloudsync_workspace_binding(
    pool: &SqlitePool,
) -> Result<String, CloudsyncWorkspaceError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let binding = load_or_create_binding(&mut transaction).await?;
    transaction.commit().await?;
    Ok(binding.workspace_id)
}

pub async fn cloudsync_workspace_is_claimed_by(
    pool: &SqlitePool,
    account_user_id: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;

    let Some(value_json) =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_WORKSPACE_BINDING_ID)
            .fetch_optional(pool)
            .await?
    else {
        return Ok(false);
    };
    let binding = parse_binding(&value_json)?;

    Ok(binding.workspace_id == account_user_id
        && binding.account_user_id.as_deref() == Some(account_user_id))
}

pub async fn bind_cloudsync_account(
    pool: &SqlitePool,
    account_user_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let binding = load_or_create_binding(&mut transaction).await?;

    if binding
        .account_user_id
        .as_deref()
        .is_some_and(|id| id != account_user_id)
    {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
    }

    if binding.account_user_id.is_none() {
        save_binding(
            &mut transaction,
            &CloudsyncWorkspaceBinding {
                workspace_id: binding.workspace_id,
                account_user_id: Some(account_user_id.to_string()),
            },
        )
        .await?;
    }

    transaction.commit().await?;
    Ok(())
}

pub async fn claim_cloudsync_workspace(
    pool: &SqlitePool,
    account_user_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    claim_cloudsync_workspace_cancellable(pool, account_user_id, || false).await
}

pub async fn claim_cloudsync_workspace_cancellable(
    pool: &SqlitePool,
    account_user_id: &str,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;
    check_workspace_claim_cancellation(&mut is_cancelled)?;

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = claim_cloudsync_workspace_in_transaction(
        &mut transaction,
        account_user_id,
        &mut is_cancelled,
    )
    .await;
    let result = result.and_then(|()| check_workspace_claim_cancellation(&mut is_cancelled));
    match result {
        Ok(()) => transaction.commit().await.map_err(Into::into),
        Err(CloudsyncWorkspaceError::ClaimCancelled) => {
            transaction.rollback().await?;
            Err(CloudsyncWorkspaceError::ClaimCancelled)
        }
        Err(error) => Err(error),
    }
}

async fn claim_cloudsync_workspace_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    check_workspace_claim_cancellation(is_cancelled)?;
    let binding = load_or_create_binding(transaction).await?;
    check_workspace_claim_cancellation(is_cancelled)?;
    if binding
        .account_user_id
        .as_deref()
        .is_some_and(|id| id != account_user_id)
    {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
    }
    if binding.workspace_id == account_user_id
        && binding.account_user_id.as_deref() == Some(account_user_id)
    {
        return Ok(());
    }

    for table_name in crate::E2EE_DOMAIN_TABLES {
        reject_foreign_workspace_rows_in_batches(
            transaction,
            table_name,
            &binding.workspace_id,
            account_user_id,
            is_cancelled,
        )
        .await?;
    }

    for table_name in crate::E2EE_DOMAIN_TABLES {
        claim_workspace_rows_in_batches(
            transaction,
            table_name,
            &binding.workspace_id,
            account_user_id,
            is_cancelled,
        )
        .await?;
    }

    rekey_local_user_identity(
        transaction,
        &binding.workspace_id,
        account_user_id,
        is_cancelled,
    )
    .await?;
    if binding.workspace_id != LEGACY_DEFAULT_USER_ID {
        rekey_local_user_identity(
            transaction,
            LEGACY_DEFAULT_USER_ID,
            account_user_id,
            is_cancelled,
        )
        .await?;
    }

    if binding.workspace_id != account_user_id
        || binding.account_user_id.as_deref() != Some(account_user_id)
    {
        save_binding(
            transaction,
            &CloudsyncWorkspaceBinding {
                workspace_id: account_user_id.to_string(),
                account_user_id: Some(account_user_id.to_string()),
            },
        )
        .await?;
        check_workspace_claim_cancellation(is_cancelled)?;
    }
    check_workspace_claim_cancellation(is_cancelled)?;
    Ok(())
}

fn check_workspace_claim_cancellation(
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    if is_cancelled() {
        Err(CloudsyncWorkspaceError::ClaimCancelled)
    } else {
        Ok(())
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

fn validated_account_user_id(account_user_id: &str) -> Result<&str, CloudsyncWorkspaceError> {
    let account_user_id = account_user_id.trim();
    if account_user_id.is_empty() || account_user_id == LEGACY_DEFAULT_USER_ID {
        return Err(CloudsyncWorkspaceError::InvalidWorkspaceId);
    }
    Ok(account_user_id)
}

async fn reject_foreign_workspace_rows_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    local_workspace_id: &str,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut after_id = None;
    loop {
        check_workspace_claim_cancellation(is_cancelled)?;
        let rows =
            load_text_column_batch(transaction, table_name, "workspace_id", after_id.as_deref())
                .await?;
        check_workspace_claim_cancellation(is_cancelled)?;
        if rows.iter().any(|(_, workspace_id)| {
            !workspace_id.is_empty()
                && workspace_id != local_workspace_id
                && workspace_id != account_user_id
        }) {
            return Err(CloudsyncWorkspaceError::ForeignWorkspace {
                table: table_name.to_string(),
            });
        }
        let row_count = rows.len();
        let Some((last_id, _)) = rows.last() else {
            return Ok(());
        };
        after_id = Some(last_id.clone());
        if row_count < CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn claim_workspace_rows_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    local_workspace_id: &str,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut after_id = None;
    loop {
        check_workspace_claim_cancellation(is_cancelled)?;
        let rows =
            load_text_column_batch(transaction, table_name, "workspace_id", after_id.as_deref())
                .await?;
        check_workspace_claim_cancellation(is_cancelled)?;
        let row_count = rows.len();
        let Some((last_id, _)) = rows.last() else {
            return Ok(());
        };
        after_id = Some(last_id.clone());
        let ids = rows
            .into_iter()
            .filter_map(|(id, workspace_id)| {
                (workspace_id.is_empty()
                    || (local_workspace_id != account_user_id
                        && workspace_id == local_workspace_id))
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            check_workspace_claim_cancellation(is_cancelled)?;
            update_text_column_for_ids(
                transaction,
                table_name,
                "workspace_id",
                account_user_id,
                &ids,
            )
            .await?;
            check_workspace_claim_cancellation(is_cancelled)?;
        }
        if row_count < CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn load_text_column_batch(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    column_name: &str,
    after_id: Option<&str>,
) -> Result<Vec<(String, String)>, CloudsyncWorkspaceError> {
    let mut query =
        QueryBuilder::<Sqlite>::new(format!("SELECT id, {column_name} FROM {table_name}"));
    if let Some(after_id) = after_id {
        query.push(" WHERE id > ").push_bind(after_id);
    }
    query
        .push(" ORDER BY id LIMIT ")
        .push_bind(CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE as i64);
    query
        .build_query_as()
        .fetch_all(&mut **transaction)
        .await
        .map_err(Into::into)
}

async fn update_text_column_for_ids(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    column_name: &str,
    value: &str,
    ids: &[String],
) -> Result<(), CloudsyncWorkspaceError> {
    let mut query =
        QueryBuilder::<Sqlite>::new(format!("UPDATE {table_name} SET {column_name} = "));
    query.push_bind(value).push(" WHERE id IN (");
    let mut ids_query = query.separated(", ");
    for id in ids {
        ids_query.push_bind(id);
    }
    ids_query.push_unseparated(")");
    query.build().execute(&mut **transaction).await?;
    Ok(())
}

async fn rekey_local_user_identity(
    transaction: &mut Transaction<'_, Sqlite>,
    source_user_id: &str,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    check_workspace_claim_cancellation(is_cancelled)?;
    if source_user_id.is_empty() || source_user_id == account_user_id {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO humans (
           id, workspace_id, owner_user_id, organization_id, name, email, phone,
           job_title, linkedin_username, memo, pinned, pin_order, metadata_json,
           created_at, updated_at, deleted_at
         )
         SELECT ?, workspace_id, ?, organization_id, name, email, phone,
           job_title, linkedin_username, memo, pinned, pin_order, metadata_json,
           created_at, updated_at, deleted_at
         FROM humans
         WHERE id = ?
         ON CONFLICT(id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           owner_user_id = excluded.owner_user_id,
           organization_id = CASE WHEN humans.organization_id = ''
             THEN excluded.organization_id ELSE humans.organization_id END,
           name = CASE WHEN humans.name = '' THEN excluded.name ELSE humans.name END,
           email = CASE WHEN humans.email = '' THEN excluded.email ELSE humans.email END,
           phone = CASE WHEN humans.phone = '' THEN excluded.phone ELSE humans.phone END,
           job_title = CASE WHEN humans.job_title = ''
             THEN excluded.job_title ELSE humans.job_title END,
           linkedin_username = CASE WHEN humans.linkedin_username = ''
             THEN excluded.linkedin_username ELSE humans.linkedin_username END,
           memo = CASE WHEN humans.memo = '' THEN excluded.memo ELSE humans.memo END,
           pinned = max(humans.pinned, excluded.pinned),
           pin_order = COALESCE(humans.pin_order, excluded.pin_order),
           metadata_json = CASE WHEN humans.metadata_json IN ('', '{}')
             THEN excluded.metadata_json ELSE humans.metadata_json END,
           created_at = min(humans.created_at, excluded.created_at),
           updated_at = max(humans.updated_at, excluded.updated_at),
           deleted_at = CASE
             WHEN humans.deleted_at IS NULL OR excluded.deleted_at IS NULL THEN NULL
             ELSE max(humans.deleted_at, excluded.deleted_at)
           END",
    )
    .bind(account_user_id)
    .bind(account_user_id)
    .bind(source_user_id)
    .execute(&mut **transaction)
    .await?;
    check_workspace_claim_cancellation(is_cancelled)?;

    tombstone_duplicate_self_participants_in_batches(
        transaction,
        source_user_id,
        account_user_id,
        is_cancelled,
    )
    .await?;

    for (table, column) in USER_ID_REFERENCES {
        rekey_user_reference_in_batches(
            transaction,
            table,
            column,
            source_user_id,
            account_user_id,
            is_cancelled,
        )
        .await?;
    }

    sqlx::query("DELETE FROM humans WHERE id = ?")
        .bind(source_user_id)
        .execute(&mut **transaction)
        .await?;
    check_workspace_claim_cancellation(is_cancelled)?;
    Ok(())
}

async fn tombstone_duplicate_self_participants_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    source_user_id: &str,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut after_id = None;
    loop {
        check_workspace_claim_cancellation(is_cancelled)?;
        let rows = load_text_column_batch(
            transaction,
            "session_participants",
            "human_id",
            after_id.as_deref(),
        )
        .await?;
        check_workspace_claim_cancellation(is_cancelled)?;
        let row_count = rows.len();
        let Some((last_id, _)) = rows.last() else {
            return Ok(());
        };
        after_id = Some(last_id.clone());
        let ids = rows
            .into_iter()
            .filter_map(|(id, human_id)| (human_id == source_user_id).then_some(id))
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            check_workspace_claim_cancellation(is_cancelled)?;
            let mut query = QueryBuilder::<Sqlite>::new(
                "UPDATE session_participants AS duplicate
                 SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE duplicate.id IN (",
            );
            {
                let mut ids_query = query.separated(", ");
                for id in &ids {
                    ids_query.push_bind(id);
                }
                ids_query.push_unseparated(")");
            }
            query
                .push(" AND duplicate.human_id = ")
                .push_bind(source_user_id)
                .push(
                    " AND duplicate.deleted_at IS NULL
                      AND EXISTS (
                        SELECT 1
                        FROM session_participants AS keeper
                        WHERE keeper.session_id = duplicate.session_id
                          AND keeper.deleted_at IS NULL
                          AND keeper.id <> duplicate.id
                          AND (
                            keeper.human_id = ",
                )
                .push_bind(account_user_id)
                .push(" OR (keeper.human_id = ")
                .push_bind(source_user_id)
                .push(
                    " AND keeper.id < duplicate.id)
                          )
                      )",
                );
            query.build().execute(&mut **transaction).await?;
            check_workspace_claim_cancellation(is_cancelled)?;
        }
        if row_count < CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn rekey_user_reference_in_batches(
    transaction: &mut Transaction<'_, Sqlite>,
    table_name: &str,
    column_name: &str,
    source_user_id: &str,
    account_user_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), CloudsyncWorkspaceError> {
    let mut after_id = None;
    loop {
        check_workspace_claim_cancellation(is_cancelled)?;
        let rows =
            load_text_column_batch(transaction, table_name, column_name, after_id.as_deref())
                .await?;
        check_workspace_claim_cancellation(is_cancelled)?;
        let row_count = rows.len();
        let Some((last_id, _)) = rows.last() else {
            return Ok(());
        };
        after_id = Some(last_id.clone());
        let ids = rows
            .into_iter()
            .filter_map(|(id, value)| (value == source_user_id).then_some(id))
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            check_workspace_claim_cancellation(is_cancelled)?;
            update_text_column_for_ids(transaction, table_name, column_name, account_user_id, &ids)
                .await?;
            check_workspace_claim_cancellation(is_cancelled)?;
        }
        if row_count < CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn load_or_create_binding(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<CloudsyncWorkspaceBinding, CloudsyncWorkspaceError> {
    if let Some(value_json) =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_WORKSPACE_BINDING_ID)
            .fetch_optional(&mut **transaction)
            .await?
    {
        return parse_binding(&value_json);
    }

    let binding = CloudsyncWorkspaceBinding {
        workspace_id: uuid::Uuid::new_v4().to_string(),
        account_user_id: None,
    };
    let value_json =
        serde_json::to_string(&binding).map_err(|_| CloudsyncWorkspaceError::InvalidBinding)?;
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_WORKSPACE_BINDING_ID)
        .bind(value_json)
        .execute(&mut **transaction)
        .await?;
    Ok(binding)
}

async fn require_claimed_binding(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_WORKSPACE_BINDING_ID)
            .fetch_optional(&mut **transaction)
            .await?;
    let Some(value_json) = value_json else {
        return Err(CloudsyncWorkspaceError::InvalidBinding);
    };
    let binding = parse_binding(&value_json)?;
    if binding.workspace_id != account_user_id
        || binding.account_user_id.as_deref() != Some(account_user_id)
    {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
    }
    Ok(())
}

fn parse_binding(value_json: &str) -> Result<CloudsyncWorkspaceBinding, CloudsyncWorkspaceError> {
    let binding: CloudsyncWorkspaceBinding =
        serde_json::from_str(value_json).map_err(|_| CloudsyncWorkspaceError::InvalidBinding)?;
    if binding.workspace_id.trim().is_empty() {
        return Err(CloudsyncWorkspaceError::InvalidBinding);
    }
    Ok(binding)
}

fn parse_cloudsync_recovery_state(
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

async fn save_binding(
    transaction: &mut Transaction<'_, Sqlite>,
    binding: &CloudsyncWorkspaceBinding,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json =
        serde_json::to_string(binding).map_err(|_| CloudsyncWorkspaceError::InvalidBinding)?;
    sqlx::query(
        "UPDATE app_settings SET value_json = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(value_json)
    .bind(CLOUDSYNC_WORKSPACE_BINDING_ID)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> anlg_db_core::Db {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    fn recovery_key(workspace_id: &str) -> anlg_e2ee::WorkspaceKey {
        anlg_e2ee::RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
            .unwrap()
            .workspace_key(workspace_id)
            .unwrap()
    }

    async fn mark_full_resync_pending(pool: &SqlitePool, generation: &str) {
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .bind(serde_json::to_string(generation).unwrap())
            .execute(pool)
            .await
            .unwrap();
    }

    fn projection(
        account_user_id: &str,
        workspaces: Vec<CloudsyncWorkspaceProjectionEntry>,
    ) -> CloudsyncWorkspaceProjection {
        CloudsyncWorkspaceProjection {
            account_user_id: account_user_id.to_string(),
            personal_workspace_id: account_user_id.to_string(),
            workspaces,
        }
    }

    fn projected_workspace(
        id: &str,
        owner_user_id: &str,
        kind: &str,
        membership_id: &str,
        role: &str,
        name: &str,
    ) -> CloudsyncWorkspaceProjectionEntry {
        CloudsyncWorkspaceProjectionEntry {
            id: id.to_string(),
            owner_user_id: owner_user_id.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            membership_id: membership_id.to_string(),
            role: role.to_string(),
            membership_created_at: "2026-07-16T00:01:00Z".to_string(),
            membership_updated_at: "2026-07-16T00:02:00Z".to_string(),
            created_at: "2026-07-16T00:00:00Z".to_string(),
            updated_at: "2026-07-16T00:00:00Z".to_string(),
        }
    }

    async fn seed_legacy_workspace_projection_rows(pool: &SqlitePool, last_row_index: i64) {
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1
               FROM sequence
               WHERE row_index < ?
             )
             INSERT INTO workspaces (id, owner_user_id, kind, name)
             SELECT
               printf('legacy-workspace-%04d', row_index),
               'legacy-owner',
               'shared',
               'Legacy'
             FROM sequence",
        )
        .bind(last_row_index)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1
               FROM sequence
               WHERE row_index < ?
             )
             INSERT INTO workspace_memberships (id, workspace_id, user_id, role)
             SELECT
               printf('legacy-membership-%04d', row_index),
               printf('legacy-workspace-%04d', row_index),
               'user-a',
               'member'
             FROM sequence",
        )
        .bind(last_row_index)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn recovery_state_and_phase_transitions_are_crash_safe() {
        let db = test_db().await;
        let generation = "generation-1";
        let key = recovery_key("user-a");
        mark_full_resync_pending(db.pool(), generation).await;

        let state =
            ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
                .await
                .unwrap();
        assert_eq!(state.phase, CloudsyncRecoveryPhase::NeedFirstLogout);
        assert_eq!(
            ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
                .await
                .unwrap(),
            state
        );
        assert!(
            !advance_cloudsync_recovery_phase(
                db.pool(),
                "other-generation",
                CloudsyncRecoveryPhase::NeedFirstLogout,
                CloudsyncRecoveryPhase::NeedBarrierInsert,
            )
            .await
            .unwrap()
        );
        assert!(
            advance_cloudsync_recovery_phase(
                db.pool(),
                generation,
                CloudsyncRecoveryPhase::NeedFirstLogout,
                CloudsyncRecoveryPhase::NeedBarrierInsert,
            )
            .await
            .unwrap()
        );
        assert!(
            !advance_cloudsync_recovery_phase(
                db.pool(),
                generation,
                CloudsyncRecoveryPhase::NeedFirstLogout,
                CloudsyncRecoveryPhase::NeedBarrierInsert,
            )
            .await
            .unwrap()
        );
        assert_eq!(
            cloudsync_recovery_state(db.pool())
                .await
                .unwrap()
                .unwrap()
                .phase,
            CloudsyncRecoveryPhase::NeedBarrierInsert
        );
    }

    #[tokio::test]
    async fn poison_recovery_generation_is_idempotent_and_restores_its_pending_marker() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        for id in [
            CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
            CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
        ] {
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
                .bind(id)
                .bind(serde_json::to_string("stale-generation").unwrap())
                .execute(db.pool())
                .await
                .unwrap();
        }

        let generation = stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
            .await
            .unwrap();
        assert_eq!(
            stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
                .await
                .unwrap(),
            generation
        );
        let stale_markers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_settings
             WHERE id IN (?, ?)",
        )
        .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
        .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(stale_markers, 0);

        let key = recovery_key("user-a");
        ensure_cloudsync_recovery_state(db.pool(), &generation, "user-a", "user-a", &key)
            .await
            .unwrap();
        sqlx::query("DELETE FROM app_settings WHERE id = ?")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();

        assert_eq!(
            stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
                .await
                .unwrap(),
            generation
        );
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(generation.clone())
        );
        assert!(matches!(
            stage_cloudsync_poison_recovery(db.pool(), "user-a", "workspace-b").await,
            Err(CloudsyncWorkspaceError::RecoveryConflict)
        ));
        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string("conflicting-generation").unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(matches!(
            stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a").await,
            Err(CloudsyncWorkspaceError::RecoveryConflict)
        ));
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some("conflicting-generation".to_string())
        );
    }

    #[tokio::test]
    async fn active_recovery_generation_survives_restart_during_logout_windows() {
        for phase in [
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedCleanReceive,
        ] {
            let db = test_db().await;
            claim_cloudsync_workspace(db.pool(), "user-a")
                .await
                .unwrap();
            let projection = projection(
                "user-a",
                vec![projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                )],
            );
            let generation = commit_cloudsync_workspace_projection(db.pool(), &projection, true)
                .await
                .unwrap()
                .unwrap();
            let key = recovery_key("user-a");
            ensure_cloudsync_recovery_state(db.pool(), &generation, "user-a", "user-a", &key)
                .await
                .unwrap();

            if phase == CloudsyncRecoveryPhase::NeedCleanReceive {
                for (expected, next) in [
                    (
                        CloudsyncRecoveryPhase::NeedFirstLogout,
                        CloudsyncRecoveryPhase::NeedBarrierInsert,
                    ),
                    (
                        CloudsyncRecoveryPhase::NeedBarrierInsert,
                        CloudsyncRecoveryPhase::NeedBarrierConfirm,
                    ),
                    (
                        CloudsyncRecoveryPhase::NeedBarrierConfirm,
                        CloudsyncRecoveryPhase::NeedCleanReceive,
                    ),
                ] {
                    assert!(
                        advance_cloudsync_recovery_phase(db.pool(), &generation, expected, next,)
                            .await
                            .unwrap()
                    );
                }
            }

            sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
                .bind(serde_json::to_string("superseding-generation").unwrap())
                .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
                .execute(db.pool())
                .await
                .unwrap();

            let resumed = commit_cloudsync_workspace_projection(db.pool(), &projection, true)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(resumed, generation);
            assert_eq!(
                cloudsync_full_resync_generation(db.pool()).await.unwrap(),
                Some(generation.clone())
            );
            let state = cloudsync_recovery_state(db.pool()).await.unwrap().unwrap();
            assert_eq!(state.generation, generation);
            assert_eq!(state.phase, phase);
        }
    }

    #[tokio::test]
    async fn superseded_recovery_cannot_advance_or_clear_the_new_generation() {
        let db = test_db().await;
        let generation = "generation-1";
        let replacement = "generation-2";
        let key = recovery_key("user-a");
        mark_full_resync_pending(db.pool(), generation).await;
        ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
            .await
            .unwrap();

        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string(replacement).unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            !advance_cloudsync_recovery_phase(
                db.pool(),
                generation,
                CloudsyncRecoveryPhase::NeedFirstLogout,
                CloudsyncRecoveryPhase::NeedBarrierInsert,
            )
            .await
            .unwrap()
        );

        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string(generation).unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();
        for (expected, next) in [
            (
                CloudsyncRecoveryPhase::NeedFirstLogout,
                CloudsyncRecoveryPhase::NeedBarrierInsert,
            ),
            (
                CloudsyncRecoveryPhase::NeedBarrierInsert,
                CloudsyncRecoveryPhase::NeedBarrierConfirm,
            ),
            (
                CloudsyncRecoveryPhase::NeedBarrierConfirm,
                CloudsyncRecoveryPhase::NeedCleanReceive,
            ),
            (
                CloudsyncRecoveryPhase::NeedCleanReceive,
                CloudsyncRecoveryPhase::NeedWitnessRepair,
            ),
            (
                CloudsyncRecoveryPhase::NeedWitnessRepair,
                CloudsyncRecoveryPhase::NeedBarrierCleanup,
            ),
            (
                CloudsyncRecoveryPhase::NeedBarrierCleanup,
                CloudsyncRecoveryPhase::NeedTransportResume,
            ),
        ] {
            assert!(
                advance_cloudsync_recovery_phase(db.pool(), generation, expected, next)
                    .await
                    .unwrap()
            );
        }
        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string(replacement).unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();

        assert!(
            !complete_cloudsync_recovery(db.pool(), generation)
                .await
                .unwrap()
        );
        assert_eq!(
            cloudsync_full_resync_generation(db.pool())
                .await
                .unwrap()
                .as_deref(),
            Some(replacement)
        );
        assert!(cloudsync_recovery_state(db.pool()).await.unwrap().is_some());

        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string(generation).unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            complete_cloudsync_recovery(db.pool(), generation)
                .await
                .unwrap()
        );
        assert!(
            cloudsync_full_resync_generation(db.pool())
                .await
                .unwrap()
                .is_none()
        );
        assert!(cloudsync_recovery_state(db.pool()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn recovery_barrier_requires_exact_authenticated_content() {
        let db = test_db().await;
        let generation = "generation-1";
        let key = recovery_key("user-a");
        mark_full_resync_pending(db.pool(), generation).await;
        let state =
            ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
                .await
                .unwrap();

        assert!(
            insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
                .await
                .unwrap()
        );
        assert!(
            cloudsync_recovery_barrier_is_exact(db.pool(), &state, &key)
                .await
                .unwrap()
        );
        assert!(
            !insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
                .await
                .unwrap()
        );

        sqlx::query("UPDATE e2ee_records SET payload = 'wrong' WHERE id = ?")
            .bind(&state.barrier_id)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            !cloudsync_recovery_barrier_is_exact(db.pool(), &state, &key)
                .await
                .unwrap()
        );
        assert!(matches!(
            insert_cloudsync_recovery_barrier(db.pool(), &state, &key).await,
            Err(CloudsyncWorkspaceError::RecoveryConflict)
        ));
        assert!(matches!(
            delete_cloudsync_recovery_barrier(db.pool(), &state, &key).await,
            Err(CloudsyncWorkspaceError::RecoveryConflict)
        ));

        sqlx::query("UPDATE e2ee_records SET workspace_id = ?, payload = ? WHERE id = ?")
            .bind(&state.workspace_id)
            .bind(&state.barrier_payload)
            .bind(&state.barrier_id)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            delete_cloudsync_recovery_barrier(db.pool(), &state, &key)
                .await
                .unwrap()
        );
        assert!(
            !delete_cloudsync_recovery_barrier(db.pool(), &state, &key)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn workspace_projection_replaces_stale_server_rows() {
        let db = test_db().await;
        replace_cloudsync_workspace_projection(
            db.pool(),
            &projection(
                "user-a",
                vec![
                    projected_workspace(
                        "user-a",
                        "user-a",
                        "personal",
                        "membership-personal",
                        "owner",
                        "Personal",
                    ),
                    projected_workspace(
                        "workspace-shared",
                        "user-b",
                        "shared",
                        "membership-shared",
                        "member",
                        "Shared",
                    ),
                ],
            ),
        )
        .await
        .unwrap();

        replace_cloudsync_workspace_projection(
            db.pool(),
            &projection(
                "user-a",
                vec![projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "My notes",
                )],
            ),
        )
        .await
        .unwrap();

        let workspaces: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let memberships: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, workspace_id, user_id, role, created_at, updated_at
                 FROM workspace_memberships ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(
            workspaces,
            vec![("user-a".to_string(), "My notes".to_string())]
        );
        assert_eq!(
            memberships,
            vec![(
                "membership-personal".to_string(),
                "user-a".to_string(),
                "user-a".to_string(),
                "owner".to_string(),
                "2026-07-16T00:01:00Z".to_string(),
                "2026-07-16T00:02:00Z".to_string(),
            )]
        );
    }

    #[tokio::test]
    async fn workspace_reconciliation_stages_revoked_sessions_before_projection_commit() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![
                projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                ),
                projected_workspace(
                    "workspace-shared",
                    "user-b",
                    "shared",
                    "membership-shared",
                    "member",
                    "Shared",
                ),
            ],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-personal', 'user-a', 'user-a', 'Personal'),
                    ('session-shared', 'workspace-shared', 'user-b', 'Shared')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let personal_only = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        let plan = stage_cloudsync_workspace_reconciliation(db.pool(), &personal_only)
            .await
            .unwrap();

        assert_eq!(
            plan,
            CloudsyncWorkspaceReconciliationPlan {
                granted_workspace_ids: vec![],
                revoked_workspace_ids: vec!["workspace-shared".to_string()],
            }
        );
        assert!(plan.requires_replica_reset());
        assert!(plan.requires_full_resync());
        let memberships_before_commit: Vec<String> = sqlx::query_scalar(
            "SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let queued: Vec<(String, String)> = sqlx::query_as(
            "SELECT session_id, workspace_id
             FROM cloudsync_session_evictions ORDER BY session_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            memberships_before_commit,
            vec!["user-a".to_string(), "workspace-shared".to_string()]
        );
        assert_eq!(
            queued,
            vec![("session-shared".to_string(), "workspace-shared".to_string(),)]
        );

        let generation = commit_cloudsync_workspace_projection(
            db.pool(),
            &personal_only,
            plan.requires_full_resync(),
        )
        .await
        .unwrap()
        .unwrap();

        let memberships_after_commit: Vec<String> = sqlx::query_scalar(
            "SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(memberships_after_commit, vec!["user-a".to_string()]);
        assert_eq!(session_count, 2);
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(generation.clone())
        );
        assert!(
            cloudsync_full_resync_requires_reset(db.pool(), &generation)
                .await
                .unwrap()
        );
        mark_cloudsync_full_resync_reset_applied(db.pool(), &generation)
            .await
            .unwrap();
        assert!(
            !cloudsync_full_resync_requires_reset(db.pool(), &generation)
                .await
                .unwrap()
        );
        let newer_generation =
            commit_cloudsync_workspace_projection(db.pool(), &personal_only, true)
                .await
                .unwrap()
                .unwrap();
        assert!(
            cloudsync_full_resync_requires_reset(db.pool(), &newer_generation)
                .await
                .unwrap()
        );
        clear_cloudsync_full_resync_pending(db.pool(), &generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(newer_generation.clone())
        );
        mark_cloudsync_full_resync_reset_applied(db.pool(), &newer_generation)
            .await
            .unwrap();
        clear_cloudsync_full_resync_pending(db.pool(), &newer_generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            None
        );
        assert!(
            cloudsync_full_resync_requires_reset(db.pool(), &newer_generation)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn cancelled_workspace_reconciliation_rolls_back_the_first_eviction_batch() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![
                projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                ),
                projected_workspace(
                    "workspace-shared",
                    "user-b",
                    "shared",
                    "membership-shared",
                    "member",
                    "Shared",
                ),
            ],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1 FROM sequence WHERE row_index < 255
             )
             INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             SELECT
               printf('shared-%03d', row_index),
               'workspace-shared',
               'user-b',
               'Shared'
             FROM sequence",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let personal_only = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stage_cloudsync_workspace_reconciliation_cancellable(db.pool(), &personal_only, || {
                cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 7
            }),
        )
        .await
        .expect("workspace reconciliation did not drain after cancellation")
        .unwrap_err();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ProjectionCancelled
        ));
        assert_eq!(
            cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
            7
        );

        let queued_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let memberships: Vec<String> = sqlx::query_scalar(
            "SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(queued_count, 0);
        assert_eq!(
            memberships,
            vec!["user-a".to_string(), "workspace-shared".to_string()]
        );

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-cancelled-stage', 'user-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled workspace reconciliation kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn cancelled_reconciliation_rolls_back_a_large_legacy_membership_scan() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let personal = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        replace_cloudsync_workspace_projection(db.pool(), &personal)
            .await
            .unwrap();
        seed_legacy_workspace_projection_rows(db.pool(), 511).await;

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stage_cloudsync_workspace_reconciliation_cancellable(db.pool(), &personal, || {
                cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 6
            }),
        )
        .await
        .expect("legacy membership reconciliation did not drain after cancellation")
        .unwrap_err();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ProjectionCancelled
        ));
        assert_eq!(
            cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
            6
        );

        let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let membership_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(workspace_count, 513);
        assert_eq!(membership_count, 513);

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-legacy-scan-cancel', 'user-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled legacy membership scan kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn cancelled_projection_commit_rolls_back_projection_and_eviction_cleanup() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![
                projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                ),
                projected_workspace(
                    "workspace-shared",
                    "user-b",
                    "shared",
                    "membership-shared",
                    "member",
                    "Shared",
                ),
            ],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1 FROM sequence WHERE row_index < 255
             )
             INSERT INTO cloudsync_session_evictions (session_id, workspace_id)
             SELECT printf('eviction-%03d', row_index), 'user-a'
             FROM sequence",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let personal_only = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Changed",
            )],
        );

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            commit_cloudsync_workspace_projection_cancellable(
                db.pool(),
                &personal_only,
                true,
                || cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 11,
            ),
        )
        .await
        .expect("workspace projection commit did not drain after cancellation")
        .unwrap_err();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ProjectionCancelled
        ));
        assert_eq!(
            cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
            11
        );

        let queued_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let workspaces: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let memberships: Vec<String> = sqlx::query_scalar(
            "SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(queued_count, 256);
        assert_eq!(
            workspaces,
            vec![
                ("user-a".to_string(), "Personal".to_string()),
                ("workspace-shared".to_string(), "Shared".to_string()),
            ]
        );
        assert_eq!(
            memberships,
            vec!["user-a".to_string(), "workspace-shared".to_string()]
        );
        assert!(
            cloudsync_full_resync_generation(db.pool())
                .await
                .unwrap()
                .is_none()
        );

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-cancelled-commit', 'user-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled workspace projection commit kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn cancelled_projection_commit_rolls_back_large_legacy_table_deletes() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        seed_legacy_workspace_projection_rows(db.pool(), 511).await;
        let changed = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Changed",
            )],
        );

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            commit_cloudsync_workspace_projection_cancellable(db.pool(), &changed, true, || {
                cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 7
            }),
        )
        .await
        .expect("legacy projection deletion did not drain after cancellation")
        .unwrap_err();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ProjectionCancelled
        ));
        assert_eq!(
            cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
            7
        );

        let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let membership_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let personal_name: String =
            sqlx::query_scalar("SELECT name FROM workspaces WHERE id = 'user-a'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(workspace_count, 513);
        assert_eq!(membership_count, 513);
        assert_eq!(personal_name, "Personal");
        assert!(
            cloudsync_full_resync_generation(db.pool())
                .await
                .unwrap()
                .is_none()
        );

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-legacy-delete-cancel', 'user-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled legacy projection deletion kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn late_projection_cancellation_rolls_back_100k_rows_within_two_seconds() {
        const LEGACY_ROW_COUNT: usize = 100_000;

        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        seed_legacy_workspace_projection_rows(db.pool(), (LEGACY_ROW_COUNT - 1) as i64).await;
        let changed = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Changed",
            )],
        );

        let rows_per_table = LEGACY_ROW_COUNT + 1;
        let batches_per_table =
            rows_per_table.div_ceil(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize);
        let cancellation_barrier = 3 + (batches_per_table * 4);
        let mut cancellation_checks = 0;
        let mut cancellation_started_at = None;
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            commit_cloudsync_workspace_projection_cancellable(db.pool(), &changed, true, || {
                cancellation_checks += 1;
                if cancellation_checks >= cancellation_barrier {
                    cancellation_started_at.get_or_insert_with(std::time::Instant::now);
                    true
                } else {
                    false
                }
            }),
        )
        .await
        .expect("100k-row projection did not reach late cancellation")
        .unwrap_err();
        let rollback_elapsed = cancellation_started_at
            .expect("projection cancellation timestamp was not captured")
            .elapsed();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ProjectionCancelled
        ));
        assert_eq!(cancellation_checks, cancellation_barrier);
        assert!(
            rollback_elapsed < std::time::Duration::from_secs(2),
            "100k-row projection rollback took {rollback_elapsed:?}"
        );

        let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let membership_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(workspace_count, rows_per_table as i64);
        assert_eq!(membership_count, rows_per_table as i64);

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-100k-projection-cancel', 'user-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("100k-row projection rollback kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn workspace_projection_batches_have_composite_keyset_indexes() {
        let db = test_db().await;
        let session_index_columns: Vec<String> = sqlx::query_scalar(
            "SELECT name
             FROM pragma_index_info('idx_sessions_workspace_id_id')
             ORDER BY seqno",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let eviction_index_columns: Vec<String> = sqlx::query_scalar(
            "SELECT name
             FROM pragma_index_info(
               'idx_cloudsync_session_evictions_workspace_id_session_id'
             )
             ORDER BY seqno",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let membership_index_columns: Vec<String> = sqlx::query_scalar(
            "SELECT name
             FROM pragma_index_info(
               'idx_workspace_memberships_user_deleted_id'
             )
             ORDER BY seqno",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(
            session_index_columns,
            vec!["workspace_id".to_string(), "id".to_string()]
        );
        assert_eq!(
            eviction_index_columns,
            vec!["workspace_id".to_string(), "session_id".to_string()]
        );
        assert_eq!(
            membership_index_columns,
            vec![
                "user_id".to_string(),
                "deleted_at".to_string(),
                "id".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn detects_whether_the_encrypted_replica_has_local_records() {
        let db = test_db().await;

        assert!(
            cloudsync_encrypted_replica_is_empty(db.pool())
                .await
                .unwrap()
        );

        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES ('record-a', 'workspace-a', 'payload-a')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert!(
            !cloudsync_encrypted_replica_is_empty(db.pool())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn full_resync_reset_markers_classify_legacy_poison_and_checked_resets() {
        let db = test_db().await;
        let generation = "generation-a";
        let value_json = serde_json::to_string(generation).unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .bind(&value_json)
            .execute(db.pool())
            .await
            .unwrap();

        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ResetRequired
        );
        assert!(
            !mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), "stale-generation")
                .await
                .unwrap()
        );

        mark_cloudsync_full_resync_reset_applied(db.pool(), generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::PoisonRecoveryRequired
        );

        assert!(
            mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
                .await
                .unwrap()
        );
        assert!(
            mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
                .await
                .unwrap()
        );
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
        );

        clear_cloudsync_full_resync_pending(db.pool(), "stale-generation")
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
        );

        clear_cloudsync_full_resync_pending(db.pool(), generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            None
        );
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ResetRequired
        );
        let remaining_markers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_settings
             WHERE id IN (?, ?, ?)",
        )
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
        .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(remaining_markers, 0);
    }

    #[tokio::test]
    async fn checked_reset_marker_is_atomic_idempotent_and_generation_scoped() {
        let db = test_db().await;
        let generation = "generation-a";
        let value_json = serde_json::to_string(generation).unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .bind(&value_json)
            .execute(db.pool())
            .await
            .unwrap();

        assert!(
            mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
                .await
                .unwrap()
        );
        assert!(
            mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
                .await
                .unwrap()
        );
        let applied_markers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_settings
             WHERE id IN (?, ?) AND value_json = ?",
        )
        .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
        .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
        .bind(&value_json)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(applied_markers, 2);

        let newer_generation = "generation-b";
        let newer_value_json = serde_json::to_string(newer_generation).unwrap();
        sqlx::query(
            "UPDATE app_settings
             SET value_json = ?
             WHERE id = ?",
        )
        .bind(&newer_value_json)
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            !mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
                .await
                .unwrap()
        );
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), newer_generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ResetRequired
        );

        assert!(
            mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), newer_generation)
                .await
                .unwrap()
        );
        clear_cloudsync_full_resync_pending(db.pool(), generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(newer_generation.to_string())
        );
        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), newer_generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
        );
    }

    #[tokio::test]
    async fn stale_legacy_only_reset_marker_requires_poison_recovery() {
        let db = test_db().await;
        let generation = "generation-a";
        let newer_generation = "generation-b";
        let newer_value_json = serde_json::to_string(newer_generation).unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .bind(newer_value_json)
            .execute(db.pool())
            .await
            .unwrap();
        mark_cloudsync_full_resync_reset_applied(db.pool(), generation)
            .await
            .unwrap();

        assert_eq!(
            cloudsync_full_resync_reset_state(db.pool(), newer_generation)
                .await
                .unwrap(),
            CloudsyncFullResyncResetState::PoisonRecoveryRequired
        );
    }

    #[tokio::test]
    async fn reauthorized_workspace_cancels_staged_session_evictions() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let current = projection(
            "user-a",
            vec![
                projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                ),
                projected_workspace(
                    "workspace-shared",
                    "user-b",
                    "shared",
                    "membership-shared",
                    "member",
                    "Shared",
                ),
            ],
        );
        replace_cloudsync_workspace_projection(db.pool(), &current)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, title)
             VALUES ('session-shared', 'workspace-shared', 'Shared')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let personal_only = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        stage_cloudsync_workspace_reconciliation(db.pool(), &personal_only)
            .await
            .unwrap();

        commit_cloudsync_workspace_projection(db.pool(), &current, false)
            .await
            .unwrap();

        let queued_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(queued_count, 0);
    }

    #[tokio::test]
    async fn workspace_reconciliation_requires_the_claimed_account() {
        let db = test_db().await;
        let error = stage_cloudsync_workspace_reconciliation(
            db.pool(),
            &projection(
                "user-a",
                vec![projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                )],
            ),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
        let queue_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(queue_count, 0);
    }

    #[tokio::test]
    async fn cloudsync_write_filter_scope_is_local_and_versioned() {
        let db = test_db().await;
        assert!(
            !cloudsync_write_filter_installed(db.pool(), "user-a")
                .await
                .unwrap()
        );

        set_cloudsync_personal_write_scope(db.pool(), "user-a")
            .await
            .unwrap();
        mark_cloudsync_write_filter_installed(db.pool())
            .await
            .unwrap();

        let writable_workspace_ids: Vec<String> = sqlx::query_scalar(
            "SELECT allowed_workspace_id
                 FROM cloudsync_writable_workspaces
                 ORDER BY allowed_workspace_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(writable_workspace_ids, vec!["user-a".to_string()]);
        assert!(
            cloudsync_write_filter_installed(db.pool(), "user-a")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn invalid_workspace_projections_preserve_existing_rows() {
        let db = test_db().await;
        let valid = projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        );
        replace_cloudsync_workspace_projection(db.pool(), &valid)
            .await
            .unwrap();

        let mut missing_personal = valid.clone();
        missing_personal.personal_workspace_id = "workspace-missing".to_string();
        let mut invalid_role = valid.clone();
        invalid_role.workspaces[0].role = "viewer".to_string();
        let mut invalid_membership_timestamp = valid.clone();
        invalid_membership_timestamp.workspaces[0].membership_created_at = String::new();
        let mut invalid_kind = valid.clone();
        invalid_kind.workspaces.push(projected_workspace(
            "workspace-shared",
            "user-b",
            "team",
            "membership-shared",
            "member",
            "Shared",
        ));
        let mut duplicate_personal = valid.clone();
        duplicate_personal.workspaces.push(projected_workspace(
            "workspace-personal-2",
            "user-b",
            "personal",
            "membership-personal-2",
            "owner",
            "Other personal",
        ));
        let mut duplicate_workspace = valid.clone();
        duplicate_workspace.workspaces.push(projected_workspace(
            "user-a",
            "user-b",
            "shared",
            "membership-shared",
            "member",
            "Shared",
        ));
        let mut duplicate_membership = valid.clone();
        duplicate_membership.workspaces.push(projected_workspace(
            "workspace-shared",
            "user-b",
            "shared",
            "membership-personal",
            "member",
            "Shared",
        ));

        for invalid in [
            missing_personal,
            invalid_role,
            invalid_membership_timestamp,
            invalid_kind,
            duplicate_personal,
            duplicate_workspace,
            duplicate_membership,
        ] {
            let error = replace_cloudsync_workspace_projection(db.pool(), &invalid)
                .await
                .unwrap_err();
            assert!(matches!(
                error,
                CloudsyncWorkspaceError::InvalidWorkspaceProjection
            ));
        }

        let workspaces: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let memberships: Vec<(String, String)> =
            sqlx::query_as("SELECT id, workspace_id FROM workspace_memberships ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(
            workspaces,
            vec![("user-a".to_string(), "Personal".to_string())]
        );
        assert_eq!(
            memberships,
            vec![("membership-personal".to_string(), "user-a".to_string(),)]
        );
    }

    #[tokio::test]
    async fn local_workspace_binding_is_stable() {
        let db = test_db().await;

        let first = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
        let second = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

        assert_eq!(first, second);
        assert!(!first.is_empty());
    }

    #[tokio::test]
    async fn account_binding_is_durable_without_rekeying_rows() {
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

        sqlx::query(
            "INSERT INTO humans (id, workspace_id, owner_user_id, name)
             VALUES (?, ?, ?, 'Local user')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', ?, ?, 'Session')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();

        bind_cloudsync_account(db.pool(), "user-a").await.unwrap();

        let binding: (String, String) = sqlx::query_as(
            "SELECT json_extract(value_json, '$.workspace_id'),
                    json_extract(value_json, '$.account_user_id')
             FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let human: (String, String, String) = sqlx::query_as(
            "SELECT id, workspace_id, owner_user_id FROM humans WHERE name = 'Local user'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let session: (String, String) =
            sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
                .fetch_one(db.pool())
                .await
                .unwrap();

        assert_eq!(binding, (local_workspace.clone(), "user-a".to_string()));
        assert_eq!(
            human,
            (
                local_workspace.clone(),
                local_workspace.clone(),
                local_workspace.clone(),
            )
        );
        assert_eq!(session, (local_workspace.clone(), local_workspace));
        assert!(
            !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn account_binding_rejects_switching_before_and_after_claim() {
        let db = test_db().await;

        bind_cloudsync_account(db.pool(), "user-a").await.unwrap();
        let error = bind_cloudsync_account(db.pool(), "user-b")
            .await
            .unwrap_err();
        assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        bind_cloudsync_account(db.pool(), "user-a").await.unwrap();
        let error = bind_cloudsync_account(db.pool(), "user-b")
            .await
            .unwrap_err();
        assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
    }

    #[tokio::test]
    async fn claim_rekeys_every_synced_table_and_is_idempotent() {
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
        let statements = [
            (
                "INSERT INTO organizations (id, workspace_id) VALUES ('org', ?)",
                local_workspace.as_str(),
            ),
            (
                "INSERT INTO humans (id, workspace_id) VALUES ('human', ?)",
                "",
            ),
            (
                "INSERT INTO sessions (id, workspace_id) VALUES ('session', ?)",
                local_workspace.as_str(),
            ),
            (
                "INSERT INTO session_documents (id, session_id, workspace_id) VALUES ('document', 'session', ?)",
                "",
            ),
            (
                "INSERT INTO transcripts (id, session_id, workspace_id) VALUES ('transcript', 'session', ?)",
                "",
            ),
            (
                "INSERT INTO session_participants (id, session_id, workspace_id) VALUES ('participant', 'session', ?)",
                "",
            ),
            (
                "INSERT INTO action_items (id, session_id, workspace_id) VALUES ('action', 'session', ?)",
                "",
            ),
            (
                "INSERT INTO session_attachments (id, session_id, workspace_id) VALUES ('attachment', 'session', ?)",
                "",
            ),
        ];
        for (statement, workspace_id) in statements {
            sqlx::query(statement)
                .bind(workspace_id)
                .execute(db.pool())
                .await
                .unwrap();
        }

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let changes_before_repeat: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(db.pool())
            .await
            .unwrap();
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let changes_after_repeat: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(changes_after_repeat, changes_before_repeat);

        for table_name in crate::E2EE_DOMAIN_TABLES {
            let sql = format!(
                "SELECT COUNT(*) FROM {} WHERE workspace_id <> 'user-a'",
                table_name
            );
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .fetch_one(db.pool())
                .await
                .unwrap();
            assert_eq!(count, 0, "{table_name} was not claimed");
        }
    }

    #[tokio::test]
    async fn cancelled_claim_rolls_back_and_releases_local_writes() {
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO humans (id, workspace_id, owner_user_id, name)
             VALUES (?, ?, ?, 'Local user')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1 FROM sequence WHERE row_index < 255
             )
             INSERT INTO action_items (
               id, workspace_id, created_by, updated_by, text
             )
             SELECT
               printf('action-%03d', row_index),
               ?,
               ?,
               ?,
               'Legacy action'
             FROM sequence",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();
        let dirty_rows_before_claim: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
                .fetch_one(db.pool())
                .await
                .unwrap();

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let scan_checks = (crate::E2EE_DOMAIN_TABLES.len() * 2) + 4;
        let cancellation_barrier = 3 + scan_checks + 4;
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            claim_cloudsync_workspace_cancellable(db.pool(), "user-a", || {
                cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
                    >= cancellation_barrier
            }),
        )
        .await
        .expect("workspace claim did not drain after cancellation")
        .unwrap_err();
        assert!(matches!(error, CloudsyncWorkspaceError::ClaimCancelled));
        assert_eq!(
            cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
            cancellation_barrier
        );

        let changed_action_items: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM action_items
             WHERE workspace_id <> ? OR created_by <> ? OR updated_by <> ?",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let account_human_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE id = 'user-a'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let dirty_rows_after_cancellation: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(changed_action_items, 0);
        assert_eq!(account_human_count, 0);
        assert_eq!(dirty_rows_after_cancellation, dirty_rows_before_claim);
        assert!(
            !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
                .await
                .unwrap()
        );

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-cancel', ?, ?, 'Local write')",
            )
            .bind(&local_workspace)
            .bind(&local_workspace)
            .execute(db.pool()),
        )
        .await
        .expect("cancelled workspace claim kept the database busy")
        .unwrap();

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let stale_action_items: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM action_items
             WHERE workspace_id <> 'user-a'
                OR created_by <> 'user-a'
                OR updated_by <> 'user-a'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(stale_action_items, 0);
    }

    #[tokio::test]
    async fn late_claim_cancellation_rolls_back_100k_rows_within_two_seconds() {
        const LEGACY_ROW_COUNT: usize = 100_000;

        assert_eq!(crate::E2EE_DOMAIN_TABLES.first(), Some(&"action_items"));
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
        sqlx::query(
            "WITH RECURSIVE sequence(row_index) AS (
               VALUES (0)
               UNION ALL
               SELECT row_index + 1
               FROM sequence
               WHERE row_index < 99999
             )
             INSERT INTO action_items (
               id, workspace_id, created_by, updated_by, text
             )
             SELECT
               printf('claim-stress-%06d', row_index),
               ?,
               ?,
               ?,
               'Legacy action'
             FROM sequence",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();

        let action_item_batches = LEGACY_ROW_COUNT.div_ceil(CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE);
        let empty_domain_tables = crate::E2EE_DOMAIN_TABLES.len() - 1;
        let cancellation_barrier =
            3 + (action_item_batches * 2) + (empty_domain_tables * 2) + (action_item_batches * 4);
        let mut cancellation_checks = 0;
        let mut cancellation_started_at = None;
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            claim_cloudsync_workspace_cancellable(db.pool(), "user-a", || {
                cancellation_checks += 1;
                if cancellation_checks >= cancellation_barrier {
                    cancellation_started_at.get_or_insert_with(std::time::Instant::now);
                    true
                } else {
                    false
                }
            }),
        )
        .await
        .expect("100k-row claim did not reach late cancellation")
        .unwrap_err();
        let rollback_elapsed = cancellation_started_at
            .expect("claim cancellation timestamp was not captured")
            .elapsed();
        assert!(matches!(error, CloudsyncWorkspaceError::ClaimCancelled));
        assert_eq!(cancellation_checks, cancellation_barrier);
        assert!(
            rollback_elapsed < std::time::Duration::from_secs(2),
            "100k-row claim rollback took {rollback_elapsed:?}"
        );

        let changed_action_items: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM action_items
             WHERE workspace_id <> ? OR created_by <> ? OR updated_by <> ?",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(changed_action_items, 0);
        assert!(
            !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
                .await
                .unwrap()
        );

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-100k-claim-cancel', ?, ?, 'Local write')",
            )
            .bind(&local_workspace)
            .bind(&local_workspace)
            .execute(db.pool()),
        )
        .await
        .expect("100k-row claim rollback kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn detects_an_existing_account_claim() {
        let db = test_db().await;

        assert!(
            !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
                .await
                .unwrap()
        );
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();

        assert!(
            cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
                .await
                .unwrap()
        );
        assert!(
            !cloudsync_workspace_is_claimed_by(db.pool(), "user-b")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn claim_rekeys_local_user_identities_and_references() {
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

        sqlx::query(
            "INSERT INTO humans (id, workspace_id, owner_user_id, name, email)
             VALUES (?, ?, ?, '', 'local@example.com'),
                    (?, ?, ?, 'Local user', '')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organizations (id, workspace_id, owner_user_id)
             VALUES ('org', ?, ?)",
        )
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session', ?, ?)",
        )
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents
               (id, workspace_id, session_id, created_by, updated_by)
             VALUES ('document', ?, 'session', ?, ?)",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, session_id, owner_user_id)
             VALUES ('transcript', ?, 'session', ?)",
        )
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_participants
               (id, workspace_id, session_id, owner_user_id, human_id)
             VALUES ('participant', ?, 'session', ?, ?)",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO action_items
               (id, workspace_id, session_id, assignee_human_id, created_by, updated_by)
             VALUES ('action', ?, 'session', ?, ?, ?)",
        )
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .bind(&local_workspace)
        .bind(LEGACY_DEFAULT_USER_ID)
        .execute(db.pool())
        .await
        .unwrap();

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();

        let humans: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, name, email FROM humans ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(
            humans,
            vec![(
                "user-a".to_string(),
                "Local user".to_string(),
                "local@example.com".to_string(),
            )]
        );

        for (table, column) in USER_ID_REFERENCES {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE {column} IN (?, ?)");
            let stale_count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(&local_workspace)
                .bind(LEGACY_DEFAULT_USER_ID)
                .fetch_one(db.pool())
                .await
                .unwrap();
            assert_eq!(stale_count, 0, "stale identity in {table}.{column}");
        }

        let participant: (String, String) = sqlx::query_as(
            "SELECT owner_user_id, human_id FROM session_participants WHERE id = 'participant'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(participant, ("user-a".to_string(), "user-a".to_string()));
        let action: (String, String, String) = sqlx::query_as(
            "SELECT assignee_human_id, created_by, updated_by FROM action_items WHERE id = 'action'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            action,
            (
                "user-a".to_string(),
                "user-a".to_string(),
                "user-a".to_string(),
            )
        );
    }

    #[tokio::test]
    async fn claim_tombstones_duplicate_self_participants_before_rekeying() {
        let db = test_db().await;
        let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

        sqlx::query("INSERT INTO humans (id, workspace_id) VALUES (?, ?), ('user-a', ?)")
            .bind(&local_workspace)
            .bind(&local_workspace)
            .bind(&local_workspace)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', ?)")
            .bind(&local_workspace)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_participants (id, workspace_id, session_id, human_id)
             VALUES ('legacy-self', ?, 'session', ?),
                    ('account-self', ?, 'session', 'user-a')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();

        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM session_participants
             WHERE session_id = 'session' AND human_id = 'user-a' AND deleted_at IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let legacy_deleted_at: Option<String> = sqlx::query_scalar(
            "SELECT deleted_at FROM session_participants WHERE id = 'legacy-self'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(active_count, 1);
        assert!(legacy_deleted_at.is_some());
    }

    #[tokio::test]
    async fn claim_rejects_account_switching() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();

        let error = claim_cloudsync_workspace(db.pool(), "user-b")
            .await
            .unwrap_err();

        assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
    }

    #[tokio::test]
    async fn claim_rejects_foreign_workspace_rows() {
        let db = test_db().await;
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', 'other-user')")
            .execute(db.pool())
            .await
            .unwrap();

        let error = claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            CloudsyncWorkspaceError::ForeignWorkspace { table } if table == "sessions"
        ));
    }

    #[tokio::test]
    async fn repeated_claim_allows_shared_workspace_rows() {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('shared-session', 'workspace-b', 'user-b')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();

        let workspace_id: String =
            sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'shared-session'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(workspace_id, "workspace-b");
    }
}
