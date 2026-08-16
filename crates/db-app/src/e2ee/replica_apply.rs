use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anlg_e2ee::WorkspaceKeyring;
use serde_json::{Value, json};
use sqlx::{QueryBuilder, Sqlite, SqlitePool, Transaction};

use super::cooperative::yield_once;
use super::replica_storage::{
    clear_stale_apply_guards, delete_row, insert_apply_guard, insert_row, load_row_local_states,
    normalize_replica_payload_hashes, read_field, record_version_order, remove_apply_guard,
    replica_records_still_current, restore_local_payload, row_changed_since_snapshot, row_exists,
    table_columns, update_field, upsert_local_state,
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
            return Err(E2eeReplicaError::ReplicaApplyTooLarge);
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
            if !E2EE_DOMAIN_TABLES.contains(&field.table.as_str()) {
                return Err(E2eeReplicaError::InvalidField);
            }
            let group = (record.workspace_id, field.table, field.row_id);
            group_pending
                .entry(group.clone())
                .or_default()
                .push((record.id.clone(), selected_generations[&record.id]));
            groups.entry(group).or_default().insert(field.field);
        }
        delete_reconciled_replica_entries(pool, &reconciled, is_cancelled).await?;
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
    for (attempted_rows, (group, changed_fields)) in groups.into_iter().enumerate() {
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
        if changed_fields.iter().any(|field| {
            field != ROW_MANIFEST_FIELD
                && (field == "id" || field == "workspace_id" || !columns.contains(field))
        }) {
            return Err(E2eeReplicaError::InvalidField);
        }

        check_e2ee_apply_cancellation(is_cancelled)?;
        let encrypted_records = load_encrypted_row_group(
            pool,
            keyring,
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
                };
                upsert_local_state(&mut transaction, &state).await?;
                rollback_if_cancelled!(transaction, is_cancelled);
                stats.applied_fields += 1;
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
        for record in records {
            rollback_if_cancelled!(transaction, is_cancelled);
            let record_key = keyring
                .get(&record.field.key_id)
                .ok_or(E2eeReplicaError::InvalidRow)?;
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
                    deferred_pending_ids.insert(record.record_id.clone());
                    continue;
                };
                rollback_if_cancelled!(transaction, is_cancelled);
                let matches_snapshot = keyring.generations().any(|key| {
                    key.value_tag(&table, &row_id, field_name, false, &current) == state.value_tag
                });
                if !matches_snapshot {
                    stats.skipped_local_changes += 1;
                    deferred_pending_ids.insert(record.record_id.clone());
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
            };
            upsert_local_state(&mut transaction, &state).await?;
            rollback_if_cancelled!(transaction, is_cancelled);
            states.insert(state.record_id.clone(), state);
            stats.applied_fields += 1;
        }
        remove_apply_guard(&mut transaction, &workspace_id, &table, &row_id).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        pending.retain(|(record_id, _)| !deferred_pending_ids.contains(record_id));
        delete_reconciled_replica_entries_in_transaction(&mut transaction, &pending).await?;
        rollback_if_cancelled!(transaction, is_cancelled);
        commit_e2ee_apply_transaction(transaction, is_cancelled).await?;
    }

    check_e2ee_apply_cancellation(is_cancelled)?;
    stats.remaining_replica_changes |= has_pending_e2ee_replica_entries(pool, keys).await?;
    check_e2ee_apply_cancellation(is_cancelled)?;
    Ok(stats)
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
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<Vec<EncryptedRecord>> {
    let (workspace_id, table, row_id) = row;
    let mut record_ids = keyring
        .generations()
        .flat_map(|key| {
            columns
                .iter()
                .filter(|field| !matches!(field.as_str(), "id" | "workspace_id"))
                .map(|field| key.blind_field_id(table, row_id, field))
                .chain(std::iter::once(key.blind_field_id(
                    table,
                    row_id,
                    ROW_MANIFEST_FIELD,
                )))
        })
        .collect::<Vec<_>>();
    record_ids.sort_unstable();

    check_e2ee_apply_cancellation(is_cancelled)?;
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT
           replica.id,
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
