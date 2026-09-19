use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anlg_e2ee::WorkspaceKeyring;
use serde_json::{Value, json};
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};

use super::chunks::{
    ChunkPart, chunk_count_field, chunk_field, chunk_size_for, join_chunks, parse_array,
    parse_chunk_field, split_chunks,
};
use super::conflicts::{ConflictCopy, ConflictLoser, record_conflict};
use super::cooperative::yield_once;
use super::merge::merge_concurrent_field;
use super::replica_storage::{
    E2eeParkReason, ParkedRecord, clear_stale_apply_guards, delete_row, dirty_row_edited_at_ms,
    insert_apply_guard, insert_row, load_or_create_writer_id, load_row_local_states,
    load_row_local_states_from_pool, mark_local_state_for_republish,
    normalize_replica_payload_hashes, park_records, queue_dirty_row, read_column, read_field,
    record_version_order, remove_apply_guard, replica_records_still_current, restore_local_payload,
    row_changed_since_snapshot, row_exists, table_columns, update_field, upsert_local_state,
};
use super::witness::repair_e2ee_replica_from_witness_bounded_cancellable;
use super::{
    DecryptedRecord, E2EE_APPLY_BYTE_LIMIT, E2EE_APPLY_PREFLIGHT_RECORD_LIMIT,
    E2EE_APPLY_ROW_LIMIT, E2EE_DOMAIN_TABLES, E2EE_WITNESS_REPAIR_BYTE_LIMIT,
    E2EE_WITNESS_REPAIR_RECORD_LIMIT, E2eeReplicaError, E2eeReplicaResult, E2eeReplicaStats,
    EncryptedRecord, EncryptedRecordMetadata, LocalState, ROW_MANIFEST_FIELD,
};

pub async fn apply_e2ee_replica_changes(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<E2eeReplicaStats> {
    apply_received_e2ee_replica_changes_with_witness(pool, keys, true).await
}

pub async fn apply_received_e2ee_replica_changes_with_witness(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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
    keys: &HashMap<String, WorkspaceKeyring>,
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

pub(super) async fn apply_received_e2ee_replica_changes_with_witness_bounded(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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

pub(super) async fn load_changed_e2ee_record_metadata(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<Vec<EncryptedRecordMetadata>> {
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "WITH page AS MATERIALIZED (
           SELECT pending.record_id AS id, pending.workspace_id, pending.generation
           FROM e2ee_replica_pending AS pending
           INDEXED BY idx_e2ee_replica_pending_workspace_record
           WHERE pending.workspace_id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for workspace_id in workspace_ids {
            separated.push_bind(workspace_id);
        }
    }
    query.push(")");
    query
        .push(
            "
           ORDER BY pending.workspace_id, pending.record_id
           LIMIT ",
        )
        .push_bind(E2EE_APPLY_PREFLIGHT_RECORD_LIMIT)
        .push(
            "
         )
         SELECT
           page.id,
           page.workspace_id,
           page.generation,
           COALESCE(
             LENGTH(CAST(replica.id AS BLOB))
               + LENGTH(CAST(replica.workspace_id AS BLOB))
               + LENGTH(CAST(replica.payload AS BLOB))
               + 256,
             0
           ) AS record_bytes,
           replica.id IS NOT NULL
             AND EXISTS(
               SELECT 1
               FROM e2ee_witness_records AS witness
               WHERE witness.workspace_id = replica.workspace_id
                 AND witness.record_id = replica.id
                 AND witness.payload_hash = replica_hash.payload_hash
             ) AS witnessed,
           replica.id IS NOT NULL
           AND replica.payload != ''
           AND (
             replica_hash.record_id IS NULL
             OR local.record_id IS NULL
             OR local.workspace_id != replica.workspace_id
             OR local.payload_hash != replica_hash.payload_hash
           ) AS changed
         FROM page
         LEFT JOIN e2ee_records AS replica
           ON replica.id = page.id
          AND replica.workspace_id = page.workspace_id
         LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
           ON replica_hash.record_id = replica.id
          AND replica_hash.workspace_id = replica.workspace_id
         LEFT JOIN e2ee_local_state AS local
           ON local.record_id = replica.id
         ORDER BY page.workspace_id, page.id",
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
               AND witness.payload_hash = replica_hash.payload_hash
           ) AS witnessed
         FROM e2ee_records AS replica
         LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
           ON replica_hash.record_id = replica.id
          AND replica_hash.workspace_id = replica.workspace_id
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

pub(super) async fn apply_e2ee_replica_changes_inner(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
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
    normalize_replica_payload_hashes(pool, keys, is_cancelled).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut groups = BTreeMap::<(String, String, String), BTreeSet<String>>::new();
    let mut group_pending = BTreeMap::<(String, String, String), Vec<(String, i64)>>::new();
    let mut stats = E2eeReplicaStats::default();
    let metadata = load_changed_e2ee_record_metadata(pool, keys).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut selected_ids = Vec::new();
    let mut selected_generations = HashMap::new();
    let mut reconciled = Vec::new();
    let mut parked = Vec::new();
    let mut selected_bytes = 0_usize;
    for record in &metadata {
        check_e2ee_apply_cancellation(is_cancelled)?;
        if !record.changed || require_witness && !record.witnessed {
            if record.changed {
                stats.rejected_unwitnessed += 1;
            }
            reconciled.push((record.id.clone(), record.generation));
            continue;
        }
        let record_bytes =
            usize::try_from(record.record_bytes).map_err(|_| E2eeReplicaError::InvalidRow)?;
        if record_bytes > max_bytes {
            parked.push(ParkedRecord {
                record_id: record.id.clone(),
                workspace_id: record.workspace_id.clone(),
                generation: record.generation,
                reason: E2eeParkReason::TooLarge,
                table_name: String::new(),
                field_name: String::new(),
            });
            continue;
        }
        if !selected_ids.is_empty() && selected_bytes.saturating_add(record_bytes) > max_bytes {
            stats.remaining_replica_changes = true;
            break;
        }
        selected_bytes = selected_bytes.saturating_add(record_bytes);
        selected_generations.insert(record.id.clone(), record.generation);
        selected_ids.push(record.id.clone());
    }
    delete_reconciled_replica_entries(pool, &reconciled, is_cancelled).await?;

    let mut column_cache = HashMap::<String, HashSet<String>>::new();
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
                reconciled.push((record.id.clone(), selected_generations[&record.id]));
                continue;
            }
            let field = key.open_field(&record.workspace_id, &record.id, &record.payload)?;
            check_e2ee_apply_cancellation(is_cancelled)?;
            // A newer client may sync tables or columns this build does not
            // have yet. Park those records instead of failing the round; the
            // rest of the row still applies.
            let park_reason = if !E2EE_DOMAIN_TABLES.contains(&field.table.as_str()) {
                Some(E2eeParkReason::UnknownTable)
            } else if field.field == ROW_MANIFEST_FIELD {
                None
            } else {
                if !column_cache.contains_key(&field.table) {
                    check_e2ee_apply_cancellation(is_cancelled)?;
                    let columns = table_columns(pool, &field.table).await?;
                    check_e2ee_apply_cancellation(is_cancelled)?;
                    column_cache.insert(field.table.clone(), columns);
                }
                let columns = &column_cache[&field.table];
                let known = columns.contains(&field.field)
                    || parse_chunk_field(&field.table, &field.field)
                        .is_some_and(|(column, _)| columns.contains(column));
                (field.field == "id" || field.field == "workspace_id" || !known)
                    .then_some(E2eeParkReason::UnknownField)
            };
            if let Some(reason) = park_reason {
                parked.push(ParkedRecord {
                    record_id: record.id.clone(),
                    workspace_id: record.workspace_id.clone(),
                    generation: selected_generations[&record.id],
                    reason,
                    table_name: field.table,
                    field_name: field.field,
                });
                continue;
            }
            let group = (record.workspace_id, field.table, field.row_id);
            group_pending
                .entry(group.clone())
                .or_default()
                .push((record.id.clone(), selected_generations[&record.id]));
            let chunk_fields = groups.entry(group).or_default();
            if field.field.contains('#') {
                chunk_fields.insert(field.field);
            }
        }
        delete_reconciled_replica_entries(pool, &reconciled, is_cancelled).await?;
    }
    stats.parked_records += park_replica_records(pool, &parked, is_cancelled).await?;

    let mut attempted_bytes = 0_usize;
    macro_rules! rollback_if_cancelled {
        ($transaction:ident, $is_cancelled:expr) => {
            if ($is_cancelled)() {
                return rollback_cancelled_e2ee_apply($transaction).await;
            }
        };
    }
    for (attempted_rows, (group, pending_chunk_fields)) in groups.into_iter().enumerate() {
        if attempted_rows >= max_rows {
            stats.remaining_replica_changes = true;
            break;
        }
        let (workspace_id, table, row_id) = group;
        let mut pending = group_pending
            .remove(&(workspace_id.clone(), table.clone(), row_id.clone()))
            .unwrap_or_default();
        check_e2ee_apply_cancellation(is_cancelled)?;
        if attempted_rows > 0 {
            yield_once().await;
            check_e2ee_apply_cancellation(is_cancelled)?;
        }
        let keyring = &keys[&workspace_id];
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

        check_e2ee_apply_cancellation(is_cancelled)?;
        let mut chunk_fields = pending_chunk_fields;
        for column in columns
            .iter()
            .filter(|column| chunk_size_for(&table, column).is_some())
        {
            chunk_fields.insert(chunk_count_field(column));
        }
        for state in load_row_local_states_from_pool(pool, &workspace_id, &table, &row_id)
            .await?
            .into_values()
        {
            if parse_chunk_field(&table, &state.field_name).is_some() {
                chunk_fields.insert(state.field_name);
            }
        }
        check_e2ee_apply_cancellation(is_cancelled)?;
        let Some(encrypted_records) = load_encrypted_row_group(
            pool,
            keyring,
            (&workspace_id, &table, &row_id),
            &columns,
            &chunk_fields,
            max_bytes,
            is_cancelled,
        )
        .await?
        else {
            stats.parked_records +=
                park_oversized_row(pool, &workspace_id, &table, &pending, is_cancelled).await?;
            continue;
        };
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
            stats.parked_records +=
                park_oversized_row(pool, &workspace_id, &table, &pending, is_cancelled).await?;
            continue;
        }
        if attempted_rows > 0 && attempted_bytes.saturating_add(row_bytes) > max_bytes {
            stats.remaining_replica_changes = true;
            break;
        }
        attempted_bytes = attempted_bytes.saturating_add(row_bytes);
        let mut records_by_field = BTreeMap::<String, DecryptedRecord>::new();
        for record in encrypted_records {
            check_e2ee_apply_cancellation(is_cancelled)?;
            if require_witness && !record.witnessed {
                continue;
            }
            let field = keyring.open_field(&record.workspace_id, &record.id, &record.payload)?;
            check_e2ee_apply_cancellation(is_cancelled)?;
            if field.table != table || field.row_id != row_id {
                return Err(E2eeReplicaError::InvalidField);
            }
            let known =
                columns.contains(&field.field) || parse_chunk_field(&table, &field.field).is_some();
            if field.field != ROW_MANIFEST_FIELD
                && (field.field == "id" || field.field == "workspace_id" || !known || field.deleted)
            {
                return Err(E2eeReplicaError::InvalidField);
            }
            let payload_hash = anlg_e2ee::payload_hash(&record.payload);
            check_e2ee_apply_cancellation(is_cancelled)?;
            let field_name = field.field.clone();
            let candidate = DecryptedRecord {
                record_id: record.id,
                workspace_id: record.workspace_id,
                payload_hash,
                payload: record.payload,
                field,
            };
            let candidate_is_newer = records_by_field.get(&field_name).is_none_or(|current| {
                (candidate.field.key_id == keyring.active().key_id())
                    .cmp(&(current.field.key_id == keyring.active().key_id()))
                    .then_with(|| candidate.field.revision.cmp(&current.field.revision))
                    .then_with(|| candidate.field.writer_id.cmp(&current.field.writer_id))
                    .then_with(|| candidate.payload_hash.cmp(&current.payload_hash))
                    == Ordering::Greater
            });
            if candidate_is_newer {
                records_by_field.insert(field_name, candidate);
            }
        }
        load_remaining_chunk_records(
            pool,
            keyring,
            (&workspace_id, &table, &row_id),
            &columns,
            require_witness,
            &mut records_by_field,
        )
        .await?;
        let mut records = records_by_field.into_values().collect::<Vec<_>>();

        check_e2ee_apply_cancellation(is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        if !replica_records_still_current(&mut transaction, &records).await? {
            rollback_if_cancelled!(transaction, is_cancelled);
            delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
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
                Some(state) => incoming_is_stale(state, &record)?,
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
            delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
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
            delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        };
        let manifest = records.swap_remove(manifest_index);
        let manifest_key = keyring
            .get(&manifest.field.key_id)
            .ok_or(E2eeReplicaError::InvalidRow)?;
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
                keyring,
                &workspace_id,
                &table,
                &row_id,
                row_was_present,
                &states,
            )
            .await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            // A remote delete or recreate of a row this device also changed
            // waits until the local change publishes. Plain field edits go on
            // to the field loop, which orders each field by edit time.
            if locally_changed && (manifest.field.deleted || !row_was_present) {
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
                    manifest_key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, true, &Value::Null);
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
                    edited_at_ms: edited_at_ms_i64(manifest.field.edited_at_ms),
                    republish: false,
                };
                upsert_local_state(&mut transaction, &state).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                stats.applied_fields += 1;
                super::library::queue_other_connections(
                    &mut transaction,
                    &workspace_id,
                    &table,
                    &row_id,
                    state.edited_at_ms.unwrap_or(0),
                )
                .await?;
                remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending)
                    .await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
                continue;
            }

            if !row_was_present {
                insert_row(&mut transaction, &table, &workspace_id, &row_id).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                if !row_exists(&mut transaction, &table, &workspace_id, &row_id).await? {
                    rollback_if_cancelled!(transaction, is_cancelled);
                    // Another workspace owns this row ID. Let hydration yield
                    // instead of retrying the unchanged pending record forever.
                    stats.skipped_local_changes += records.len() as u64 + 1;
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
            let value_tag =
                manifest_key.value_tag(&table, &row_id, ROW_MANIFEST_FIELD, false, &json!(true));
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
                edited_at_ms: edited_at_ms_i64(manifest.field.edited_at_ms),
                republish: false,
            };
            upsert_local_state(&mut transaction, &state).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        } else if !row_was_present || manifest.field.deleted {
            remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
            continue;
        }

        let mut deferred_pending_ids = HashSet::new();
        let mut republish_merged_row = false;
        let mut chunk_columns = BTreeMap::<String, ChunkedColumnRecords>::new();
        let mut plain_records = Vec::with_capacity(records.len());
        for record in records {
            match parse_chunk_field(&table, &record.field.field) {
                Some((column, part)) => {
                    let column = column.to_string();
                    let entry = chunk_columns.entry(column).or_default();
                    match part {
                        ChunkPart::Count => entry.count = Some(record),
                        ChunkPart::Index(index) => {
                            entry.chunks.insert(index, record);
                        }
                    }
                }
                None => plain_records.push(record),
            }
        }
        for record in plain_records {
            rollback_if_cancelled!(transaction, is_cancelled);
            let record_key = keyring
                .get(&record.field.key_id)
                .ok_or(E2eeReplicaError::InvalidRow)?;
            let field_name = record.field.field.as_str();
            let mut merged_value: Option<Value> = None;
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
                    deferred_pending_ids.insert(record.record_id.clone());
                    continue;
                };
                rollback_if_cancelled!(transaction, is_cancelled);
                let matches_snapshot = keyring.generations().any(|key| {
                    key.value_tag(&table, &row_id, field_name, false, &current) == state.value_tag
                });
                if !matches_snapshot {
                    // This device changed the field since it last synced, so
                    // the two edits are concurrent. The later edit wins and
                    // the other is kept as a conflict copy. Without edit times
                    // the local change wins, as before.
                    let local_edited_at_ms =
                        dirty_row_edited_at_ms(&mut transaction, &workspace_id, &table, &row_id)
                            .await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    let local_writer_id = load_or_create_writer_id(&mut transaction).await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    let incoming_wins = match (record.field.edited_at_ms, local_edited_at_ms) {
                        (Some(incoming), Some(local)) => {
                            let incoming = i64::try_from(incoming).unwrap_or(i64::MAX);
                            incoming > local
                                || (incoming == local && record.field.writer_id > local_writer_id)
                        }
                        _ => false,
                    };
                    // Documents merge block by block; only regions both sides
                    // rewrote fall back to the later edit, and then the other
                    // side's whole document is kept as the conflict copy.
                    let merged = merge_concurrent_field(
                        keyring,
                        &workspace_id,
                        &table,
                        field_name,
                        state,
                        &current,
                        &record.field.value,
                        !incoming_wins,
                    );
                    let record_loser = merged.as_ref().is_none_or(|merged| merged.had_conflicts);
                    if let Some(merged) = merged {
                        merged_value = Some(merged.value);
                    }
                    if !record_loser {
                        // Clean merge: nothing was lost.
                    } else if incoming_wins {
                        let local_value_tag = keyring
                            .active()
                            .value_tag(&table, &row_id, field_name, false, &current);
                        let recorded = field_keeps_conflict_copies(field_name)
                            && record_conflict(
                                &mut transaction,
                                &ConflictCopy {
                                    id: format!("{}:local:{local_value_tag}", record.record_id),
                                    workspace_id: &workspace_id,
                                    table_name: &table,
                                    row_id: &row_id,
                                    field_name,
                                    lost_side: ConflictLoser::Local,
                                    writer_id: &local_writer_id,
                                    revision: state.revision,
                                    edited_at_ms: local_edited_at_ms,
                                    value: &current,
                                },
                            )
                            .await?;
                        rollback_if_cancelled!(transaction, is_cancelled);
                        stats.recorded_conflicts += u64::from(recorded);
                    } else {
                        let recorded = field_keeps_conflict_copies(field_name)
                            && record_conflict(
                                &mut transaction,
                                &ConflictCopy {
                                    id: format!("{}:{}", record.record_id, record.payload_hash),
                                    workspace_id: &workspace_id,
                                    table_name: &table,
                                    row_id: &row_id,
                                    field_name,
                                    lost_side: ConflictLoser::Remote,
                                    writer_id: &record.field.writer_id,
                                    revision: i64::try_from(record.field.revision)
                                        .map_err(|_| E2eeReplicaError::InvalidRow)?,
                                    edited_at_ms: edited_at_ms_i64(record.field.edited_at_ms),
                                    value: &record.field.value,
                                },
                            )
                            .await?;
                        rollback_if_cancelled!(transaction, is_cancelled);
                        stats.recorded_conflicts += u64::from(recorded);
                        if merged_value.is_none() {
                            stats.skipped_local_changes += 1;
                            deferred_pending_ids.insert(record.record_id.clone());
                            continue;
                        }
                    }
                } else if let (Some(incoming), Some(local)) =
                    (record.field.edited_at_ms, state.edited_at_ms)
                    && incoming_edit_is_older(
                        i64::try_from(incoming).unwrap_or(i64::MAX),
                        &record.field.writer_id,
                        local,
                        &state.writer_id,
                    )
                {
                    // The record won the revision race but was written before
                    // the value this device already holds. Keep the later
                    // edit, remember the earlier one, and republish above the
                    // incoming revision so every replica converges on it.
                    let recorded = field_keeps_conflict_copies(field_name)
                        && record_conflict(
                            &mut transaction,
                            &ConflictCopy {
                                id: format!("{}:{}", record.record_id, record.payload_hash),
                                workspace_id: &workspace_id,
                                table_name: &table,
                                row_id: &row_id,
                                field_name,
                                lost_side: ConflictLoser::Remote,
                                writer_id: &record.field.writer_id,
                                revision: i64::try_from(record.field.revision)
                                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                                edited_at_ms: Some(i64::try_from(incoming).unwrap_or(i64::MAX)),
                                value: &record.field.value,
                            },
                        )
                        .await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    mark_local_state_for_republish(
                        &mut transaction,
                        &record.record_id,
                        i64::try_from(record.field.revision)
                            .map_err(|_| E2eeReplicaError::InvalidRow)?,
                    )
                    .await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    queue_dirty_row(&mut transaction, &workspace_id, &table, &row_id).await?;
                    rollback_if_cancelled!(transaction, is_cancelled);
                    stats.recorded_conflicts += u64::from(recorded);
                    continue;
                }
            }

            update_field(
                &mut transaction,
                &table,
                &workspace_id,
                &row_id,
                field_name,
                merged_value.as_ref().unwrap_or(&record.field.value),
            )
            .await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            if merged_value.is_some() {
                republish_merged_row = true;
                stats.merged_fields += 1;
            }
            // Preserve the row's existing wire format. Introducing chunks here
            // would make an otherwise compatible row unreadable by 1.4.23.
            if states.values().any(|state| {
                parse_chunk_field(&table, &state.field_name)
                    .is_some_and(|(column, _)| column == field_name)
            }) {
                republish_merged_row = true;
            }
            let value_tag =
                record_key.value_tag(&table, &row_id, field_name, false, &record.field.value);
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
                edited_at_ms: edited_at_ms_i64(record.field.edited_at_ms),
                republish: false,
            };
            upsert_local_state(&mut transaction, &state).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        }
        for (column, column_records) in chunk_columns {
            rollback_if_cancelled!(transaction, is_cancelled);
            let outcome = apply_chunked_column(
                &mut transaction,
                keyring,
                (&workspace_id, &table, &row_id),
                &column,
                column_records,
                &mut states,
                row_materialized,
            )
            .await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            match outcome {
                ChunkedColumnOutcome::Deferred(record_ids) => {
                    stats.skipped_local_changes += record_ids.len() as u64;
                    stats.incomplete_chunk_columns += 1;
                    deferred_pending_ids.extend(record_ids);
                }
                ChunkedColumnOutcome::Applied {
                    applied_fields,
                    kept_local,
                } => {
                    stats.applied_fields += applied_fields;
                    republish_merged_row |= kept_local;
                }
            }
        }
        if republish_merged_row {
            // A merged document differs from the incoming record, so the next
            // encrypt round publishes it with a fresh edit time. Queued after
            // the field loop so the row's write time stays that of the local
            // edit while the remaining fields are ordered.
            queue_dirty_row(&mut transaction, &workspace_id, &table, &row_id).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
        }
        let edited_at_ms = states
            .values()
            .filter_map(|state| state.edited_at_ms)
            .max()
            .unwrap_or(0);
        super::library::queue_other_connections(
            &mut transaction,
            &workspace_id,
            &table,
            &row_id,
            edited_at_ms,
        )
        .await?;
        remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        pending.retain(|(record_id, _)| !deferred_pending_ids.contains(record_id));
        let identity =
            super::library::LibraryIdentity::load(&mut *transaction, &workspace_id).await?;
        crate::session_deletion::reconcile_session_deletion(
            &mut transaction,
            &identity.local_workspace_id,
            &table,
            identity.local_row_id(&table, &row_id),
        )
        .await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
    }

    check_e2ee_apply_cancellation(is_cancelled)?;
    stats.remaining_replica_changes |= has_pending_e2ee_replica_entries(pool, keys).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    Ok(stats)
}

// Bookkeeping columns still resolve by edit time, but their values mean
// nothing to the user, so they never produce conflict copies.
fn field_keeps_conflict_copies(field_name: &str) -> bool {
    !field_name.contains('#')
        && !matches!(
            field_name,
            "updated_at"
                | "created_at"
                | "updated_by"
                | "created_by"
                | "content_version"
                | "deleted_at"
        )
}

#[derive(Default)]
struct ChunkedColumnRecords {
    count: Option<DecryptedRecord>,
    chunks: BTreeMap<usize, DecryptedRecord>,
}

enum ChunkedColumnOutcome {
    Deferred(Vec<String>),
    Applied {
        applied_fields: u64,
        kept_local: bool,
    },
}

// The chunk count record says how many chunk records the column has; fetch
// any this row group did not already load so the column can be rebuilt whole.
async fn load_remaining_chunk_records(
    pool: &SqlitePool,
    keyring: &WorkspaceKeyring,
    row: (&str, &str, &str),
    columns: &HashSet<String>,
    require_witness: bool,
    records_by_field: &mut BTreeMap<String, DecryptedRecord>,
) -> E2eeReplicaResult<()> {
    let (workspace_id, table, row_id) = row;
    let mut missing = Vec::new();
    for column in columns
        .iter()
        .filter(|column| chunk_size_for(table, column).is_some())
    {
        let Some(count) = records_by_field
            .get(&chunk_count_field(column))
            .and_then(|record| record.field.value.as_u64())
        else {
            continue;
        };
        for index in 0..usize::try_from(count).unwrap_or(usize::MAX).min(1 << 20) {
            let field = chunk_field(column, index);
            if !records_by_field.contains_key(&field) {
                missing.extend(
                    keyring
                        .generations()
                        .map(|key| key.blind_field_id(table, row_id, &field)),
                );
            }
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    for record in load_encrypted_records_by_id(pool, &missing).await? {
        if record.workspace_id != workspace_id || (require_witness && !record.witnessed) {
            continue;
        }
        let Ok(field) = keyring.open_field(&record.workspace_id, &record.id, &record.payload)
        else {
            continue;
        };
        if field.table != table || field.row_id != row_id {
            continue;
        }
        let payload_hash = anlg_e2ee::payload_hash(&record.payload);
        records_by_field
            .entry(field.field.clone())
            .or_insert(DecryptedRecord {
                record_id: record.id,
                workspace_id: record.workspace_id,
                payload_hash,
                payload: record.payload,
                field,
            });
    }
    Ok(())
}

/// Rebuilds a chunked column from its chunk records, chunk by chunk: a chunk
/// this device changed since it last synced is kept when its edit is later
/// (or when edit times are unknown), otherwise the incoming chunk wins. The
/// column is only written once every chunk the count names is available, so a
/// partly received transcript never lands.
async fn apply_chunked_column(
    transaction: &mut Transaction<'_, Sqlite>,
    keyring: &WorkspaceKeyring,
    row: (&str, &str, &str),
    column: &str,
    column_records: ChunkedColumnRecords,
    states: &mut HashMap<String, LocalState>,
    row_materialized: bool,
) -> E2eeReplicaResult<ChunkedColumnOutcome> {
    let (workspace_id, table, row_id) = row;
    let all_record_ids = || {
        column_records
            .count
            .iter()
            .chain(column_records.chunks.values())
            .map(|record| record.record_id.clone())
            .collect::<Vec<_>>()
    };
    let Some(count_record) = column_records.count.as_ref() else {
        return Ok(ChunkedColumnOutcome::Deferred(all_record_ids()));
    };
    let Some(count) = count_record
        .field
        .value
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
    else {
        return Err(E2eeReplicaError::InvalidField);
    };
    if (0..count).any(|index| !column_records.chunks.contains_key(&index)) {
        return Ok(ChunkedColumnOutcome::Deferred(all_record_ids()));
    }

    let chunk_size = chunk_size_for(table, column).unwrap_or(1);
    let current = read_column(transaction, table, workspace_id, row_id, column).await?;
    let local_chunks = current
        .as_ref()
        .and_then(parse_array)
        .map(|items| split_chunks(&items, chunk_size))
        .unwrap_or_default();
    let mut local_edit: Option<(Option<i64>, String)> = None;
    let mut merged = Vec::with_capacity(count);
    let mut applied_fields = 0;
    let mut kept_local = false;
    let mut applied_states = Vec::new();
    for index in 0..count {
        let record = &column_records.chunks[&index];
        let remote_chunk = record.field.value.as_array().cloned().unwrap_or_default();
        let local_chunk = local_chunks.get(index).cloned();
        let state = states.get(&record.record_id);
        let keep_local = match (state, local_chunk.as_ref()) {
            (Some(state), Some(local_chunk)) if !row_materialized => {
                let local_value = Value::Array(local_chunk.clone());
                let matches_snapshot = keyring.generations().any(|key| {
                    key.value_tag(table, row_id, &record.field.field, false, &local_value)
                        == state.value_tag
                });
                if matches_snapshot {
                    // Unchanged here; take the record unless it is an older
                    // edit that merely won the revision race.
                    match (record.field.edited_at_ms, state.edited_at_ms) {
                        (Some(incoming), Some(local))
                            if state.payload_hash != record.payload_hash
                                && incoming_edit_is_older(
                                    i64::try_from(incoming).unwrap_or(i64::MAX),
                                    &record.field.writer_id,
                                    local,
                                    &state.writer_id,
                                ) =>
                        {
                            mark_local_state_for_republish(
                                transaction,
                                &record.record_id,
                                i64::try_from(record.field.revision)
                                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                            )
                            .await?;
                            true
                        }
                        _ => false,
                    }
                } else if state.payload_hash == record.payload_hash {
                    true
                } else {
                    if local_edit.is_none() {
                        local_edit = Some((
                            dirty_row_edited_at_ms(transaction, workspace_id, table, row_id)
                                .await?,
                            load_or_create_writer_id(transaction).await?,
                        ));
                    }
                    let (local_edited_at_ms, local_writer_id) =
                        local_edit.as_ref().expect("local edit loaded");
                    let incoming_wins = match (record.field.edited_at_ms, local_edited_at_ms) {
                        (Some(incoming), Some(local)) => {
                            let incoming = i64::try_from(incoming).unwrap_or(i64::MAX);
                            incoming > *local
                                || (incoming == *local && record.field.writer_id > *local_writer_id)
                        }
                        _ => false,
                    };
                    !incoming_wins
                }
            }
            _ => false,
        };
        if keep_local {
            kept_local = true;
            merged.push(local_chunk.unwrap_or_default());
            continue;
        }
        merged.push(remote_chunk);
        if state.is_none_or(|state| state.payload_hash != record.payload_hash) {
            applied_states.push(record);
        }
    }

    let joined = join_chunks(&merged);
    if current.as_ref() != Some(&joined) {
        update_field(transaction, table, workspace_id, row_id, column, &joined).await?;
    }
    for record in applied_states {
        let key = keyring
            .get(&record.field.key_id)
            .ok_or(E2eeReplicaError::InvalidRow)?;
        let state = local_state_for_record(key, table, row_id, record);
        upsert_local_state(transaction, &state).await?;
        states.insert(state.record_id.clone(), state);
        applied_fields += 1;
    }
    if states
        .get(&count_record.record_id)
        .is_none_or(|state| state.payload_hash != count_record.payload_hash)
    {
        let key = keyring
            .get(&count_record.field.key_id)
            .ok_or(E2eeReplicaError::InvalidRow)?;
        let state = local_state_for_record(key, table, row_id, count_record);
        upsert_local_state(transaction, &state).await?;
        states.insert(state.record_id.clone(), state);
        applied_fields += 1;
    }
    // Chunk state beyond the new count is stale; drop it so a later shrink or
    // regrowth compares against nothing.
    let stale_ids = states
        .values()
        .filter(|state| {
            matches!(
                parse_chunk_field(table, &state.field_name),
                Some((state_column, ChunkPart::Index(index)))
                    if state_column == column && index >= count
            )
        })
        .map(|state| state.record_id.clone())
        .collect::<Vec<_>>();
    for record_id in stale_ids {
        sqlx::query("DELETE FROM e2ee_local_state WHERE record_id = ?")
            .bind(&record_id)
            .execute(&mut **transaction)
            .await?;
        states.remove(&record_id);
    }
    // The plain column's own state belongs to the legacy format.
    sqlx::query(
        "DELETE FROM e2ee_local_state
         WHERE workspace_id = ? AND table_name = ? AND row_id = ? AND field_name = ?",
    )
    .bind(workspace_id)
    .bind(table)
    .bind(row_id)
    .bind(column)
    .execute(&mut **transaction)
    .await?;
    states.retain(|_, state| !(state.field_name == column && state.row_id == row_id));
    Ok(ChunkedColumnOutcome::Applied {
        applied_fields,
        kept_local,
    })
}

fn local_state_for_record(
    key: &anlg_e2ee::WorkspaceKey,
    table: &str,
    row_id: &str,
    record: &DecryptedRecord,
) -> LocalState {
    LocalState {
        record_id: record.record_id.clone(),
        workspace_id: record.workspace_id.clone(),
        table_name: table.to_string(),
        row_id: row_id.to_string(),
        field_name: record.field.field.clone(),
        revision: i64::try_from(record.field.revision).unwrap_or(i64::MAX),
        writer_id: record.field.writer_id.clone(),
        value_tag: key.value_tag(
            table,
            row_id,
            &record.field.field,
            record.field.deleted,
            &record.field.value,
        ),
        payload_hash: record.payload_hash.clone(),
        payload: record.payload.clone(),
        edited_at_ms: edited_at_ms_i64(record.field.edited_at_ms),
        republish: false,
    }
}

fn edited_at_ms_i64(edited_at_ms: Option<u64>) -> Option<i64> {
    edited_at_ms.map(|ms| i64::try_from(ms).unwrap_or(i64::MAX))
}

fn incoming_edit_is_older(
    incoming_edited_at_ms: i64,
    incoming_writer_id: &str,
    local_edited_at_ms: i64,
    local_writer_id: &str,
) -> bool {
    incoming_edited_at_ms < local_edited_at_ms
        || (incoming_edited_at_ms == local_edited_at_ms && incoming_writer_id < local_writer_id)
}

// Lower revisions are replays and are always rejected. Equal revisions from
// different writers are concurrent edits: when both carry an edit time the
// field loop orders them by it; otherwise the writer order decides as before.
fn incoming_is_stale(state: &LocalState, record: &DecryptedRecord) -> E2eeReplicaResult<bool> {
    let state_revision = u64::try_from(state.revision).map_err(|_| E2eeReplicaError::InvalidRow)?;
    Ok(match record.field.revision.cmp(&state_revision) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => match (record.field.edited_at_ms, state.edited_at_ms) {
            (Some(_), Some(_)) => false,
            _ => record_version_order(state, record)? == Ordering::Less,
        },
    })
}

async fn delete_reconciled_replica_entries(
    pool: &SqlitePool,
    entries: &[(String, i64)],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Err(error) = check_e2ee_apply_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    delete_reconciled_replica_entries_in_transaction(&mut transaction, entries).await?;
    commit_e2ee_apply_transaction(transaction, is_cancelled).await
}

async fn delete_reconciled_replica_entries_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    entries: &[(String, i64)],
) -> E2eeReplicaResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        "DELETE FROM e2ee_replica_pending WHERE (record_id, generation) IN (",
    );
    query.push_values(entries, |mut row, (record_id, generation)| {
        row.push_bind(record_id).push_bind(generation);
    });
    query.push(")").build().execute(&mut **transaction).await?;
    Ok(())
}

async fn has_pending_e2ee_replica_entries(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKeyring>,
) -> E2eeReplicaResult<bool> {
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT EXISTS(
           SELECT 1
           FROM e2ee_replica_pending AS pending
           WHERE pending.workspace_id IN (",
    );
    let mut separated = query.separated(", ");
    for workspace_id in workspace_ids {
        separated.push_bind(workspace_id);
    }
    separated.push_unseparated(") LIMIT 1)");
    Ok(query.build_query_scalar().fetch_one(pool).await?)
}

async fn load_encrypted_row_group(
    pool: &SqlitePool,
    keyring: &WorkspaceKeyring,
    row: (&str, &str, &str),
    columns: &HashSet<String>,
    chunk_fields: &BTreeSet<String>,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Option<Vec<EncryptedRecord>>> {
    let (workspace_id, table, row_id) = row;
    let mut record_ids = keyring
        .generations()
        .flat_map(|key| {
            columns
                .iter()
                .filter(|field| !matches!(field.as_str(), "id" | "workspace_id"))
                .chain(chunk_fields.iter())
                .map(|field| key.blind_field_id(table, row_id, field))
                .chain(std::iter::once(key.blind_field_id(
                    table,
                    row_id,
                    ROW_MANIFEST_FIELD,
                )))
        })
        .collect::<Vec<_>>();
    record_ids.sort_unstable();
    record_ids.dedup();

    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT
           replica.id,
           replica.workspace_id,
           0 AS generation,
           LENGTH(CAST(replica.id AS BLOB))
             + LENGTH(CAST(replica.workspace_id AS BLOB))
             + LENGTH(CAST(replica.payload AS BLOB))
             + 256 AS record_bytes,
           EXISTS(
             SELECT 1
             FROM e2ee_witness_records AS witness
             WHERE witness.workspace_id = replica.workspace_id
               AND witness.record_id = replica.id
               AND witness.payload_hash = replica_hash.payload_hash
           ) AS witnessed,
           1 AS changed
         FROM e2ee_records AS replica
         LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
           ON replica_hash.record_id = replica.id
          AND replica_hash.workspace_id = replica.workspace_id
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
        return Ok(None);
    }
    check_e2ee_apply_cancellation(is_cancelled)?;
    let records = load_encrypted_records_by_id(pool, &record_ids).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    Ok(Some(
        records
            .into_iter()
            .filter(|record| record.workspace_id == workspace_id)
            .collect(),
    ))
}

async fn park_replica_records(
    pool: &SqlitePool,
    records: &[ParkedRecord],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<u64> {
    if records.is_empty() {
        return Ok(0);
    }
    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Err(error) = check_e2ee_apply_cancellation(is_cancelled) {
        transaction.rollback().await?;
        return Err(error);
    }
    park_records(&mut transaction, records).await?;
    commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
    Ok(records.len() as u64)
}

// A row whose ciphertext exceeds the apply budget waits as a unit so a partial
// row never lands; startup requeues it once a build can take it.
async fn park_oversized_row(
    pool: &SqlitePool,
    workspace_id: &str,
    table: &str,
    pending: &[(String, i64)],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<u64> {
    let records = pending
        .iter()
        .map(|(record_id, generation)| ParkedRecord {
            record_id: record_id.clone(),
            workspace_id: workspace_id.to_string(),
            generation: *generation,
            reason: E2eeParkReason::TooLarge,
            table_name: table.to_string(),
            field_name: String::new(),
        })
        .collect::<Vec<_>>();
    park_replica_records(pool, &records, is_cancelled).await
}
