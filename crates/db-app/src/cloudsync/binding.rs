use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};

use super::CloudsyncWorkspaceError;

pub const CLOUDSYNC_WORKSPACE_BINDING_ID: &str = "cloudsync_workspace_binding";
pub(super) const LEGACY_DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";
pub(super) const CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE: usize = 128;

pub(super) const USER_ID_REFERENCES: &[(&str, &str)] = &[
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

#[derive(Deserialize, Serialize)]
struct CloudsyncWorkspaceBinding {
    workspace_id: String,
    account_user_id: Option<String>,
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

pub(super) fn validated_account_user_id(
    account_user_id: &str,
) -> Result<&str, CloudsyncWorkspaceError> {
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

pub(super) async fn require_claimed_binding(
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
