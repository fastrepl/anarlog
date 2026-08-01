use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anlg_e2ee::{OpenedField, WorkspaceKey};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, QueryBuilder, Row, Sqlite, SqlitePool, Transaction, TypeInfo, ValueRef};

mod witness;

#[cfg(test)]
use witness::PENDING_E2EE_WITNESS_UPLOADS_SQL;
pub use witness::{
    E2eeWitnessEvent, E2eeWitnessRepairOutcome, E2eeWitnessUpload,
    acknowledge_e2ee_witness_uploads, acknowledge_e2ee_witness_uploads_cancellable,
    advance_e2ee_witness_cursor, e2ee_witness_cursor, has_e2ee_local_state,
    has_pending_e2ee_witness_repairs, has_pending_e2ee_witness_repairs_cancellable,
    merge_e2ee_witness_events, merge_e2ee_witness_events_cancellable, pending_e2ee_witness_uploads,
    pending_e2ee_witness_uploads_cancellable, repair_e2ee_replica_from_witness_bounded,
    repair_e2ee_replica_from_witness_bounded_cancellable,
};

pub const E2EE_DOMAIN_TABLES: &[&str] = &[
    "action_items",
    "humans",
    "organizations",
    "session_attachments",
    "session_documents",
    "session_participants",
    "sessions",
    "transcripts",
];

const ROW_MANIFEST_FIELD: &str = "$row";
const E2EE_ENCRYPT_ROW_LIMIT: i64 = 64;
const E2EE_APPLY_ROW_LIMIT: usize = 16;
const E2EE_APPLY_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const E2EE_APPLY_PREFLIGHT_RECORD_LIMIT: i64 = 64;
const E2EE_WITNESS_REPAIR_RECORD_LIMIT: i64 = 64;
const E2EE_WITNESS_REPAIR_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const ACTIVE_CAPTURE_MARKER_PREDICATE: &str = "
  length(capture.id) > length('capture_lifecycle_pending:')
  AND json_type(capture.marker_json, '$.version') IN ('integer', 'real')
  AND json_extract(capture.marker_json, '$.version') = 1
  AND CASE json_extract(capture.marker_json, '$.phase')
    WHEN 'capturing' THEN 1
    WHEN 'finalizing' THEN 0
    ELSE CASE
      WHEN json_extract(capture.marker_json, '$.summaryMode') IN ('regenerate', 'if_empty')
        THEN 0
      ELSE 1
    END
  END = 1
  AND json_type(capture.marker_json, '$.sessionId') = 'text'
  AND json_extract(capture.marker_json, '$.sessionId')
    = substr(capture.id, length('capture_lifecycle_pending:') + 1)
  AND json_type(capture.marker_json, '$.transcriptId') = 'text'
  AND json_extract(capture.marker_json, '$.transcriptId') != ''
  AND json_type(capture.marker_json, '$.startedAt') IN ('integer', 'real')
  AND abs(json_extract(capture.marker_json, '$.startedAt')) <= 1.7976931348623157e308
  AND json_type(capture.marker_json, '$.createdAt') = 'text'
  AND json_type(capture.marker_json, '$.audioOffsetMs') IN ('integer', 'real')
  AND abs(json_extract(capture.marker_json, '$.audioOffsetMs')) <= 1.7976931348623157e308
  AND json_type(capture.marker_json, '$.preserveExistingTranscript') IN ('true', 'false')
  AND json_type(capture.marker_json, '$.ownerUserId') = 'text'
  AND json_type(capture.marker_json, '$.memo') = 'text'";

#[derive(Debug, thiserror::Error)]
pub enum E2eeReplicaError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Crypto(#[from] anlg_e2ee::Error),
    #[error("encrypted replica contains an invalid table or field")]
    InvalidField,
    #[error("encrypted replica contains an unsupported SQLite value")]
    UnsupportedValue,
    #[error("encrypted replica contains an invalid row")]
    InvalidRow,
    #[error("encrypted replica row exceeds the apply byte limit")]
    ReplicaApplyTooLarge,
    #[error("encrypted witness record exceeds the repair byte limit")]
    WitnessRepairTooLarge,
    #[error("encrypted witness upload exceeds the batch byte limit")]
    WitnessUploadTooLarge,
    #[error("encrypted replica operation was cancelled")]
    Cancelled,
    #[error("encrypted replica rollback was detected")]
    RollbackDetected,
}

pub type E2eeReplicaResult<T> = std::result::Result<T, E2eeReplicaError>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct E2eeReplicaStats {
    pub encrypted_fields: u64,
    pub applied_fields: u64,
    pub skipped_local_changes: u64,
    pub rejected_rollbacks: u64,
    pub rejected_unwitnessed: u64,
    pub remaining_replica_changes: bool,
}

#[derive(Clone, sqlx::FromRow)]
struct LocalState {
    record_id: String,
    workspace_id: String,
    table_name: String,
    row_id: String,
    field_name: String,
    revision: i64,
    writer_id: String,
    value_tag: String,
    payload_hash: String,
    payload: String,
}

#[derive(sqlx::FromRow)]
struct EncryptedRecord {
    id: String,
    workspace_id: String,
    payload: String,
    witnessed: bool,
}

#[derive(sqlx::FromRow)]
struct EncryptedRecordMetadata {
    id: String,
    workspace_id: String,
    record_bytes: i64,
    witnessed: bool,
    changed: bool,
}

struct DecryptedRecord {
    record_id: String,
    workspace_id: String,
    payload_hash: String,
    payload: String,
    field: OpenedField,
}

#[derive(Clone, sqlx::FromRow)]
struct DirtyRow {
    workspace_id: String,
    table_name: String,
    row_id: String,
    generation: i64,
}

struct PreparedEncryptedField {
    state: LocalState,
    previous_payload_hash: Option<String>,
    expected_witness_version: Option<WitnessVersion>,
}

struct PreparedDirtyRow {
    dirty: DirtyRow,
    fields: Vec<PreparedEncryptedField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WitnessVersion {
    revision: i64,
    writer_id: String,
    payload_hash: String,
}

pub async fn encrypt_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, false, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, true, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, true, &is_cancelled).await
}

async fn encrypt_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    defer_active_captures: bool,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<E2eeReplicaStats> {
    if keys.is_empty() {
        return Ok(E2eeReplicaStats::default());
    }

    let mut stats = E2eeReplicaStats::default();
    if is_cancelled() {
        stats.remaining_replica_changes = true;
        return Ok(stats);
    }
    loop {
        let batch = encrypt_e2ee_replica_changes_bounded_inner(
            pool,
            keys,
            E2EE_ENCRYPT_ROW_LIMIT,
            defer_active_captures,
            is_cancelled,
        )
        .await?;
        stats.encrypted_fields += batch.encrypted_fields;
        stats.remaining_replica_changes = batch.remaining_replica_changes;
        if is_cancelled() {
            stats.remaining_replica_changes = true;
            break;
        }
        if !stats.remaining_replica_changes {
            break;
        }
        yield_once().await;
    }
    Ok(stats)
}

pub async fn encrypt_e2ee_replica_changes_bounded(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, false, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_bounded_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, true, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, true, &is_cancelled).await
}

async fn encrypt_e2ee_replica_changes_bounded_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
    defer_active_captures: bool,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<E2eeReplicaStats> {
    if keys.is_empty() || max_rows <= 0 {
        return Ok(E2eeReplicaStats::default());
    }

    let (dirty_rows, queued_remaining) = load_dirty_rows_page(
        pool,
        keys,
        max_rows.min(E2EE_ENCRYPT_ROW_LIMIT),
        defer_active_captures,
    )
    .await?;
    if dirty_rows.is_empty() {
        return Ok(E2eeReplicaStats::default());
    }
    let mut stats = E2eeReplicaStats {
        remaining_replica_changes: queued_remaining,
        ..Default::default()
    };
    if is_cancelled() {
        stats.remaining_replica_changes = true;
        return Ok(stats);
    }
    let writer_id = {
        check_e2ee_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let writer_id = load_or_create_writer_id(&mut transaction).await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
        writer_id
    };
    if is_cancelled() {
        stats.remaining_replica_changes = true;
        return Ok(stats);
    }
    for dirty in dirty_rows {
        let key = &keys[&dirty.workspace_id];
        let prepared =
            prepare_dirty_row_cancellable(pool, key, &writer_id, dirty, is_cancelled).await?;
        if is_cancelled() {
            stats.remaining_replica_changes = true;
            break;
        }
        stats.encrypted_fields += persist_prepared_dirty_row_cancellable(
            pool,
            prepared,
            defer_active_captures,
            is_cancelled,
        )
        .await?;
        if is_cancelled() {
            stats.remaining_replica_changes = true;
            break;
        }
    }
    if !stats.remaining_replica_changes && !is_cancelled() {
        stats.remaining_replica_changes =
            !load_dirty_rows_inner(pool, keys, 1, defer_active_captures)
                .await?
                .is_empty();
    }
    if is_cancelled() {
        stats.remaining_replica_changes = true;
    }
    Ok(stats)
}

pub async fn has_pending_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_replica_changes_inner(pool, keys, false).await
}

pub async fn has_pending_e2ee_replica_changes_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_replica_changes_inner(pool, keys, true).await
}

pub async fn has_pending_e2ee_dirty_rows_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<bool> {
    if keys.is_empty() {
        return Ok(false);
    }
    Ok(!load_dirty_rows_inner(pool, keys, 1, true).await?.is_empty())
}

async fn has_pending_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    defer_active_captures: bool,
) -> E2eeReplicaResult<bool> {
    if keys.is_empty() {
        return Ok(false);
    }
    if !load_dirty_rows_inner(pool, keys, 1, defer_active_captures)
        .await?
        .is_empty()
    {
        return Ok(true);
    }
    has_pending_e2ee_witness_repairs(pool, keys, true).await
}

pub async fn apply_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    apply_e2ee_replica_changes_inner(
        pool,
        keys,
        false,
        E2EE_APPLY_ROW_LIMIT,
        E2EE_APPLY_BYTE_LIMIT,
        &|| false,
    )
    .await
}

pub async fn apply_e2ee_replica_changes_with_witness(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    apply_received_e2ee_replica_changes_with_witness(pool, keys, true).await
}

pub async fn apply_received_e2ee_replica_changes_with_witness(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    snapshot_complete: bool,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    apply_received_e2ee_replica_changes_with_witness_cancellable(
        pool,
        keys,
        snapshot_complete,
        || false,
    )
    .await
}

pub async fn apply_received_e2ee_replica_changes_with_witness_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    snapshot_complete: bool,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    apply_received_e2ee_replica_changes_with_witness_bounded(
        pool,
        keys,
        snapshot_complete,
        E2EE_WITNESS_REPAIR_RECORD_LIMIT,
        E2EE_WITNESS_REPAIR_BYTE_LIMIT,
        &is_cancelled,
    )
    .await
}

async fn apply_received_e2ee_replica_changes_with_witness_bounded(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    snapshot_complete: bool,
    max_repair_records: i64,
    max_repair_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<E2eeReplicaStats> {
    check_e2ee_apply_cancellation(is_cancelled)?;
    let repair_remaining = if snapshot_complete {
        repair_e2ee_replica_from_witness_bounded_cancellable(
            pool,
            keys,
            true,
            max_repair_records,
            max_repair_bytes,
            is_cancelled,
        )
        .await?
        .remaining
    } else {
        false
    };
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut stats = apply_e2ee_replica_changes_inner(
        pool,
        keys,
        true,
        E2EE_APPLY_ROW_LIMIT,
        E2EE_APPLY_BYTE_LIMIT,
        is_cancelled,
    )
    .await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    stats.remaining_replica_changes |= repair_remaining;
    Ok(stats)
}

fn check_e2ee_apply_cancellation(
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    if is_cancelled() {
        Err(E2eeReplicaError::Cancelled)
    } else {
        Ok(())
    }
}

async fn rollback_cancelled_e2ee_apply<T>(
    transaction: Transaction<'_, Sqlite>,
) -> E2eeReplicaResult<T> {
    transaction.rollback().await?;
    Err(E2eeReplicaError::Cancelled)
}

async fn commit_e2ee_apply_transaction(
    transaction: Transaction<'_, Sqlite>,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    if is_cancelled() {
        return rollback_cancelled_e2ee_apply(transaction).await;
    }
    transaction.commit().await?;
    check_e2ee_apply_cancellation(is_cancelled)
}

async fn load_changed_e2ee_record_metadata(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    cursor: Option<&(String, String)>,
) -> E2eeReplicaResult<Vec<EncryptedRecordMetadata>> {
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "WITH page AS MATERIALIZED (
           SELECT input.id, input.workspace_id
           FROM e2ee_records AS input
           INDEXED BY idx_e2ee_records_workspace
           WHERE input.workspace_id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for workspace_id in workspace_ids {
            separated.push_bind(workspace_id);
        }
    }
    query.push(")");
    if let Some((workspace_id, record_id)) = cursor {
        query
            .push(
                "
           AND (
             input.workspace_id > ",
            )
            .push_bind(workspace_id)
            .push(
                "
             OR (
               input.workspace_id = ",
            )
            .push_bind(workspace_id)
            .push(
                "
               AND input.id > ",
            )
            .push_bind(record_id)
            .push(
                "
             )
           )",
            );
    }
    query
        .push(
            "
           ORDER BY input.workspace_id, input.id
           LIMIT ",
        )
        .push_bind(E2EE_APPLY_PREFLIGHT_RECORD_LIMIT)
        .push(
            "
         )
         SELECT
           replica.id,
           replica.workspace_id,
           LENGTH(CAST(replica.id AS BLOB))
             + LENGTH(CAST(replica.workspace_id AS BLOB))
             + LENGTH(CAST(replica.payload AS BLOB))
             + 256 AS record_bytes,
           EXISTS(
             SELECT 1
             FROM e2ee_witness_records AS witness
             WHERE witness.workspace_id = replica.workspace_id
               AND witness.record_id = replica.id
               AND witness.payload = replica.payload
           ) AS witnessed,
           (
             local.record_id IS NULL
             OR local.workspace_id != replica.workspace_id
             OR local.payload != replica.payload
           ) AS changed
         FROM page
         INNER JOIN e2ee_records AS replica
           ON replica.id = page.id
          AND replica.workspace_id = page.workspace_id
         LEFT JOIN e2ee_local_state AS local
           ON local.record_id = replica.id
         ORDER BY replica.workspace_id, replica.id",
        );
    Ok(query.build_query_as().fetch_all(pool).await?)
}

async fn load_encrypted_records_by_id(
    pool: &SqlitePool,
    record_ids: &[String],
) -> E2eeReplicaResult<Vec<EncryptedRecord>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT
           replica.id,
           replica.workspace_id,
           replica.payload,
           EXISTS(
             SELECT 1
             FROM e2ee_witness_records AS witness
             WHERE witness.workspace_id = replica.workspace_id
               AND witness.record_id = replica.id
               AND witness.payload = replica.payload
           ) AS witnessed
         FROM e2ee_records AS replica
         WHERE replica.id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for record_id in record_ids {
            separated.push_bind(record_id);
        }
    }
    query.push(") ORDER BY replica.workspace_id, replica.id");
    Ok(query.build_query_as().fetch_all(pool).await?)
}

async fn apply_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    require_witness: bool,
    max_rows: usize,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<E2eeReplicaStats> {
    if keys.is_empty() || max_rows == 0 || max_bytes == 0 {
        return Ok(E2eeReplicaStats::default());
    }

    check_e2ee_apply_cancellation(is_cancelled)?;
    clear_stale_apply_guards(pool).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut groups = BTreeMap::<(String, String, String), BTreeSet<String>>::new();
    let mut stats = E2eeReplicaStats::default();
    let mut cursor = None::<(String, String)>;

    'preflight: loop {
        check_e2ee_apply_cancellation(is_cancelled)?;
        let metadata = load_changed_e2ee_record_metadata(pool, keys, cursor.as_ref()).await?;
        check_e2ee_apply_cancellation(is_cancelled)?;
        if metadata.is_empty() {
            break;
        }

        let metadata_complete =
            metadata.len() < usize::try_from(E2EE_APPLY_PREFLIGHT_RECORD_LIMIT).unwrap();
        let mut selected_ids = Vec::new();
        let mut selected_bytes = 0_usize;
        let mut processed_metadata = 0_usize;
        for record in &metadata {
            check_e2ee_apply_cancellation(is_cancelled)?;
            if record.changed {
                if require_witness && !record.witnessed {
                    stats.rejected_unwitnessed += 1;
                } else {
                    let record_bytes = usize::try_from(record.record_bytes)
                        .map_err(|_| E2eeReplicaError::InvalidRow)?;
                    if record_bytes > max_bytes {
                        return Err(E2eeReplicaError::ReplicaApplyTooLarge);
                    }
                    if !selected_ids.is_empty()
                        && selected_bytes.saturating_add(record_bytes) > max_bytes
                    {
                        break;
                    }
                    selected_bytes = selected_bytes.saturating_add(record_bytes);
                    selected_ids.push(record.id.clone());
                }
            }
            cursor = Some((record.workspace_id.clone(), record.id.clone()));
            processed_metadata += 1;
        }

        if !selected_ids.is_empty() {
            check_e2ee_apply_cancellation(is_cancelled)?;
            let records = load_encrypted_records_by_id(pool, &selected_ids).await?;
            check_e2ee_apply_cancellation(is_cancelled)?;
            for record in records {
                check_e2ee_apply_cancellation(is_cancelled)?;
                let Some(key) = keys.get(&record.workspace_id) else {
                    continue;
                };
                if require_witness && !record.witnessed {
                    stats.rejected_unwitnessed += 1;
                    continue;
                }
                let field = key.open_field(&record.workspace_id, &record.id, &record.payload)?;
                check_e2ee_apply_cancellation(is_cancelled)?;
                if !E2EE_DOMAIN_TABLES.contains(&field.table.as_str()) {
                    return Err(E2eeReplicaError::InvalidField);
                }
                groups
                    .entry((record.workspace_id, field.table, field.row_id))
                    .or_default()
                    .insert(field.field);
                if groups.len() > max_rows {
                    stats.remaining_replica_changes = true;
                    break 'preflight;
                }
            }
        }

        if processed_metadata == metadata.len() && metadata_complete {
            break;
        }
        yield_once().await;
        check_e2ee_apply_cancellation(is_cancelled)?;
    }

    let mut column_cache = HashMap::<String, HashSet<String>>::new();
    let mut attempted_bytes = 0_usize;
    macro_rules! rollback_if_cancelled {
        ($transaction:ident, $is_cancelled:expr) => {
            if ($is_cancelled)() {
                return rollback_cancelled_e2ee_apply($transaction).await;
            }
        };
    }
    for (attempted_rows, ((workspace_id, table, row_id), changed_fields)) in
        groups.into_iter().enumerate()
    {
        if attempted_rows >= max_rows {
            stats.remaining_replica_changes = true;
            break;
        }
        check_e2ee_apply_cancellation(is_cancelled)?;
        if attempted_rows > 0 {
            yield_once().await;
            check_e2ee_apply_cancellation(is_cancelled)?;
        }
        let key = &keys[&workspace_id];
        let columns = match column_cache.get(&table) {
            Some(columns) => columns.clone(),
            None => {
                check_e2ee_apply_cancellation(is_cancelled)?;
                let columns = table_columns(pool, &table).await?;
                check_e2ee_apply_cancellation(is_cancelled)?;
                column_cache.insert(table.clone(), columns.clone());
                columns
            }
        };
        if changed_fields.iter().any(|field| {
            field != ROW_MANIFEST_FIELD
                && (field == "id" || field == "workspace_id" || !columns.contains(field))
        }) {
            return Err(E2eeReplicaError::InvalidField);
        }

        check_e2ee_apply_cancellation(is_cancelled)?;
        let encrypted_records = load_encrypted_row_group(
            pool,
            key,
            (&workspace_id, &table, &row_id),
            &columns,
            max_bytes,
            is_cancelled,
        )
        .await?;
        check_e2ee_apply_cancellation(is_cancelled)?;
        let row_bytes = encrypted_records
            .iter()
            .try_fold(0_usize, |total, record| {
                total
                    .checked_add(record.id.len())
                    .and_then(|total| total.checked_add(record.workspace_id.len()))
                    .and_then(|total| total.checked_add(record.payload.len()))
                    .and_then(|total| total.checked_add(256))
                    .ok_or(E2eeReplicaError::InvalidRow)
            })?;
        if row_bytes > max_bytes {
            return Err(E2eeReplicaError::ReplicaApplyTooLarge);
        }
        if attempted_rows > 0 && attempted_bytes.saturating_add(row_bytes) > max_bytes {
            stats.remaining_replica_changes = true;
            break;
        }
        attempted_bytes = attempted_bytes.saturating_add(row_bytes);
        let mut records = Vec::with_capacity(encrypted_records.len());
        for record in encrypted_records {
            check_e2ee_apply_cancellation(is_cancelled)?;
            if require_witness && !record.witnessed {
                continue;
            }
            let field = key.open_field(&record.workspace_id, &record.id, &record.payload)?;
            check_e2ee_apply_cancellation(is_cancelled)?;
            if field.table != table || field.row_id != row_id {
                return Err(E2eeReplicaError::InvalidField);
            }
            if field.field != ROW_MANIFEST_FIELD
                && (field.field == "id"
                    || field.field == "workspace_id"
                    || !columns.contains(&field.field)
                    || field.deleted)
            {
                return Err(E2eeReplicaError::InvalidField);
            }
            let payload_hash = anlg_e2ee::payload_hash(&record.payload);
            check_e2ee_apply_cancellation(is_cancelled)?;
            records.push(DecryptedRecord {
                record_id: record.id,
                workspace_id: record.workspace_id,
                payload_hash,
                payload: record.payload,
                field,
            });
        }

        check_e2ee_apply_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        if !replica_records_still_current(&mut transaction, &records).await? {
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        }
        rollback_if_cancelled!(transaction, is_cancelled);
        insert_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        let mut states =
            load_row_local_states(&mut transaction, &workspace_id, &table, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        let mut stale_manifest = false;
        let mut accepted_records = Vec::with_capacity(records.len());
        for record in records {
            rollback_if_cancelled!(transaction, is_cancelled);
            let is_stale = match states.get(&record.record_id) {
                Some(state) => record_version_order(state, &record)? == Ordering::Less,
                None => false,
            };
            if is_stale {
                restore_local_payload(&mut transaction, &states[&record.record_id]).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                stale_manifest |= record.field.field == ROW_MANIFEST_FIELD;
                stats.rejected_rollbacks += 1;
                continue;
            }
            accepted_records.push(record);
        }
        records = accepted_records;
        if stale_manifest {
            remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        }
        let Some(manifest_index) = records
            .iter()
            .position(|record| record.field.field == ROW_MANIFEST_FIELD)
        else {
            remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        };
        let manifest = records.swap_remove(manifest_index);
        let manifest_state = states.get(&manifest.record_id).cloned();
        let manifest_unchanged = manifest_state
            .as_ref()
            .is_some_and(|state| state.payload_hash == manifest.payload_hash);
        let row_was_present = row_exists(&mut transaction, &table, &workspace_id, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        let mut row_materialized = false;

        if !manifest_unchanged {
            let locally_changed = row_changed_since_snapshot(
                &mut transaction,
                key,
                &workspace_id,
                &table,
                &row_id,
                row_was_present,
                &states,
            )
            .await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            if locally_changed {
                stats.skipped_local_changes += records.len() as u64 + 1;
                remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
                continue;
            }

            if manifest.field.deleted {
                delete_row(&mut transaction, &table, &workspace_id, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                let value_tag =
                    key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, true, &Value::Null);
                let state = LocalState {
                    record_id: manifest.record_id,
                    workspace_id: workspace_id.clone(),
                    table_name: table.clone(),
                    row_id: row_id.clone(),
                    field_name: ROW_MANIFEST_FIELD.to_string(),
                    revision: i64::try_from(manifest.field.revision)
                        .map_err(|_| E2eeReplicaError::InvalidRow)?,
                    writer_id: manifest.field.writer_id,
                    value_tag,
                    payload_hash: manifest.payload_hash,
                    payload: manifest.payload,
                };
                upsert_local_state(&mut transaction, &state).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                stats.applied_fields += 1;
                remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
                continue;
            }

            if !row_was_present {
                insert_row(&mut transaction, &table, &workspace_id, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                if !row_exists(&mut transaction, &table, &workspace_id, &row_id).await? {
                    rollback_if_cancelled!(transaction, is_cancelled);
                    remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
                    continue;
                }
                rollback_if_cancelled!(transaction, is_cancelled);
                sqlx::query(
                    "DELETE FROM e2ee_local_state
                     WHERE workspace_id = ?
                       AND table_name = ?
                       AND row_id = ?
                       AND field_name != ?",
                )
                .bind(&workspace_id)
                .bind(&table)
                .bind(&row_id)
                .bind(ROW_MANIFEST_FIELD)
                .execute(&mut *transaction)
                .await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                states.retain(|_, state| state.field_name == ROW_MANIFEST_FIELD);
                row_materialized = true;
            }
            let value_tag = key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, false, &json!(true));
            let state = LocalState {
                record_id: manifest.record_id,
                workspace_id: workspace_id.clone(),
                table_name: table.clone(),
                row_id: row_id.clone(),
                field_name: ROW_MANIFEST_FIELD.to_string(),
                revision: i64::try_from(manifest.field.revision)
                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                writer_id: manifest.field.writer_id,
                value_tag,
                payload_hash: manifest.payload_hash,
                payload: manifest.payload,
            };
            upsert_local_state(&mut transaction, &state).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        } else if !row_was_present || manifest.field.deleted {
            remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        }

        for record in records {
            rollback_if_cancelled!(transaction, is_cancelled);
            let field_name = record.field.field.as_str();
            if field_name == ROW_MANIFEST_FIELD
                || field_name == "id"
                || field_name == "workspace_id"
                || !columns.contains(field_name)
                || record.field.deleted
            {
                return Err(E2eeReplicaError::InvalidField);
            }
            if !row_materialized
                && states
                    .get(&record.record_id)
                    .is_some_and(|state| state.payload_hash == record.payload_hash)
            {
                continue;
            }
            if !row_materialized && let Some(state) = states.get(&record.record_id) {
                let Some(current) =
                    read_field(&mut transaction, &table, &workspace_id, &row_id, field_name)
                        .await?
                else {
                    rollback_if_cancelled!(transaction, is_cancelled);
                    stats.skipped_local_changes += 1;
                    continue;
                };
                rollback_if_cancelled!(transaction, is_cancelled);
                let current_tag = key.value_tag(&table, &row_id, field_name, false, &current);
                if current_tag != state.value_tag {
                    stats.skipped_local_changes += 1;
                    continue;
                }
            }

            update_field(
                &mut transaction,
                &table,
                &workspace_id,
                &row_id,
                field_name,
                &record.field.value,
            )
            .await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            let value_tag = key.value_tag(&table, &row_id, field_name, false, &record.field.value);
            let state = LocalState {
                record_id: record.record_id,
                workspace_id: record.workspace_id,
                table_name: table.clone(),
                row_id: row_id.clone(),
                field_name: field_name.to_string(),
                revision: i64::try_from(record.field.revision)
                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                writer_id: record.field.writer_id,
                value_tag,
                payload_hash: record.payload_hash,
                payload: record.payload,
            };
            upsert_local_state(&mut transaction, &state).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        }
        remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
    }

    check_e2ee_apply_cancellation(is_cancelled)?;
    Ok(stats)
}

#[cfg(test)]
async fn load_dirty_rows(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
) -> E2eeReplicaResult<Vec<DirtyRow>> {
    load_dirty_rows_inner(pool, keys, max_rows, false).await
}

async fn load_dirty_rows_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
    defer_active_captures: bool,
) -> E2eeReplicaResult<Vec<DirtyRow>> {
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT dirty.workspace_id, dirty.table_name, dirty.row_id, dirty.generation
         FROM e2ee_dirty_rows AS dirty
         WHERE dirty.workspace_id IN (",
    );
    let mut separated = query.separated(", ");
    for workspace_id in workspace_ids {
        separated.push_bind(workspace_id);
    }
    separated.push_unseparated(")");
    if defer_active_captures {
        push_active_capture_exclusion(&mut query);
    }
    query
        .push(" ORDER BY dirty.workspace_id, dirty.table_name, dirty.row_id LIMIT ")
        .push_bind(max_rows);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

async fn load_dirty_rows_page(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    max_rows: i64,
    defer_active_captures: bool,
) -> E2eeReplicaResult<(Vec<DirtyRow>, bool)> {
    if max_rows <= 0 {
        return Ok((Vec::new(), false));
    }
    let page_limit = max_rows.saturating_add(1);
    let mut rows = load_dirty_rows_inner(pool, keys, page_limit, defer_active_captures).await?;
    let max_rows = usize::try_from(max_rows).map_err(|_| E2eeReplicaError::InvalidRow)?;
    let remaining = rows.len() > max_rows;
    rows.truncate(max_rows);
    Ok((rows, remaining))
}

fn push_active_capture_exclusion(query: &mut QueryBuilder<Sqlite>) {
    query.push(
        " AND NOT (
           dirty.table_name = 'transcripts'
           AND EXISTS (
             SELECT 1
             FROM (
               SELECT
                 id,
                 CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END AS marker_json
               FROM app_settings
               WHERE id GLOB 'capture_lifecycle_pending:*'
             ) AS capture
             WHERE ",
    );
    query.push(ACTIVE_CAPTURE_MARKER_PREDICATE);
    query.push(
        "
               AND json_extract(capture.marker_json, '$.transcriptId') = dirty.row_id
           )
         )",
    );
}

fn check_e2ee_cancellation(is_cancelled: &(impl Fn() -> bool + Sync)) -> E2eeReplicaResult<()> {
    if is_cancelled() {
        Err(E2eeReplicaError::Cancelled)
    } else {
        Ok(())
    }
}

async fn active_capture_marker_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    transcript_id: &str,
) -> E2eeReplicaResult<bool> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT EXISTS (
           SELECT 1
           FROM (
             SELECT
               id,
               CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END AS marker_json
             FROM app_settings
             WHERE id GLOB 'capture_lifecycle_pending:*'
           ) AS capture
           WHERE ",
    );
    query.push(ACTIVE_CAPTURE_MARKER_PREDICATE);
    query.push(" AND json_extract(capture.marker_json, '$.transcriptId') = ");
    query.push_bind(transcript_id);
    query.push(")");
    Ok(query
        .build_query_scalar()
        .fetch_one(&mut **transaction)
        .await?)
}

async fn yield_once() {
    let mut yielded = false;
    std::future::poll_fn(|context| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

#[cfg(test)]
async fn prepare_dirty_row(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    writer_id: &str,
    dirty: DirtyRow,
) -> E2eeReplicaResult<PreparedDirtyRow> {
    prepare_dirty_row_cancellable(pool, key, writer_id, dirty, &|| false).await
}

async fn prepare_dirty_row_cancellable(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    writer_id: &str,
    dirty: DirtyRow,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<PreparedDirtyRow> {
    check_e2ee_cancellation(is_cancelled)?;
    if !E2EE_DOMAIN_TABLES.contains(&dirty.table_name.as_str()) {
        return Err(E2eeReplicaError::InvalidField);
    }
    if dirty.row_id.is_empty() || dirty.generation <= 0 {
        return Err(E2eeReplicaError::InvalidRow);
    }

    let states = load_row_local_states_from_pool(
        pool,
        &dirty.workspace_id,
        &dirty.table_name,
        &dirty.row_id,
    )
    .await?;
    check_e2ee_cancellation(is_cancelled)?;
    let sql = format!(
        "SELECT * FROM {} WHERE id = ? AND workspace_id = ? LIMIT 1",
        dirty.table_name
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(&dirty.row_id)
        .bind(&dirty.workspace_id)
        .fetch_optional(pool)
        .await?;
    check_e2ee_cancellation(is_cancelled)?;
    let manifest_id = key.blind_field_id(&dirty.table_name, &dirty.row_id, ROW_MANIFEST_FIELD);
    let tombstone_tag = key.value_tag(
        &dirty.table_name,
        &dirty.row_id,
        ROW_MANIFEST_FIELD,
        true,
        &Value::Null,
    );
    let recreating = row.is_some()
        && states
            .get(&manifest_id)
            .is_some_and(|state| state.value_tag == tombstone_tag);
    let mut values = Vec::new();
    let mut record_ids = vec![manifest_id.clone()];
    if let Some(row) = row.as_ref() {
        for (index, column) in row.columns().iter().enumerate() {
            check_e2ee_cancellation(is_cancelled)?;
            let field_name = column.name();
            if matches!(field_name, "id" | "workspace_id") {
                continue;
            }
            values.push((field_name.to_string(), sqlite_value(row, index)?));
            record_ids.push(key.blind_field_id(&dirty.table_name, &dirty.row_id, field_name));
        }
    }
    check_e2ee_cancellation(is_cancelled)?;
    let witness_versions = load_witness_versions(pool, &dirty.workspace_id, &record_ids).await?;
    check_e2ee_cancellation(is_cancelled)?;
    let mut fields = Vec::new();

    if row.is_some() {
        for (field_name, value) in values {
            check_e2ee_cancellation(is_cancelled)?;
            let record_id = key.blind_field_id(&dirty.table_name, &dirty.row_id, &field_name);
            if let Some(field) = prepare_encrypted_field(
                key,
                &states,
                &dirty.workspace_id,
                &dirty.table_name,
                &dirty.row_id,
                &field_name,
                writer_id,
                witness_versions.get(&record_id),
                recreating,
                false,
                value,
            )? {
                fields.push(field);
            }
            check_e2ee_cancellation(is_cancelled)?;
            yield_once().await;
        }
        let row_changed = recreating || !fields.is_empty();
        check_e2ee_cancellation(is_cancelled)?;
        if let Some(field) = prepare_encrypted_field(
            key,
            &states,
            &dirty.workspace_id,
            &dirty.table_name,
            &dirty.row_id,
            ROW_MANIFEST_FIELD,
            writer_id,
            witness_versions.get(&manifest_id),
            row_changed,
            false,
            json!(true),
        )? {
            fields.insert(0, field);
        }
        check_e2ee_cancellation(is_cancelled)?;
    } else if states.contains_key(&manifest_id)
        && let Some(field) = prepare_encrypted_field(
            key,
            &states,
            &dirty.workspace_id,
            &dirty.table_name,
            &dirty.row_id,
            ROW_MANIFEST_FIELD,
            writer_id,
            witness_versions.get(&manifest_id),
            false,
            true,
            Value::Null,
        )?
    {
        fields.push(field);
    }

    check_e2ee_cancellation(is_cancelled)?;
    Ok(PreparedDirtyRow { dirty, fields })
}

async fn load_witness_versions(
    pool: &SqlitePool,
    workspace_id: &str,
    record_ids: &[String],
) -> E2eeReplicaResult<HashMap<String, WitnessVersion>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT record_id, revision, writer_id, payload_hash
         FROM e2ee_witness_records
         WHERE workspace_id = ",
    );
    query.push_bind(workspace_id);
    query.push(" AND record_id IN (");
    let mut separated = query.separated(", ");
    for record_id in record_ids {
        separated.push_bind(record_id);
    }
    separated.push_unseparated(")");
    let versions: Vec<(String, i64, String, String)> =
        query.build_query_as().fetch_all(pool).await?;
    Ok(versions
        .into_iter()
        .map(|(record_id, revision, writer_id, payload_hash)| {
            (
                record_id,
                WitnessVersion {
                    revision,
                    writer_id,
                    payload_hash,
                },
            )
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn prepare_encrypted_field(
    key: &WorkspaceKey,
    states: &HashMap<String, LocalState>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
    field: &str,
    writer_id: &str,
    witness_version: Option<&WitnessVersion>,
    force: bool,
    deleted: bool,
    value: Value,
) -> E2eeReplicaResult<Option<PreparedEncryptedField>> {
    let record_id = key.blind_field_id(table, row_id, field);
    let value_tag = key.value_tag(table, row_id, field, deleted, &value);
    let previous = states.get(&record_id);
    if !force && previous.is_some_and(|state| state.value_tag == value_tag) {
        return Ok(None);
    }

    let revision = previous
        .map(|state| state.revision)
        .unwrap_or(0)
        .max(witness_version.map(|version| version.revision).unwrap_or(0))
        .checked_add(1)
        .ok_or(E2eeReplicaError::InvalidRow)?;
    let revision = u64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    let sealed = key.seal_field(
        workspace_id,
        table,
        row_id,
        field,
        writer_id,
        revision,
        deleted,
        value,
    )?;
    let payload_hash = anlg_e2ee::payload_hash(&sealed.payload);
    Ok(Some(PreparedEncryptedField {
        previous_payload_hash: previous.map(|state| state.payload_hash.clone()),
        expected_witness_version: witness_version.cloned(),
        state: LocalState {
            record_id: sealed.record_id,
            workspace_id: workspace_id.to_string(),
            table_name: table.to_string(),
            row_id: row_id.to_string(),
            field_name: field.to_string(),
            revision: i64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?,
            writer_id: writer_id.to_string(),
            value_tag,
            payload_hash,
            payload: sealed.payload,
        },
    }))
}

#[cfg(test)]
async fn persist_prepared_dirty_row(
    pool: &SqlitePool,
    prepared: PreparedDirtyRow,
) -> E2eeReplicaResult<u64> {
    persist_prepared_dirty_row_inner(pool, prepared, false).await
}

#[cfg(test)]
async fn persist_prepared_dirty_row_inner(
    pool: &SqlitePool,
    prepared: PreparedDirtyRow,
    defer_active_captures: bool,
) -> E2eeReplicaResult<u64> {
    persist_prepared_dirty_row_cancellable(pool, prepared, defer_active_captures, &|| false).await
}

async fn persist_prepared_dirty_row_cancellable(
    pool: &SqlitePool,
    prepared: PreparedDirtyRow,
    defer_active_captures: bool,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<u64> {
    check_e2ee_cancellation(is_cancelled)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    let generation: Option<i64> = sqlx::query_scalar(
        "SELECT generation
         FROM e2ee_dirty_rows
         WHERE workspace_id = ? AND table_name = ? AND row_id = ?",
    )
    .bind(&prepared.dirty.workspace_id)
    .bind(&prepared.dirty.table_name)
    .bind(&prepared.dirty.row_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    if generation != Some(prepared.dirty.generation) {
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
        return Ok(0);
    }
    let active_capture = if defer_active_captures && prepared.dirty.table_name == "transcripts" {
        active_capture_marker_exists(&mut transaction, &prepared.dirty.row_id).await?
    } else {
        false
    };
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    if active_capture {
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
        return Ok(0);
    }

    for field in &prepared.fields {
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let current_payload_hash: Option<String> = sqlx::query_scalar(
            "SELECT payload_hash
             FROM e2ee_local_state
             WHERE record_id = ?",
        )
        .bind(&field.state.record_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        if current_payload_hash != field.previous_payload_hash {
            transaction.commit().await?;
            check_e2ee_cancellation(is_cancelled)?;
            return Ok(0);
        }
        let current_witness_version: Option<(i64, String, String)> = sqlx::query_as(
            "SELECT revision, writer_id, payload_hash
             FROM e2ee_witness_records
             WHERE workspace_id = ? AND record_id = ?",
        )
        .bind(&field.state.workspace_id)
        .bind(&field.state.record_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let current_witness_version =
            current_witness_version.map(|(revision, writer_id, payload_hash)| WitnessVersion {
                revision,
                writer_id,
                payload_hash,
            });
        if current_witness_version != field.expected_witness_version {
            transaction.commit().await?;
            check_e2ee_cancellation(is_cancelled)?;
            return Ok(0);
        }
    }

    for field in &prepared.fields {
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let result = sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               payload = excluded.payload,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE e2ee_records.workspace_id = excluded.workspace_id
               AND e2ee_records.payload != excluded.payload",
        )
        .bind(&field.state.record_id)
        .bind(&field.state.workspace_id)
        .bind(&field.state.payload)
        .execute(&mut *transaction)
        .await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        if result.rows_affected() == 0 {
            let current: Option<(String, String)> = sqlx::query_as(
                "SELECT workspace_id, payload
                 FROM e2ee_records
                 WHERE id = ?",
            )
            .bind(&field.state.record_id)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Err(error) = check_e2ee_cancellation(is_cancelled) {
                transaction.rollback().await?;
                return Err(error);
            }
            if !current.is_some_and(|(workspace_id, payload)| {
                workspace_id == field.state.workspace_id && payload == field.state.payload
            }) {
                return Err(E2eeReplicaError::RollbackDetected);
            }
        }
        upsert_local_state(&mut transaction, &field.state).await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
    }

    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    let deleted = sqlx::query(
        "DELETE FROM e2ee_dirty_rows
         WHERE workspace_id = ? AND table_name = ? AND row_id = ? AND generation = ?",
    )
    .bind(&prepared.dirty.workspace_id)
    .bind(&prepared.dirty.table_name)
    .bind(&prepared.dirty.row_id)
    .bind(prepared.dirty.generation)
    .execute(&mut *transaction)
    .await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    if deleted.rows_affected() != 1 {
        return Err(E2eeReplicaError::InvalidRow);
    }
    let encrypted_fields =
        u64::try_from(prepared.fields.len()).map_err(|_| E2eeReplicaError::InvalidRow)?;
    transaction.commit().await?;
    check_e2ee_cancellation(is_cancelled)?;
    Ok(encrypted_fields)
}

async fn load_encrypted_row_group(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    row: (&str, &str, &str),
    columns: &HashSet<String>,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Vec<EncryptedRecord>> {
    let (workspace_id, table, row_id) = row;
    let mut record_ids = columns
        .iter()
        .filter(|field| !matches!(field.as_str(), "id" | "workspace_id"))
        .map(|field| key.blind_field_id(table, row_id, field))
        .collect::<Vec<_>>();
    record_ids.push(key.blind_field_id(table, row_id, ROW_MANIFEST_FIELD));
    record_ids.sort_unstable();

    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT
           replica.id,
           replica.workspace_id,
           LENGTH(CAST(replica.id AS BLOB))
             + LENGTH(CAST(replica.workspace_id AS BLOB))
             + LENGTH(CAST(replica.payload AS BLOB))
             + 256 AS record_bytes,
           EXISTS(
             SELECT 1
             FROM e2ee_witness_records AS witness
             WHERE witness.workspace_id = replica.workspace_id
               AND witness.record_id = replica.id
               AND witness.payload = replica.payload
           ) AS witnessed,
           1 AS changed
         FROM e2ee_records AS replica
         WHERE replica.workspace_id = ",
    );
    query.push_bind(workspace_id);
    query.push(" AND replica.id IN (");
    let mut separated = query.separated(", ");
    for record_id in &record_ids {
        separated.push_bind(record_id);
    }
    separated.push_unseparated(") ORDER BY replica.id");
    let metadata: Vec<EncryptedRecordMetadata> = query.build_query_as().fetch_all(pool).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    let row_bytes = metadata.iter().try_fold(0_usize, |total, record| {
        let record_bytes =
            usize::try_from(record.record_bytes).map_err(|_| E2eeReplicaError::InvalidRow)?;
        total
            .checked_add(record_bytes)
            .ok_or(E2eeReplicaError::InvalidRow)
    })?;
    if row_bytes > max_bytes {
        return Err(E2eeReplicaError::ReplicaApplyTooLarge);
    }
    check_e2ee_apply_cancellation(is_cancelled)?;
    let records = load_encrypted_records_by_id(pool, &record_ids).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    Ok(records
        .into_iter()
        .filter(|record| record.workspace_id == workspace_id)
        .collect())
}

async fn replica_records_still_current(
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

async fn load_row_local_states_from_pool(
    pool: &SqlitePool,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<HashMap<String, LocalState>> {
    let states: Vec<LocalState> = sqlx::query_as(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision,
                writer_id, value_tag, payload_hash, payload
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

async fn load_row_local_states(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<HashMap<String, LocalState>> {
    let states: Vec<LocalState> = sqlx::query_as(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision,
                writer_id, value_tag, payload_hash, payload
         FROM e2ee_local_state
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

async fn insert_apply_guard(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
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

async fn clear_stale_apply_guards(pool: &SqlitePool) -> E2eeReplicaResult<()> {
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

async fn remove_apply_guard(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    table: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
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

async fn upsert_local_state(
    transaction: &mut Transaction<'_, Sqlite>,
    state: &LocalState,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "INSERT INTO e2ee_local_state (
           record_id, workspace_id, table_name, row_id, field_name, revision,
           writer_id, value_tag, payload_hash, payload
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(record_id) DO UPDATE SET
           workspace_id = excluded.workspace_id,
           table_name = excluded.table_name,
           row_id = excluded.row_id,
           field_name = excluded.field_name,
           revision = excluded.revision,
           writer_id = excluded.writer_id,
           value_tag = excluded.value_tag,
           payload_hash = excluded.payload_hash,
           payload = excluded.payload,
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
    .bind(&state.payload)
    .execute(&mut **transaction)
    .await?;
    reconcile_e2ee_witness_pending(transaction, &state.record_id).await?;
    Ok(())
}

async fn reconcile_e2ee_witness_pending(
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

async fn load_or_create_writer_id(
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

fn record_version_order(
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

async fn restore_local_payload(
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
    Ok(())
}

async fn table_columns(pool: &SqlitePool, table: &str) -> E2eeReplicaResult<HashSet<String>> {
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

async fn row_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ? AND workspace_id = ?)");
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?)
}

async fn insert_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let sql =
        format!("INSERT INTO {table} (id, workspace_id) VALUES (?, ?) ON CONFLICT(id) DO NOTHING");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn delete_row(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
) -> E2eeReplicaResult<()> {
    let sql = format!("DELETE FROM {table} WHERE id = ? AND workspace_id = ?");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn read_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
) -> E2eeReplicaResult<Option<Value>> {
    let sql = format!("SELECT {field} FROM {table} WHERE id = ? AND workspace_id = ? LIMIT 1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row_id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;
    row.as_ref().map(|row| sqlite_value(row, 0)).transpose()
}

async fn update_field(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    workspace_id: &str,
    row_id: &str,
    field: &str,
    value: &Value,
) -> E2eeReplicaResult<()> {
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
async fn row_changed_since_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    key: &WorkspaceKey,
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
            let current_tag = key.value_tag(table, row_id, ROW_MANIFEST_FIELD, !row_exists, &value);
            if current_tag != state.value_tag {
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
        let current_tag = key.value_tag(table, row_id, &state.field_name, false, &value);
        if current_tag != state.value_tag {
            return Ok(true);
        }
    }
    Ok(row_exists && !found_manifest)
}

fn sqlite_value(row: &SqliteRow, index: usize) -> E2eeReplicaResult<Value> {
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

#[cfg(test)]
mod tests;
