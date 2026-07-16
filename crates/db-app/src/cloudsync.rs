use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub const CLOUDSYNC_WORKSPACE_BINDING_ID: &str = "cloudsync_workspace_binding";
const CLOUDSYNC_FULL_RESYNC_PENDING_ID: &str = "cloudsync_full_resync_pending";
const CLOUDSYNC_WRITE_FILTER_VERSION_ID: &str = "cloudsync_write_filter_version";
const CLOUDSYNC_WRITE_FILTER_VERSION: &str = "writable-workspaces-v1";
const LEGACY_DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

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
    InvalidWorkspaceId,
    InvalidBinding,
    InvalidWorkspaceProjection,
    AccountMismatch,
    ForeignWorkspace { table: String },
}

impl std::fmt::Display for CloudsyncWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::InvalidWorkspaceId => write!(f, "workspace ID is invalid"),
            Self::InvalidBinding => write!(f, "local CloudSync workspace binding is invalid"),
            Self::InvalidWorkspaceProjection => {
                write!(f, "CloudSync workspace projection is invalid")
            }
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
        ("action_items", true),
        ("calendars", false),
        ("chat_groups", false),
        ("chat_messages", false),
        ("daily_notes", false),
        ("entity_mentions", false),
        ("events", false),
        ("humans", true),
        ("organizations", true),
        ("session_attachments", true),
        ("session_documents", true),
        ("session_participants", true),
        ("session_tags", false),
        ("sessions", true),
        ("tags", false),
        ("templates", false),
        ("transcripts", true),
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
    cloudsync_table_registry()
        .iter()
        .any(|table| table.enabled && table.table_name == table_name)
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
    if binding.workspace_id == account_user_id
        && binding.account_user_id.as_deref() == Some(account_user_id)
    {
        transaction.commit().await?;
        return Ok(());
    }

    for table in cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
    {
        let foreign_sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE workspace_id <> '' AND workspace_id <> ? AND workspace_id <> ?)",
            table.table_name
        );
        let has_foreign_workspace: bool =
            sqlx::query_scalar(sqlx::AssertSqlSafe(foreign_sql.as_str()))
                .bind(&binding.workspace_id)
                .bind(account_user_id)
                .fetch_one(&mut *transaction)
                .await?;
        if has_foreign_workspace {
            return Err(CloudsyncWorkspaceError::ForeignWorkspace {
                table: table.table_name.clone(),
            });
        }
    }

    for table in cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
    {
        let update_sql = if binding.workspace_id == account_user_id {
            format!(
                "UPDATE {} SET workspace_id = ? WHERE workspace_id = ''",
                table.table_name
            )
        } else {
            format!(
                "UPDATE {} SET workspace_id = ? WHERE workspace_id = '' OR workspace_id = ?",
                table.table_name
            )
        };
        let mut query = sqlx::query(sqlx::AssertSqlSafe(update_sql.as_str())).bind(account_user_id);
        if binding.workspace_id != account_user_id {
            query = query.bind(&binding.workspace_id);
        }
        query.execute(&mut *transaction).await?;
    }

    rekey_local_user_identity(&mut transaction, &binding.workspace_id, account_user_id).await?;
    if binding.workspace_id != LEGACY_DEFAULT_USER_ID {
        rekey_local_user_identity(&mut transaction, LEGACY_DEFAULT_USER_ID, account_user_id)
            .await?;
    }

    if binding.workspace_id != account_user_id
        || binding.account_user_id.as_deref() != Some(account_user_id)
    {
        save_binding(
            &mut transaction,
            &CloudsyncWorkspaceBinding {
                workspace_id: account_user_id.to_string(),
                account_user_id: Some(account_user_id.to_string()),
            },
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(())
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
    validate_cloudsync_workspace_projection(projection)?;

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    require_claimed_binding(&mut transaction, &projection.account_user_id).await?;

    let existing_workspace_ids: Vec<String> = sqlx::query_scalar(
        "SELECT workspace_id
         FROM workspace_memberships
         WHERE user_id = ? AND deleted_at IS NULL",
    )
    .bind(&projection.account_user_id)
    .fetch_all(&mut *transaction)
    .await?;
    let existing_workspace_ids = existing_workspace_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
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
        sqlx::query(
            "INSERT INTO cloudsync_session_evictions (session_id, workspace_id)
             SELECT id, workspace_id
             FROM sessions
             WHERE workspace_id = ?
             ON CONFLICT(session_id) DO UPDATE SET
               workspace_id = excluded.workspace_id,
               queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               attempt_count = 0,
               last_attempt_at = NULL,
               last_error = ''",
        )
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(CloudsyncWorkspaceReconciliationPlan {
        granted_workspace_ids,
        revoked_workspace_ids,
    })
}

pub async fn commit_cloudsync_workspace_projection(
    pool: &SqlitePool,
    projection: &CloudsyncWorkspaceProjection,
    require_full_resync: bool,
) -> Result<Option<String>, CloudsyncWorkspaceError> {
    write_cloudsync_workspace_projection(pool, projection, require_full_resync, true).await
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

pub async fn clear_cloudsync_full_resync_pending(
    pool: &SqlitePool,
    generation: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let value_json = serde_json::to_string(generation)
        .map_err(|_| CloudsyncWorkspaceError::InvalidWorkspaceProjection)?;
    sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(value_json)
        .execute(pool)
        .await?;
    Ok(())
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
    validate_cloudsync_workspace_projection(projection)?;

    let full_resync_generation = require_full_resync.then(|| uuid::Uuid::new_v4().to_string());
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if require_claimed_account {
        require_claimed_binding(&mut transaction, &projection.account_user_id).await?;
    }
    sqlx::query("DELETE FROM workspace_memberships")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM workspaces")
        .execute(&mut *transaction)
        .await?;

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
        .execute(&mut *transaction)
        .await?;
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
        .execute(&mut *transaction)
        .await?;

        sqlx::query("DELETE FROM cloudsync_session_evictions WHERE workspace_id = ?")
            .bind(&workspace.id)
            .execute(&mut *transaction)
            .await?;
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
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(full_resync_generation)
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

async fn rekey_local_user_identity(
    transaction: &mut Transaction<'_, Sqlite>,
    source_user_id: &str,
    account_user_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
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

    sqlx::query(
        "UPDATE session_participants AS duplicate
         SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE duplicate.human_id = ?
           AND duplicate.deleted_at IS NULL
           AND EXISTS (
             SELECT 1
             FROM session_participants AS keeper
             WHERE keeper.session_id = duplicate.session_id
               AND keeper.deleted_at IS NULL
               AND keeper.id <> duplicate.id
               AND (
                 keeper.human_id = ?
                 OR (keeper.human_id = ? AND keeper.id < duplicate.id)
               )
           )",
    )
    .bind(source_user_id)
    .bind(account_user_id)
    .bind(source_user_id)
    .execute(&mut **transaction)
    .await?;

    for (table, column) in USER_ID_REFERENCES {
        let update_sql = format!("UPDATE {table} SET {column} = ? WHERE {column} = ?");
        sqlx::query(sqlx::AssertSqlSafe(update_sql.as_str()))
            .bind(account_user_id)
            .bind(source_user_id)
            .execute(&mut **transaction)
            .await?;
    }

    sqlx::query("DELETE FROM humans WHERE id = ?")
        .bind(source_user_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
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

    async fn test_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
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
        let newer_generation =
            commit_cloudsync_workspace_projection(db.pool(), &personal_only, true)
                .await
                .unwrap()
                .unwrap();
        clear_cloudsync_full_resync_pending(db.pool(), &generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(newer_generation.clone())
        );
        clear_cloudsync_full_resync_pending(db.pool(), &newer_generation)
            .await
            .unwrap();
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            None
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

        for table in cloudsync_table_registry()
            .iter()
            .filter(|table| table.enabled)
        {
            let sql = format!(
                "SELECT COUNT(*) FROM {} WHERE workspace_id <> 'user-a'",
                table.table_name
            );
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .fetch_one(db.pool())
                .await
                .unwrap();
            assert_eq!(count, 0, "{} was not claimed", table.table_name);
        }
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
