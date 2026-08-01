use super::super::{
    encrypt_e2ee_replica_changes, encrypt_e2ee_replica_changes_bounded, load_dirty_rows,
    load_or_create_writer_id, persist_prepared_dirty_row_cancellable, prepare_dirty_row,
};
use super::*;
use anlg_e2ee::RecoveryKey;
use serde_json::json;
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
        acknowledge_checks.fetch_add(1, Ordering::SeqCst) >= acknowledge_cancel_after_first_commit
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
