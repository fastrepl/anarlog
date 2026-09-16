use sqlx::{Sqlite, SqlitePool, Transaction};

use super::CloudsyncWorkspaceError;
use super::binding::{load_or_create_binding, validated_account_user_id};

pub(super) async fn has_connection(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
    library_workspace_id: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM local_library_connections
         WHERE account_user_id = ? AND library_workspace_id = ?)",
    )
    .bind(account_user_id)
    .bind(library_workspace_id)
    .fetch_one(&mut **transaction)
    .await?)
}

pub(super) async fn has_library_connections(
    transaction: &mut Transaction<'_, Sqlite>,
    library_workspace_id: &str,
) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM local_library_connections WHERE library_workspace_id = ?)",
    )
    .bind(library_workspace_id)
    .fetch_one(&mut **transaction)
    .await
}

pub(super) async fn activate_connection(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
    library_workspace_id: &str,
) -> Result<bool, CloudsyncWorkspaceError> {
    let connected = has_connection(transaction, account_user_id, library_workspace_id).await?;
    if connected {
        let already_active: bool = sqlx::query_scalar(
            "SELECT active FROM local_library_connections WHERE account_user_id = ?",
        )
        .bind(account_user_id)
        .fetch_one(&mut **transaction)
        .await?;
        if already_active {
            return Ok(true);
        }
        // Legacy recovery uses singleton settings. Preserve them with their account
        // before making a different connection visible to the sync runtime.
        let previous: Option<String> = sqlx::query_scalar(
            "SELECT account_user_id FROM local_library_connections
             WHERE active = 1 OR account_user_id = ? ORDER BY active DESC LIMIT 1",
        )
        .bind(library_workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;
        const RECOVERY_IDS: [&str; 4] = [
            super::CLOUDSYNC_FULL_RESYNC_PENDING_ID,
            super::CLOUDSYNC_FULL_RESYNC_RECOVERY_ID,
            super::CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
            super::CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
        ];
        let mut recovery = serde_json::Map::new();
        for id in RECOVERY_IDS {
            let value: Option<String> =
                sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&mut **transaction)
                    .await?;
            if let Some(value) = value {
                recovery.insert(id.to_string(), value.into());
            }
            sqlx::query("DELETE FROM app_settings WHERE id = ?")
                .bind(id)
                .execute(&mut **transaction)
                .await?;
        }
        if let Some(previous) = previous {
            sqlx::query("UPDATE local_library_connections SET recovery_settings = ? WHERE account_user_id = ?")
                .bind(serde_json::Value::Object(recovery).to_string()).bind(previous).execute(&mut **transaction).await?;
        }
        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             SELECT key, value FROM local_library_connections, json_each(recovery_settings)
             WHERE account_user_id = ?",
        )
        .bind(account_user_id)
        .execute(&mut **transaction)
        .await?;
        // Transfer attempts carry account-specific remote reservations. Reconcile
        // fresh attempts from the destination's attachment state after switching.
        sqlx::query(
            "UPDATE attachment_transfer_jobs SET phase = 'completed',
               attempt_count = attempt_count + 1, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE workspace_id = ? AND phase != 'completed'",
        ).bind(library_workspace_id).execute(&mut **transaction).await?;
        sqlx::query("UPDATE local_library_connections SET active = 0 WHERE active = 1")
            .execute(&mut **transaction)
            .await?;
        sqlx::query("UPDATE local_library_connections SET active = 1 WHERE account_user_id = ?")
            .bind(account_user_id)
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "UPDATE session_attachments SET
               cloud_object_key = COALESCE((SELECT cloud_object_key FROM local_library_attachment_state
                 WHERE account_user_id = ?1 AND attachment_id = session_attachments.id), ''),
               cloud_sync_enabled = COALESCE((SELECT cloud_sync_enabled FROM local_library_attachment_state
                 WHERE account_user_id = ?1 AND attachment_id = session_attachments.id), cloud_sync_enabled)
             WHERE workspace_id = ?2",
        ).bind(account_user_id).bind(library_workspace_id).execute(&mut **transaction).await?;
    }
    Ok(connected)
}

pub async fn connect_local_library(
    pool: &SqlitePool,
    account_user_id: &str,
    expected_library_workspace_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let account_user_id = validated_account_user_id(account_user_id)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let binding = load_or_create_binding(&mut transaction).await?;
    if binding.workspace_id != expected_library_workspace_id {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
    }
    // A shared workspace can never be adopted as a personal sync destination.
    let foreign_workspace: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ? AND kind != 'personal')",
    )
    .bind(account_user_id)
    .fetch_one(&mut *transaction)
    .await?;
    if foreign_workspace {
        return Err(CloudsyncWorkspaceError::AccountMismatch);
    }
    if let Some(previous) = binding.account_user_id.as_deref() {
        save_connection(&mut transaction, previous, &binding.workspace_id).await?;
    }
    // Older clients cannot interpret separate local and remote identities, even
    // for the first connection. Installing the migration alone remains safe;
    // explicitly connecting a distinct account opts into the new semantics.
    if account_user_id != binding.workspace_id {
        sqlx::query("UPDATE _anlg_schema_compat SET min_supported_version = MAX(min_supported_version, 20260916043000) WHERE id = 0")
            .execute(&mut *transaction).await?;
    }
    save_connection(&mut transaction, account_user_id, &binding.workspace_id).await?;
    activate_connection(&mut transaction, account_user_id, &binding.workspace_id).await?;
    for table in crate::E2EE_DOMAIN_TABLES {
        let sql = format!(
            "INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
             SELECT ?, ?, CASE WHEN ? = 'humans' AND id = ? THEN ? ELSE id END
             FROM {table} WHERE workspace_id = ?
             ON CONFLICT(workspace_id, table_name, row_id) DO UPDATE SET
               generation = e2ee_dirty_rows.generation + 1"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(account_user_id)
            .bind(table)
            .bind(table)
            .bind(&binding.workspace_id)
            .bind(account_user_id)
            .bind(&binding.workspace_id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

async fn save_connection(
    transaction: &mut Transaction<'_, Sqlite>,
    account_user_id: &str,
    library_workspace_id: &str,
) -> Result<(), CloudsyncWorkspaceError> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT library_workspace_id FROM local_library_connections WHERE account_user_id = ?",
    )
    .bind(account_user_id)
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some(existing) = existing {
        return if existing == library_workspace_id {
            Ok(())
        } else {
            Err(CloudsyncWorkspaceError::AccountMismatch)
        };
    }
    sqlx::query(
        "INSERT INTO local_library_connections (account_user_id, library_workspace_id)
         VALUES (?, ?) ON CONFLICT(account_user_id) DO NOTHING",
    )
    .bind(account_user_id)
    .bind(library_workspace_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO local_library_attachment_state
           (account_user_id, attachment_id, cloud_object_key, cloud_sync_enabled)
         SELECT ?1, id, CASE WHEN ?1 = workspace_id THEN cloud_object_key ELSE '' END, cloud_sync_enabled
         FROM session_attachments WHERE workspace_id = ?2
         ON CONFLICT(account_user_id, attachment_id) DO NOTHING",
    ).bind(account_user_id).bind(library_workspace_id).execute(&mut **transaction).await?;
    Ok(())
}

/// Resolve the encryption namespace without changing the local attachment path.
pub async fn local_library_remote_workspace(
    pool: &SqlitePool,
    library_workspace_id: &str,
) -> sqlx::Result<String> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT account_user_id FROM local_library_connections
         WHERE library_workspace_id = ? AND active = 1",
    )
    .bind(library_workspace_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| library_workspace_id.to_string()))
}
