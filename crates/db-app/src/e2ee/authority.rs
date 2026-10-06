use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

use super::{E2eeReplicaError, E2eeReplicaResult, LocalState};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eeCloudEvent {
    pub record_id: String,
    pub payload_hash: String,
    pub payload: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eeCloudBatch {
    pub mutation_id: String,
    pub base_sequence: u64,
    pub initialize: bool,
    pub events: Vec<E2eeCloudEvent>,
}

pub async fn e2ee_cloud_authority_enabled(
    pool: &SqlitePool,
    workspace_id: &str,
) -> E2eeReplicaResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM e2ee_cloud_authority WHERE workspace_id = ?)",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?)
}

pub async fn configure_e2ee_cloud_authority(
    pool: &SqlitePool,
    workspace_id: &str,
    after: Option<u64>,
) -> E2eeReplicaResult<()> {
    let after = after
        .map(i64::try_from)
        .transpose()
        .map_err(|_| E2eeReplicaError::RollbackDetected)?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let previous: Option<Option<i64>> = sqlx::query_scalar(
        "SELECT after_sequence FROM e2ee_cloud_authority WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await?;
    if previous.flatten().is_some_and(|value| Some(value) != after) {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    sqlx::query(
        "INSERT INTO e2ee_cloud_authority(workspace_id, after_sequence) VALUES (?, ?)
                ON CONFLICT(workspace_id) DO UPDATE SET after_sequence = excluded.after_sequence",
    )
    .bind(workspace_id)
    .bind(after)
    .execute(&mut *tx)
    .await?;
    if previous.is_none() {
        // Freeze unsent legacy snapshots before any cloud history can retire them.
        sqlx::query("INSERT INTO e2ee_cloud_outbox
                    SELECT local.workspace_id, local.record_id, local.table_name, local.row_id, local.payload_hash, local.payload, witness.payload_hash, witness.payload
                    FROM e2ee_witness_pending AS pending JOIN e2ee_local_state_resolved AS local
                      ON local.workspace_id = pending.workspace_id AND local.record_id = pending.record_id
                    LEFT JOIN e2ee_witness_records_resolved AS witness ON witness.workspace_id = local.workspace_id AND witness.record_id = local.record_id
                    WHERE local.workspace_id = ?")
            .bind(workspace_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub(super) async fn enqueue(
    tx: &mut Transaction<'_, Sqlite>,
    state: &LocalState,
) -> E2eeReplicaResult<()> {
    sqlx::query("INSERT INTO e2ee_cloud_outbox
                SELECT ?, ?, ?, ?, ?, ?, (SELECT payload_hash FROM e2ee_witness_records WHERE workspace_id = ? AND record_id = ?), (SELECT payload FROM e2ee_witness_records_resolved WHERE workspace_id = ? AND record_id = ?) WHERE EXISTS(SELECT 1 FROM e2ee_cloud_authority WHERE workspace_id = ?)
                ON CONFLICT(workspace_id, record_id) DO UPDATE SET
                  table_name = excluded.table_name, row_id = excluded.row_id,
                  payload_hash = excluded.payload_hash, payload = excluded.payload")
        .bind(&state.workspace_id).bind(&state.record_id).bind(&state.table_name).bind(&state.row_id)
        .bind(&state.payload_hash).bind(&state.payload).bind(&state.workspace_id).bind(&state.record_id).bind(&state.workspace_id).bind(&state.record_id).bind(&state.workspace_id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn pending_e2ee_cloud_batch(
    pool: &SqlitePool,
    workspace_id: &str,
    initialize: bool,
) -> E2eeReplicaResult<Option<E2eeCloudBatch>> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let existing: Option<(String, i64, bool, String)> = sqlx::query_as(
        "SELECT mutation_id, base_sequence, initialize, events_json FROM e2ee_cloud_batches WHERE workspace_id = ?")
        .bind(workspace_id).fetch_optional(&mut *tx).await?;
    if let Some((mutation_id, base_sequence, initialize, events_json)) = existing {
        return Ok(Some(E2eeCloudBatch {
            mutation_id,
            base_sequence: u64::try_from(base_sequence)
                .map_err(|_| E2eeReplicaError::InvalidRow)?,
            initialize,
            events: serde_json::from_str(&events_json).map_err(|_| E2eeReplicaError::InvalidRow)?,
        }));
    }
    let row: Option<(String, String)> = sqlx::query_as("SELECT table_name, row_id FROM e2ee_cloud_outbox WHERE workspace_id = ? ORDER BY CASE table_name WHEN 'session_documents' THEN 1 WHEN 'transcripts' THEN 2 ELSE 0 END, table_name, row_id LIMIT 1")
        .bind(workspace_id).fetch_optional(&mut *tx).await?;
    let Some((table, row)) = row else {
        return Ok(None);
    };
    let (count, bytes): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(length(CAST(payload AS BLOB)) + length(record_id) + length(payload_hash) + 256), 0)
         FROM e2ee_cloud_outbox WHERE workspace_id = ? AND table_name = ? AND row_id = ?")
        .bind(workspace_id).bind(&table).bind(&row).fetch_one(&mut *tx).await?;
    if count > 1024 || bytes > 48 * 1024 * 1024 {
        return Err(E2eeReplicaError::WitnessUploadTooLarge);
    }
    let values: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT record_id, payload_hash, payload FROM e2ee_cloud_outbox WHERE workspace_id = ? AND table_name = ? AND row_id = ? ORDER BY record_id")
        .bind(workspace_id).bind(&table).bind(&row).fetch_all(&mut *tx).await?;
    let events: Vec<_> = values
        .into_iter()
        .map(|(record_id, payload_hash, payload)| E2eeCloudEvent {
            record_id,
            payload_hash,
            payload,
        })
        .collect();
    let base_sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT last_sequence FROM e2ee_witness_state WHERE workspace_id = ?), 0)",
    )
    .bind(workspace_id)
    .fetch_one(&mut *tx)
    .await?;
    let batch = E2eeCloudBatch {
        mutation_id: uuid::Uuid::new_v4().to_string(),
        base_sequence: u64::try_from(base_sequence).map_err(|_| E2eeReplicaError::InvalidRow)?,
        initialize,
        events,
    };
    sqlx::query("INSERT INTO e2ee_cloud_batches VALUES (?, ?, ?, ?, ?)")
        .bind(workspace_id)
        .bind(&batch.mutation_id)
        .bind(base_sequence)
        .bind(initialize)
        .bind(serde_json::to_string(&batch.events).map_err(|_| E2eeReplicaError::InvalidRow)?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(batch))
}

/// Only a confirmed CAS rejection may abandon an in-flight identity without a receipt.
pub async fn reject_e2ee_cloud_batch(
    pool: &SqlitePool,
    workspace_id: &str,
    mutation_id: &str,
) -> E2eeReplicaResult<()> {
    sqlx::query("DELETE FROM e2ee_cloud_batches WHERE workspace_id = ? AND mutation_id = ?")
        .bind(workspace_id)
        .bind(mutation_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn acknowledge_e2ee_cloud_batch(
    pool: &SqlitePool,
    workspace_id: &str,
    batch: &E2eeCloudBatch,
) -> E2eeReplicaResult<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    for event in &batch.events {
        // A newer edit may already have replaced the acknowledged snapshot.
        sqlx::query("DELETE FROM e2ee_cloud_outbox WHERE workspace_id = ? AND record_id = ? AND payload_hash = ?")
            .bind(workspace_id).bind(&event.record_id).bind(&event.payload_hash).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM e2ee_cloud_batches WHERE workspace_id = ? AND mutation_id = ?")
        .bind(workspace_id)
        .bind(&batch.mutation_id)
        .execute(&mut *tx)
        .await?;
    for event in &batch.events {
        sqlx::query("INSERT INTO e2ee_witness_repair_pending(record_id, workspace_id)
                    SELECT record_id, workspace_id FROM e2ee_witness_records WHERE workspace_id = ? AND record_id = ?
                    ON CONFLICT(record_id) DO UPDATE SET generation = e2ee_witness_repair_pending.generation + 1")
            .bind(workspace_id).bind(&event.record_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO e2ee_replica_pending(record_id, workspace_id)
                    SELECT record_id, workspace_id FROM e2ee_witness_records WHERE workspace_id = ? AND record_id = ?
                    ON CONFLICT(record_id) DO UPDATE SET generation = e2ee_replica_pending.generation + 1")
            .bind(workspace_id).bind(&event.record_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn preserve_e2ee_cloud_conflicts(
    pool: &SqlitePool,
    workspace_id: &str,
    keyring: &anlg_e2ee::WorkspaceKeyring,
    batch: &E2eeCloudBatch,
) -> E2eeReplicaResult<()> {
    use super::chunks::{chunk_field, join_chunks, parse_chunk_field};
    use super::conflicts::{ConflictCopy, ConflictLoser, record_conflict};
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let mut copied_columns = std::collections::HashSet::new();
    for event in &batch.events {
        let incoming: Option<(String, String)> = sqlx::query_as(
            "SELECT witness.payload_hash, witness.payload FROM e2ee_witness_records_resolved AS witness
             JOIN e2ee_cloud_outbox AS pending ON pending.workspace_id = witness.workspace_id AND pending.record_id = witness.record_id
             WHERE witness.workspace_id = ? AND witness.record_id = ?
               AND witness.payload_hash != ? AND witness.payload_hash IS NOT pending.base_payload_hash")
            .bind(workspace_id).bind(&event.record_id).bind(&event.payload_hash).fetch_optional(&mut *tx).await?;
        let Some((_hash, payload)) = incoming else {
            continue;
        };
        let field = keyring.open_field(workspace_id, &event.record_id, &payload)?;
        let mut name = field.field.as_str();
        let mut value = field.value.clone();
        if let Some((column, _)) = parse_chunk_field(&field.table, &field.field) {
            if !copied_columns.insert((
                field.table.clone(),
                field.row_id.clone(),
                column.to_string(),
            )) {
                continue;
            }
            name = column;
            let key = keyring
                .get(&field.key_id)
                .ok_or(E2eeReplicaError::InvalidRow)?;
            let count_id = key.blind_field_id(&field.table, &field.row_id, &format!("{column}#n"));
            let count_payload: Option<String> = sqlx::query_scalar("SELECT payload FROM e2ee_witness_records_resolved WHERE workspace_id = ? AND record_id = ?")
                .bind(workspace_id).bind(&count_id).fetch_optional(&mut *tx).await?;
            let Some(count_payload) = count_payload else {
                continue;
            };
            let count = keyring
                .open_field(workspace_id, &count_id, &count_payload)?
                .value
                .as_u64()
                .ok_or(E2eeReplicaError::InvalidRow)?;
            if count > 1024 {
                return Err(E2eeReplicaError::WitnessUploadTooLarge);
            }
            let mut chunks = Vec::new();
            for index in 0..count as usize {
                let id =
                    key.blind_field_id(&field.table, &field.row_id, &chunk_field(column, index));
                let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM e2ee_witness_records_resolved WHERE workspace_id = ? AND record_id = ?")
                    .bind(workspace_id).bind(&id).fetch_optional(&mut *tx).await?;
                let Some(payload) = payload else {
                    break;
                };
                let opened = keyring.open_field(workspace_id, &id, &payload)?;
                let Some(chunk) = opened.value.as_array() else {
                    break;
                };
                chunks.push(chunk.clone());
            }
            if chunks.len() != count as usize {
                continue;
            }
            value = join_chunks(&chunks);
        }
        if name == super::ROW_MANIFEST_FIELD
            || !super::replica_apply::field_keeps_conflict_copies(name)
        {
            continue;
        }
        let tag =
            keyring
                .active()
                .value_tag(&field.table, &field.row_id, name, field.deleted, &value);
        record_conflict(
            &mut tx,
            &ConflictCopy {
                id: format!("cloud:{workspace_id}:{}:{name}:{tag}", field.row_id),
                workspace_id,
                table_name: &field.table,
                row_id: &field.row_id,
                field_name: name,
                lost_side: ConflictLoser::Remote,
                writer_id: &field.writer_id,
                revision: i64::try_from(field.revision)
                    .map_err(|_| E2eeReplicaError::InvalidRow)?,
                edited_at_ms: field.edited_at_ms.and_then(|v| i64::try_from(v).ok()),
                value: &value,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn rebase_e2ee_cloud_documents(
    pool: &SqlitePool,
    workspace_id: &str,
    keyring: &anlg_e2ee::WorkspaceKeyring,
) -> E2eeReplicaResult<()> {
    use super::replica_storage::{
        insert_apply_guard, read_field, remove_apply_guard, restore_local_payload, update_field,
        upsert_local_state,
    };
    use anlg_tiptap::merge::{MergeSide, merge_documents};
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let in_flight: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM e2ee_cloud_batches WHERE workspace_id = ?)",
    )
    .bind(workspace_id)
    .fetch_one(&mut *tx)
    .await?;
    if in_flight {
        return Ok(());
    }
    let documents: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT pending.record_id, pending.payload, pending.base_payload, witness.payload_hash, witness.payload
         FROM e2ee_cloud_outbox AS pending JOIN e2ee_witness_records_resolved AS witness
           ON witness.workspace_id = pending.workspace_id AND witness.record_id = pending.record_id
         WHERE pending.workspace_id = ? AND pending.table_name = 'session_documents'
           AND (pending.table_name, pending.row_id) = (
             SELECT table_name, row_id FROM e2ee_cloud_outbox WHERE workspace_id = pending.workspace_id
             ORDER BY CASE table_name WHEN 'session_documents' THEN 1 WHEN 'transcripts' THEN 2 ELSE 0 END, table_name, row_id LIMIT 1)
           AND pending.base_payload IS NOT NULL AND witness.payload_hash IS NOT pending.base_payload_hash
           AND witness.payload_hash != pending.payload_hash")
        .bind(workspace_id).fetch_all(&mut *tx).await?;
    for (record_id, local_payload, base_payload, remote_hash, remote_payload) in documents {
        let local = keyring.open_field(workspace_id, &record_id, &local_payload)?;
        if local.field != "body" {
            continue;
        }
        let base = keyring.open_field(workspace_id, &record_id, &base_payload)?;
        let remote = keyring.open_field(workspace_id, &record_id, &remote_payload)?;
        let parse = |v: &serde_json::Value| {
            v.as_str()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        };
        let Some(current) = read_field(
            &mut tx,
            &local.table,
            workspace_id,
            &local.row_id,
            &local.field,
        )
        .await?
        else {
            continue;
        };
        let (Some(base_doc), Some(local_doc), Some(remote_doc)) =
            (parse(&base.value), parse(&current), parse(&remote.value))
        else {
            continue;
        };
        let Some(merged) = merge_documents(&base_doc, &local_doc, &remote_doc, MergeSide::Local)
        else {
            continue;
        };
        if !merged.conflicts.is_empty() {
            super::conflicts::record_conflict(
                &mut tx,
                &super::conflicts::ConflictCopy {
                    id: format!("{record_id}:{remote_hash}"),
                    workspace_id,
                    table_name: &remote.table,
                    row_id: &remote.row_id,
                    field_name: &remote.field,
                    lost_side: super::conflicts::ConflictLoser::Remote,
                    writer_id: &remote.writer_id,
                    revision: i64::try_from(remote.revision)
                        .map_err(|_| E2eeReplicaError::InvalidRow)?,
                    edited_at_ms: remote.edited_at_ms.and_then(|v| i64::try_from(v).ok()),
                    value: &remote.value,
                },
            )
            .await?;
        }
        let value = serde_json::Value::String(
            serde_json::to_string(&merged.doc).map_err(|_| E2eeReplicaError::InvalidRow)?,
        );
        let revision = local
            .revision
            .max(remote.revision)
            .checked_add(1)
            .ok_or(E2eeReplicaError::InvalidRow)?;
        let key = keyring
            .get(&local.key_id)
            .ok_or(E2eeReplicaError::InvalidRow)?;
        let sealed = key.seal_field_at(
            workspace_id,
            &local.table,
            &local.row_id,
            &local.field,
            &local.writer_id,
            revision,
            local.deleted,
            local.edited_at_ms,
            value.clone(),
        )?;
        let state = LocalState {
            record_id: sealed.record_id,
            workspace_id: workspace_id.to_string(),
            table_name: local.table,
            row_id: local.row_id,
            field_name: local.field,
            revision: i64::try_from(revision).map_err(|_| E2eeReplicaError::InvalidRow)?,
            writer_id: local.writer_id,
            value_tag: key.value_tag(
                &remote.table,
                &remote.row_id,
                &remote.field,
                local.deleted,
                &value,
            ),
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
            edited_at_ms: local.edited_at_ms.and_then(|v| i64::try_from(v).ok()),
            republish: false,
        };
        {
            insert_apply_guard(&mut tx, workspace_id, &state.table_name, &state.row_id).await?;
            update_field(
                &mut tx,
                &state.table_name,
                workspace_id,
                &state.row_id,
                &state.field_name,
                &value,
            )
            .await?;
            restore_local_payload(&mut tx, &state).await?;
            upsert_local_state(&mut tx, &state).await?;
            remove_apply_guard(&mut tx, workspace_id, &state.table_name, &state.row_id).await?;
        }
        sqlx::query("UPDATE e2ee_cloud_outbox SET payload_hash = ?, payload = ?, base_payload_hash = ?, base_payload = ? WHERE workspace_id = ? AND record_id = ?")
            .bind(&state.payload_hash).bind(&state.payload).bind(&remote_hash).bind(&remote_payload)
            .bind(workspace_id).bind(&record_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn set_e2ee_cloud_pull_in_progress(
    pool: &SqlitePool,
    workspace_id: &str,
    active: bool,
) -> E2eeReplicaResult<()> {
    sqlx::query("UPDATE e2ee_cloud_authority SET pull_in_progress = ? WHERE workspace_id = ?")
        .bind(active)
        .bind(workspace_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn e2ee_cloud_accepted_head(
    pool: &SqlitePool,
    workspace_id: &str,
) -> E2eeReplicaResult<u64> {
    let head: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) FROM e2ee_witness_records WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    u64::try_from(head).map_err(|_| E2eeReplicaError::RollbackDetected)
}
