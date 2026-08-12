#[cfg(test)]
use std::collections::HashMap;

use anlg_e2ee::OpenedField;
#[cfg(test)]
use anlg_e2ee::WorkspaceKey;
#[cfg(test)]
use serde_json::{Value, json};
#[cfg(test)]
use sqlx::SqlitePool;

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
    "synced_preferences",
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
    generation: i64,
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

mod cooperative;
mod replica_apply;
mod replica_encrypt;
mod replica_storage;

use cooperative::yield_once;
pub use replica_apply::{
    apply_e2ee_replica_changes, apply_e2ee_replica_changes_with_witness,
    apply_received_e2ee_replica_changes_with_witness,
    apply_received_e2ee_replica_changes_with_witness_cancellable,
};
#[cfg(test)]
use replica_apply::{
    apply_e2ee_replica_changes_inner, apply_received_e2ee_replica_changes_with_witness_bounded,
    load_changed_e2ee_record_metadata,
};
use replica_encrypt::check_e2ee_cancellation;
pub use replica_encrypt::{
    encrypt_e2ee_replica_changes, encrypt_e2ee_replica_changes_bounded,
    encrypt_e2ee_replica_changes_bounded_deferring_active_captures,
    encrypt_e2ee_replica_changes_bounded_deferring_active_captures_cancellable,
    encrypt_e2ee_replica_changes_deferring_active_captures,
    encrypt_e2ee_replica_changes_deferring_active_captures_cancellable,
    has_pending_e2ee_dirty_rows_deferring_active_captures, has_pending_e2ee_replica_changes,
    has_pending_e2ee_replica_changes_deferring_active_captures,
};
#[cfg(test)]
use replica_encrypt::{
    load_dirty_rows, load_dirty_rows_page, persist_prepared_dirty_row,
    persist_prepared_dirty_row_cancellable, persist_prepared_dirty_row_inner, prepare_dirty_row,
};
#[cfg(test)]
use replica_storage::load_or_create_writer_id;
use replica_storage::reconcile_e2ee_witness_pending;

#[cfg(test)]
mod tests;
