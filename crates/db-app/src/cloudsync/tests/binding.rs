use super::*;

#[tokio::test]
async fn local_workspace_binding_is_stable() {
    let db = test_db().await;

    let first = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
    let second = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[tokio::test]
async fn account_binding_is_durable_without_rekeying_rows() {
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

    sqlx::query(
        "INSERT INTO humans (id, workspace_id, owner_user_id, name)
         VALUES (?, ?, ?, 'Local user')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', ?, ?, 'Session')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();

    bind_cloudsync_account(db.pool(), "user-a").await.unwrap();

    let binding: (String, String) = sqlx::query_as(
        "SELECT json_extract(value_json, '$.workspace_id'),
                json_extract(value_json, '$.account_user_id')
         FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let human: (String, String, String) = sqlx::query_as(
        "SELECT id, workspace_id, owner_user_id FROM humans WHERE name = 'Local user'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let session: (String, String) =
        sqlx::query_as("SELECT workspace_id, owner_user_id FROM sessions WHERE id = 'session'")
            .fetch_one(db.pool())
            .await
            .unwrap();

    assert_eq!(binding, (local_workspace.clone(), "user-a".to_string()));
    assert_eq!(
        human,
        (
            local_workspace.clone(),
            local_workspace.clone(),
            local_workspace.clone(),
        )
    );
    assert_eq!(session, (local_workspace.clone(), local_workspace));
    assert!(
        !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn account_binding_rejects_switching_before_and_after_claim() {
    let db = test_db().await;

    bind_cloudsync_account(db.pool(), "user-a").await.unwrap();
    let error = bind_cloudsync_account(db.pool(), "user-b")
        .await
        .unwrap_err();
    assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    bind_cloudsync_account(db.pool(), "user-a").await.unwrap();
    let error = bind_cloudsync_account(db.pool(), "user-b")
        .await
        .unwrap_err();
    assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
}

#[tokio::test]
async fn claim_rekeys_every_synced_table_and_is_idempotent() {
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
    let statements = [
        (
            "INSERT INTO organizations (id, workspace_id) VALUES ('org', ?)",
            local_workspace.as_str(),
        ),
        (
            "INSERT INTO humans (id, workspace_id) VALUES ('human', ?)",
            "",
        ),
        (
            "INSERT INTO sessions (id, workspace_id) VALUES ('session', ?)",
            local_workspace.as_str(),
        ),
        (
            "INSERT INTO session_documents (id, session_id, workspace_id) VALUES ('document', 'session', ?)",
            "",
        ),
        (
            "INSERT INTO transcripts (id, session_id, workspace_id) VALUES ('transcript', 'session', ?)",
            "",
        ),
        (
            "INSERT INTO session_participants (id, session_id, workspace_id) VALUES ('participant', 'session', ?)",
            "",
        ),
        (
            "INSERT INTO action_items (id, session_id, workspace_id) VALUES ('action', 'session', ?)",
            "",
        ),
        (
            "INSERT INTO session_attachments (id, session_id, workspace_id) VALUES ('attachment', 'session', ?)",
            "",
        ),
    ];
    for (statement, workspace_id) in statements {
        sqlx::query(statement)
            .bind(workspace_id)
            .execute(db.pool())
            .await
            .unwrap();
    }

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let changes_before_repeat: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let changes_after_repeat: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(db.pool())
        .await
        .unwrap();

    assert_eq!(changes_after_repeat, changes_before_repeat);

    for table_name in crate::E2EE_DOMAIN_TABLES {
        let sql = format!(
            "SELECT COUNT(*) FROM {} WHERE workspace_id <> 'user-a'",
            table_name
        );
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{table_name} was not claimed");
    }
}

#[tokio::test]
async fn cancelled_claim_rolls_back_and_releases_local_writes() {
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO humans (id, workspace_id, owner_user_id, name)
         VALUES (?, ?, ?, 'Local user')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1 FROM sequence WHERE row_index < 255
         )
         INSERT INTO action_items (
           id, workspace_id, created_by, updated_by, text
         )
         SELECT
           printf('action-%03d', row_index),
           ?,
           ?,
           ?,
           'Legacy action'
         FROM sequence",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();
    let dirty_rows_before_claim: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let scan_checks = (crate::E2EE_DOMAIN_TABLES.len() * 2) + 4;
    let cancellation_barrier = 3 + scan_checks + 4;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        claim_cloudsync_workspace_cancellable(db.pool(), "user-a", || {
            cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
                >= cancellation_barrier
        }),
    )
    .await
    .expect("workspace claim did not drain after cancellation")
    .unwrap_err();
    assert!(matches!(error, CloudsyncWorkspaceError::ClaimCancelled));
    assert_eq!(
        cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
        cancellation_barrier
    );

    let changed_action_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM action_items
         WHERE workspace_id <> ? OR created_by <> ? OR updated_by <> ?",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let account_human_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE id = 'user-a'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let dirty_rows_after_cancellation: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(changed_action_items, 0);
    assert_eq!(account_human_count, 0);
    assert_eq!(dirty_rows_after_cancellation, dirty_rows_before_claim);
    assert!(
        !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
            .await
            .unwrap()
    );

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-cancel', ?, ?, 'Local write')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool()),
    )
    .await
    .expect("cancelled workspace claim kept the database busy")
    .unwrap();

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let stale_action_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM action_items
         WHERE workspace_id <> 'user-a'
            OR created_by <> 'user-a'
            OR updated_by <> 'user-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stale_action_items, 0);
}

#[tokio::test]
async fn late_claim_cancellation_rolls_back_100k_rows_within_two_seconds() {
    const LEGACY_ROW_COUNT: usize = 100_000;

    assert_eq!(crate::E2EE_DOMAIN_TABLES.first(), Some(&"action_items"));
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1
           FROM sequence
           WHERE row_index < 99999
         )
         INSERT INTO action_items (
           id, workspace_id, created_by, updated_by, text
         )
         SELECT
           printf('claim-stress-%06d', row_index),
           ?,
           ?,
           ?,
           'Legacy action'
         FROM sequence",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();

    let action_item_batches = LEGACY_ROW_COUNT.div_ceil(CLOUDSYNC_WORKSPACE_CLAIM_BATCH_SIZE);
    let empty_domain_tables = crate::E2EE_DOMAIN_TABLES.len() - 1;
    let cancellation_barrier =
        3 + (action_item_batches * 2) + (empty_domain_tables * 2) + (action_item_batches * 4);
    let mut cancellation_checks = 0;
    let mut cancellation_started_at = None;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        claim_cloudsync_workspace_cancellable(db.pool(), "user-a", || {
            cancellation_checks += 1;
            if cancellation_checks >= cancellation_barrier {
                cancellation_started_at.get_or_insert_with(std::time::Instant::now);
                true
            } else {
                false
            }
        }),
    )
    .await
    .expect("100k-row claim did not reach late cancellation")
    .unwrap_err();
    let rollback_elapsed = cancellation_started_at
        .expect("claim cancellation timestamp was not captured")
        .elapsed();
    assert!(matches!(error, CloudsyncWorkspaceError::ClaimCancelled));
    assert_eq!(cancellation_checks, cancellation_barrier);
    assert!(
        rollback_elapsed < std::time::Duration::from_secs(2),
        "100k-row claim rollback took {rollback_elapsed:?}"
    );

    let changed_action_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM action_items
         WHERE workspace_id <> ? OR created_by <> ? OR updated_by <> ?",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(changed_action_items, 0);
    assert!(
        !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
            .await
            .unwrap()
    );

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-100k-claim-cancel', ?, ?, 'Local write')",
        )
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool()),
    )
    .await
    .expect("100k-row claim rollback kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn detects_an_existing_account_claim() {
    let db = test_db().await;

    assert!(
        !cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
            .await
            .unwrap()
    );
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();

    assert!(
        cloudsync_workspace_is_claimed_by(db.pool(), "user-a")
            .await
            .unwrap()
    );
    assert!(
        !cloudsync_workspace_is_claimed_by(db.pool(), "user-b")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn claim_rekeys_local_user_identities_and_references() {
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

    sqlx::query(
        "INSERT INTO humans (id, workspace_id, owner_user_id, name, email)
         VALUES (?, ?, ?, '', 'local@example.com'),
                (?, ?, ?, 'Local user', '')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO organizations (id, workspace_id, owner_user_id)
         VALUES ('org', ?, ?)",
    )
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id)
         VALUES ('session', ?, ?)",
    )
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents
           (id, workspace_id, session_id, created_by, updated_by)
         VALUES ('document', ?, 'session', ?, ?)",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, session_id, owner_user_id)
         VALUES ('transcript', ?, 'session', ?)",
    )
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_participants
           (id, workspace_id, session_id, owner_user_id, human_id)
         VALUES ('participant', ?, 'session', ?, ?)",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO action_items
           (id, workspace_id, session_id, assignee_human_id, created_by, updated_by)
         VALUES ('action', ?, 'session', ?, ?, ?)",
    )
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .bind(&local_workspace)
    .bind(LEGACY_DEFAULT_USER_ID)
    .execute(db.pool())
    .await
    .unwrap();

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();

    let humans: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, name, email FROM humans ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        humans,
        vec![(
            "user-a".to_string(),
            "Local user".to_string(),
            "local@example.com".to_string(),
        )]
    );

    for (table, column) in USER_ID_REFERENCES {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {column} IN (?, ?)");
        let stale_count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(&local_workspace)
            .bind(LEGACY_DEFAULT_USER_ID)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(stale_count, 0, "stale identity in {table}.{column}");
    }

    let participant: (String, String) = sqlx::query_as(
        "SELECT owner_user_id, human_id FROM session_participants WHERE id = 'participant'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(participant, ("user-a".to_string(), "user-a".to_string()));
    let action: (String, String, String) = sqlx::query_as(
        "SELECT assignee_human_id, created_by, updated_by FROM action_items WHERE id = 'action'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        action,
        (
            "user-a".to_string(),
            "user-a".to_string(),
            "user-a".to_string(),
        )
    );
}

#[tokio::test]
async fn claim_tombstones_duplicate_self_participants_before_rekeying() {
    let db = test_db().await;
    let local_workspace = ensure_cloudsync_workspace_binding(db.pool()).await.unwrap();

    sqlx::query("INSERT INTO humans (id, workspace_id) VALUES (?, ?), ('user-a', ?)")
        .bind(&local_workspace)
        .bind(&local_workspace)
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', ?)")
        .bind(&local_workspace)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO session_participants (id, workspace_id, session_id, human_id)
         VALUES ('legacy-self', ?, 'session', ?),
                ('account-self', ?, 'session', 'user-a')",
    )
    .bind(&local_workspace)
    .bind(&local_workspace)
    .bind(&local_workspace)
    .execute(db.pool())
    .await
    .unwrap();

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_participants
         WHERE session_id = 'session' AND human_id = 'user-a' AND deleted_at IS NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let legacy_deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM session_participants WHERE id = 'legacy-self'")
            .fetch_one(db.pool())
            .await
            .unwrap();

    assert_eq!(active_count, 1);
    assert!(legacy_deleted_at.is_some());
}

#[tokio::test]
async fn claim_rejects_account_switching() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();

    let error = claim_cloudsync_workspace(db.pool(), "user-b")
        .await
        .unwrap_err();

    assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
}

#[tokio::test]
async fn claim_rejects_foreign_workspace_rows() {
    let db = test_db().await;
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('session', 'other-user')")
        .execute(db.pool())
        .await
        .unwrap();

    let error = claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ForeignWorkspace { table } if table == "sessions"
    ));
}

#[tokio::test]
async fn repeated_claim_allows_shared_workspace_rows() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id)
         VALUES ('shared-session', 'workspace-b', 'user-b')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();

    let workspace_id: String =
        sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'shared-session'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(workspace_id, "workspace-b");
}
