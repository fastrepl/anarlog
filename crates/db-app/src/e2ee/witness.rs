use std::collections::HashMap;

use anlg_e2ee::{OpenedField, WorkspaceKey};
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};

use super::replica_storage::{prune_ciphertext_archive, retain_ciphertext};
use super::{
    E2EE_DOMAIN_TABLES, E2eeReplicaError, E2eeReplicaResult, LocalState, check_e2ee_cancellation,
    reconcile_e2ee_witness_pending, yield_once,
};

const E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT: i64 = 128;
pub(super) const PENDING_E2EE_WITNESS_UPLOADS_SQL: &str = "
  SELECT
    pending.record_id,
    LENGTH(CAST(COALESCE(local.payload, '') AS BLOB))
      + LENGTH(CAST(local.record_id AS BLOB))
      + LENGTH(CAST(local.payload_hash AS BLOB))
      + 256
  FROM e2ee_witness_pending AS pending
  INDEXED BY idx_e2ee_witness_pending_workspace_record
  CROSS JOIN e2ee_local_state_resolved AS local
  WHERE pending.workspace_id = ?
    AND local.record_id = pending.record_id
    AND local.workspace_id = pending.workspace_id
  ORDER BY pending.record_id
  LIMIT ?";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct E2eeWitnessRepairOutcome {
    pub repaired_records: u64,
    pub remaining: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct E2eeWitnessUpload {
    pub record_id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub writer_id: String,
    pub payload_hash: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct E2eeWitnessEvent {
    pub sequence: u64,
    pub record_id: String,
    pub workspace_id: String,
    pub payload_hash: String,
    pub payload: String,
}

#[derive(Clone, sqlx::FromRow)]
struct WitnessRecord {
    workspace_id: String,
    record_id: String,
    revision: i64,
    writer_id: String,
    payload_hash: String,
    payload: String,
    sequence: i64,
}

#[derive(sqlx::FromRow)]
struct WitnessRepairRow {
    workspace_id: String,
    record_id: String,
    revision: i64,
    writer_id: String,
    payload_hash: String,
    payload: String,
    sequence: i64,
    generation: i64,
}

pub async fn e2ee_witness_cursor(pool: &SqlitePool, workspace_id: &str) -> E2eeReplicaResult<u64> {
    let sequence: Option<i64> =
        sqlx::query_scalar("SELECT last_sequence FROM e2ee_witness_state WHERE workspace_id = ?")
            .bind(workspace_id)
            .fetch_optional(pool)
            .await?;
    u64::try_from(sequence.unwrap_or(0)).map_err(|_| E2eeReplicaError::RollbackDetected)
}

pub async fn has_e2ee_local_state(
    pool: &SqlitePool,
    workspace_id: &str,
) -> E2eeReplicaResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM e2ee_local_state WHERE workspace_id = ?
         )",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?)
}

pub async fn pending_e2ee_witness_uploads(
    pool: &SqlitePool,
    workspace_id: &str,
    key: &WorkspaceKey,
    max_records: usize,
    max_bytes: usize,
) -> E2eeReplicaResult<Vec<E2eeWitnessUpload>> {
    pending_e2ee_witness_uploads_inner(pool, workspace_id, key, max_records, max_bytes, &|| false)
        .await
}

pub async fn pending_e2ee_witness_uploads_cancellable(
    pool: &SqlitePool,
    workspace_id: &str,
    key: &WorkspaceKey,
    max_records: usize,
    max_bytes: usize,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<Vec<E2eeWitnessUpload>> {
    pending_e2ee_witness_uploads_inner(
        pool,
        workspace_id,
        key,
        max_records,
        max_bytes,
        &is_cancelled,
    )
    .await
}

async fn pending_e2ee_witness_uploads_inner(
    pool: &SqlitePool,
    workspace_id: &str,
    key: &WorkspaceKey,
    max_records: usize,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Vec<E2eeWitnessUpload>> {
    check_e2ee_cancellation(is_cancelled)?;
    if max_records == 0 || max_bytes == 0 {
        return Ok(Vec::new());
    }
    let max_records =
        i64::try_from(max_records).map_err(|_| E2eeReplicaError::WitnessUploadTooLarge)?;

    let mut transaction = pool.begin().await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    let pending: Vec<(String, i64)> = sqlx::query_as(PENDING_E2EE_WITNESS_UPLOADS_SQL)
        .bind(workspace_id)
        .bind(max_records)
        .fetch_all(&mut *transaction)
        .await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    if pending.is_empty() {
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
        return Ok(Vec::new());
    }

    let mut selected_ids = Vec::with_capacity(pending.len());
    let mut selected_bytes = 0_usize;
    for (record_id, upload_bytes) in pending {
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let upload_bytes =
            usize::try_from(upload_bytes).map_err(|_| E2eeReplicaError::InvalidRow)?;
        if selected_bytes.saturating_add(upload_bytes) > max_bytes {
            if selected_ids.is_empty() {
                return Err(E2eeReplicaError::WitnessUploadTooLarge);
            }
            break;
        }
        selected_ids.push(record_id);
        selected_bytes = selected_bytes.saturating_add(upload_bytes);
    }

    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT record_id, workspace_id, table_name, row_id, field_name, revision,
                writer_id, value_tag, payload_hash, payload
         FROM e2ee_local_state_resolved
         WHERE workspace_id = ",
    );
    query.push_bind(workspace_id);
    query.push(" AND record_id IN (");
    {
        let mut separated = query.separated(", ");
        for record_id in &selected_ids {
            separated.push_bind(record_id);
        }
    }
    query.push(") ORDER BY record_id");
    let states: Vec<LocalState> = query.build_query_as().fetch_all(&mut *transaction).await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    if states.len() != selected_ids.len()
        || !states
            .iter()
            .map(|state| state.record_id.as_str())
            .eq(selected_ids.iter().map(String::as_str))
    {
        return Err(E2eeReplicaError::InvalidRow);
    }
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    transaction.commit().await?;
    check_e2ee_cancellation(is_cancelled)?;

    let loaded_bytes = states.iter().fold(0_usize, |total, state| {
        total
            .saturating_add(state.payload.len())
            .saturating_add(state.record_id.len())
            .saturating_add(state.payload_hash.len())
            .saturating_add(256)
    });
    if loaded_bytes > max_bytes {
        return Err(E2eeReplicaError::WitnessUploadTooLarge);
    }

    let mut uploads = Vec::with_capacity(states.len());
    for state in states {
        check_e2ee_cancellation(is_cancelled)?;
        validate_witness_payload(
            key,
            &state.workspace_id,
            &state.record_id,
            &state.payload_hash,
            &state.payload,
            state.revision,
            &state.writer_id,
        )?;
        check_e2ee_cancellation(is_cancelled)?;
        uploads.push(E2eeWitnessUpload {
            record_id: state.record_id,
            workspace_id: state.workspace_id,
            revision: u64::try_from(state.revision).map_err(|_| E2eeReplicaError::InvalidRow)?,
            writer_id: state.writer_id,
            payload_hash: state.payload_hash,
            payload: state.payload,
        });
        yield_once().await;
    }
    Ok(uploads)
}

pub async fn acknowledge_e2ee_witness_uploads(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    uploads: &[E2eeWitnessUpload],
) -> E2eeReplicaResult<()> {
    acknowledge_e2ee_witness_uploads_inner(pool, key, uploads, &|| false).await
}

pub async fn acknowledge_e2ee_witness_uploads_cancellable(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    uploads: &[E2eeWitnessUpload],
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<()> {
    acknowledge_e2ee_witness_uploads_inner(pool, key, uploads, &is_cancelled).await
}

async fn acknowledge_e2ee_witness_uploads_inner(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    uploads: &[E2eeWitnessUpload],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    let mut records = Vec::with_capacity(uploads.len());
    for upload in uploads {
        check_e2ee_cancellation(is_cancelled)?;
        let revision = i64::try_from(upload.revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
        validate_witness_payload(
            key,
            &upload.workspace_id,
            &upload.record_id,
            &upload.payload_hash,
            &upload.payload,
            revision,
            &upload.writer_id,
        )?;
        check_e2ee_cancellation(is_cancelled)?;
        records.push(WitnessRecord {
            workspace_id: upload.workspace_id.clone(),
            record_id: upload.record_id.clone(),
            revision,
            writer_id: upload.writer_id.clone(),
            payload_hash: upload.payload_hash.clone(),
            payload: upload.payload.clone(),
            sequence: 0,
        });
        yield_once().await;
    }
    for record in records {
        check_e2ee_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        upsert_witness_record(&mut transaction, &record).await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
    }
    Ok(())
}

pub async fn merge_e2ee_witness_events(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    workspace_id: &str,
    events: &[E2eeWitnessEvent],
) -> E2eeReplicaResult<()> {
    merge_e2ee_witness_events_inner(pool, key, workspace_id, events, &|| false).await
}

pub async fn merge_e2ee_witness_events_cancellable(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    workspace_id: &str,
    events: &[E2eeWitnessEvent],
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<()> {
    merge_e2ee_witness_events_inner(pool, key, workspace_id, events, &is_cancelled).await
}

async fn merge_e2ee_witness_events_inner(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    workspace_id: &str,
    events: &[E2eeWitnessEvent],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    let mut previous_sequence = 0;
    let mut records = Vec::with_capacity(events.len());
    for event in events {
        check_e2ee_cancellation(is_cancelled)?;
        if event.workspace_id != workspace_id
            || event.sequence == 0
            || event.sequence <= previous_sequence
        {
            return Err(E2eeReplicaError::InvalidRow);
        }
        let sequence = i64::try_from(event.sequence).map_err(|_| E2eeReplicaError::InvalidRow)?;
        let field = key.open_field(workspace_id, &event.record_id, &event.payload)?;
        check_e2ee_cancellation(is_cancelled)?;
        let revision = i64::try_from(field.revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
        validate_opened_witness_field(
            &field,
            &event.payload_hash,
            &event.payload,
            revision,
            &field.writer_id,
        )?;
        records.push(WitnessRecord {
            workspace_id: workspace_id.to_string(),
            record_id: event.record_id.clone(),
            revision,
            writer_id: field.writer_id,
            payload_hash: event.payload_hash.clone(),
            payload: event.payload.clone(),
            sequence,
        });
        previous_sequence = event.sequence;
        yield_once().await;
    }
    for record in records {
        check_e2ee_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        upsert_witness_record(&mut transaction, &record).await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
    }
    Ok(())
}

pub async fn advance_e2ee_witness_cursor(
    pool: &SqlitePool,
    workspace_id: &str,
    through_sequence: u64,
) -> E2eeReplicaResult<()> {
    let through_sequence =
        i64::try_from(through_sequence).map_err(|_| E2eeReplicaError::InvalidRow)?;
    let current: Option<i64> =
        sqlx::query_scalar("SELECT last_sequence FROM e2ee_witness_state WHERE workspace_id = ?")
            .bind(workspace_id)
            .fetch_optional(pool)
            .await?;
    if current.is_some_and(|current| through_sequence < current) {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    sqlx::query(
        "INSERT INTO e2ee_witness_state (workspace_id, last_sequence)
         VALUES (?, ?)
         ON CONFLICT(workspace_id) DO UPDATE SET
           last_sequence = excluded.last_sequence,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE excluded.last_sequence >= e2ee_witness_state.last_sequence",
    )
    .bind(workspace_id)
    .bind(through_sequence)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn repair_e2ee_replica_from_witness_bounded(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    max_records: i64,
    max_bytes: usize,
) -> E2eeReplicaResult<E2eeWitnessRepairOutcome> {
    repair_e2ee_replica_from_witness_bounded_inner(
        pool,
        keys,
        include_missing,
        max_records,
        max_bytes,
        &|| false,
    )
    .await
}

pub async fn repair_e2ee_replica_from_witness_bounded_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    max_records: i64,
    max_bytes: usize,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeWitnessRepairOutcome> {
    repair_e2ee_replica_from_witness_bounded_inner(
        pool,
        keys,
        include_missing,
        max_records,
        max_bytes,
        &is_cancelled,
    )
    .await
}

async fn repair_e2ee_replica_from_witness_bounded_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    max_records: i64,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<E2eeWitnessRepairOutcome> {
    check_e2ee_cancellation(is_cancelled)?;
    if keys.is_empty() || max_records <= 0 || max_bytes == 0 {
        let outcome = E2eeWitnessRepairOutcome {
            repaired_records: 0,
            remaining: has_pending_e2ee_witness_repairs_inner(
                pool,
                keys,
                include_missing,
                is_cancelled,
            )
            .await?,
        };
        check_e2ee_cancellation(is_cancelled)?;
        return Ok(outcome);
    }

    let records = load_bounded_e2ee_witness_repairs(
        pool,
        keys,
        include_missing,
        max_records,
        max_bytes,
        is_cancelled,
    )
    .await?;
    check_e2ee_cancellation(is_cancelled)?;
    let repaired_records = persist_e2ee_witness_repairs(pool, keys, &records, is_cancelled).await?;
    check_e2ee_cancellation(is_cancelled)?;
    Ok(E2eeWitnessRepairOutcome {
        repaired_records,
        remaining: has_pending_e2ee_witness_repairs_inner(
            pool,
            keys,
            include_missing,
            is_cancelled,
        )
        .await?,
    })
}

pub async fn has_pending_e2ee_witness_repairs(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_witness_repairs_inner(pool, keys, include_missing, &|| false).await
}

pub async fn has_pending_e2ee_witness_repairs_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_witness_repairs_inner(pool, keys, include_missing, &is_cancelled).await
}

async fn has_pending_e2ee_witness_repairs_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<bool> {
    check_e2ee_cancellation(is_cancelled)?;
    if keys.is_empty() {
        return Ok(false);
    }
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT EXISTS(
           SELECT 1
           FROM e2ee_witness_repair_pending AS pending
           INNER JOIN e2ee_witness_records AS witness
             ON witness.workspace_id = pending.workspace_id
            AND witness.record_id = pending.record_id",
    );
    if !include_missing {
        query.push(
            " INNER JOIN e2ee_records AS replica
                ON replica.id = pending.record_id
               AND replica.workspace_id = pending.workspace_id",
        );
    }
    query.push(" WHERE pending.workspace_id IN (");
    let mut separated = query.separated(", ");
    for workspace_id in workspace_ids {
        separated.push_bind(workspace_id);
    }
    separated.push_unseparated(") LIMIT 1)");
    let pending = query.build_query_scalar().fetch_one(pool).await?;
    check_e2ee_cancellation(is_cancelled)?;
    Ok(pending)
}

async fn load_bounded_e2ee_witness_repairs(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    max_records: i64,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Vec<(WitnessRecord, i64)>> {
    check_e2ee_cancellation(is_cancelled)?;
    if keys.is_empty() || max_records <= 0 {
        return Ok(Vec::new());
    }
    let max_records =
        usize::try_from(max_records).map_err(|_| E2eeReplicaError::WitnessRepairTooLarge)?;
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut selected_keys = Vec::with_capacity(
        max_records.min(
            usize::try_from(E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT)
                .map_err(|_| E2eeReplicaError::InvalidRow)?,
        ),
    );
    let mut selected_bytes = 0_usize;
    let mut metadata_query = QueryBuilder::<Sqlite>::new(
        "WITH page AS MATERIALIZED (
           SELECT pending.workspace_id, pending.record_id, pending.generation
           FROM e2ee_witness_repair_pending AS pending
           INDEXED BY idx_e2ee_witness_repair_pending_workspace_record
           WHERE pending.workspace_id IN (",
    );
    {
        let mut separated = metadata_query.separated(", ");
        for workspace_id in &workspace_ids {
            separated.push_bind(workspace_id);
        }
    }
    metadata_query
        .push(") ORDER BY pending.workspace_id, pending.record_id LIMIT ")
        .push_bind(E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT)
        .push(
            ")
         SELECT
           page.workspace_id,
           page.record_id,
           page.generation,
           COALESCE(
             LENGTH(CAST(witness.workspace_id AS BLOB))
               + LENGTH(CAST(witness.record_id AS BLOB))
               + LENGTH(CAST(witness.payload AS BLOB))
               + 256,
             0
           ) AS record_bytes,
           witness.record_id IS NOT NULL
             AND (
               (replica.id IS NULL AND ",
        )
        .push_bind(include_missing)
        .push(
            ")
               OR (
                 replica.id IS NOT NULL
                 AND (
                   replica.workspace_id != witness.workspace_id
                   OR replica.payload_hash != witness.payload_hash
                 )
               )
             ) AS needs_repair
         FROM page
         LEFT JOIN e2ee_witness_records_resolved AS witness
           ON witness.workspace_id = page.workspace_id
          AND witness.record_id = page.record_id
         LEFT JOIN e2ee_records AS replica
           ON replica.id = page.record_id
         ORDER BY page.workspace_id, page.record_id",
        );
    let metadata: Vec<(String, String, i64, i64, bool)> =
        metadata_query.build_query_as().fetch_all(pool).await?;
    check_e2ee_cancellation(is_cancelled)?;
    let mut reconciled = Vec::new();
    for (workspace_id, record_id, generation, record_bytes, needs_repair) in metadata {
        if !needs_repair {
            reconciled.push((record_id, generation));
            continue;
        }
        if selected_keys.len() >= max_records {
            break;
        }
        let record_bytes =
            usize::try_from(record_bytes).map_err(|_| E2eeReplicaError::InvalidRow)?;
        if selected_bytes.saturating_add(record_bytes) > max_bytes {
            if selected_keys.is_empty() {
                return Err(E2eeReplicaError::WitnessRepairTooLarge);
            }
            break;
        }
        selected_bytes = selected_bytes.saturating_add(record_bytes);
        selected_keys.push((workspace_id, record_id, generation));
    }
    delete_reconciled_witness_repairs(pool, &reconciled, is_cancelled).await?;
    finish_e2ee_witness_repair_selection(pool, &selected_keys, is_cancelled).await
}

async fn finish_e2ee_witness_repair_selection(
    pool: &SqlitePool,
    selected_keys: &[(String, String, i64)],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Vec<(WitnessRecord, i64)>> {
    check_e2ee_cancellation(is_cancelled)?;
    if selected_keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut query =
        QueryBuilder::<Sqlite>::new("WITH wanted(workspace_id, record_id, generation) AS (");
    query.push_values(selected_keys, |mut row, key| {
        row.push_bind(&key.0).push_bind(&key.1).push_bind(key.2);
    });
    query.push(
        ")
         SELECT
           witness.workspace_id,
           witness.record_id,
           witness.revision,
           witness.writer_id,
           witness.payload_hash,
           witness.payload,
           witness.sequence,
           wanted.generation
         FROM wanted
         INNER JOIN e2ee_witness_records_resolved AS witness
           ON witness.workspace_id = wanted.workspace_id
          AND witness.record_id = wanted.record_id
         ORDER BY witness.workspace_id, witness.record_id",
    );
    let records: Vec<WitnessRepairRow> = query.build_query_as().fetch_all(pool).await?;
    check_e2ee_cancellation(is_cancelled)?;
    if records.len() != selected_keys.len()
        || !records.iter().zip(selected_keys).all(
            |(record, (workspace_id, record_id, generation))| {
                &record.workspace_id == workspace_id
                    && &record.record_id == record_id
                    && record.generation == *generation
            },
        )
    {
        return Err(E2eeReplicaError::InvalidRow);
    }
    Ok(records
        .into_iter()
        .map(|record| {
            (
                WitnessRecord {
                    workspace_id: record.workspace_id,
                    record_id: record.record_id,
                    revision: record.revision,
                    writer_id: record.writer_id,
                    payload_hash: record.payload_hash,
                    payload: record.payload,
                    sequence: record.sequence,
                },
                record.generation,
            )
        })
        .collect())
}

async fn persist_e2ee_witness_repairs(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    records: &[(WitnessRecord, i64)],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<u64> {
    let mut repaired_records = 0_u64;
    for (record, pending_generation) in records {
        check_e2ee_cancellation(is_cancelled)?;
        let key = keys
            .get(&record.workspace_id)
            .ok_or(E2eeReplicaError::InvalidRow)?;
        validate_witness_payload(
            key,
            &record.workspace_id,
            &record.record_id,
            &record.payload_hash,
            &record.payload,
            record.revision,
            &record.writer_id,
        )?;
        check_e2ee_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let current_witness: Option<(i64, String, String, String, i64)> = sqlx::query_as(
            "SELECT revision, writer_id, payload_hash, payload, sequence
             FROM e2ee_witness_records_resolved
             WHERE workspace_id = ? AND record_id = ?",
        )
        .bind(&record.workspace_id)
        .bind(&record.record_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        let current_matches = current_witness.is_some_and(
            |(revision, writer_id, payload_hash, payload, sequence)| {
                revision == record.revision
                    && writer_id == record.writer_id
                    && payload_hash == record.payload_hash
                    && payload == record.payload
                    && sequence == record.sequence
            },
        );
        if !current_matches {
            delete_reconciled_witness_repairs_in_transaction(
                &mut transaction,
                &[(record.record_id.clone(), *pending_generation)],
            )
            .await?;
            transaction.commit().await?;
            check_e2ee_cancellation(is_cancelled)?;
            continue;
        }
        let result = sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload, payload_hash)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               payload = excluded.payload,
               payload_hash = excluded.payload_hash,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE e2ee_records.workspace_id = excluded.workspace_id
               AND e2ee_records.payload != excluded.payload",
        )
        .bind(&record.record_id)
        .bind(&record.workspace_id)
        .bind(&record.payload)
        .bind(&record.payload_hash)
        .execute(&mut *transaction)
        .await?;
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        if result.rows_affected() != 1 {
            let current: Option<(String, String)> = sqlx::query_as(
                "SELECT workspace_id, payload
                 FROM e2ee_records
                 WHERE id = ?",
            )
            .bind(&record.record_id)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Err(error) = check_e2ee_cancellation(is_cancelled) {
                transaction.rollback().await?;
                return Err(error);
            }
            if !current.is_some_and(|(workspace_id, payload)| {
                workspace_id == record.workspace_id && payload == record.payload
            }) {
                return Err(E2eeReplicaError::RollbackDetected);
            }
        } else {
            repaired_records += 1;
        }
        sqlx::query("DELETE FROM e2ee_witness_repair_pending WHERE record_id = ?")
            .bind(&record.record_id)
            .execute(&mut *transaction)
            .await?;
        prune_ciphertext_archive(&mut transaction, &record.workspace_id, &record.record_id).await?;
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
    }
    Ok(repaired_records)
}

async fn delete_reconciled_witness_repairs(
    pool: &SqlitePool,
    entries: &[(String, i64)],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    check_e2ee_cancellation(is_cancelled)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Err(error) = check_e2ee_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    delete_reconciled_witness_repairs_in_transaction(&mut transaction, entries).await?;
    transaction.commit().await?;
    check_e2ee_cancellation(is_cancelled)
}

async fn delete_reconciled_witness_repairs_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    entries: &[(String, i64)],
) -> E2eeReplicaResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        "DELETE FROM e2ee_witness_repair_pending WHERE (record_id, generation) IN (",
    );
    query.push_values(entries, |mut row, (record_id, generation)| {
        row.push_bind(record_id).push_bind(generation);
    });
    query.push(")").build().execute(&mut **transaction).await?;
    Ok(())
}

async fn upsert_witness_record(
    transaction: &mut Transaction<'_, Sqlite>,
    record: &WitnessRecord,
) -> E2eeReplicaResult<()> {
    retain_ciphertext(
        transaction,
        &record.workspace_id,
        &record.record_id,
        &record.payload_hash,
        &record.payload,
    )
    .await?;
    sqlx::query(
        "INSERT INTO e2ee_witness_records (
           workspace_id, record_id, revision, writer_id, payload_hash, sequence
         ) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, record_id) DO UPDATE SET
           revision = excluded.revision,
           writer_id = excluded.writer_id,
           payload_hash = excluded.payload_hash,
           sequence = MAX(e2ee_witness_records.sequence, excluded.sequence),
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE excluded.revision > e2ee_witness_records.revision
            OR (
              excluded.revision = e2ee_witness_records.revision
              AND excluded.writer_id > e2ee_witness_records.writer_id
            )
            OR (
              excluded.revision = e2ee_witness_records.revision
              AND excluded.writer_id = e2ee_witness_records.writer_id
              AND excluded.payload_hash > e2ee_witness_records.payload_hash
            )
            OR (
              excluded.revision = e2ee_witness_records.revision
              AND excluded.writer_id = e2ee_witness_records.writer_id
              AND excluded.payload_hash = e2ee_witness_records.payload_hash
              AND excluded.sequence > e2ee_witness_records.sequence
            )",
    )
    .bind(&record.workspace_id)
    .bind(&record.record_id)
    .bind(record.revision)
    .bind(&record.writer_id)
    .bind(&record.payload_hash)
    .bind(record.sequence)
    .execute(&mut **transaction)
    .await?;
    reconcile_e2ee_witness_pending(transaction, &record.record_id).await?;
    prune_ciphertext_archive(transaction, &record.workspace_id, &record.record_id).await?;
    Ok(())
}

fn validate_witness_payload(
    key: &WorkspaceKey,
    workspace_id: &str,
    record_id: &str,
    payload_hash: &str,
    payload: &str,
    revision: i64,
    writer_id: &str,
) -> E2eeReplicaResult<()> {
    if anlg_e2ee::payload_hash(payload) != payload_hash {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    let field = key.open_field(workspace_id, record_id, payload)?;
    validate_opened_witness_field(&field, payload_hash, payload, revision, writer_id)
}

fn validate_opened_witness_field(
    field: &OpenedField,
    payload_hash: &str,
    payload: &str,
    revision: i64,
    writer_id: &str,
) -> E2eeReplicaResult<()> {
    if !E2EE_DOMAIN_TABLES.contains(&field.table.as_str())
        || i64::try_from(field.revision).ok() != Some(revision)
        || field.writer_id != writer_id
        || anlg_e2ee::payload_hash(payload) != payload_hash
    {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    Ok(())
}

#[cfg(test)]
mod witness_cancellation_tests;
