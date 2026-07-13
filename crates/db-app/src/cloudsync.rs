use std::sync::LazyLock;

use hypr_db_core::CloudsyncTableSpec;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

pub const CLOUDSYNC_WORKSPACE_BINDING_ID: &str = "cloudsync_workspace_binding";
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
    AccountMismatch,
    ForeignWorkspace { table: String },
}

impl std::fmt::Display for CloudsyncWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::InvalidWorkspaceId => write!(f, "workspace ID is invalid"),
            Self::InvalidBinding => write!(f, "local CloudSync workspace binding is invalid"),
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

pub async fn claim_cloudsync_workspace(
    pool: &SqlitePool,
    account_user_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let account_user_id = account_user_id.trim();
    if account_user_id.is_empty() || account_user_id == LEGACY_DEFAULT_USER_ID {
        return Err(CloudsyncWorkspaceError::InvalidWorkspaceId);
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let binding = load_or_create_binding(&mut transaction).await?;
    if binding
        .account_user_id
        .as_deref()
        .is_some_and(|id| id != account_user_id)
    {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
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
        let binding: CloudsyncWorkspaceBinding = serde_json::from_str(&value_json)
            .map_err(|_| CloudsyncWorkspaceError::InvalidBinding)?;
        if binding.workspace_id.trim().is_empty() {
            return Err(CloudsyncWorkspaceError::InvalidBinding);
        }
        return Ok(binding);
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

    #[tokio::test]
    async fn local_workspace_binding_is_stable() {
        let db = test_db().await;

        let first = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
        let second = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

        assert_eq!(first, second);
        assert!(!first.is_empty());
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
}
