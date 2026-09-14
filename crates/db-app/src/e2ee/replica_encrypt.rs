use std::collections::HashMap;

use anlg_e2ee::{WorkspaceKey, WorkspaceKeyring};
use serde_json::{Value, json};
use sqlx::{Column, QueryBuilder, Row, Sqlite, SqlitePool, Transaction};

use super::chunks::{
    ChunkPart, chunk_count_field, chunk_field, chunk_size_for, parse_array, parse_chunk_field,
    split_chunks,
};
use super::cooperative::yield_once;
use super::replica_storage::{
    load_or_create_writer_id, load_row_local_states_from_pool, sqlite_value, upsert_local_state,
};
use super::witness::has_pending_e2ee_witness_repairs;
use super::{
    ACTIVE_CAPTURE_MARKER_PREDICATE, DirtyRow, E2EE_DOMAIN_TABLES, E2EE_ENCRYPT_ROW_LIMIT,
    E2eeReplicaError, E2eeReplicaResult, E2eeReplicaStats, LocalState, PreparedDirtyRow,
    PreparedEncryptedField, ROW_MANIFEST_FIELD, WitnessVersion,
};

pub async fn encrypt_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, false, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, true, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_inner(pool, keys, true, &is_cancelled).await
}

async fn encrypt_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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
        stats.incomplete_chunk_columns += batch.incomplete_chunk_columns;
        stats.remaining_replica_changes = batch.remaining_replica_changes;
        if is_cancelled() {
            stats.remaining_replica_changes = true;
            break;
        }
        if !stats.remaining_replica_changes
            || (batch.incomplete_chunk_columns > 0 && batch.encrypted_fields == 0)
        {
            break;
        }
        yield_once().await;
    }
    Ok(stats)
}

pub async fn encrypt_e2ee_replica_changes_bounded(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    max_rows: i64,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, false, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_bounded_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    max_rows: i64,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, true, &|| false).await
}

pub async fn encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    max_rows: i64,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    encrypt_e2ee_replica_changes_bounded_inner(pool, keys, max_rows, true, &is_cancelled).await
}

async fn encrypt_e2ee_replica_changes_bounded_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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
        let keyring = &keys[&dirty.workspace_id];
        let Some(prepared) =
            prepare_dirty_row_cancellable(pool, keyring, &writer_id, dirty, is_cancelled).await?
        else {
            stats.incomplete_chunk_columns += 1;
            stats.remaining_replica_changes = true;
            continue;
        };
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
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_replica_changes_inner(pool, keys, false).await
}

pub async fn has_pending_e2ee_replica_changes_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<bool> {
    has_pending_e2ee_replica_changes_inner(pool, keys, true).await
}

pub async fn has_pending_e2ee_dirty_rows_deferring_active_captures(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<bool> {
    if keys.is_empty() {
        return Ok(false);
    }
    Ok(!load_dirty_rows_inner(pool, keys, 1, true).await?.is_empty())
}

async fn has_pending_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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

#[cfg(test)]
pub(super) async fn load_dirty_rows(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    max_rows: i64,
) -> E2eeReplicaResult<Vec<DirtyRow>> {
    load_dirty_rows_inner(pool, keys, max_rows, false).await
}

async fn load_dirty_rows_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
    max_rows: i64,
    defer_active_captures: bool,
) -> E2eeReplicaResult<Vec<DirtyRow>> {
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT dirty.workspace_id, dirty.table_name, dirty.row_id, dirty.generation,
                dirty.dirtied_at_ms
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

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

pub(super) async fn load_dirty_rows_page(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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

pub(super) fn check_e2ee_cancellation(
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
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

#[cfg(test)]
pub(super) async fn prepare_dirty_row(
    pool: &SqlitePool,
    key: &WorkspaceKey,
    writer_id: &str,
    dirty: DirtyRow,
) -> E2eeReplicaResult<PreparedDirtyRow> {
    prepare_dirty_row_cancellable(
        pool,
        &WorkspaceKeyring::new(key.clone()),
        writer_id,
        dirty,
        &|| false,
    )
    .await?
    .ok_or(E2eeReplicaError::InvalidRow)
}

async fn prepare_dirty_row_cancellable(
    pool: &SqlitePool,
    keyring: &WorkspaceKeyring,
    writer_id: &str,
    dirty: DirtyRow,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Option<PreparedDirtyRow>> {
    let key = keyring.active();
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
    // Rows dirtied before the write-time column existed fall back to now.
    let edited_at_ms = if dirty.dirtied_at_ms > 0 {
        dirty.dirtied_at_ms
    } else {
        current_time_ms()
    };
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
    // A restored row can contain defaults before its first chunked column is
    // applied. Metadata edits must not publish those placeholders, even when
    // unrelated conflicts keep the row outside a bounded hydration batch.
    let restored_row = states.values().any(|state| {
        state.field_name == ROW_MANIFEST_FIELD
            && keyring.generations().any(|key| {
                key.value_tag(
                    &dirty.table_name,
                    &dirty.row_id,
                    ROW_MANIFEST_FIELD,
                    false,
                    &json!(true),
                ) == state.value_tag
            })
    });
    if !recreating
        && restored_row
        && row.as_ref().is_some_and(|row| {
            row.columns().iter().any(|column| {
                let field_name = column.name();
                chunk_size_for(&dirty.table_name, field_name).is_some()
                    && !states.values().any(|state| {
                        state.field_name == field_name
                            || state.field_name == chunk_count_field(field_name)
                    })
            })
        })
    {
        return Ok(None);
    }
    let mut values = Vec::new();
    let mut retired_fields = Vec::new();
    if let Some(row) = row.as_ref() {
        for (index, column) in row.columns().iter().enumerate() {
            check_e2ee_cancellation(is_cancelled)?;
            let field_name = column.name();
            if matches!(field_name, "id" | "workspace_id") {
                continue;
            }
            let value = sqlite_value(row, index)?;
            if let Some(chunk_size) = chunk_size_for(&dirty.table_name, field_name)
                && let Some(items) = parse_array(&value)
            {
                let chunks = split_chunks(&items, chunk_size);
                for (chunk_index, chunk) in chunks.iter().enumerate() {
                    values.push((
                        chunk_field(field_name, chunk_index),
                        Value::Array(chunk.clone()),
                    ));
                }
                values.push((chunk_count_field(field_name), json!(chunks.len())));
                // Chunks the array no longer reaches are sealed as null so
                // every replica drops them.
                let previous_count = states
                    .values()
                    .filter_map(|state| {
                        match parse_chunk_field(&dirty.table_name, &state.field_name) {
                            Some((column, ChunkPart::Index(index))) if column == field_name => {
                                Some(index + 1)
                            }
                            _ => None,
                        }
                    })
                    .max()
                    .unwrap_or(0);
                for chunk_index in chunks.len()..previous_count {
                    values.push((chunk_field(field_name, chunk_index), Value::Null));
                }
                retired_fields.push(field_name.to_string());
                continue;
            }
            values.push((field_name.to_string(), value));
        }
    }
    let mut record_ids = vec![manifest_id.clone()];
    record_ids.extend(
        values.iter().map(|(field_name, _)| {
            key.blind_field_id(&dirty.table_name, &dirty.row_id, field_name)
        }),
    );
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
                Some(edited_at_ms),
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
            Some(edited_at_ms),
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
            Some(edited_at_ms),
            Value::Null,
        )?
    {
        fields.push(field);
    }

    check_e2ee_cancellation(is_cancelled)?;
    Ok(Some(PreparedDirtyRow {
        dirty,
        fields,
        retired_fields,
    }))
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
    edited_at_ms: Option<i64>,
    value: Value,
) -> E2eeReplicaResult<Option<PreparedEncryptedField>> {
    let record_id = key.blind_field_id(table, row_id, field);
    let value_tag = key.value_tag(table, row_id, field, deleted, &value);
    let previous = states.get(&record_id);
    // A republish re-seals the value this device already holds above a
    // superseded revision; it keeps the edit time of that value.
    let republish = previous.is_some_and(|state| state.republish);
    if !force && !republish && previous.is_some_and(|state| state.value_tag == value_tag) {
        return Ok(None);
    }
    let edited_at_ms = if republish {
        previous
            .and_then(|state| state.edited_at_ms)
            .or(edited_at_ms)
    } else {
        edited_at_ms
    };

    let revision = previous
        .map(|state| state.revision)
        .unwrap_or(0)
        .max(witness_version.map(|version| version.revision).unwrap_or(0))
        .checked_add(1)
        .ok_or(E2eeReplicaError::InvalidRow)?;
    let revision = u64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    let sealed = key.seal_field_at(
        workspace_id,
        table,
        row_id,
        field,
        writer_id,
        revision,
        deleted,
        edited_at_ms.and_then(|ms| u64::try_from(ms).ok()),
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
            edited_at_ms,
            republish: false,
        },
    }))
}

#[cfg(test)]
pub(super) async fn persist_prepared_dirty_row(
    pool: &SqlitePool,
    prepared: PreparedDirtyRow,
) -> E2eeReplicaResult<u64> {
    persist_prepared_dirty_row_inner(pool, prepared, false).await
}

#[cfg(test)]
pub(super) async fn persist_prepared_dirty_row_inner(
    pool: &SqlitePool,
    prepared: PreparedDirtyRow,
    defer_active_captures: bool,
) -> E2eeReplicaResult<u64> {
    persist_prepared_dirty_row_cancellable(pool, prepared, defer_active_captures, &|| false).await
}

pub(super) async fn persist_prepared_dirty_row_cancellable(
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

    for field_name in &prepared.retired_fields {
        if let Err(error) = check_e2ee_cancellation(is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        sqlx::query(
            "DELETE FROM e2ee_local_state
             WHERE workspace_id = ? AND table_name = ? AND row_id = ? AND field_name = ?",
        )
        .bind(&prepared.dirty.workspace_id)
        .bind(&prepared.dirty.table_name)
        .bind(&prepared.dirty.row_id)
        .bind(field_name)
        .execute(&mut *transaction)
        .await?;
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
