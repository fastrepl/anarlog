use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use db_app::{
    apply_e2ee_replica_changes, claim_cloudsync_workspace, cloudsync_table_registry,
    encrypt_e2ee_replica_changes, prepare_schema,
};
use hypr_db_core::{CloudsyncAuth, CloudsyncRuntimeConfig, Db, DbOpenOptions, DbStorage};
use hypr_e2ee::{RecoveryKey, WorkspaceKey};

const SYNC_TIMEOUT: Duration = Duration::from_secs(90);
const SYNC_ATTEMPTS: usize = 3;
const POLICY_SYNC_TIMEOUT: Duration = Duration::from_secs(30);
const REPLICA_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(90);
const REPLICA_VISIBILITY_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn cloudsync_config(auth: CloudsyncAuth, wait_ms: i64, max_retries: i64) -> CloudsyncRuntimeConfig {
    CloudsyncRuntimeConfig {
        connection_string: std::env::var("ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID")
            .expect("ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID must be set"),
        auth,
        tables: cloudsync_table_registry().to_vec(),
        sync_interval_ms: 86_400_000,
        wait_ms: Some(wait_ms),
        max_retries: Some(max_retries),
    }
}

fn token_auth(token: &str) -> CloudsyncAuth {
    CloudsyncAuth::Token {
        token: token.to_string(),
    }
}

fn workspace_keys(workspace_id: &str, recovery_key_env: &str) -> HashMap<String, WorkspaceKey> {
    let recovery_key = RecoveryKey::parse(
        &std::env::var(recovery_key_env)
            .unwrap_or_else(|_| panic!("{recovery_key_env} must be set")),
    )
    .unwrap_or_else(|error| panic!("{recovery_key_env} is invalid: {error}"));
    HashMap::from([(
        workspace_id.to_string(),
        recovery_key.workspace_key(workspace_id).unwrap(),
    )])
}

async fn setup_db_with_network_options(
    auth: CloudsyncAuth,
    workspace_id: &str,
    wait_ms: i64,
    max_retries: i64,
) -> Db {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();

    prepare_schema(&db).await.unwrap();
    claim_cloudsync_workspace(db.pool(), workspace_id)
        .await
        .unwrap();
    db.cloudsync_configure(cloudsync_config(auth, wait_ms, max_retries))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_start())
        .await
        .expect("cloudsync start timed out")
        .unwrap();
    db
}

async fn setup_db(token: &str, workspace_id: &str) -> Db {
    setup_db_with_network_options(token_auth(token), workspace_id, 5_000, 3).await
}

async fn setup_policy_db(token: &str, workspace_id: &str) -> Db {
    setup_db_with_network_options(token_auth(token), workspace_id, 2_500, 1).await
}

async fn stop_db(db: &Db, label: &str) {
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_stop())
        .await
        .unwrap_or_else(|_| panic!("{label} stop timed out"))
        .unwrap();
}

async fn sync_ok(db: &Db, label: &str) {
    let mut result = None;
    let mut last_failure = None;
    for attempt in 1..=SYNC_ATTEMPTS {
        match tokio::time::timeout(SYNC_TIMEOUT, db.cloudsync_trigger_sync()).await {
            Ok(Ok(value)) => {
                result = Some(value);
                break;
            }
            Ok(Err(error)) => last_failure = Some(error.to_string()),
            Err(_) => last_failure = Some("timed out".to_string()),
        }
        if attempt < SYNC_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    let result = result.unwrap_or_else(|| {
        panic!(
            "{label} failed after {SYNC_ATTEMPTS} attempts: {}",
            last_failure.as_deref().unwrap_or("unknown error")
        )
    });
    if let Some(send) = result.send {
        assert_eq!(send.status, "synced", "{label} send did not complete");
        assert!(send.last_failure.is_none(), "{label} send failed");
    }
    if let Some(receive) = result.receive {
        assert!(receive.complete, "{label} receive did not complete");
        assert!(receive.error.is_none(), "{label} receive failed");
        assert!(
            receive.last_failure.is_none(),
            "{label} receive did not complete"
        );
    }
    let status = db.cloudsync_status().await.unwrap();
    assert!(
        status.last_error.is_none(),
        "{label} recorded a sync error: {:?}",
        status.last_error
    );
}

async fn sync_full_snapshot(db: &Db, label: &str) {
    db.cloudsync_network_reset_sync_version()
        .await
        .unwrap_or_else(|error| panic!("{label} snapshot reset failed: {error}"));
    sync_ok(db, label).await;
    sync_ok(db, &format!("{label} retry")).await;
}

async fn assert_pending_change(db: &Db, operation: &str) {
    assert_eq!(
        db.cloudsync_status().await.unwrap().has_unsent_changes,
        Some(true),
        "{operation} was not tracked as a pending CloudSync change"
    );
}

struct EncryptedNote {
    session_id: String,
    document_id: String,
    title: String,
    body: String,
    record_ids: Vec<String>,
}

async fn insert_encrypted_note(
    db: &Db,
    workspace_id: &str,
    keys: &HashMap<String, WorkspaceKey>,
    marker: u128,
) -> EncryptedNote {
    let note = EncryptedNote {
        session_id: format!("cloudsync-e2ee-session-{marker}"),
        document_id: format!("cloudsync-e2ee-document-{marker}"),
        title: format!("Encrypted note {marker}"),
        body: format!(
            r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"private note {marker}"}}]}}]}}"#
        ),
        record_ids: Vec::new(),
    };

    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&note.session_id)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(&note.title)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, workspace_id, session_id, title, body) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&note.document_id)
    .bind(workspace_id)
    .bind(&note.session_id)
    .bind(&note.title)
    .bind(&note.body)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let stats = encrypt_e2ee_replica_changes(db.pool(), keys).await.unwrap();
    assert!(stats.encrypted_fields > 0);
    let records: Vec<(String, String)> =
        sqlx::query_as("SELECT id, payload FROM e2ee_records WHERE workspace_id = ?")
            .bind(workspace_id)
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert!(!records.is_empty());
    assert!(records.iter().all(|(_, payload)| {
        !payload.contains(&note.session_id)
            && !payload.contains(&note.document_id)
            && !payload.contains(&note.title)
            && !payload.contains(&note.body)
            && !payload.contains(&marker.to_string())
    }));

    EncryptedNote {
        record_ids: records.into_iter().map(|(id, _)| id).collect(),
        ..note
    }
}

async fn cleanup_encrypted_records(db: &Db, record_ids: &[String], label: &str) {
    let mut transaction = db.pool().begin().await.unwrap();
    for id in record_ids {
        sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .unwrap();
    }
    transaction.commit().await.unwrap();
    sync_ok(db, label).await;
}

async fn cleanup_verification_workspace(token: &str, workspace_id: &str, label: &str) {
    let db = setup_db(token, workspace_id).await;
    sync_full_snapshot(&db, &format!("{label} cleanup snapshot")).await;
    let deleted = sqlx::query("DELETE FROM e2ee_records WHERE workspace_id = ?")
        .bind(workspace_id)
        .execute(db.pool())
        .await
        .unwrap();
    if deleted.rows_affected() > 0 {
        sync_ok(&db, &format!("{label} cleanup delete")).await;
    }
    stop_db(&db, label).await;
}

async fn setup_stale_record_client(
    owner_token: &str,
    attacker_token: &str,
    owner_workspace_id: &str,
    record_id: &str,
    operation: &str,
) -> Db {
    let db = setup_db(owner_token, owner_workspace_id).await;
    sync_full_snapshot(&db, &format!("{operation} owner snapshot")).await;
    let record_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
        .bind(record_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        record_count, 1,
        "{operation} owner record was not downloaded"
    );

    db.cloudsync_network_set_token(attacker_token)
        .await
        .unwrap_or_else(|error| panic!("{operation} attacker reauthentication failed: {error}"));
    assert_eq!(
        db.cloudsync_status().await.unwrap().has_unsent_changes,
        Some(false),
        "{operation} reauthentication introduced pending changes"
    );
    db
}

fn is_rls_policy_denial(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let mentions_rls = message.contains("row-level security")
        || message.contains("row level security")
        || message
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|part| part == "rls");
    let mentions_denial = [
        "denied",
        "deny",
        "rejected",
        "forbidden",
        "violation",
        "failed",
        "failure",
        "not allowed",
        "not authorized",
    ]
    .iter()
    .any(|needle| message.contains(needle));

    mentions_rls && mentions_denial
}

async fn expect_policy_sync_completed_or_denied(db: &Db, operation: &str) -> Result<(), String> {
    let outcome = tokio::time::timeout(POLICY_SYNC_TIMEOUT, db.cloudsync_trigger_sync())
        .await
        .unwrap_or_else(|_| panic!("{operation} foreign-write sync timed out"));
    let mut evidence = Vec::new();
    let mut clean_send = false;
    let mut clean_receive = true;
    match outcome {
        Ok(result) => {
            if let Some(send) = result.send {
                clean_send =
                    send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none();
                evidence.push(format!("send status: {}", send.status));
                if let Some(last_failure) = send.last_failure {
                    evidence.push(format!("send failure: {last_failure}"));
                }
            }
            if let Some(receive) = result.receive {
                if let Some(error) = receive.error {
                    clean_receive = false;
                    evidence.push(format!("receive error: {error}"));
                }
                if let Some(last_failure) = receive.last_failure {
                    clean_receive = false;
                    evidence.push(format!("receive failure: {last_failure}"));
                }
            }
        }
        Err(error) => evidence.push(error.to_string()),
    }
    let status = db.cloudsync_status().await.unwrap();
    let runtime_clean = status.last_error.is_none();
    if let Some(error) = &status.last_error {
        evidence.push(format!("runtime error: {error}"));
    }

    if evidence.iter().any(|entry| is_rls_policy_denial(entry))
        || (clean_send && clean_receive && runtime_clean)
    {
        Ok(())
    } else {
        Err(format!(
            "{operation} neither completed a clean policy send nor returned an RLS denial; sync evidence: {}",
            evidence.join(" | ")
        ))
    }
}

#[test]
fn rls_policy_denial_matcher_rejects_generic_failures() {
    assert!(is_rls_policy_denial(
        "RLS policy denied INSERT on e2ee_records"
    ));
    assert!(is_rls_policy_denial(
        "row-level security policy check failed"
    ));
    assert!(!is_rls_policy_denial(
        "401 database_auth_failed: database credentials were rejected: Invalid APIKEY"
    ));
    assert!(!is_rls_policy_denial("connection timed out"));
    assert!(!is_rls_policy_denial("access token expired"));
    assert!(!is_rls_policy_denial("network retry policy failed"));
}

#[test]
fn cloudsync_enables_only_the_encrypted_replica() {
    let enabled_tables: Vec<&str> = cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
        .map(|table| table.table_name.as_str())
        .collect();

    assert_eq!(enabled_tables, ["e2ee_records"]);
}

#[tokio::test]
#[ignore = "external E2EE verification only; requires ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID, ANARLOG_CLOUDSYNC_WORKSPACE_A, ANARLOG_CLOUDSYNC_TOKEN_A, and ANARLOG_CLOUDSYNC_RECOVERY_KEY_A"]
async fn same_personal_workspace_syncs_and_decrypts_a_real_note() {
    let workspace_id = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let token =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A").expect("ANARLOG_CLOUDSYNC_TOKEN_A must be set");
    let keys = workspace_keys(&workspace_id, "ANARLOG_CLOUDSYNC_RECOVERY_KEY_A");
    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let db_a = setup_db(&token, &workspace_id).await;
    let note = insert_encrypted_note(&db_a, &workspace_id, &keys, marker).await;
    sync_ok(&db_a, "first personal client upload").await;

    let db_b = setup_db(&token, &workspace_id).await;
    db_b.cloudsync_network_reset_sync_version()
        .await
        .expect("second personal client snapshot reset failed");
    tokio::time::timeout(REPLICA_VISIBILITY_TIMEOUT, async {
        let mut attempt = 1;
        loop {
            sync_ok(
                &db_b,
                &format!("second personal client download attempt {attempt}"),
            )
            .await;
            let stats = apply_e2ee_replica_changes(db_b.pool(), &keys)
                .await
                .expect("second personal client could not apply encrypted replica changes");
            if stats.applied_fields > 0 {
                break;
            }

            attempt += 1;
            tokio::time::sleep(REPLICA_VISIBILITY_POLL_INTERVAL).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "second personal client did not receive encrypted fields within {} seconds",
            REPLICA_VISIBILITY_TIMEOUT.as_secs()
        )
    });

    let downloaded_title: Option<String> =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = ? AND workspace_id = ?")
            .bind(&note.session_id)
            .bind(&workspace_id)
            .fetch_optional(db_b.pool())
            .await
            .unwrap();
    let downloaded_body: Option<String> = sqlx::query_scalar(
        "SELECT body FROM session_documents WHERE id = ? AND session_id = ? AND workspace_id = ?",
    )
    .bind(&note.document_id)
    .bind(&note.session_id)
    .bind(&workspace_id)
    .fetch_optional(db_b.pool())
    .await
    .unwrap();

    cleanup_encrypted_records(&db_a, &note.record_ids, "personal note cleanup").await;
    stop_db(&db_a, "first personal client").await;
    stop_db(&db_b, "second personal client").await;

    assert_eq!(downloaded_title.as_deref(), Some(note.title.as_str()));
    assert_eq!(downloaded_body.as_deref(), Some(note.body.as_str()));
}

#[tokio::test]
#[ignore = "external E2EE policy verification only; requires ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID, ANARLOG_CLOUDSYNC_WORKSPACE_A/B, ANARLOG_CLOUDSYNC_TOKEN_A/B, and ANARLOG_CLOUDSYNC_RECOVERY_KEY_B"]
async fn personal_workspace_tokens_block_foreign_encrypted_writes() {
    let workspace_a = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let workspace_b = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_B")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_B must be set");
    assert_ne!(workspace_a, workspace_b);
    let token_a =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A").expect("ANARLOG_CLOUDSYNC_TOKEN_A must be set");
    let token_b =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_B").expect("ANARLOG_CLOUDSYNC_TOKEN_B must be set");
    let keys_b = workspace_keys(&workspace_b, "ANARLOG_CLOUDSYNC_RECOVERY_KEY_B");
    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let owner = setup_db(&token_b, &workspace_b).await;
    let note = insert_encrypted_note(&owner, &workspace_b, &keys_b, marker).await;
    let owner_record_id =
        keys_b[&workspace_b].blind_field_id("sessions", &note.session_id, "title");
    let initial_owner_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&owner_record_id)
            .fetch_one(owner.pool())
            .await
            .unwrap();
    sync_ok(&owner, "workspace B initial encrypted note upload").await;

    let updated_title = format!("Updated encrypted note {marker}");
    sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
        .bind(&updated_title)
        .bind(&note.session_id)
        .execute(owner.pool())
        .await
        .unwrap();
    let update_stats = encrypt_e2ee_replica_changes(owner.pool(), &keys_b)
        .await
        .unwrap();
    assert!(update_stats.encrypted_fields > 0);
    let owner_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&owner_record_id)
        .fetch_one(owner.pool())
        .await
        .unwrap();
    assert_ne!(owner_payload, initial_owner_payload);
    sync_ok(&owner, "workspace B encrypted note update").await;

    let owner_round_trip = setup_db(&token_b, &workspace_b).await;
    sync_full_snapshot(&owner_round_trip, "workspace B update round trip").await;
    let round_trip_stats = apply_e2ee_replica_changes(owner_round_trip.pool(), &keys_b)
        .await
        .unwrap();
    assert!(round_trip_stats.applied_fields > 0);
    let round_trip_title: Option<String> =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = ? AND workspace_id = ?")
            .bind(&note.session_id)
            .bind(&workspace_b)
            .fetch_optional(owner_round_trip.pool())
            .await
            .unwrap();
    stop_db(&owner_round_trip, "workspace B update round-trip client").await;

    let insert_attacker = setup_policy_db(&token_a, &workspace_a).await;
    sync_full_snapshot(&insert_attacker, "workspace A foreign read check").await;
    let foreign_read_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&owner_record_id)
            .fetch_one(insert_attacker.pool())
            .await
            .unwrap();
    let foreign_insert_id = format!("cloudsync-e2ee-foreign-insert-{marker}");
    sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
        .bind(&foreign_insert_id)
        .bind(&workspace_b)
        .bind(format!("blocked-insert-{marker}"))
        .execute(insert_attacker.pool())
        .await
        .unwrap();
    assert_pending_change(&insert_attacker, "foreign INSERT").await;
    let insert_result =
        expect_policy_sync_completed_or_denied(&insert_attacker, "workspace A foreign INSERT")
            .await;
    stop_db(&insert_attacker, "foreign INSERT attacker").await;

    let reassignment_attacker = setup_policy_db(&token_a, &workspace_a).await;
    let reassignment_id = format!("cloudsync-e2ee-workspace-reassignment-{marker}");
    let reassignment_payload = format!("workspace-a-owned-{marker}");
    sqlx::query("INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, ?, ?)")
        .bind(&reassignment_id)
        .bind(&workspace_a)
        .bind(&reassignment_payload)
        .execute(reassignment_attacker.pool())
        .await
        .unwrap();
    sync_ok(
        &reassignment_attacker,
        "workspace A reassignment fixture upload",
    )
    .await;
    let reassigned = sqlx::query("UPDATE e2ee_records SET workspace_id = ? WHERE id = ?")
        .bind(&workspace_b)
        .bind(&reassignment_id)
        .execute(reassignment_attacker.pool())
        .await
        .unwrap();
    assert_eq!(reassigned.rows_affected(), 1);
    assert_pending_change(&reassignment_attacker, "foreign workspace reassignment").await;
    let reassignment_result = expect_policy_sync_completed_or_denied(
        &reassignment_attacker,
        "workspace A foreign workspace reassignment",
    )
    .await;
    stop_db(&reassignment_attacker, "workspace reassignment attacker").await;

    let update_attacker = setup_stale_record_client(
        &token_b,
        &token_a,
        &workspace_b,
        &owner_record_id,
        "foreign UPDATE",
    )
    .await;
    let updated = sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(format!("blocked-update-{marker}"))
        .bind(&owner_record_id)
        .execute(update_attacker.pool())
        .await
        .unwrap();
    assert_eq!(updated.rows_affected(), 1);
    assert_pending_change(&update_attacker, "foreign payload UPDATE").await;
    let update_result =
        expect_policy_sync_completed_or_denied(&update_attacker, "workspace A foreign UPDATE")
            .await;
    stop_db(&update_attacker, "foreign UPDATE attacker").await;

    let delete_attacker = setup_stale_record_client(
        &token_b,
        &token_a,
        &workspace_b,
        &owner_record_id,
        "foreign DELETE",
    )
    .await;
    let deleted = sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
        .bind(&owner_record_id)
        .execute(delete_attacker.pool())
        .await
        .unwrap();
    assert_eq!(deleted.rows_affected(), 1);
    assert_pending_change(&delete_attacker, "foreign DELETE").await;
    let delete_result =
        expect_policy_sync_completed_or_denied(&delete_attacker, "workspace A foreign DELETE")
            .await;
    stop_db(&delete_attacker, "foreign DELETE attacker").await;

    let verifier = setup_db(&token_b, &workspace_b).await;
    sync_full_snapshot(&verifier, "workspace B policy verification").await;
    let verified_payload: Option<String> =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&owner_record_id)
            .fetch_optional(verifier.pool())
            .await
            .unwrap();
    let foreign_insert_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&foreign_insert_id)
            .fetch_one(verifier.pool())
            .await
            .unwrap();
    let reassigned_foreign_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
            .bind(&reassignment_id)
            .fetch_one(verifier.pool())
            .await
            .unwrap();
    let mut leaked_record_ids = Vec::new();
    if foreign_insert_count != 0 {
        leaked_record_ids.push(foreign_insert_id.clone());
    }
    if reassigned_foreign_count != 0 {
        leaked_record_ids.push(reassignment_id.clone());
    }
    if !leaked_record_ids.is_empty() {
        cleanup_encrypted_records(&verifier, &leaked_record_ids, "leaked foreign row cleanup")
            .await;
    }

    let workspace_a_verifier = setup_db(&token_a, &workspace_a).await;
    sync_full_snapshot(
        &workspace_a_verifier,
        "workspace A reassignment verification",
    )
    .await;
    let reassignment_owner_row: Option<(String, String)> =
        sqlx::query_as("SELECT workspace_id, payload FROM e2ee_records WHERE id = ?")
            .bind(&reassignment_id)
            .fetch_optional(workspace_a_verifier.pool())
            .await
            .unwrap();
    if reassignment_owner_row.is_some() {
        cleanup_encrypted_records(
            &workspace_a_verifier,
            std::slice::from_ref(&reassignment_id),
            "workspace A reassignment fixture cleanup",
        )
        .await;
    }

    cleanup_encrypted_records(
        &owner,
        &note.record_ids,
        "workspace B encrypted record cleanup",
    )
    .await;
    stop_db(&owner, "workspace B owner").await;
    stop_db(&verifier, "workspace B verifier").await;
    stop_db(&workspace_a_verifier, "workspace A verifier").await;

    assert_eq!(foreign_read_count, 0, "workspace A read workspace B");
    assert_eq!(round_trip_title.as_deref(), Some(updated_title.as_str()));
    assert!(
        insert_result.is_ok(),
        "foreign INSERT returned an unexpected sync result: {:?}",
        insert_result.err()
    );
    assert!(
        update_result.is_ok(),
        "foreign UPDATE returned an unexpected sync result: {:?}",
        update_result.err()
    );
    assert!(
        delete_result.is_ok(),
        "foreign DELETE returned an unexpected sync result: {:?}",
        delete_result.err()
    );
    assert!(
        reassignment_result.is_ok(),
        "foreign workspace reassignment returned an unexpected sync result: {:?}",
        reassignment_result.err()
    );
    assert_eq!(
        verified_payload.as_deref(),
        Some(owner_payload.as_str()),
        "workspace A updated or deleted workspace B's encrypted record"
    );
    assert_eq!(
        foreign_insert_count, 0,
        "workspace A inserted an encrypted record into workspace B"
    );
    assert_eq!(
        reassigned_foreign_count, 0,
        "workspace A reassigned its encrypted record into workspace B"
    );
    assert_eq!(
        reassignment_owner_row,
        Some((workspace_a, reassignment_payload)),
        "workspace A's encrypted record did not remain in workspace A"
    );
}

#[tokio::test]
#[ignore = "deployment cleanup only; requires ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID, ANARLOG_CLOUDSYNC_WORKSPACE_A/B with deploy-e2ee-a-/deploy-e2ee-b- prefixes, and ANARLOG_CLOUDSYNC_TOKEN_A/B"]
async fn cleanup_e2ee_verification_workspaces() {
    let workspace_a = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let workspace_b = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_B")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_B must be set");
    assert!(
        workspace_a.starts_with("deploy-e2ee-a-"),
        "cleanup is restricted to deploy-e2ee-a-* workspaces"
    );
    assert!(
        workspace_b.starts_with("deploy-e2ee-b-"),
        "cleanup is restricted to deploy-e2ee-b-* workspaces"
    );
    let token_a =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A").expect("ANARLOG_CLOUDSYNC_TOKEN_A must be set");
    let token_b =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_B").expect("ANARLOG_CLOUDSYNC_TOKEN_B must be set");

    let cleanup_a = tokio::spawn(async move {
        cleanup_verification_workspace(&token_a, &workspace_a, "workspace A verification").await;
    })
    .await;
    let cleanup_b = tokio::spawn(async move {
        cleanup_verification_workspace(&token_b, &workspace_b, "workspace B verification").await;
    })
    .await;
    let failures = [
        cleanup_a.err().map(|error| format!("workspace A: {error}")),
        cleanup_b.err().map(|error| format!("workspace B: {error}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "verification workspace cleanup failed: {}",
        failures.join("; ")
    );
}
