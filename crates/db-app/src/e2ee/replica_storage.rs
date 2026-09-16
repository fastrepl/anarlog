use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use anlg_e2ee::WorkspaceKeyring;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, Transaction, TypeInfo, ValueRef};

use super::chunks::{chunk_size_for, chunk_value, parse_array, parse_chunk_field, split_chunks};
use super::library::LibraryIdentity;
use super::{
    DecryptedRecord, E2EE_DOMAIN_TABLES, E2eeReplicaError, E2eeReplicaResult, LocalState,
    ROW_MANIFEST_FIELD, check_e2ee_cancellation, yield_once,
};

pub(super) async fn normalize_replica_payload_hashes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    loop {
        check_e2ee_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT replica.id, replica.workspace_id, replica.payload
             FROM e2ee_replica_pending AS pending
             INDEXED BY idx_e2ee_replica_pending_workspace_record
             INNER JOIN e2ee_records AS replica
               ON replica.id = pending.record_id
              AND replica.workspace_id = pending.workspace_id
             LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
               ON replica_hash.record_id = replica.id
             WHERE pending.workspace_id IN (",
        );
        {
            let mut separated = query.separated(", ");
            for workspace_id in &workspace_ids {
                separated.push_bind(workspace_id);
            }
        }
        query.push(
            ")
             AND (
               replica_hash.record_id IS NULL
               OR replica_hash.workspace_id != replica.workspace_id
               OR replica_hash.payload_hash = ''
             )
             AND replica.payload != ''
             ORDER BY pending.workspace_id, pending.record_id
             LIMIT 64",
        );
        let records: Vec<(String, String, String)> =
            query.build_query_as().fetch_all(&mut *transaction).await?;
        if records.is_empty() {
            transaction.commit().await?;
            check_e2ee_cancellation(is_cancelled)?;
            return Ok(());
        }
        for (record_id, workspace_id, payload) in records {
            if let Err(error) = check_e2ee_cancellation(is_cancelled) {
                transaction.rollback().await?;
                return Err(error);
            }
            let payload_hash = anlg_e2ee::payload_hash(&payload);
            upsert_replica_payload_hash(
                &mut transaction,
                &workspace_id,
                &record_id,
                &payload_hash,
                &payload,
            )
            .await?;
        }
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
        yield_once().await;
    }
}

pub(super) async fn upsert_replica_payload_hash(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    record_id: &str,
    payload_hash: &str,
    payload: &str,
) -> E2eeReplicaResult<()> {
    if workspace_id.is_empty()
        || record_id.is_empty()
        || payload.is_empty()
        || anlg_e2ee::payload_hash(payload) != payload_hash
    {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    let result = sqlx::query(
        "INSERT INTO e2ee_replica_payload_hashes (record_id, workspace_id, payload_hash)
         SELECT ?, ?, ?
         WHERE EXISTS (
           SELECT 1 FROM e2ee_records
           WHERE id = ? AND workspace_id = ? AND payload = ?
         )
         ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           payload_hash = excluded.payload_hash,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(record_id)
    .bind(workspace_id)
    .bind(payload_hash)
    .bind(record_id)
    .bind(workspace_id)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    Ok(())
}

pub(super) async fn replica_records_still_current(
    transaction: &mut Transaction<'_, Sqlite>,
    records: &[DecryptedRecord],
) -> E2eeReplicaResult<bool> {
    for record in records {
        let current: Option<(String, String)> = sqlx::query_as(
            "SELECT workspace_id, payload
             FROM e2ee_records
             WHERE id = ?",
        )
        .bind(&record.record_id)
        .fetch_optional(&mut **transaction)
        .await?;
        if !current.is_some_and(|(workspace_id, payload)| {
            workspace_id == record.workspace_id && payload == record.payload
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) async fn load_row_local_states_from_pool(
    pool: &SqlitePool,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<HashMap<String, LocalState>> {
    let states: Vec<LocalState> = sqlx::query_as(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision,
                writer_id, value_tag, payload_hash, '' AS payload, edited_at_ms, republish
         FROM e2ee_local_state
         WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .fetch_all(pool)
    .await?;
    Ok(states
        .into_iter()
        .map(|state| (state.record_id.clone(), state))
        .collect())
}

pub(super) async fn load_row_local_states(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<HashMap<String, LocalState>> {
    let states: Vec<LocalState> = sqlx::query_as(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision,
                writer_id, value_tag, payload_hash, payload, edited_at_ms, republish
         FROM e2ee_local_state_resolved
         WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(states
        .into_iter()
        .map(|state| (state.record_id.clone(), state))
        .collect())
}

pub(super) async fn insert_apply_guard(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    if !identity.active {
        return Err(E2eeReplicaError::Cancelled);
    }
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    sqlx::query(
        "INSERT INTO e2ee_apply_guard (workspace_id, table_name, row_id)
         VALUES (?, ?, ?)",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn clear_stale_apply_guards(pool: &SqlitePool) -> E2eeReplicaResult<()> {
    let has_guards: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_apply_guard LIMIT 1)")
            .fetch_one(pool)
            .await?;
    if has_guards {
        sqlx::query("DELETE FROM e2ee_apply_guard")
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub(super) async fn remove_apply_guard(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    sqlx::query(
        "DELETE FROM e2ee_apply_guard
         WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn upsert_local_state(
    transaction: &mut Transaction<'_, Sqlite>,
    state: &LocalState,
) -> E2eeReplicaResult<()> {
    retain_ciphertext(
        transaction,
        &state.workspace_id,
        &state.record_id,
        &state.payload_hash,
        &state.payload,
    )
    .await?;
    sqlx::query(
        "INSERT INTO e2ee_local_state (
           record_id, workspace_id, table_name, row_id, field_name, revision,
           writer_id, value_tag, payload_hash, edited_at_ms, republish
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           table_name = excluded.table_name,
           row_id = excluded.row_id,
           field_name = excluded.field_name,
           revision = excluded.revision,
           writer_id = excluded.writer_id,
           value_tag = excluded.value_tag,
           payload_hash = excluded.payload_hash,
           edited_at_ms = excluded.edited_at_ms,
           republish = excluded.republish,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(&state.record_id)
    .bind(&state.workspace_id)
    .bind(&state.table_name)
    .bind(&state.row_id)
    .bind(&state.field_name)
    .bind(state.revision)
    .bind(&state.writer_id)
    .bind(&state.value_tag)
    .bind(&state.payload_hash)
    .bind(state.edited_at_ms)
    .bind(state.republish)
    .execute(&mut **transaction)
    .await?;
    reconcile_e2ee_witness_pending(transaction, &state.record_id).await?;
    prune_ciphertext_archive(transaction, &state.workspace_id, &state.record_id).await?;
    Ok(())
}

pub(super) async fn retain_ciphertext(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    record_id: &str,
    payload_hash: &str,
    payload: &str,
) -> E2eeReplicaResult<()> {
    if workspace_id.is_empty()
        || record_id.is_empty()
        || payload_hash.is_empty()
        || payload.is_empty()
        || anlg_e2ee::payload_hash(payload) != payload_hash
    {
        return Err(E2eeReplicaError::RollbackDetected);
    }

    let current: Option<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, payload
         FROM e2ee_records
         WHERE id = ?",
    )
    .bind(record_id)
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some((current_workspace_id, current_payload)) = current
        && current_workspace_id == workspace_id
        && current_payload == payload
    {
        upsert_replica_payload_hash(transaction, workspace_id, record_id, payload_hash, payload)
            .await?;
        return Ok(());
    }

    sqlx::query(
        "INSERT OR IGNORE INTO e2ee_ciphertext_archive (
           workspace_id, record_id, payload_hash, payload
         ) VALUES (?, ?, ?, ?)",
    )
    .bind(workspace_id)
    .bind(record_id)
    .bind(payload_hash)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn prune_ciphertext_archive(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    record_id: &str,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "DELETE FROM e2ee_ciphertext_archive AS archive
         WHERE archive.workspace_id = ?
           AND archive.record_id = ?
           AND (
             EXISTS (
               SELECT 1 FROM e2ee_records AS replica
               WHERE replica.workspace_id = archive.workspace_id
                 AND replica.id = archive.record_id
                 AND replica.payload = archive.payload
             )
             OR (
               NOT EXISTS (
                 SELECT 1 FROM e2ee_local_state AS local
                 WHERE local.workspace_id = archive.workspace_id
                   AND local.record_id = archive.record_id
                   AND local.payload_hash = archive.payload_hash
               )
               AND NOT EXISTS (
                 SELECT 1 FROM e2ee_witness_records AS witness
                 WHERE witness.workspace_id = archive.workspace_id
                   AND witness.record_id = archive.record_id
                   AND witness.payload_hash = archive.payload_hash
               )
             )
           )",
    )
    .bind(workspace_id)
    .bind(record_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn reconcile_e2ee_witness_pending(
    transaction: &mut Transaction<'_, Sqlite>,
    record_id: &str,
) -> E2eeReplicaResult<()> {
    sqlx::query("DELETE FROM e2ee_witness_pending WHERE record_id = ?")
        .bind(record_id)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(
        "INSERT INTO e2ee_witness_pending (record_id, workspace_id)
         SELECT local.record_id, local.workspace_id
         FROM e2ee_local_state AS local
         LEFT JOIN e2ee_witness_records AS witness
           ON witness.workspace_id = local.workspace_id
          AND witness.record_id = local.record_id
         WHERE local.record_id = ?
           AND (
             witness.record_id IS NULL
             OR local.revision > witness.revision
             OR (local.revision = witness.revision AND local.writer_id > witness.writer_id)
             OR (
               local.revision = witness.revision
               AND local.writer_id = witness.writer_id
               AND local.payload_hash > witness.payload_hash
             )
           )",
    )
    .bind(record_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// The value this device holds is newer than a record that arrived with a
/// higher revision. Keep the value and have the next encrypt round publish it
/// above that revision, with its original edit time, so every replica lands on
/// the later edit.
pub(super) async fn mark_local_state_for_republish(
    transaction: &mut Transaction<'_, Sqlite>,
    record_id: &str,
    superseding_revision: i64,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "UPDATE e2ee_local_state
         SET revision = MAX(revision, ?),
             republish = 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE record_id = ?",
    )
    .bind(superseding_revision)
    .bind(record_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn queue_dirty_row(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
         VALUES (?, ?, ?)
         ON CONFLICT (workspace_id, table_name, row_id) DO UPDATE SET
           generation = e2ee_dirty_rows.generation + 1",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn dirty_row_edited_at_ms(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<Option<i64>> {
    let dirtied_at_ms: Option<i64> = sqlx::query_scalar(
        "SELECT dirtied_at_ms FROM e2ee_dirty_rows
         WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(dirtied_at_ms.filter(|ms| *ms > 0))
}

pub(super) async fn load_or_create_writer_id(
    transaction: &mut Transaction<'_, Sqlite>,
) -> E2eeReplicaResult<String> {
    if let Some(writer_id) =
        sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
            .fetch_optional(&mut **transaction)
            .await?
    {
        return Ok(writer_id);
    }

    let writer_id = uuid::Uuid::new_v4().simple().to_string();
    sqlx::query("INSERT INTO e2ee_local_device (id, writer_id) VALUES ('local', ?)")
        .bind(&writer_id)
        .execute(&mut **transaction)
        .await?;
    Ok(writer_id)
}

/// Why a received record is waiting for a more capable build instead of
/// blocking the sync round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum E2eeParkReason {
    UnknownTable,
    UnknownField,
    TooLarge,
}

impl E2eeParkReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTable => "unknown_table",
            Self::UnknownField => "unknown_field",
            Self::TooLarge => "too_large",
        }
    }
}

pub(super) struct ParkedRecord {
    pub record_id: String,
    pub workspace_id: String,
    pub generation: i64,
    pub reason: E2eeParkReason,
    pub table_name: String,
    pub field_name: String,
}

pub(super) async fn park_records(
    transaction: &mut Transaction<'_, Sqlite>,
    records: &[ParkedRecord],
) -> E2eeReplicaResult<()> {
    if records.is_empty() {
        return Ok(());
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        "INSERT INTO e2ee_parked_records (record_id, workspace_id, reason, table_name, field_name) ",
    );
    query.push_values(records, |mut row, record| {
        row.push_bind(&record.record_id)
            .push_bind(&record.workspace_id)
            .push_bind(record.reason.as_str())
            .push_bind(&record.table_name)
            .push_bind(&record.field_name);
    });
    query.push(
        " ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           reason = excluded.reason,
           table_name = excluded.table_name,
           field_name = excluded.field_name",
    );
    query.build().execute(&mut **transaction).await?;

    let mut query = QueryBuilder::<Sqlite>::new(
        "DELETE FROM e2ee_replica_pending WHERE (record_id, generation) IN (",
    );
    query.push_values(records, |mut row, record| {
        row.push_bind(&record.record_id)
            .push_bind(record.generation);
    });
    query.push(")").build().execute(&mut **transaction).await?;
    Ok(())
}

/// Moves every parked record back into the apply queue. Runs at startup, after
/// migrations, so a build that gained a table, a column, or a larger limit
/// retries what an older build had to skip.
pub async fn requeue_parked_e2ee_records(pool: &SqlitePool) -> sqlx::Result<u64> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let requeued = sqlx::query(
        "INSERT INTO e2ee_replica_pending (record_id, workspace_id)
         SELECT record_id, workspace_id FROM e2ee_parked_records
         WHERE true
         ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           generation = e2ee_replica_pending.generation + 1",
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    sqlx::query("DELETE FROM e2ee_parked_records")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(requeued)
}

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct E2eeParkedRecordSummary {
    pub reason: String,
    pub table_name: String,
    pub count: i64,
}

pub async fn parked_e2ee_record_summary(
    pool: &SqlitePool,
) -> sqlx::Result<Vec<E2eeParkedRecordSummary>> {
    sqlx::query_as(
        "SELECT reason, table_name, COUNT(*) AS count
         FROM e2ee_parked_records
         GROUP BY reason, table_name
         ORDER BY reason, table_name",
    )
    .fetch_all(pool)
    .await
}

pub(super) fn record_version_order(
    state: &LocalState,
    record: &DecryptedRecord,
) -> E2eeReplicaResult<Ordering> {
    let state_revision = u64::try_from(state.revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    Ok(record
        .field
        .revision
        .cmp(&state_revision)
        .then_with(|| record.field.writer_id.cmp(&state.writer_id))
        .then_with(|| record.payload_hash.cmp(&state.payload_hash)))
}

pub(super) async fn restore_local_payload(
    transaction: &mut Transaction<'_, Sqlite>,
    state: &LocalState,
) -> E2eeReplicaResult<()> {
    if state.payload.is_empty() || anlg_e2ee::payload_hash(&state.payload) != state.payload_hash {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    sqlx::query(
        "UPDATE e2ee_records
         SET payload = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(&state.payload)
    .bind(&state.record_id)
    .bind(&state.workspace_id)
    .execute(&mut **transaction)
    .await?;
    upsert_replica_payload_hash(
        transaction,
        &state.workspace_id,
        &state.record_id,
        &state.payload_hash,
        &state.payload,
    )
    .await?;
    Ok(())
}

pub(super) async fn table_columns(
    pool: &SqlitePool,
    table: &str,
) -> E2eeReplicaResult<HashSet<String>> {
    if !E2EE_DOMAIN_TABLES.contains(&table) {
        return Err(E2eeReplicaError::InvalidField);
    }
    let sql = format!("PRAGMA table_info({table})");
    let columns = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.try_get("name"))
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(columns.into_iter().collect())
}

pub(super) async fn row_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<bool> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ? AND workspace_id = ?)");
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?)
}

pub(super) async fn insert_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    let sql =
        format!("INSERT INTO {table} (id, workspace_id) VALUES (?, ?) ON CONFLICT(id) DO NOTHING");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub(super) async fn delete_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    let sql = format!("DELETE FROM {table} WHERE id = ? AND workspace_id = ?");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

/// Reads a field as it would be sealed: a real column's value, or for a
/// virtual chunk field the chunk (or chunk count) cut from its column.
pub(super) async fn read_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
) -> E2eeReplicaResult<Option<Value>> {
    if let Some((column, part)) = parse_chunk_field(table, field) {
        let Some(value) = read_column(transaction, table, workspace_id, row_id, column).await?
        else {
            return Ok(None);
        };
        let chunk_size = chunk_size_for(table, column).unwrap_or(1);
        let items = parse_array(&value).unwrap_or_default();
        return Ok(Some(chunk_value(&split_chunks(&items, chunk_size), part)));
    }
    read_column(transaction, table, workspace_id, row_id, field).await
}

pub(super) async fn read_column(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
) -> E2eeReplicaResult<Option<Value>> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    let sql = format!("SELECT {field} FROM {table} WHERE id = ? AND workspace_id = ? LIMIT 1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;
    match row.as_ref() {
        Some(row) => Ok(Some(
            identity
                .remote_value(
                    &mut **transaction,
                    table,
                    row_id,
                    field,
                    sqlite_value(row, 0)?,
                )
                .await?,
        )),
        None => Ok(None),
    }
}

pub(super) async fn update_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
    value: &Value,
) -> E2eeReplicaResult<()> {
    let identity = LibraryIdentity::load(&mut **transaction, workspace_id).await?;
    let value = &identity.to_local(table, field, value.clone());
    let workspace_id = identity.local_workspace_id.as_str();
    let row_id = identity.local_row_id(table, row_id);
    let mut query = QueryBuilder::new(format!("UPDATE {table} SET {field} = "));
    push_json_bind(&mut query, value)?;
    query.push(" WHERE id = ").push_bind(row_id);
    query.push(" AND workspace_id = ").push_bind(workspace_id);
    let result = query.build().execute(&mut **transaction).await?;
    if result.rows_affected() != 1 {
        return Err(E2eeReplicaError::InvalidRow);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn row_changed_since_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    keyring: &WorkspaceKeyring,
    workspace_id: &str,
    table: &str,
    row_id: &str,
    row_exists: bool,
    states: &HashMap<String, LocalState>,
) -> E2eeReplicaResult<bool> {
    let row_states = states.values().filter(|state| {
        state.workspace_id == workspace_id && state.table_name == table && state.row_id == row_id
    });
    let mut found_manifest = false;
    for state in row_states {
        if state.field_name == ROW_MANIFEST_FIELD {
            found_manifest = true;
            let value = if row_exists { json!(true) } else { Value::Null };
            let matches_snapshot = keyring.generations().any(|key| {
                key.value_tag(table, row_id, ROW_MANIFEST_FIELD, !row_exists, &value)
                    == state.value_tag
            });
            if !matches_snapshot {
                return Ok(true);
            }
            continue;
        }
        if !row_exists {
            continue;
        }
        let Some(value) =
            read_field(transaction, table, workspace_id, row_id, &state.field_name).await?
        else {
            return Ok(true);
        };
        let matches_snapshot = keyring.generations().any(|key| {
            key.value_tag(table, row_id, &state.field_name, false, &value) == state.value_tag
        });
        if !matches_snapshot {
            return Ok(true);
        }
    }
    Ok(row_exists && !found_manifest)
}

pub(super) fn sqlite_value(row: &SqliteRow, index: usize) -> E2eeReplicaResult<Value> {
    let raw = row.try_get_raw(index)?;
    if raw.is_null() {
        return Ok(Value::Null);
    }

    match raw.type_info().name() {
        "INTEGER" => Ok(json!(row.try_get::<i64, _>(index)?)),
        "REAL" => Ok(json!(row.try_get::<f64, _>(index)?)),
        "TEXT" => Ok(json!(row.try_get::<String, _>(index)?)),
        "BLOB" => Ok(json!({
            "$anarlog_blob": URL_SAFE_NO_PAD.encode(row.try_get::<Vec<u8>, _>(index)?)
        })),
        _ => Err(E2eeReplicaError::UnsupportedValue),
    }
}

fn push_json_bind(query: &mut QueryBuilder<Sqlite>, value: &Value) -> E2eeReplicaResult<()> {
    match value {
        Value::Null => {
            query.push_bind(None::<String>);
        }
        Value::Bool(value) => {
            query.push_bind(i64::from(*value));
        }
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                query.push_bind(value);
            } else if let Some(value) = value.as_f64() {
                query.push_bind(value);
            } else {
                return Err(E2eeReplicaError::UnsupportedValue);
            }
        }
        Value::String(value) => {
            query.push_bind(value);
        }
        Value::Object(value) if value.len() == 1 && value.contains_key("$anarlog_blob") => {
            let bytes = value
                .get("$anarlog_blob")
                .and_then(Value::as_str)
                .ok_or(E2eeReplicaError::UnsupportedValue)
                .and_then(|value| {
                    URL_SAFE_NO_PAD
                        .decode(value)
                        .map_err(|_| E2eeReplicaError::UnsupportedValue)
                })?;
            query.push_bind(bytes);
        }
        Value::Array(_) | Value::Object(_) => {
            return Err(E2eeReplicaError::UnsupportedValue);
        }
    }
    Ok(())
}
