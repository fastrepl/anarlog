use std::collections::HashSet;

use anlg_e2ee::{SnapshotRecord, WorkspaceKeyring, WorkspaceSnapshotPayload};
use sqlx::SqlitePool;

use super::authority::configure_e2ee_cloud_authority;
use super::replica_storage::upsert_replica_payload_hash;
use super::witness::{
    WitnessRecord, e2ee_witness_cursor, upsert_witness_record_in_batch,
    validate_opened_witness_field,
};
use super::{E2eeReplicaError, E2eeReplicaResult, check_e2ee_cancellation, yield_once};

const SNAPSHOT_IMPORT_TX_BATCH: usize = 64;

pub async fn export_e2ee_workspace_snapshot(
    pool: &SqlitePool,
    workspace_id: &str,
) -> E2eeReplicaResult<Option<WorkspaceSnapshotPayload>> {
    let cursor = e2ee_witness_cursor(pool, workspace_id).await?;
    if cursor == 0 {
        return Ok(None);
    }
    let cloud_authority_after: Option<Option<i64>> = sqlx::query_scalar(
        "SELECT after_sequence FROM e2ee_cloud_authority WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;
    let cloud_authority_after = cloud_authority_after
        .flatten()
        .map(u64::try_from)
        .transpose()
        .map_err(|_| E2eeReplicaError::RollbackDetected)?;

    let rows: Vec<(String, i64, String, String, String, i64)> = sqlx::query_as(
        "SELECT record_id, revision, writer_id, payload_hash, payload, sequence
         FROM e2ee_witness_records_resolved
         WHERE workspace_id = ?
         ORDER BY sequence, record_id",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    let records = rows
        .into_iter()
        .filter(|(_, _, _, _, payload, _)| !payload.is_empty())
        .map(
            |(record_id, revision, writer_id, payload_hash, payload, sequence)| SnapshotRecord {
                record_id,
                revision,
                writer_id,
                payload_hash,
                payload,
                sequence,
            },
        )
        .collect();

    Ok(Some(WorkspaceSnapshotPayload {
        workspace_id: workspace_id.to_string(),
        snapshot_sequence: cursor,
        cloud_authority_after,
        records,
    }))
}

pub async fn import_e2ee_workspace_snapshot(
    pool: &SqlitePool,
    keyring: &WorkspaceKeyring,
    snapshot: &WorkspaceSnapshotPayload,
) -> E2eeReplicaResult<usize> {
    import_e2ee_workspace_snapshot_cancellable(pool, keyring, snapshot, || false).await
}

pub async fn import_e2ee_workspace_snapshot_cancellable(
    pool: &SqlitePool,
    keyring: &WorkspaceKeyring,
    snapshot: &WorkspaceSnapshotPayload,
    is_cancelled: impl Fn() -> bool + Sync,
) -> E2eeReplicaResult<usize> {
    check_e2ee_cancellation(&is_cancelled)?;
    if snapshot.snapshot_sequence == 0 {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    let cursor = e2ee_witness_cursor(pool, &snapshot.workspace_id).await?;
    if snapshot.snapshot_sequence < cursor {
        return Err(E2eeReplicaError::RollbackDetected);
    }
    if snapshot.snapshot_sequence == cursor && cursor > 0 {
        if snapshot.cloud_authority_after.is_some() {
            configure_e2ee_cloud_authority(
                pool,
                &snapshot.workspace_id,
                snapshot.cloud_authority_after,
            )
            .await?;
        }
        return Ok(0);
    }

    let mut seen_record_ids = HashSet::with_capacity(snapshot.records.len());
    let mut validated = Vec::with_capacity(snapshot.records.len());
    for record in &snapshot.records {
        check_e2ee_cancellation(&is_cancelled)?;
        if !seen_record_ids.insert(record.record_id.as_str())
            || record.sequence <= 0
            || u64::try_from(record.sequence)
                .ok()
                .is_none_or(|seq| seq > snapshot.snapshot_sequence)
        {
            return Err(E2eeReplicaError::InvalidRow);
        }
        let field =
            keyring.open_field(&snapshot.workspace_id, &record.record_id, &record.payload)?;
        check_e2ee_cancellation(&is_cancelled)?;
        validate_opened_witness_field(
            &field,
            &record.payload_hash,
            &record.payload,
            record.revision,
            &record.writer_id,
        )?;
        validated.push(WitnessRecord {
            workspace_id: snapshot.workspace_id.clone(),
            record_id: record.record_id.clone(),
            revision: record.revision,
            writer_id: record.writer_id.clone(),
            payload_hash: record.payload_hash.clone(),
            payload: record.payload.clone(),
            sequence: record.sequence,
        });
        yield_once().await;
    }

    if snapshot.cloud_authority_after.is_some() {
        configure_e2ee_cloud_authority(
            pool,
            &snapshot.workspace_id,
            snapshot.cloud_authority_after,
        )
        .await?;
    }

    let has_outbox: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_cloud_outbox WHERE workspace_id = ?)")
            .bind(&snapshot.workspace_id)
            .fetch_one(pool)
            .await?;
    let has_local_or_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM e2ee_local_state WHERE workspace_id = ?)
             OR EXISTS(SELECT 1 FROM e2ee_witness_pending WHERE workspace_id = ?)",
    )
    .bind(&snapshot.workspace_id)
    .bind(&snapshot.workspace_id)
    .fetch_one(pool)
    .await?;

    for chunk in validated.chunks(SNAPSHOT_IMPORT_TX_BATCH) {
        check_e2ee_cancellation(&is_cancelled)?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(error) = check_e2ee_cancellation(&is_cancelled) {
            transaction.rollback().await?;
            return Err(error);
        }
        for record in chunk {
            let in_outbox: bool = if has_outbox {
                sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM e2ee_cloud_outbox WHERE workspace_id = ? AND record_id = ? AND payload_hash != ?)",
                )
                .bind(&record.workspace_id)
                .bind(&record.record_id)
                .bind(&record.payload_hash)
                .fetch_one(&mut *transaction)
                .await?
            } else {
                false
            };

            if !in_outbox {
                let result = sqlx::query(
                    "INSERT INTO e2ee_records (id, workspace_id, payload)
                     VALUES (?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET
                       payload = excluded.payload,
                       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE e2ee_records.workspace_id = excluded.workspace_id
                       AND e2ee_records.payload != excluded.payload",
                )
                .bind(&record.record_id)
                .bind(&record.workspace_id)
                .bind(&record.payload)
                .execute(&mut *transaction)
                .await?;
                if result.rows_affected() != 1 {
                    let current: Option<(String, String)> = sqlx::query_as(
                        "SELECT workspace_id, payload FROM e2ee_records WHERE id = ?",
                    )
                    .bind(&record.record_id)
                    .fetch_optional(&mut *transaction)
                    .await?;
                    if !current.is_some_and(|(workspace_id, payload)| {
                        workspace_id == record.workspace_id && payload == record.payload
                    }) {
                        transaction.rollback().await?;
                        return Err(E2eeReplicaError::RollbackDetected);
                    }
                }
                upsert_replica_payload_hash(
                    &mut transaction,
                    &record.workspace_id,
                    &record.record_id,
                    &record.payload_hash,
                    &record.payload,
                )
                .await?;
            }

            let outbox_hashes = if has_outbox {
                vec![record.payload_hash.clone()]
            } else {
                Vec::new()
            };
            upsert_witness_record_in_batch(
                &mut transaction,
                record,
                &outbox_hashes,
                has_local_or_pending,
            )
            .await?;

            if !in_outbox {
                // `e2ee_records` already matches the imported witness record; leave `e2ee_replica_pending`
                // intact so hydration materializes domain rows on the next apply pass.
                sqlx::query("DELETE FROM e2ee_witness_repair_pending WHERE record_id = ?")
                    .bind(&record.record_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            if let Err(error) = check_e2ee_cancellation(&is_cancelled) {
                transaction.rollback().await?;
                return Err(error);
            }
        }
        transaction.commit().await?;
        check_e2ee_cancellation(&is_cancelled)?;
    }

    super::witness::advance_e2ee_witness_cursor(
        pool,
        &snapshot.workspace_id,
        snapshot.snapshot_sequence,
    )
    .await?;
    Ok(validated.len())
}
