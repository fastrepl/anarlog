use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anlg_e2ee::{OpenedField, WorkspaceKey};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, QueryBuilder, Row, Sqlite, SqlitePool, Transaction, TypeInfo, ValueRef};

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
const E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT: i64 = 128;
const E2EE_WITNESS_REPAIR_SCAN_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const PENDING_E2EE_WITNESS_UPLOADS_SQL: &str = "
  SELECT
    pending.record_id,
    LENGTH(CAST(local.payload AS BLOB))
      + LENGTH(CAST(local.record_id AS BLOB))
      + LENGTH(CAST(local.payload_hash AS BLOB))
      + 256
  FROM e2ee_witness_pending AS pending
  INDEXED BY idx_e2ee_witness_pending_workspace_record
  CROSS JOIN e2ee_local_state AS local
  WHERE pending.workspace_id = ?
    AND local.record_id = pending.record_id
    AND local.workspace_id = pending.workspace_id
  ORDER BY pending.record_id
  LIMIT ?";
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

#[cfg(test)]
mod witness_cancellation_tests {
    use super::*;
    use anlg_e2ee::RecoveryKey;
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn test_db() -> anlg_db_core::Db {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    fn workspace_key() -> WorkspaceKey {
        RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
            .unwrap()
            .workspace_key("workspace-a")
            .unwrap()
    }

    fn witness_events(key: &WorkspaceKey, count: usize) -> Vec<E2eeWitnessEvent> {
        (0..count)
            .map(|index| {
                let row_id = format!("session-{index:03}");
                let sealed = key
                    .seal_field(
                        "workspace-a",
                        "sessions",
                        &row_id,
                        "title",
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        1,
                        false,
                        json!(format!("Witness {index}")),
                    )
                    .unwrap();
                E2eeWitnessEvent {
                    sequence: u64::try_from(index + 1).unwrap(),
                    record_id: sealed.record_id,
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
                    payload: sealed.payload,
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn cancelled_witness_merge_releases_local_writes_without_advancing_the_cursor() {
        let db = test_db().await;
        let key = workspace_key();
        let events = witness_events(&key, 4);
        let checks = AtomicUsize::new(0);
        let cancel_after_first_commit = events.len() * 2 + 3;

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            merge_e2ee_witness_events_cancellable(db.pool(), &key, "workspace-a", &events, || {
                checks.fetch_add(1, Ordering::SeqCst) >= cancel_after_first_commit
            }),
        )
        .await
        .expect("witness merge cancellation exceeded the activity deadline")
        .unwrap_err();

        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            e2ee_witness_cursor(db.pool(), "workspace-a").await.unwrap(),
            0
        );
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-merge-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled witness merge kept the database busy")
        .unwrap();

        merge_e2ee_witness_events(db.pool(), &key, "workspace-a", &events)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            i64::try_from(events.len()).unwrap()
        );
    }

    #[tokio::test]
    async fn cancelled_witness_repair_finishes_one_record_and_releases_local_writes() {
        let db = test_db().await;
        let key = workspace_key();
        let workspace_keys = HashMap::from([("workspace-a".to_string(), key.clone())]);
        let events = witness_events(&key, 4);
        merge_e2ee_witness_events(db.pool(), &key, "workspace-a", &events)
            .await
            .unwrap();
        let records: Vec<WitnessRecord> = sqlx::query_as(
            "SELECT workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
             FROM e2ee_witness_records
             ORDER BY workspace_id, record_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let checks = AtomicUsize::new(0);

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            persist_e2ee_witness_repairs(db.pool(), &workspace_keys, &records, &|| {
                checks.fetch_add(1, Ordering::SeqCst) >= 5
            }),
        )
        .await
        .expect("witness repair cancellation exceeded the activity deadline")
        .unwrap_err();

        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-repair-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled witness repair kept the database busy")
        .unwrap();

        repair_e2ee_replica_from_witness_bounded(db.pool(), &workspace_keys, true, 4, usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            i64::try_from(events.len()).unwrap()
        );
    }

    #[tokio::test]
    async fn cancelled_encryption_rolls_back_the_current_row_and_releases_local_writes() {
        let db = test_db().await;
        let key = workspace_key();
        let workspace_keys = HashMap::from([("workspace-a".to_string(), key.clone())]);
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Encrypt me')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let writer_id = {
            let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
            let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
            transaction.commit().await.unwrap();
            writer_id
        };
        let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let prepared = prepare_dirty_row(db.pool(), &key, &writer_id, dirty)
            .await
            .unwrap();
        let checks = AtomicUsize::new(0);
        let cancel_after_first_replica_write = 5 + prepared.fields.len() * 3;

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            persist_prepared_dirty_row_cancellable(db.pool(), prepared, false, &|| {
                checks.fetch_add(1, Ordering::SeqCst) >= cancel_after_first_replica_write
            }),
        )
        .await
        .expect("encryption cancellation exceeded the activity deadline")
        .unwrap_err();

        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_local_state")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM e2ee_dirty_rows WHERE row_id = 'session-1'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            1
        );
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-encrypt-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled encryption kept the database busy")
        .unwrap();

        encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 1)
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM e2ee_records WHERE workspace_id = 'workspace-a'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap()
                > 0
        );
    }

    #[tokio::test]
    async fn cancelled_all_match_witness_scan_stays_bounded_and_releases_local_writes() {
        let db = test_db().await;
        let key = workspace_key();
        let workspace_keys = HashMap::from([("workspace-a".to_string(), key)]);
        let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "WITH digits(value) AS (
               VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9)
             ),
             rows(value) AS (
               SELECT a.value * 1000 + b.value * 100 + c.value * 10 + d.value
               FROM digits AS a
               CROSS JOIN digits AS b
               CROSS JOIN digits AS c
               CROSS JOIN digits AS d
             )
             INSERT INTO e2ee_witness_records (
               workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
             )
             SELECT
               'workspace-a',
               printf('record-%06d', value),
               1,
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'matching-hash',
               'matching-payload',
               value + 1
             FROM rows",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT record_id, workspace_id, payload
             FROM e2ee_witness_records",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        let checks = AtomicUsize::new(0);

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            has_pending_e2ee_witness_repairs_cancellable(db.pool(), &workspace_keys, true, || {
                checks.fetch_add(1, Ordering::SeqCst) >= 5
            }),
        )
        .await
        .expect("all-match witness cancellation exceeded the activity deadline")
        .unwrap_err();

        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert!(checks.load(Ordering::SeqCst) <= 7);
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-witness-scan-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled witness scan kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn cancelled_witness_upload_processing_releases_local_writes() {
        let db = test_db().await;
        let key = workspace_key();
        let workspace_keys = HashMap::from([("workspace-a".to_string(), key.clone())]);
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Publish me')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let uploads = pending_e2ee_witness_uploads(db.pool(), "workspace-a", &key, 16, usize::MAX)
            .await
            .unwrap();
        assert!(!uploads.is_empty());

        let pending_checks = AtomicUsize::new(0);
        let pending_cancel_after_first_decrypt = uploads.len() + 7;
        let error = pending_e2ee_witness_uploads_cancellable(
            db.pool(),
            "workspace-a",
            &key,
            16,
            usize::MAX,
            || pending_checks.fetch_add(1, Ordering::SeqCst) >= pending_cancel_after_first_decrypt,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, E2eeReplicaError::Cancelled));
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-upload-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled witness upload loading kept the database busy")
        .unwrap();

        let acknowledge_checks = AtomicUsize::new(0);
        let acknowledge_cancel_after_first_commit = uploads.len() * 2 + 3;
        let error = acknowledge_e2ee_witness_uploads_cancellable(db.pool(), &key, &uploads, || {
            acknowledge_checks.fetch_add(1, Ordering::SeqCst)
                >= acknowledge_cancel_after_first_commit
        })
        .await
        .unwrap_err();
        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_records")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-ack-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled witness acknowledgement kept the database busy")
        .unwrap();
    }
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
         FROM e2ee_local_state
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

    let (records, selected_more) = load_bounded_e2ee_witness_repairs(
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
        remaining: selected_more,
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
    let (records, _) =
        load_bounded_e2ee_witness_repairs(pool, keys, include_missing, 1, usize::MAX, is_cancelled)
            .await?;
    Ok(!records.is_empty())
}

async fn load_bounded_e2ee_witness_repairs(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    include_missing: bool,
    max_records: i64,
    max_bytes: usize,
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<(Vec<WitnessRecord>, bool)> {
    check_e2ee_cancellation(is_cancelled)?;
    if keys.is_empty() || max_records <= 0 {
        return Ok((Vec::new(), false));
    }
    let max_records =
        usize::try_from(max_records).map_err(|_| E2eeReplicaError::WitnessRepairTooLarge)?;
    let mut workspace_ids = keys.keys().collect::<Vec<_>>();
    workspace_ids.sort_unstable();
    let mut selected = Vec::with_capacity(
        max_records.min(
            usize::try_from(E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT)
                .map_err(|_| E2eeReplicaError::InvalidRow)?,
        ),
    );
    let mut selected_bytes = 0_usize;
    let mut after: Option<(String, String)> = None;
    loop {
        check_e2ee_cancellation(is_cancelled)?;
        let mut metadata_query = QueryBuilder::<Sqlite>::new(
            "SELECT
               workspace_id,
               record_id,
               LENGTH(CAST(workspace_id AS BLOB))
                 + LENGTH(CAST(record_id AS BLOB))
                 + LENGTH(CAST(payload AS BLOB))
                 + 256
             FROM e2ee_witness_records
             WHERE workspace_id IN (",
        );
        {
            let mut separated = metadata_query.separated(", ");
            for workspace_id in &workspace_ids {
                separated.push_bind(workspace_id);
            }
        }
        metadata_query.push(")");
        if let Some((after_workspace_id, after_record_id)) = &after {
            metadata_query
                .push(" AND (workspace_id > ")
                .push_bind(after_workspace_id)
                .push(" OR (workspace_id = ")
                .push_bind(after_workspace_id)
                .push(" AND record_id > ")
                .push_bind(after_record_id)
                .push("))");
        }
        metadata_query
            .push(" ORDER BY workspace_id, record_id LIMIT ")
            .push_bind(E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT);
        let metadata: Vec<(String, String, i64)> =
            metadata_query.build_query_as().fetch_all(pool).await?;
        check_e2ee_cancellation(is_cancelled)?;
        if metadata.is_empty() {
            return Ok((selected, false));
        }
        let page_is_full = metadata.len()
            == usize::try_from(E2EE_WITNESS_REPAIR_SCAN_PAGE_LIMIT)
                .map_err(|_| E2eeReplicaError::InvalidRow)?;
        let page_after = metadata
            .last()
            .map(|(workspace_id, record_id, _)| (workspace_id.clone(), record_id.clone()))
            .ok_or(E2eeReplicaError::InvalidRow)?;

        let mut offset = 0;
        while offset < metadata.len() {
            check_e2ee_cancellation(is_cancelled)?;
            let mut end = offset;
            let mut scan_bytes = 0_usize;
            while end < metadata.len() {
                let record_bytes =
                    usize::try_from(metadata[end].2).map_err(|_| E2eeReplicaError::InvalidRow)?;
                if end > offset
                    && scan_bytes.saturating_add(record_bytes) > E2EE_WITNESS_REPAIR_SCAN_BYTE_LIMIT
                {
                    break;
                }
                scan_bytes = scan_bytes.saturating_add(record_bytes);
                end += 1;
                if scan_bytes >= E2EE_WITNESS_REPAIR_SCAN_BYTE_LIMIT {
                    break;
                }
            }

            let chunk = &metadata[offset..end];
            let mut records_query = QueryBuilder::<Sqlite>::new(
                "SELECT workspace_id, record_id, revision, writer_id, payload_hash, payload,
                        sequence
                 FROM e2ee_witness_records
                 WHERE ",
            );
            for (index, (workspace_id, record_id, _)) in chunk.iter().enumerate() {
                if index > 0 {
                    records_query.push(" OR ");
                }
                records_query
                    .push("(workspace_id = ")
                    .push_bind(workspace_id)
                    .push(" AND record_id = ")
                    .push_bind(record_id)
                    .push(")");
            }
            records_query.push(" ORDER BY workspace_id, record_id");
            let records: Vec<WitnessRecord> =
                records_query.build_query_as().fetch_all(pool).await?;
            check_e2ee_cancellation(is_cancelled)?;
            if records.len() != chunk.len()
                || !records
                    .iter()
                    .zip(chunk)
                    .all(|(record, (workspace_id, record_id, _))| {
                        &record.workspace_id == workspace_id && &record.record_id == record_id
                    })
            {
                return Err(E2eeReplicaError::InvalidRow);
            }

            let mut replica_query = QueryBuilder::<Sqlite>::new(
                "SELECT id, workspace_id, payload FROM e2ee_records WHERE id IN (",
            );
            {
                let mut separated = replica_query.separated(", ");
                for record in &records {
                    separated.push_bind(&record.record_id);
                }
            }
            replica_query.push(")");
            let replicas: Vec<(String, String, String)> =
                replica_query.build_query_as().fetch_all(pool).await?;
            check_e2ee_cancellation(is_cancelled)?;
            let replicas = replicas
                .into_iter()
                .map(|(record_id, workspace_id, payload)| (record_id, (workspace_id, payload)))
                .collect::<HashMap<_, _>>();

            for (record, (_, _, record_bytes)) in records.into_iter().zip(chunk) {
                check_e2ee_cancellation(is_cancelled)?;
                let needs_repair = match replicas.get(&record.record_id) {
                    None => include_missing,
                    Some((workspace_id, payload)) => {
                        workspace_id != &record.workspace_id || payload != &record.payload
                    }
                };
                if !needs_repair {
                    continue;
                }
                if selected.len() >= max_records {
                    return Ok((selected, true));
                }
                let record_bytes =
                    usize::try_from(*record_bytes).map_err(|_| E2eeReplicaError::InvalidRow)?;
                if selected_bytes.saturating_add(record_bytes) > max_bytes {
                    if selected.is_empty() {
                        return Err(E2eeReplicaError::WitnessRepairTooLarge);
                    }
                    return Ok((selected, true));
                }
                selected_bytes = selected_bytes.saturating_add(record_bytes);
                selected.push(record);
                if selected.len() >= max_records {
                    return Ok((selected, true));
                }
            }
            offset = end;
            yield_once().await;
        }

        if !page_is_full {
            return Ok((selected, false));
        }
        after = Some(page_after);
        yield_once().await;
    }
}

async fn persist_e2ee_witness_repairs(
    pool: &SqlitePool,
    keys: &HashMap<String, WorkspaceKey>,
    records: &[WitnessRecord],
    is_cancelled: &(impl Fn() -> bool + Sync),
) -> E2eeReplicaResult<u64> {
    let mut repaired_records = 0_u64;
    for record in records {
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
             FROM e2ee_witness_records
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
            transaction.commit().await?;
            check_e2ee_cancellation(is_cancelled)?;
            continue;
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
        .bind(&record.record_id)
        .bind(&record.workspace_id)
        .bind(&record.payload)
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
        transaction.commit().await?;
        check_e2ee_cancellation(is_cancelled)?;
    }
    Ok(repaired_records)
}

async fn upsert_witness_record(
    transaction: &mut Transaction<'_, Sqlite>,
    record: &WitnessRecord,
) -> E2eeReplicaResult<()> {
    sqlx::query(
        "INSERT INTO e2ee_witness_records (
           workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
         ) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, record_id) DO UPDATE SET
           revision = excluded.revision,
           writer_id = excluded.writer_id,
           payload_hash = excluded.payload_hash,
           payload = excluded.payload,
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
    .bind(&record.payload)
    .bind(record.sequence)
    .execute(&mut **transaction)
    .await?;
    reconcile_e2ee_witness_pending(transaction, &record.record_id).await?;
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
mod tests {
    use super::*;
    use anlg_e2ee::RecoveryKey;

    async fn test_db() -> anlg_db_core::Db {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    fn keys(workspace_id: &str) -> HashMap<String, WorkspaceKey> {
        let recovery =
            RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
                .unwrap();
        HashMap::from([(
            workspace_id.to_string(),
            recovery.workspace_key(workspace_id).unwrap(),
        )])
    }

    async fn copy_replica(source: &SqlitePool, target: &SqlitePool) {
        let records: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, workspace_id, payload FROM e2ee_records")
                .fetch_all(source)
                .await
                .unwrap();
        for (id, workspace_id, payload) in records {
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            )
            .bind(id)
            .bind(workspace_id)
            .bind(payload)
            .execute(target)
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn encrypts_only_opaque_records() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Secret planning')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let stats = encrypt_e2ee_replica_changes(db.pool(), &keys("workspace-a"))
            .await
            .unwrap();
        let payloads: Vec<String> = sqlx::query_scalar("SELECT payload FROM e2ee_records")
            .fetch_all(db.pool())
            .await
            .unwrap();

        assert!(stats.encrypted_fields > 1);
        assert!(!payloads.is_empty());
        assert!(payloads.iter().all(|payload| {
            !payload.contains("Secret planning")
                && !payload.contains("session-1")
                && !payload.contains("sessions")
        }));
    }

    #[tokio::test]
    async fn encrypts_and_reconstructs_attachment_metadata() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO session_attachments (
               id, workspace_id, session_id, filename, relative_path,
               content_type, size_bytes, sha256, source_type, source_id
             ) VALUES (
               'attachment-1', 'workspace-a', 'session-1', 'secret-diagram.png',
               'attachments/secret-diagram.png', 'image/png', 42,
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'note_upload', 'secret-diagram.png'
             )",
        )
        .execute(source.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO attachment_local_state (
               attachment_id, session_id, relative_path, availability
             ) VALUES (
               'attachment-1', 'session-1', 'local-only-secret.png', 'present'
             )",
        )
        .execute(source.pool())
        .await
        .unwrap();

        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let payloads: Vec<String> = sqlx::query_scalar(
            "SELECT payload FROM e2ee_records WHERE workspace_id = 'workspace-a'",
        )
        .fetch_all(source.pool())
        .await
        .unwrap();
        assert!(!payloads.is_empty());
        assert!(payloads.iter().all(|payload| {
            !payload.contains("secret-diagram.png")
                && !payload.contains("attachments/secret-diagram.png")
                && !payload.contains("local-only-secret.png")
                && !payload
                    .contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        }));

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let attachment: (String, String, String, i64, String) = sqlx::query_as(
            "SELECT filename, relative_path, content_type, size_bytes, sha256
             FROM session_attachments WHERE id = 'attachment-1'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        assert_eq!(
            attachment,
            (
                "secret-diagram.png".to_string(),
                "attachments/secret-diagram.png".to_string(),
                "image/png".to_string(),
                42,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            )
        );
        let local_state_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM attachment_local_state")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(local_state_count, 0);
    }

    #[tokio::test]
    async fn reconstructs_every_protected_table_and_applies_deletions() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        for (table, id) in [
            ("action_items", "action-1"),
            ("humans", "human-1"),
            ("organizations", "organization-1"),
            ("session_attachments", "attachment-1"),
            ("session_documents", "document-1"),
            ("session_participants", "participant-1"),
            ("sessions", "session-1"),
            ("transcripts", "transcript-1"),
        ] {
            let sql = format!("INSERT INTO {table} (id, workspace_id) VALUES (?, 'workspace-a')");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .execute(source.pool())
                .await
                .unwrap();
        }
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        for table in E2EE_DOMAIN_TABLES {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE workspace_id = 'workspace-a'");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .fetch_one(target.pool())
                .await
                .unwrap();
            assert_eq!(count, 1, "{table} was not reconstructed");
        }

        sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[tokio::test]
    async fn applies_remote_changes_and_preserves_concurrent_local_edits() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        let records: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, workspace_id, payload FROM e2ee_records")
                .fetch_all(source.pool())
                .await
                .unwrap();
        for (id, workspace_id, payload) in records {
            sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
                .bind(id)
                .bind(workspace_id)
                .bind(payload)
                .execute(target.pool())
                .await
                .unwrap();
        }
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(title, "First");

        sqlx::query("UPDATE sessions SET title = 'Remote' WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let remote_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(remote_payload)
            .bind(title_record_id)
            .execute(target.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(target.pool())
            .await
            .unwrap();

        let stats = apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();

        assert_eq!(title, "Local");
        assert_eq!(stats.skipped_local_changes, 1);
    }

    #[tokio::test]
    async fn local_edits_advance_beyond_witnessed_remote_versions() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let remote_title = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "ffffffffffffffffffffffffffffffff",
                8,
                false,
                json!("Remote"),
            )
            .unwrap();
        let remote_tombstone = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                ROW_MANIFEST_FIELD,
                "ffffffffffffffffffffffffffffffff",
                8,
                true,
                Value::Null,
            )
            .unwrap();
        merge_e2ee_witness_events(
            db.pool(),
            key,
            "workspace-a",
            &[
                E2eeWitnessEvent {
                    sequence: 1,
                    record_id: remote_title.record_id,
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_title.payload),
                    payload: remote_title.payload,
                },
                E2eeWitnessEvent {
                    sequence: 2,
                    record_id: remote_tombstone.record_id,
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_tombstone.payload),
                    payload: remote_tombstone.payload,
                },
            ],
        )
        .await
        .unwrap();

        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let title_id = key.blind_field_id("sessions", "session-1", "title");
        let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
        let title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let title = key
            .open_field("workspace-a", &title_id, &title_payload)
            .unwrap();
        let manifest = key
            .open_field("workspace-a", &manifest_id, &manifest_payload)
            .unwrap();

        assert_eq!(title.revision, 9);
        assert_eq!(title.value, json!("Local"));
        assert_eq!(manifest.revision, 9);
        assert!(!manifest.deleted);
    }

    #[tokio::test]
    async fn encrypted_local_edit_advances_the_live_manifest() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
        let local_manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let local_manifest = key
            .open_field("workspace-a", &manifest_id, &local_manifest_payload)
            .unwrap();
        assert_eq!(local_manifest.revision, 2);

        let remote_tombstone = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                ROW_MANIFEST_FIELD,
                "00000000000000000000000000000000",
                2,
                true,
                Value::Null,
            )
            .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(remote_tombstone.payload)
            .bind(&manifest_id)
            .execute(db.pool())
            .await
            .unwrap();
        let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let repaired_manifest: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(manifest_id)
                .fetch_one(db.pool())
                .await
                .unwrap();

        assert_eq!(stats.rejected_rollbacks, 1);
        assert_eq!(title, "Local");
        assert_eq!(repaired_manifest, local_manifest_payload);
    }

    #[tokio::test]
    async fn local_transcript_edit_rebases_above_witnessed_field_and_tombstone() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let local_words = r#"[{"text":"preserve this local tail"}]"#;
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('transcript-1', 'workspace-a', '[{\"text\":\"base\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let words_id = key.blind_field_id("transcripts", "transcript-1", "words_json");
        let manifest_id = key.blind_field_id("transcripts", "transcript-1", ROW_MANIFEST_FIELD);
        let remote_words = key
            .seal_field(
                "workspace-a",
                "transcripts",
                "transcript-1",
                "words_json",
                "ffffffffffffffffffffffffffffffff",
                7,
                false,
                json!(r#"[{"text":"remote"}]"#),
            )
            .unwrap();
        let remote_tombstone = key
            .seal_field(
                "workspace-a",
                "transcripts",
                "transcript-1",
                ROW_MANIFEST_FIELD,
                "ffffffffffffffffffffffffffffffff",
                8,
                true,
                Value::Null,
            )
            .unwrap();
        merge_e2ee_witness_events(
            db.pool(),
            key,
            "workspace-a",
            &[
                E2eeWitnessEvent {
                    sequence: 1,
                    record_id: words_id.clone(),
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_words.payload),
                    payload: remote_words.payload.clone(),
                },
                E2eeWitnessEvent {
                    sequence: 2,
                    record_id: manifest_id.clone(),
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_tombstone.payload),
                    payload: remote_tombstone.payload.clone(),
                },
            ],
        )
        .await
        .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&remote_words.payload)
            .bind(&words_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&remote_tombstone.payload)
            .bind(&manifest_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE transcripts SET words_json = ? WHERE id = 'transcript-1'")
            .bind(local_words)
            .execute(db.pool())
            .await
            .unwrap();

        let skipped = apply_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys)
            .await
            .unwrap();
        assert!(skipped.skipped_local_changes > 0);
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let local_words_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&words_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let local_manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let rebased_words = key
            .open_field("workspace-a", &words_id, &local_words_payload)
            .unwrap();
        let rebased_manifest = key
            .open_field("workspace-a", &manifest_id, &local_manifest_payload)
            .unwrap();
        assert_eq!(rebased_words.value, json!(local_words));
        assert!(rebased_words.revision > 7);
        assert!(!rebased_manifest.deleted);
        assert!(rebased_manifest.revision > 8);

        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(remote_tombstone.payload)
            .bind(&manifest_id)
            .execute(db.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let preserved: String =
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(preserved, local_words);
    }

    #[tokio::test]
    async fn tombstoned_rows_can_be_recreated_with_retained_or_changed_fields() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        let target = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Same')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();

        for title in ["Same", "Changed"] {
            sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
                .execute(source.pool())
                .await
                .unwrap();
            encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
                .await
                .unwrap();
            copy_replica(source.pool(), target.pool()).await;
            apply_e2ee_replica_changes(target.pool(), &workspace_keys)
                .await
                .unwrap();
            let deleted: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
                    .fetch_one(target.pool())
                    .await
                    .unwrap();
            assert_eq!(deleted, 0);

            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('session-1', 'workspace-a', 'user-a', ?)",
            )
            .bind(title)
            .execute(source.pool())
            .await
            .unwrap();
            encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
                .await
                .unwrap();
            copy_replica(source.pool(), target.pool()).await;
            apply_e2ee_replica_changes(target.pool(), &workspace_keys)
                .await
                .unwrap();
            let materialized: String =
                sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
                    .fetch_one(target.pool())
                    .await
                    .unwrap();
            assert_eq!(materialized, title);
        }
    }

    #[tokio::test]
    async fn chunked_recreation_materializes_late_transcript_and_summary_fields() {
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let source = test_db().await;
        let target = test_db().await;
        let words = r#"[{"text":"the complete transcript"}]"#;
        let summary = r#"{"type":"doc","content":[{"type":"paragraph","text":"Full summary"}]}"#;
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('transcript-1', 'workspace-a', ?)",
        )
        .bind(words)
        .execute(source.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, workspace_id, kind, body)
             VALUES ('summary-1', 'workspace-a', 'summary', ?)",
        )
        .bind(summary)
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();

        let words_id = key.blind_field_id("transcripts", "transcript-1", "words_json");
        let body_id = key.blind_field_id("session_documents", "summary-1", "body");
        let transcript_manifest_id =
            key.blind_field_id("transcripts", "transcript-1", ROW_MANIFEST_FIELD);
        let summary_manifest_id =
            key.blind_field_id("session_documents", "summary-1", ROW_MANIFEST_FIELD);
        let words_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&words_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        let body_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&body_id)
                .fetch_one(source.pool())
                .await
                .unwrap();

        sqlx::query("DELETE FROM transcripts WHERE id = 'transcript-1'")
            .execute(source.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM session_documents WHERE id = 'summary-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();

        let transcript_tombstone_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&transcript_manifest_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        let summary_tombstone_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&summary_manifest_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        let transcript_tombstone = key
            .open_field(
                "workspace-a",
                &transcript_manifest_id,
                &transcript_tombstone_payload,
            )
            .unwrap();
        let summary_tombstone = key
            .open_field(
                "workspace-a",
                &summary_manifest_id,
                &summary_tombstone_payload,
            )
            .unwrap();
        let live_transcript = key
            .seal_field(
                "workspace-a",
                "transcripts",
                "transcript-1",
                ROW_MANIFEST_FIELD,
                "ffffffffffffffffffffffffffffffff",
                transcript_tombstone.revision + 1,
                false,
                json!(true),
            )
            .unwrap();
        let live_summary = key
            .seal_field(
                "workspace-a",
                "session_documents",
                "summary-1",
                ROW_MANIFEST_FIELD,
                "ffffffffffffffffffffffffffffffff",
                summary_tombstone.revision + 1,
                false,
                json!(true),
            )
            .unwrap();

        sqlx::query("DELETE FROM e2ee_records")
            .execute(target.pool())
            .await
            .unwrap();
        for (id, payload) in [
            (&transcript_manifest_id, &live_transcript.payload),
            (&summary_manifest_id, &live_summary.payload),
        ] {
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
            )
            .bind(id)
            .bind(payload)
            .execute(target.pool())
            .await
            .unwrap();
        }
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();

        let defaults: (String, String) = (
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(target.pool())
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(target.pool())
                .await
                .unwrap(),
        );
        assert_eq!(defaults, ("[]".to_string(), String::new()));

        for (id, payload) in [(&words_id, &words_payload), (&body_id, &body_payload)] {
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
            )
            .bind(id)
            .bind(payload)
            .execute(target.pool())
            .await
            .unwrap();
        }
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();

        let materialized: (String, String) = (
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
                .fetch_one(target.pool())
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(target.pool())
                .await
                .unwrap(),
        );
        assert_eq!(materialized, (words.to_string(), summary.to_string()));
    }

    #[tokio::test]
    async fn recreation_advances_retained_fields_beyond_remote_tombstone_state() {
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let local = test_db().await;
        let remote = test_db().await;
        for db in [&local, &remote] {
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('session-1', 'workspace-a', 'user-a', 'A')",
            )
            .execute(db.pool())
            .await
            .unwrap();
            encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
                .await
                .unwrap();
        }

        sqlx::query("UPDATE sessions SET title = 'B' WHERE id = 'session-1'")
            .execute(remote.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(remote.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
            .execute(remote.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(remote.pool(), &workspace_keys)
            .await
            .unwrap();

        let title_id = key.blind_field_id("sessions", "session-1", "title");
        let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
        let remote_title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_id)
                .fetch_one(remote.pool())
                .await
                .unwrap();
        let remote_manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(remote.pool())
                .await
                .unwrap();
        let remote_title = key
            .open_field("workspace-a", &title_id, &remote_title_payload)
            .unwrap();
        let remote_manifest = key
            .open_field("workspace-a", &manifest_id, &remote_manifest_payload)
            .unwrap();

        copy_replica(remote.pool(), local.pool()).await;
        apply_e2ee_replica_changes(local.pool(), &workspace_keys)
            .await
            .unwrap();
        merge_e2ee_witness_events(
            local.pool(),
            key,
            "workspace-a",
            &[
                E2eeWitnessEvent {
                    sequence: 1,
                    record_id: title_id.clone(),
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_title_payload),
                    payload: remote_title_payload,
                },
                E2eeWitnessEvent {
                    sequence: 2,
                    record_id: manifest_id.clone(),
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&remote_manifest_payload),
                    payload: remote_manifest_payload,
                },
            ],
        )
        .await
        .unwrap();
        let deleted: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
                .fetch_one(local.pool())
                .await
                .unwrap();
        assert_eq!(deleted, 0);

        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'A')",
        )
        .execute(local.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(local.pool(), &workspace_keys)
            .await
            .unwrap();

        let local_title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_id)
                .fetch_one(local.pool())
                .await
                .unwrap();
        let local_manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(local.pool())
                .await
                .unwrap();
        let local_title = key
            .open_field("workspace-a", &title_id, &local_title_payload)
            .unwrap();
        let local_manifest = key
            .open_field("workspace-a", &manifest_id, &local_manifest_payload)
            .unwrap();

        assert_eq!(local_title.value, json!("A"));
        assert!(local_title.revision > remote_title.revision);
        assert!(!local_manifest.deleted);
        assert!(local_manifest.revision > remote_manifest.revision);
    }

    #[tokio::test]
    async fn rejects_authenticated_payloads_in_the_wrong_workspace() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let sealed = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                ROW_MANIFEST_FIELD,
                "00000000000000000000000000000001",
                1,
                false,
                json!(true),
            )
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, 'workspace-b', ?)",
        )
        .bind(sealed.record_id)
        .bind(sealed.payload)
        .execute(db.pool())
        .await
        .unwrap();

        let mut wrong_keys = workspace_keys;
        wrong_keys.insert(
            "workspace-b".to_string(),
            RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
                .unwrap()
                .workspace_key("workspace-b")
                .unwrap(),
        );
        assert!(
            apply_e2ee_replica_changes(db.pool(), &wrong_keys)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_and_repairs_replayed_older_field_revisions() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let record_id =
            workspace_keys["workspace-a"].blind_field_id("sessions", "session-1", "title");
        let old_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();

        sqlx::query("UPDATE sessions SET title = 'Second' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let current_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(old_payload)
            .bind(&record_id)
            .execute(db.pool())
            .await
            .unwrap();

        let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let repaired_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stats.rejected_rollbacks, 1);
        assert_eq!(title, "Second");
        assert_eq!(repaired_payload, current_payload);
    }

    #[tokio::test]
    async fn equal_revision_payloads_converge_and_repair_the_replica() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let key = &workspace_keys["workspace-a"];
        let first = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                2,
                false,
                json!("Device A"),
            )
            .unwrap();
        let second = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                2,
                false,
                json!("Device B"),
            )
            .unwrap();
        let (winner, winner_title, replay) = (second, "Device B", first);

        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&winner.payload)
            .bind(&winner.record_id)
            .execute(db.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&replay.payload)
            .bind(&replay.record_id)
            .execute(db.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let canonical_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&winner.record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, winner_title);
        assert_eq!(canonical_payload, winner.payload);

        let third = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                3,
                false,
                json!("Clone A"),
            )
            .unwrap();
        let fourth = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                3,
                false,
                json!("Clone B"),
            )
            .unwrap();
        let (winner, winner_title, replay) =
            if anlg_e2ee::payload_hash(&third.payload) > anlg_e2ee::payload_hash(&fourth.payload) {
                (third, "Clone A", fourth)
            } else {
                (fourth, "Clone B", third)
            };

        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&winner.payload)
            .bind(&winner.record_id)
            .execute(db.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(&replay.payload)
            .bind(&replay.record_id)
            .execute(db.pool())
            .await
            .unwrap();
        let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let canonical_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&winner.record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stats.rejected_rollbacks, 1);
        assert_eq!(title, winner_title);
        assert_eq!(canonical_payload, winner.payload);
    }

    #[tokio::test]
    async fn writer_ids_are_stable_per_device_and_unique_across_devices() {
        let first = test_db().await;
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('first', 'workspace-a')")
            .execute(first.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(first.pool(), &keys("workspace-a"))
            .await
            .unwrap();
        let first_writer: String =
            sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
                .fetch_one(first.pool())
                .await
                .unwrap();
        encrypt_e2ee_replica_changes(first.pool(), &keys("workspace-a"))
            .await
            .unwrap();
        let repeated_writer: String =
            sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
                .fetch_one(first.pool())
                .await
                .unwrap();

        let second = test_db().await;
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('second', 'workspace-a')")
            .execute(second.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(second.pool(), &keys("workspace-a"))
            .await
            .unwrap();
        let second_writer: String =
            sqlx::query_scalar("SELECT writer_id FROM e2ee_local_device WHERE id = 'local'")
                .fetch_one(second.pool())
                .await
                .unwrap();

        assert_eq!(first_writer, repeated_writer);
        assert_ne!(first_writer, second_writer);
        assert_eq!(first_writer.len(), 32);
        assert_eq!(second_writer.len(), 32);
    }

    #[tokio::test]
    async fn fresh_device_does_not_materialize_an_unwitnessed_snapshot() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Archived value')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let fresh = test_db().await;
        copy_replica(source.pool(), fresh.pool()).await;
        let stats = apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
            .await
            .unwrap();

        let materialized: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
                .fetch_one(fresh.pool())
                .await
                .unwrap();
        assert_eq!(materialized, 0);
        assert!(stats.rejected_unwitnessed > 0);
    }

    #[tokio::test]
    async fn witnessed_snapshot_materializes_and_repairs_the_replica() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Witnessed value')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let encrypted: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, payload FROM e2ee_records WHERE workspace_id = 'workspace-a' ORDER BY id",
        )
        .fetch_all(source.pool())
        .await
        .unwrap();
        let events = encrypted
            .iter()
            .enumerate()
            .map(|(index, (record_id, payload))| E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: record_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(payload),
                payload: payload.clone(),
            })
            .collect::<Vec<_>>();

        let fresh = test_db().await;
        copy_replica(source.pool(), fresh.pool()).await;
        merge_e2ee_witness_events(
            fresh.pool(),
            &workspace_keys["workspace-a"],
            "workspace-a",
            &events,
        )
        .await
        .unwrap();
        advance_e2ee_witness_cursor(fresh.pool(), "workspace-a", events.last().unwrap().sequence)
            .await
            .unwrap();
        apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
            .await
            .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(fresh.pool())
            .await
            .unwrap();
        assert_eq!(title, "Witnessed value");

        sqlx::query("DELETE FROM e2ee_records")
            .execute(fresh.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes_with_witness(fresh.pool(), &workspace_keys)
            .await
            .unwrap();

        let repaired: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_records WHERE workspace_id = 'workspace-a'",
        )
        .fetch_one(fresh.pool())
        .await
        .unwrap();
        assert_eq!(repaired, i64::try_from(events.len()).unwrap());
    }

    #[tokio::test]
    async fn no_op_encryption_and_witness_repair_do_not_touch_replica_rows() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Unchanged')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let key = &workspace_keys["workspace-a"];
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE e2ee_records SET updated_at = 'sentinel' WHERE id = ?")
            .bind(&title_record_id)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
             VALUES ('workspace-a', 'sessions', 'session-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let stats = encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
            .await
            .unwrap();
        let updated_at: String =
            sqlx::query_scalar("SELECT updated_at FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stats.encrypted_fields, 0);
        assert_eq!(updated_at, "sentinel");

        merge_e2ee_witness_events(
            db.pool(),
            key,
            "workspace-a",
            &[E2eeWitnessEvent {
                sequence: 1,
                record_id: title_record_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&title_payload),
                payload: title_payload,
            }],
        )
        .await
        .unwrap();
        apply_received_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys, true)
            .await
            .unwrap();
        let repaired_updated_at: String =
            sqlx::query_scalar("SELECT updated_at FROM e2ee_records WHERE id = ?")
                .bind(title_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(repaired_updated_at, "sentinel");
    }

    #[tokio::test]
    async fn encrypts_only_rows_marked_dirty() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        for id in ["session-1", "session-2"] {
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', ?)",
            )
            .bind(id)
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
        }
        sqlx::query(
            "DELETE FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-2'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let first_manifest = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
        let second_manifest = key.blind_field_id("sessions", "session-2", ROW_MANIFEST_FIELD);
        let first_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_records WHERE id = ?)")
                .bind(first_manifest)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let second_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM e2ee_records WHERE id = ?)")
                .bind(second_manifest)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(first_exists);
        assert!(!second_exists);
    }

    #[tokio::test]
    async fn dirty_generation_preserves_a_concurrent_local_edit() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Before')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let writer_id = {
            let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
            let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
            transaction.commit().await.unwrap();
            writer_id
        };
        let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let prepared =
            prepare_dirty_row(db.pool(), &workspace_keys["workspace-a"], &writer_id, dirty)
                .await
                .unwrap();

        sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        let persisted = persist_prepared_dirty_row(db.pool(), prepared)
            .await
            .unwrap();
        let generation: i64 = sqlx::query_scalar(
            "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(persisted, 0);
        assert_eq!(generation, 2);
        assert_eq!(replica_count, 0);

        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&title_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        let field = key
            .open_field("workspace-a", &title_record_id, &payload)
            .unwrap();
        assert_eq!(field.value, json!("After"));
    }

    #[tokio::test]
    async fn capture_marker_inserted_after_prepare_prevents_transcript_persist() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, session_id, words_json)
             VALUES (
               'transcript-1',
               'workspace-a',
               'session-1',
               '[{\"text\":\"partial\"}]'
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let writer_id = {
            let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
            let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
            transaction.commit().await.unwrap();
            writer_id
        };
        let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let prepared =
            prepare_dirty_row(db.pool(), &workspace_keys["workspace-a"], &writer_id, dirty)
                .await
                .unwrap();

        sqlx::query(
            "INSERT INTO app_settings (id, value_json)
             VALUES ('capture_lifecycle_pending:session-1', ?)",
        )
        .bind(
            json!({
                "version": 1,
                "phase": "capturing",
                "sessionId": "session-1",
                "transcriptId": "transcript-1",
                "startedAt": 1_000,
                "createdAt": "2026-07-24T00:00:00.000Z",
                "audioOffsetMs": 0,
                "preserveExistingTranscript": false,
                "ownerUserId": "workspace-a",
                "memo": ""
            })
            .to_string(),
        )
        .execute(db.pool())
        .await
        .unwrap();

        let persisted = persist_prepared_dirty_row_inner(db.pool(), prepared, true)
            .await
            .unwrap();
        let dirty_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let local_state_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(persisted, 0);
        assert_eq!(dirty_count, 1);
        assert_eq!(local_state_count, 0);

        sqlx::query(
            "UPDATE app_settings
             SET value_json = json_set(value_json, '$.phase', 'finalizing')
             WHERE id = 'capture_lifecycle_pending:session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes_deferring_active_captures(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn cancelled_snapshot_finishes_current_row_and_releases_local_writes() {
        let calibration_db = test_db().await;
        let calibration_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('calibration', 'workspace-a', 'user-a', 'Snapshot row')",
        )
        .execute(calibration_db.pool())
        .await
        .unwrap();
        let calibration_checks = std::sync::atomic::AtomicUsize::new(0);
        encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
            calibration_db.pool(),
            &calibration_keys,
            E2EE_ENCRYPT_ROW_LIMIT,
            || {
                calibration_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                false
            },
        )
        .await
        .unwrap();
        let cancel_after_first_persist = calibration_checks
            .load(std::sync::atomic::Ordering::SeqCst)
            .checked_sub(3)
            .unwrap();

        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        for index in 0..129 {
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', 'Snapshot row')",
            )
            .bind(format!("session-{index:03}"))
            .execute(db.pool())
            .await
            .unwrap();
        }
        let initial_dirty_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(initial_dirty_rows, 129);

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let stats = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable(
                db.pool(),
                &workspace_keys,
                E2EE_ENCRYPT_ROW_LIMIT,
                || {
                    cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        >= cancel_after_first_persist
                },
            ),
        )
        .await
        .expect("snapshot cancellation exceeded the activity deadline")
        .unwrap();

        assert!(stats.remaining_replica_changes);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 128);

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled snapshot kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn large_dirty_queue_cancellation_stays_bounded_and_releases_local_writes() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
                SELECT 1
                UNION ALL
                SELECT id + 1 FROM rows WHERE id < 100000
            )
            INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
            SELECT 'workspace-a', 'sessions', printf('missing-%06d', id) FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let (page, remaining) =
            load_dirty_rows_page(db.pool(), &workspace_keys, E2EE_ENCRYPT_ROW_LIMIT, true)
                .await
                .unwrap();
        assert_eq!(page.len(), E2EE_ENCRYPT_ROW_LIMIT as usize);
        assert!(remaining);

        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let stats = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            encrypt_e2ee_replica_changes_deferring_active_captures_cancellable(
                db.pool(),
                &workspace_keys,
                || cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 1,
            ),
        )
        .await
        .expect("large dirty queue cancellation exceeded the activity deadline")
        .unwrap();

        assert!(stats.remaining_replica_changes);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 100_000);
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('after-large-cancel', 'workspace-a', 'user-a', 'Local write')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("large dirty queue cancellation kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn witness_change_preserves_a_prepared_dirty_row() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Local')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let writer_id = {
            let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
            let writer_id = load_or_create_writer_id(&mut transaction).await.unwrap();
            transaction.commit().await.unwrap();
            writer_id
        };
        let dirty = load_dirty_rows(db.pool(), &workspace_keys, 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        let prepared = prepare_dirty_row(db.pool(), key, &writer_id, dirty)
            .await
            .unwrap();

        let witnessed = key
            .seal_field(
                "workspace-a",
                "sessions",
                "session-1",
                "title",
                "ffffffffffffffffffffffffffffffff",
                5,
                false,
                json!("Remote"),
            )
            .unwrap();
        merge_e2ee_witness_events(
            db.pool(),
            key,
            "workspace-a",
            &[E2eeWitnessEvent {
                sequence: 1,
                record_id: witnessed.record_id,
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&witnessed.payload),
                payload: witnessed.payload,
            }],
        )
        .await
        .unwrap();

        assert_eq!(
            persist_prepared_dirty_row(db.pool(), prepared)
                .await
                .unwrap(),
            0
        );
        let dirty_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(dirty_count, 1);
        assert_eq!(replica_count, 0);

        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let title_id = key.blind_field_id("sessions", "session-1", "title");
        let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&title_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        let field = key.open_field("workspace-a", &title_id, &payload).unwrap();
        assert_eq!(field.revision, 6);
        assert_eq!(field.value, json!("Local"));
    }

    #[tokio::test]
    async fn bounded_encryption_makes_deterministic_progress() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        for index in 0..65 {
            let id = format!("session-{index:03}");
            sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?, 'workspace-a')")
                .bind(id)
                .execute(db.pool())
                .await
                .unwrap();
        }

        encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 1_000)
            .await
            .unwrap();
        let remaining: Vec<String> = sqlx::query_scalar(
            "SELECT row_id FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
             ORDER BY row_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(remaining, ["session-064"]);
        assert!(
            has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
                .await
                .unwrap()
        );

        encrypt_e2ee_replica_changes_bounded(db.pool(), &workspace_keys, 64)
            .await
            .unwrap();
        let remaining_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining_count, 0);
        assert!(
            !has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn active_capture_transcripts_do_not_starve_unrelated_rows() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        for index in 0..65 {
            let transcript_id = format!("protected-{index:03}");
            let session_id = format!("session-{index:03}");
            sqlx::query(
                "INSERT INTO transcripts (id, workspace_id, words_json)
                 VALUES (?, 'workspace-a', '[{\"text\":\"partial\"}]')",
            )
            .bind(&transcript_id)
            .execute(db.pool())
            .await
            .unwrap();
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
                .bind(format!("capture_lifecycle_pending:{session_id}"))
                .bind(
                    json!({
                        "version": 1,
                        "phase": "capturing",
                        "sessionId": session_id,
                        "transcriptId": transcript_id,
                        "startedAt": 1_000,
                        "createdAt": "2026-07-24T00:00:00.000Z",
                        "audioOffsetMs": 0,
                        "preserveExistingTranscript": false,
                        "ownerUserId": "workspace-a",
                        "memo": ""
                    })
                    .to_string(),
                )
                .execute(db.pool())
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('unrelated', 'workspace-a', '[{\"text\":\"complete\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        encrypt_e2ee_replica_changes_bounded_deferring_active_captures(
            db.pool(),
            &workspace_keys,
            64,
        )
        .await
        .unwrap();

        let remaining_protected: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id GLOB 'protected-*'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let unrelated_dirty: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE table_name = 'transcripts' AND row_id = 'unrelated'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let unrelated_records: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_local_state
             WHERE table_name = 'transcripts' AND row_id = 'unrelated'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(remaining_protected, 65);
        assert_eq!(unrelated_dirty, 0);
        assert!(unrelated_records > 0);
        assert!(
            !has_pending_e2ee_replica_changes_deferring_active_captures(
                db.pool(),
                &workspace_keys,
            )
            .await
            .unwrap()
        );
        assert!(
            has_pending_e2ee_replica_changes(db.pool(), &workspace_keys)
                .await
                .unwrap()
        );

        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn pending_witness_uploads_respect_record_and_byte_limits() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Bounded witness uploads')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];

        let first = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1, usize::MAX)
            .await
            .unwrap();
        assert_eq!(first.len(), 1);
        let first_bytes = first[0]
            .payload
            .len()
            .saturating_add(first[0].record_id.len())
            .saturating_add(first[0].payload_hash.len())
            .saturating_add(256);
        let byte_bounded =
            pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, first_bytes)
                .await
                .unwrap();
        assert_eq!(byte_bounded, first);
        assert!(matches!(
            pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, first_bytes - 1,)
                .await,
            Err(E2eeReplicaError::WitnessUploadTooLarge)
        ));

        acknowledge_e2ee_witness_uploads(db.pool(), key, &byte_bounded)
            .await
            .unwrap();
        let next = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 1, usize::MAX)
            .await
            .unwrap();
        assert_eq!(next.len(), 1);
        assert_ne!(next[0].record_id, first[0].record_id);
    }

    #[tokio::test]
    async fn pending_witness_uploads_never_scan_all_local_state() {
        let db = test_db().await;
        let explain = format!("EXPLAIN QUERY PLAN {PENDING_E2EE_WITNESS_UPLOADS_SQL}");
        let plan: Vec<(i64, i64, i64, String)> =
            sqlx::query_as(sqlx::AssertSqlSafe(explain.as_str()))
                .bind("workspace-a")
                .bind(16_i64)
                .fetch_all(db.pool())
                .await
                .unwrap();
        let details = plan
            .into_iter()
            .map(|(_, _, _, detail)| detail)
            .collect::<Vec<_>>();

        assert!(details.iter().any(|detail| {
            detail.contains(
                "SEARCH pending USING COVERING INDEX idx_e2ee_witness_pending_workspace_record",
            )
        }));
        assert!(!details.iter().any(|detail| detail.contains("SCAN local")));
    }

    #[tokio::test]
    async fn local_edits_enqueue_and_acknowledgements_drain_witness_uploads() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Before')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let initial = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
            .await
            .unwrap();
        assert!(!initial.is_empty());
        acknowledge_e2ee_witness_uploads(db.pool(), key, &initial)
            .await
            .unwrap();
        let pending_after_ack: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(pending_after_ack, 0);

        sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let title_pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
        )
        .bind(title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(title_pending);
    }

    #[tokio::test]
    async fn stale_witness_ack_keeps_a_newer_local_upload_pending() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();

        let initial = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
            .await
            .unwrap();
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let stale_title = initial
            .iter()
            .find(|upload| upload.record_id == title_record_id)
            .unwrap()
            .clone();
        acknowledge_e2ee_witness_uploads(db.pool(), key, &initial)
            .await
            .unwrap();

        sqlx::query("UPDATE sessions SET title = 'Second' WHERE id = 'session-1'")
            .execute(db.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        acknowledge_e2ee_witness_uploads(db.pool(), key, std::slice::from_ref(&stale_title))
            .await
            .unwrap();

        let pending = pending_e2ee_witness_uploads(db.pool(), "workspace-a", key, 128, usize::MAX)
            .await
            .unwrap();
        let current_title = pending
            .iter()
            .find(|upload| upload.record_id == title_record_id)
            .unwrap();
        assert!(current_title.revision > stale_title.revision);
        assert_ne!(current_title.payload, stale_title.payload);

        acknowledge_e2ee_witness_uploads(db.pool(), key, std::slice::from_ref(current_title))
            .await
            .unwrap();
        let title_pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
        )
        .bind(title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(!title_pending);
    }

    #[tokio::test]
    async fn equal_remote_state_does_not_enqueue_a_witness_upload() {
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Remote')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let uploads =
            pending_e2ee_witness_uploads(source.pool(), "workspace-a", key, 128, usize::MAX)
                .await
                .unwrap();
        let events = uploads
            .iter()
            .enumerate()
            .map(|(index, upload)| E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: upload.record_id.clone(),
                workspace_id: upload.workspace_id.clone(),
                payload_hash: upload.payload_hash.clone(),
                payload: upload.payload.clone(),
            })
            .collect::<Vec<_>>();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        merge_e2ee_witness_events(target.pool(), key, "workspace-a", &events)
            .await
            .unwrap();
        let stats = apply_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys)
            .await
            .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert!(stats.applied_fields > 0);
        assert_eq!(title, "Remote");
        assert_eq!(pending, 0);
    }

    #[tokio::test]
    async fn deleting_local_state_cascades_to_the_witness_queue() {
        let db = test_db().await;
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Pending')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let pending_before: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
        )
        .bind(&title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(pending_before);

        sqlx::query("DELETE FROM e2ee_local_state WHERE record_id = ?")
            .bind(&title_record_id)
            .execute(db.pool())
            .await
            .unwrap();

        let pending_after: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1 FROM e2ee_witness_pending WHERE record_id = ?
             )",
        )
        .bind(title_record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(!pending_after);
    }

    #[tokio::test]
    async fn remote_apply_guard_preserves_an_existing_local_dirty_marker() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title, status)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First', 'active')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(target.pool())
            .await
            .unwrap();
        let generation_before: i64 = sqlx::query_scalar(
            "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();

        sqlx::query("UPDATE sessions SET status = 'archived' WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let status_record_id = key.blind_field_id("sessions", "session-1", "status");
        let status_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&status_record_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(status_payload)
            .bind(status_record_id)
            .execute(target.pool())
            .await
            .unwrap();

        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let row: (String, String) =
            sqlx::query_as("SELECT title, status FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        let generation_after: i64 = sqlx::query_scalar(
            "SELECT generation FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        assert_eq!(row, ("Local".to_string(), "archived".to_string()));
        assert_eq!(generation_after, generation_before);

        let encrypted = encrypt_e2ee_replica_changes_bounded(target.pool(), &workspace_keys, 64)
            .await
            .unwrap();
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(target.pool())
                .await
                .unwrap();
        let title_field = key
            .open_field("workspace-a", &title_record_id, &title_payload)
            .unwrap();
        let remaining_dirty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert!(encrypted.encrypted_fields > 0);
        assert_eq!(title_field.value, json!("Local"));
        assert_eq!(remaining_dirty, 0);
    }

    #[tokio::test]
    async fn field_chunk_before_manifest_waits_then_applies_without_echo() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Chunked')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let title_id = key.blind_field_id("sessions", "session-1", "title");
        let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
        let title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        let manifest_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&manifest_id)
                .fetch_one(source.pool())
                .await
                .unwrap();

        let target = test_db().await;
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
        )
        .bind(&title_id)
        .bind(title_payload)
        .execute(target.pool())
        .await
        .unwrap();
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let before_manifest: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(before_manifest, 0);

        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
        )
        .bind(manifest_id)
        .bind(manifest_payload)
        .execute(target.pool())
        .await
        .unwrap();
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
        let dirty_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(target.pool())
            .await
            .unwrap();
        let outbound = encrypt_e2ee_replica_changes_bounded(target.pool(), &workspace_keys, 64)
            .await
            .unwrap();
        assert_eq!(title, "Chunked");
        assert_eq!(dirty_count, 0);
        assert_eq!(outbound.encrypted_fields, 0);
    }

    #[tokio::test]
    async fn witness_repairs_missing_records_only_after_snapshot_completion() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Witness recovery')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let encrypted: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, payload FROM e2ee_records
             WHERE workspace_id = 'workspace-a'
             ORDER BY id",
        )
        .fetch_all(source.pool())
        .await
        .unwrap();
        let events = encrypted
            .into_iter()
            .enumerate()
            .map(|(index, (record_id, payload))| E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id,
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&payload),
                payload,
            })
            .collect::<Vec<_>>();

        let target = test_db().await;
        merge_e2ee_witness_events(
            target.pool(),
            &workspace_keys["workspace-a"],
            "workspace-a",
            &events,
        )
        .await
        .unwrap();
        apply_received_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys, false)
            .await
            .unwrap();
        let partial_replica_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(target.pool())
            .await
            .unwrap();
        let partial_session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(partial_replica_count, 0);
        assert_eq!(partial_session_count, 0);

        apply_received_e2ee_replica_changes_with_witness(target.pool(), &workspace_keys, true)
            .await
            .unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(title, "Witness recovery");
    }

    #[tokio::test]
    async fn witness_repairs_respect_record_and_byte_limits() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Bounded witness recovery')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let encrypted: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, payload FROM e2ee_records
             WHERE workspace_id = 'workspace-a'
             ORDER BY id",
        )
        .fetch_all(source.pool())
        .await
        .unwrap();
        assert!(encrypted.len() > 2);
        let events = encrypted
            .iter()
            .enumerate()
            .map(|(index, (record_id, payload))| E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: record_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(payload),
                payload: payload.clone(),
            })
            .collect::<Vec<_>>();

        let target = test_db().await;
        merge_e2ee_witness_events(
            target.pool(),
            &workspace_keys["workspace-a"],
            "workspace-a",
            &events,
        )
        .await
        .unwrap();

        let error =
            repair_e2ee_replica_from_witness_bounded(target.pool(), &workspace_keys, true, 2, 1)
                .await
                .unwrap_err();
        assert!(matches!(error, E2eeReplicaError::WitnessRepairTooLarge));
        let empty_replica: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(empty_replica, 0);

        let first = repair_e2ee_replica_from_witness_bounded(
            target.pool(),
            &workspace_keys,
            true,
            2,
            usize::MAX,
        )
        .await
        .unwrap();
        assert_eq!(first.repaired_records, 2);
        assert!(first.remaining);

        let mut remaining = first.remaining;
        while remaining {
            let outcome = repair_e2ee_replica_from_witness_bounded(
                target.pool(),
                &workspace_keys,
                true,
                2,
                usize::MAX,
            )
            .await
            .unwrap();
            assert!(outcome.repaired_records <= 2);
            remaining = outcome.remaining;
        }

        let repaired: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(repaired, i64::try_from(events.len()).unwrap());
        assert!(
            !has_pending_e2ee_witness_repairs(target.pool(), &workspace_keys, true)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn completed_snapshots_repair_large_witness_sets_in_bounded_cycles() {
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let events = (0..130)
            .map(|index| {
                let row_id = format!("session-{index:03}");
                let sealed = key
                    .seal_field(
                        "workspace-a",
                        "sessions",
                        &row_id,
                        ROW_MANIFEST_FIELD,
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        1,
                        false,
                        json!(true),
                    )
                    .unwrap();
                E2eeWitnessEvent {
                    sequence: u64::try_from(index + 1).unwrap(),
                    record_id: sealed.record_id,
                    workspace_id: "workspace-a".to_string(),
                    payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
                    payload: sealed.payload,
                }
            })
            .collect::<Vec<_>>();

        let target = test_db().await;
        merge_e2ee_witness_events(target.pool(), key, "workspace-a", &events)
            .await
            .unwrap();
        let max_record_bytes: i64 = sqlx::query_scalar(
            "SELECT MAX(
               LENGTH(CAST(workspace_id AS BLOB))
                 + LENGTH(CAST(record_id AS BLOB))
                 + LENGTH(CAST(payload AS BLOB))
                 + 256
             )
             FROM e2ee_witness_records
             WHERE workspace_id = 'workspace-a'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        let max_repair_bytes = usize::try_from(max_record_bytes).unwrap() * 7;
        let mut cycles = 0;

        loop {
            let before: (i64, i64) = sqlx::query_as(
                "SELECT
                   COUNT(*),
                   COALESCE(SUM(
                     LENGTH(CAST(workspace_id AS BLOB))
                       + LENGTH(CAST(id AS BLOB))
                       + LENGTH(CAST(payload AS BLOB))
                       + 256
                   ), 0)
                 FROM e2ee_records",
            )
            .fetch_one(target.pool())
            .await
            .unwrap();
            let stats = apply_received_e2ee_replica_changes_with_witness_bounded(
                target.pool(),
                &workspace_keys,
                true,
                64,
                max_repair_bytes,
                &|| false,
            )
            .await
            .unwrap();
            let after: (i64, i64) = sqlx::query_as(
                "SELECT
                   COUNT(*),
                   COALESCE(SUM(
                     LENGTH(CAST(workspace_id AS BLOB))
                       + LENGTH(CAST(id AS BLOB))
                       + LENGTH(CAST(payload AS BLOB))
                       + 256
                   ), 0)
                 FROM e2ee_records",
            )
            .fetch_one(target.pool())
            .await
            .unwrap();
            let repaired_records = after.0 - before.0;
            let repaired_bytes = after.1 - before.1;
            assert!(repaired_records > 0);
            assert!(repaired_records <= 64);
            assert!(usize::try_from(repaired_bytes).unwrap() <= max_repair_bytes);
            cycles += 1;

            let pending = has_pending_e2ee_replica_changes(target.pool(), &workspace_keys)
                .await
                .unwrap();
            assert_eq!(stats.remaining_replica_changes, pending);
            if !pending {
                break;
            }
            assert!(cycles < 130);
        }

        assert!(cycles > 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(target.pool())
                .await
                .unwrap(),
            i64::try_from(events.len()).unwrap()
        );
    }

    #[tokio::test]
    async fn received_replica_rows_apply_in_bounded_cycles() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        for index in 0..20 {
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES (?, 'workspace-a', 'user-a', ?)",
            )
            .bind(format!("remote-session-{index:02}"))
            .bind(format!("Remote {index:02}"))
            .execute(source.pool())
            .await
            .unwrap();
        }
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        let mut stats = apply_e2ee_replica_changes_inner(
            target.pool(),
            &workspace_keys,
            false,
            3,
            usize::MAX,
            &|| false,
        )
        .await
        .unwrap();
        assert!(stats.remaining_replica_changes);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
            )
            .fetch_one(target.pool())
            .await
            .unwrap(),
            3
        );

        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('local-session', 'workspace-a', 'user-a', 'Local')",
        )
        .execute(target.pool())
        .await
        .unwrap();
        let mut cycles = 1;
        while stats.remaining_replica_changes {
            stats = apply_e2ee_replica_changes_inner(
                target.pool(),
                &workspace_keys,
                false,
                3,
                usize::MAX,
                &|| false,
            )
            .await
            .unwrap();
            sqlx::query("UPDATE sessions SET title = title WHERE id = 'local-session'")
                .execute(target.pool())
                .await
                .unwrap();
            cycles += 1;
            assert!(cycles < 20);
        }

        assert!(cycles > 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
            )
            .fetch_one(target.pool())
            .await
            .unwrap(),
            20
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT title FROM sessions WHERE id = 'local-session'",
            )
            .fetch_one(target.pool())
            .await
            .unwrap(),
            "Local"
        );
    }

    #[tokio::test]
    async fn cancelled_received_replica_preflight_is_bounded_and_releases_local_writes() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let db = test_db().await;
        let mut transaction = db.pool().begin().await.unwrap();
        for index in 0..512 {
            let row_id = format!("remote-session-{index:04}");
            let sealed = key
                .seal_field(
                    "workspace-a",
                    "sessions",
                    &row_id,
                    ROW_MANIFEST_FIELD,
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    json!(true),
                )
                .unwrap();
            let payload_hash = anlg_e2ee::payload_hash(&sealed.payload);
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
            )
            .bind(&sealed.record_id)
            .bind(&sealed.payload)
            .execute(&mut *transaction)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO e2ee_witness_records (
                   workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
                 ) VALUES (
                   'workspace-a', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, ?
                 )",
            )
            .bind(&sealed.record_id)
            .bind(payload_hash)
            .bind(&sealed.payload)
            .bind(i64::from(index) + 1)
            .execute(&mut *transaction)
            .await
            .unwrap();
        }
        transaction.commit().await.unwrap();

        let checks = AtomicUsize::new(0);
        let started_at = std::time::Instant::now();
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            apply_received_e2ee_replica_changes_with_witness_cancellable(
                db.pool(),
                &workspace_keys,
                false,
                || checks.fetch_add(1, AtomicOrdering::SeqCst) >= 20,
            ),
        )
        .await
        .expect("received-replica cancellation exceeded the activity deadline")
        .unwrap_err();

        assert!(matches!(error, E2eeReplicaError::Cancelled));
        assert!(started_at.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sessions WHERE id LIKE 'remote-session-%'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            0
        );
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('local-after-apply-cancel', 'workspace-a', 'user-a', 'Local')",
            )
            .execute(db.pool()),
        )
        .await
        .expect("cancelled received-replica preflight kept the database busy")
        .unwrap();
    }

    #[tokio::test]
    async fn bounded_received_preflight_skips_foreign_and_unwitnessed_prefixes() {
        let workspace_keys = keys("workspace-a");
        let key = &workspace_keys["workspace-a"];
        let db = test_db().await;
        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 256
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!foreign-%04d', id), 'workspace-z', 'foreign' FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "WITH RECURSIVE rows(id) AS (
               SELECT 1
               UNION ALL
               SELECT id + 1 FROM rows WHERE id < 256
             )
             INSERT INTO e2ee_records (id, workspace_id, payload)
             SELECT printf('!unwitnessed-%04d', id), 'workspace-a', 'unwitnessed' FROM rows",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let sealed = key
            .seal_field(
                "workspace-a",
                "sessions",
                "witnessed-session",
                ROW_MANIFEST_FIELD,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                json!(true),
            )
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES (?, 'workspace-a', ?)",
        )
        .bind(&sealed.record_id)
        .bind(&sealed.payload)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_witness_records (
               workspace_id, record_id, revision, writer_id, payload_hash, payload, sequence
             ) VALUES (
               'workspace-a', ?, 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ?, ?, 1
             )",
        )
        .bind(&sealed.record_id)
        .bind(anlg_e2ee::payload_hash(&sealed.payload))
        .bind(&sealed.payload)
        .execute(db.pool())
        .await
        .unwrap();

        let stats =
            apply_received_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys, false)
                .await
                .unwrap();

        assert!(stats.rejected_unwitnessed >= 256);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sessions WHERE id = 'witnessed-session'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn id_collision_does_not_record_unmaterialized_remote_state() {
        let recovery =
            RecoveryKey::parse("anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc")
                .unwrap();
        let workspace_keys = HashMap::from([
            (
                "workspace-a".to_string(),
                recovery.workspace_key("workspace-a").unwrap(),
            ),
            (
                "workspace-b".to_string(),
                recovery.workspace_key("workspace-b").unwrap(),
            ),
        ]);
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-b', 'user-b', 'Remote B')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Local A')",
        )
        .execute(target.pool())
        .await
        .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let remote_state_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_local_state
             WHERE workspace_id = 'workspace-b'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        let current_workspace: String =
            sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(remote_state_count, 0);
        assert_eq!(current_workspace, "workspace-a");

        sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
            .execute(target.pool())
            .await
            .unwrap();
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let remote: (String, String) =
            sqlx::query_as("SELECT workspace_id, title FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(remote, ("workspace-b".to_string(), "Remote B".to_string()));
    }

    #[tokio::test]
    async fn failed_remote_apply_rolls_back_the_guard() {
        let workspace_keys = keys("workspace-a");
        let source = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'First')",
        )
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();

        let target = test_db().await;
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Remote' WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        let key = &workspace_keys["workspace-a"];
        let title_record_id = key.blind_field_id("sessions", "session-1", "title");
        let title_payload: String =
            sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
                .bind(&title_record_id)
                .fetch_one(source.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
            .bind(title_payload)
            .bind(title_record_id)
            .execute(target.pool())
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_remote_title_update
             BEFORE UPDATE OF title ON sessions
             BEGIN
               SELECT RAISE(ABORT, 'forced apply failure');
             END",
        )
        .execute(target.pool())
        .await
        .unwrap();

        assert!(
            apply_e2ee_replica_changes(target.pool(), &workspace_keys)
                .await
                .is_err()
        );
        let guard_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_apply_guard")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(guard_count, 0);

        sqlx::query("DROP TRIGGER fail_remote_title_update")
            .execute(target.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
            .execute(target.pool())
            .await
            .unwrap();
        let dirty_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'session-1'",
        )
        .fetch_one(target.pool())
        .await
        .unwrap();
        assert_eq!(dirty_count, 1);
    }
}
