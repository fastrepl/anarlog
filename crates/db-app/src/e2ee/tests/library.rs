use super::*;

async fn personal_library() -> anlg_db_core::Db {
    let db = test_db().await;
    crate::claim_cloudsync_workspace(db.pool(), "account-a")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('note', 'account-a', 'account-a', 'My local note');
         INSERT INTO humans (id, workspace_id, owner_user_id, name)
         VALUES ('account-a', 'account-a', 'account-a', 'Me');
         INSERT INTO session_participants (id, workspace_id, owner_user_id, session_id, human_id)
         VALUES ('participant', 'account-a', 'account-a', 'note', 'account-a');
         INSERT INTO session_attachments (id, workspace_id, session_id, relative_path)
         VALUES ('audio', 'account-a', 'note', 'sessions/note/audio.wav')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn switching_sync_destination_preserves_library_and_account_a_ciphertext() {
    let source = personal_library().await;
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-a"))
        .await
        .unwrap();
    let previous: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM e2ee_records WHERE workspace_id = 'account-a' ORDER BY id",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    crate::connect_local_library(source.pool(), "account-b", "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-b"))
        .await
        .unwrap();
    let after: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM e2ee_records WHERE workspace_id = 'account-a' ORDER BY id",
    )
    .fetch_all(source.pool())
    .await
    .unwrap();
    assert_eq!(previous, after);
    let identity: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'note'")
            .fetch_one(source.pool())
            .await
            .unwrap();
    assert_eq!(identity, ("account-a".into(), "account-a".into()));

    let remote = test_db().await;
    copy_replica(source.pool(), remote.pool()).await;
    apply_e2ee_replica_changes(remote.pool(), &keys("account-b"))
        .await
        .unwrap();
    let restored: (String, String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id, title FROM sessions WHERE id = 'note'")
            .fetch_one(remote.pool())
            .await
            .unwrap();
    assert_eq!(
        restored,
        (
            "account-b".into(),
            "account-b".into(),
            "My local note".into()
        )
    );
    let participant: String =
        sqlx::query_scalar("SELECT human_id FROM session_participants WHERE id = 'participant'")
            .fetch_one(remote.pool())
            .await
            .unwrap();
    assert_eq!(participant, "account-b");
    let person: String = sqlx::query_scalar("SELECT name FROM humans WHERE id = 'account-b'")
        .fetch_one(remote.pool())
        .await
        .unwrap();
    assert_eq!(person, "Me");
    let audio: String =
        sqlx::query_scalar("SELECT relative_path FROM session_attachments WHERE id = 'audio'")
            .fetch_one(remote.pool())
            .await
            .unwrap();
    assert_eq!(audio, "sessions/note/audio.wav");
}

#[tokio::test]
async fn local_edits_are_queued_for_each_connection_and_survive_switching_back() {
    let source = personal_library().await;
    crate::connect_local_library(source.pool(), "account-b", "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-a"))
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-b"))
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Edited on B' WHERE id = 'note'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-b"))
        .await
        .unwrap();
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM e2ee_dirty_rows WHERE workspace_id = 'account-a' AND table_name = 'sessions' AND row_id = 'note')",
    ).fetch_one(source.pool()).await.unwrap();
    assert!(pending);
    crate::connect_local_library(source.pool(), "account-a", "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-a"))
        .await
        .unwrap();
    let remote = test_db().await;
    copy_replica(source.pool(), remote.pool()).await;
    apply_e2ee_replica_changes(remote.pool(), &keys("account-a"))
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'note'")
        .fetch_one(remote.pool())
        .await
        .unwrap();
    assert_eq!(title, "Edited on B");
}

#[tokio::test]
async fn incoming_personal_notes_join_the_library_without_copying_team_notes() {
    let source = personal_library().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES ('team-note', 'team-a', 'Private team')")
        .execute(source.pool()).await.unwrap();
    crate::connect_local_library(source.pool(), "account-b", "account-a")
        .await
        .unwrap();
    let remote = test_db().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES ('remote-note', 'account-b', 'account-b', 'From B')")
        .execute(remote.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(remote.pool(), &keys("account-b"))
        .await
        .unwrap();
    copy_replica(remote.pool(), source.pool()).await;
    apply_e2ee_replica_changes(source.pool(), &keys("account-b"))
        .await
        .unwrap();
    let identity: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'remote-note'")
            .fetch_one(source.pool())
            .await
            .unwrap();
    assert_eq!(identity, ("account-a".into(), "account-a".into()));
    encrypt_e2ee_replica_changes(source.pool(), &keys("account-b"))
        .await
        .unwrap();
    copy_replica(source.pool(), remote.pool()).await;
    apply_e2ee_replica_changes(remote.pool(), &keys("account-b"))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'team-note'")
        .fetch_one(remote.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn stale_library_confirmation_is_rejected_atomically() {
    let source = personal_library().await;
    assert!(
        crate::connect_local_library(source.pool(), "account-b", "stale-library")
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_library_connections")
        .fetch_one(source.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn backup_keys_and_interrupted_transfers_stay_with_each_account() {
    let db = personal_library().await;
    sqlx::query("UPDATE session_attachments SET cloud_object_key = 'backup-a', cloud_sync_enabled = 1 WHERE id = 'audio'").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO attachment_transfer_jobs (id, attachment_id, session_id, workspace_id, direction, phase, attempt_count, expected_sha256) VALUES ('job-a', 'audio', 'note', 'account-a', 'upload', 'ready', 2, '0000000000000000000000000000000000000000000000000000000000000000')").execute(db.pool()).await.unwrap();
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    let state: (String, i64) = sqlx::query_as(
        "SELECT cloud_object_key, cloud_sync_enabled FROM session_attachments WHERE id = 'audio'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (String::new(), 1));
    assert_eq!(
        crate::local_library_remote_workspace(db.pool(), "account-a")
            .await
            .unwrap(),
        "account-b"
    );
    let job: (String, i64) = sqlx::query_as(
        "SELECT phase, attempt_count FROM attachment_transfer_jobs WHERE id = 'job-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(job, ("completed".into(), 3));
    sqlx::query("UPDATE session_attachments SET cloud_object_key = 'backup-b' WHERE id = 'audio'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-a"))
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-b"))
        .await
        .unwrap();
    for (account, expected) in [("account-a", "backup-a"), ("account-b", "backup-b")] {
        let remote = test_db().await;
        copy_replica(db.pool(), remote.pool()).await;
        apply_e2ee_replica_changes(remote.pool(), &keys(account))
            .await
            .unwrap();
        let key: String = sqlx::query_scalar(
            "SELECT cloud_object_key FROM session_attachments WHERE id = 'audio'",
        )
        .fetch_one(remote.pool())
        .await
        .unwrap();
        assert_eq!(key, expected);
    }
    crate::claim_cloudsync_workspace(db.pool(), "account-a")
        .await
        .unwrap();
    let key: String =
        sqlx::query_scalar("SELECT cloud_object_key FROM session_attachments WHERE id = 'audio'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(key, "backup-a");
    assert!(
        crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-a")
            .await
            .unwrap()
    );
    assert!(
        !crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-b")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn newly_created_attachments_never_publish_another_accounts_backup_key() {
    let db = personal_library().await;
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_attachments (id, workspace_id, session_id, cloud_object_key, cloud_sync_enabled) VALUES ('new-audio', 'account-a', 'note', 'only-b', 1)").execute(db.pool()).await.unwrap();
    crate::connect_local_library(db.pool(), "account-c", "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-a"))
        .await
        .unwrap();
    let remote = test_db().await;
    copy_replica(db.pool(), remote.pool()).await;
    apply_e2ee_replica_changes(remote.pool(), &keys("account-a"))
        .await
        .unwrap();
    let state: (String, i64) = sqlx::query_as("SELECT cloud_object_key, cloud_sync_enabled FROM session_attachments WHERE id = 'new-audio'").fetch_one(remote.pool()).await.unwrap();
    assert_eq!(state, (String::new(), 0));
}

#[tokio::test]
async fn recovery_markers_are_restored_only_for_their_account() {
    let db = personal_library().await;
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('cloudsync_full_resync_pending', '\"generation-a\"')").execute(db.pool()).await.unwrap();
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    assert_eq!(
        crate::cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap(),
        None
    );
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('cloudsync_full_resync_pending', '\"generation-b\"')").execute(db.pool()).await.unwrap();
    crate::claim_cloudsync_workspace(db.pool(), "account-a")
        .await
        .unwrap();
    assert_eq!(
        crate::cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .as_deref(),
        Some("generation-a")
    );
    crate::claim_cloudsync_workspace(db.pool(), "account-b")
        .await
        .unwrap();
    assert_eq!(
        crate::cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .as_deref(),
        Some("generation-b")
    );
}

#[tokio::test]
async fn incoming_deletions_are_retained_when_switching_back() {
    let db = personal_library().await;
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-a"))
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-b"))
        .await
        .unwrap();
    let remote_b = test_db().await;
    copy_replica(db.pool(), remote_b.pool()).await;
    apply_e2ee_replica_changes(remote_b.pool(), &keys("account-b"))
        .await
        .unwrap();
    sqlx::query("DELETE FROM session_participants WHERE id = 'participant'")
        .execute(remote_b.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(remote_b.pool(), &keys("account-b"))
        .await
        .unwrap();
    copy_replica(remote_b.pool(), db.pool()).await;
    apply_e2ee_replica_changes(db.pool(), &keys("account-b"))
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_participants WHERE id = 'participant'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
    crate::claim_cloudsync_workspace(db.pool(), "account-a")
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &keys("account-a"))
        .await
        .unwrap();
    let remote_a = test_db().await;
    copy_replica(db.pool(), remote_a.pool()).await;
    apply_e2ee_replica_changes(remote_a.pool(), &keys("account-a"))
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_participants WHERE id = 'participant'")
            .fetch_one(remote_a.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn migration_stays_compatible_until_another_account_is_explicitly_connected() {
    let db = personal_library().await;
    let before: i64 =
        sqlx::query_scalar("SELECT min_supported_version FROM _anlg_schema_compat WHERE id = 0")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(before < 20260916043000);
    let steps = crate::APP_MIGRATION_STEPS;
    let old_schema = anlg_db_migrate::DbSchema {
        steps: &steps[..steps
            .iter()
            .position(|step| step.id == "20260916043000_local_library_connections")
            .unwrap()],
        validate_cloudsync_table: crate::cloudsync_alter_guard_required,
    };
    anlg_db_migrate::migrate(&db, old_schema).await.unwrap();
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    let after: i64 =
        sqlx::query_scalar("SELECT min_supported_version FROM _anlg_schema_compat WHERE id = 0")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(after, 20260916043000);
    let old_schema = anlg_db_migrate::DbSchema {
        steps: &steps[..steps
            .iter()
            .position(|step| step.id == "20260916043000_local_library_connections")
            .unwrap()],
        validate_cloudsync_table: crate::cloudsync_alter_guard_required,
    };
    assert!(matches!(
        anlg_db_migrate::migrate(&db, old_schema).await,
        Err(anlg_db_migrate::MigrateError::SchemaFromNewerApp { .. })
    ));
}

#[tokio::test]
async fn admitting_a_returning_account_waits_for_sync_configuration_to_activate_it() {
    let db = personal_library().await;
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    crate::bind_cloudsync_account(db.pool(), "account-a")
        .await
        .unwrap();
    assert!(
        crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-b")
            .await
            .unwrap()
    );
    assert!(
        !crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-a")
            .await
            .unwrap()
    );
    crate::claim_cloudsync_workspace(db.pool(), "account-a")
        .await
        .unwrap();
    assert!(
        crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-a")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn stale_incoming_records_cannot_write_into_an_inactive_connection() {
    let db = personal_library().await;
    let remote = test_db().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES ('late-a', 'account-a', 'Delayed A')").execute(remote.pool()).await.unwrap();
    encrypt_e2ee_replica_changes(remote.pool(), &keys("account-a"))
        .await
        .unwrap();
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    copy_replica(remote.pool(), db.pool()).await;
    assert!(matches!(
        apply_e2ee_replica_changes(db.pool(), &keys("account-a")).await,
        Err(E2eeReplicaError::Cancelled)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'late-a'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn a_third_account_requires_consent_even_before_the_first_upload() {
    let db = personal_library().await;
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    assert!(matches!(
        crate::bind_cloudsync_account(db.pool(), "account-c").await,
        Err(crate::CloudsyncWorkspaceError::AccountMismatch)
    ));
    assert!(matches!(
        crate::claim_cloudsync_workspace(db.pool(), "account-c").await,
        Err(crate::CloudsyncWorkspaceError::AccountMismatch)
    ));
    assert!(matches!(
        crate::stage_cloudsync_poison_recovery(db.pool(), "account-a", "account-a").await,
        Err(crate::CloudsyncWorkspaceError::AccountMismatch)
    ));
    assert_eq!(
        crate::ensure_cloudsync_workspace_binding(db.pool())
            .await
            .unwrap(),
        "account-a"
    );
    crate::connect_local_library(db.pool(), "account-c", "account-a")
        .await
        .unwrap();
    assert!(
        crate::cloudsync_workspace_is_claimed_by(db.pool(), "account-c")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn an_unclaimed_bound_library_can_connect_another_account() {
    let db = test_db().await;
    let local = crate::ensure_cloudsync_workspace_binding(db.pool())
        .await
        .unwrap();
    crate::bind_cloudsync_account(db.pool(), "account-a")
        .await
        .unwrap();
    crate::connect_local_library(db.pool(), "account-b", &local)
        .await
        .unwrap();
    let connections: Vec<String> = sqlx::query_scalar(
        "SELECT account_user_id FROM local_library_connections ORDER BY account_user_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(connections, vec!["account-a", "account-b"]);
    assert_eq!(
        crate::ensure_cloudsync_workspace_binding(db.pool())
            .await
            .unwrap(),
        local
    );
}

#[tokio::test]
async fn first_connection_with_a_distinct_local_identity_requires_a_compatible_client() {
    let db = test_db().await;
    let local = crate::ensure_cloudsync_workspace_binding(db.pool())
        .await
        .unwrap();
    crate::connect_local_library(db.pool(), "account-a", &local)
        .await
        .unwrap();
    assert_eq!(
        crate::ensure_cloudsync_workspace_binding(db.pool())
            .await
            .unwrap(),
        local
    );
    let floor: i64 =
        sqlx::query_scalar("SELECT min_supported_version FROM _anlg_schema_compat WHERE id = 0")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(floor, 20260916043000);
}

#[tokio::test]
async fn forwarded_dirty_updates_preserve_edit_time_for_active_and_inactive_accounts() {
    let db = personal_library().await;
    crate::connect_local_library(db.pool(), "account-b", "account-a")
        .await
        .unwrap();
    for active in [true, false] {
        if !active {
            crate::claim_cloudsync_workspace(db.pool(), "account-a")
                .await
                .unwrap();
        }
        sqlx::query("UPDATE e2ee_dirty_rows SET generation = generation + 1, dirtied_at_ms = 1234 WHERE workspace_id = 'account-b' AND table_name = 'sessions' AND row_id = 'note'").execute(db.pool()).await.unwrap();
        let timestamp: i64 = sqlx::query_scalar("SELECT dirtied_at_ms FROM e2ee_dirty_rows WHERE workspace_id = 'account-b' AND table_name = 'sessions' AND row_id = 'note'").fetch_one(db.pool()).await.unwrap();
        assert_eq!(timestamp, 1234);
    }
    sqlx::query("UPDATE sessions SET title = 'New local edit' WHERE id = 'note'")
        .execute(db.pool())
        .await
        .unwrap();
    let timestamp: i64 = sqlx::query_scalar("SELECT dirtied_at_ms FROM e2ee_dirty_rows WHERE workspace_id = 'account-a' AND table_name = 'sessions' AND row_id = 'note'").fetch_one(db.pool()).await.unwrap();
    assert!(timestamp > 1234);
    let forwarded: i64 = sqlx::query_scalar("SELECT dirtied_at_ms FROM e2ee_dirty_rows WHERE workspace_id = 'account-b' AND table_name = 'sessions' AND row_id = 'note'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(forwarded, timestamp);
}
