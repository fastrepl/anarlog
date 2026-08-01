use super::*;

#[tokio::test]
async fn workspace_projection_replaces_stale_server_rows() {
    let db = test_db().await;
    replace_cloudsync_workspace_projection(
        db.pool(),
        &projection(
            "user-a",
            vec![
                projected_workspace(
                    "user-a",
                    "user-a",
                    "personal",
                    "membership-personal",
                    "owner",
                    "Personal",
                ),
                projected_workspace(
                    "workspace-shared",
                    "user-b",
                    "shared",
                    "membership-shared",
                    "member",
                    "Shared",
                ),
            ],
        ),
    )
    .await
    .unwrap();

    replace_cloudsync_workspace_projection(
        db.pool(),
        &projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "My notes",
            )],
        ),
    )
    .await
    .unwrap();

    let workspaces: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let memberships: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, workspace_id, user_id, role, created_at, updated_at
             FROM workspace_memberships ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(
        workspaces,
        vec![("user-a".to_string(), "My notes".to_string())]
    );
    assert_eq!(
        memberships,
        vec![(
            "membership-personal".to_string(),
            "user-a".to_string(),
            "user-a".to_string(),
            "owner".to_string(),
            "2026-07-16T00:01:00Z".to_string(),
            "2026-07-16T00:02:00Z".to_string(),
        )]
    );
}

#[tokio::test]
async fn workspace_reconciliation_stages_revoked_sessions_before_projection_commit() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![
            projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            ),
            projected_workspace(
                "workspace-shared",
                "user-b",
                "shared",
                "membership-shared",
                "member",
                "Shared",
            ),
        ],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-personal', 'user-a', 'user-a', 'Personal'),
                ('session-shared', 'workspace-shared', 'user-b', 'Shared')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let personal_only = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    let plan = stage_cloudsync_workspace_reconciliation(db.pool(), &personal_only)
        .await
        .unwrap();

    assert_eq!(
        plan,
        CloudsyncWorkspaceReconciliationPlan {
            granted_workspace_ids: vec![],
            revoked_workspace_ids: vec!["workspace-shared".to_string()],
        }
    );
    assert!(plan.requires_replica_reset());
    assert!(plan.requires_full_resync());
    let memberships_before_commit: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let queued: Vec<(String, String)> = sqlx::query_as(
        "SELECT session_id, workspace_id
         FROM cloudsync_session_evictions ORDER BY session_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        memberships_before_commit,
        vec!["user-a".to_string(), "workspace-shared".to_string()]
    );
    assert_eq!(
        queued,
        vec![("session-shared".to_string(), "workspace-shared".to_string(),)]
    );

    let generation = commit_cloudsync_workspace_projection(
        db.pool(),
        &personal_only,
        plan.requires_full_resync(),
    )
    .await
    .unwrap()
    .unwrap();

    let memberships_after_commit: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(memberships_after_commit, vec!["user-a".to_string()]);
    assert_eq!(session_count, 2);
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        Some(generation.clone())
    );
    assert!(
        cloudsync_full_resync_requires_reset(db.pool(), &generation)
            .await
            .unwrap()
    );
    mark_cloudsync_full_resync_reset_applied(db.pool(), &generation)
        .await
        .unwrap();
    assert!(
        !cloudsync_full_resync_requires_reset(db.pool(), &generation)
            .await
            .unwrap()
    );
    let newer_generation = commit_cloudsync_workspace_projection(db.pool(), &personal_only, true)
        .await
        .unwrap()
        .unwrap();
    assert!(
        cloudsync_full_resync_requires_reset(db.pool(), &newer_generation)
            .await
            .unwrap()
    );
    clear_cloudsync_full_resync_pending(db.pool(), &generation)
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        Some(newer_generation.clone())
    );
    mark_cloudsync_full_resync_reset_applied(db.pool(), &newer_generation)
        .await
        .unwrap();
    clear_cloudsync_full_resync_pending(db.pool(), &newer_generation)
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        None
    );
    assert!(
        cloudsync_full_resync_requires_reset(db.pool(), &newer_generation)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn cancelled_workspace_reconciliation_rolls_back_the_first_eviction_batch() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![
            projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            ),
            projected_workspace(
                "workspace-shared",
                "user-b",
                "shared",
                "membership-shared",
                "member",
                "Shared",
            ),
        ],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1 FROM sequence WHERE row_index < 255
         )
         INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         SELECT
           printf('shared-%03d', row_index),
           'workspace-shared',
           'user-b',
           'Shared'
         FROM sequence",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let personal_only = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        stage_cloudsync_workspace_reconciliation_cancellable(db.pool(), &personal_only, || {
            cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 7
        }),
    )
    .await
    .expect("workspace reconciliation did not drain after cancellation")
    .unwrap_err();
    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ProjectionCancelled
    ));
    assert_eq!(
        cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
        7
    );

    let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let memberships: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(queued_count, 0);
    assert_eq!(
        memberships,
        vec!["user-a".to_string(), "workspace-shared".to_string()]
    );

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-cancelled-stage', 'user-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled workspace reconciliation kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn cancelled_reconciliation_rolls_back_a_large_legacy_membership_scan() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let personal = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    replace_cloudsync_workspace_projection(db.pool(), &personal)
        .await
        .unwrap();
    seed_legacy_workspace_projection_rows(db.pool(), 511).await;

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        stage_cloudsync_workspace_reconciliation_cancellable(db.pool(), &personal, || {
            cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 6
        }),
    )
    .await
    .expect("legacy membership reconciliation did not drain after cancellation")
    .unwrap_err();
    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ProjectionCancelled
    ));
    assert_eq!(
        cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
        6
    );

    let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let membership_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(workspace_count, 513);
    assert_eq!(membership_count, 513);

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-legacy-scan-cancel', 'user-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled legacy membership scan kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn cancelled_projection_commit_rolls_back_projection_and_eviction_cleanup() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![
            projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            ),
            projected_workspace(
                "workspace-shared",
                "user-b",
                "shared",
                "membership-shared",
                "member",
                "Shared",
            ),
        ],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    sqlx::query(
        "WITH RECURSIVE sequence(row_index) AS (
           VALUES (0)
           UNION ALL
           SELECT row_index + 1 FROM sequence WHERE row_index < 255
         )
         INSERT INTO cloudsync_session_evictions (session_id, workspace_id)
         SELECT printf('eviction-%03d', row_index), 'user-a'
         FROM sequence",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let personal_only = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Changed",
        )],
    );

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        commit_cloudsync_workspace_projection_cancellable(db.pool(), &personal_only, true, || {
            cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 11
        }),
    )
    .await
    .expect("workspace projection commit did not drain after cancellation")
    .unwrap_err();
    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ProjectionCancelled
    ));
    assert_eq!(
        cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
        11
    );

    let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let workspaces: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let memberships: Vec<String> =
        sqlx::query_scalar("SELECT workspace_id FROM workspace_memberships ORDER BY workspace_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(queued_count, 256);
    assert_eq!(
        workspaces,
        vec![
            ("user-a".to_string(), "Personal".to_string()),
            ("workspace-shared".to_string(), "Shared".to_string()),
        ]
    );
    assert_eq!(
        memberships,
        vec!["user-a".to_string(), "workspace-shared".to_string()]
    );
    assert!(
        cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .is_none()
    );

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-cancelled-commit', 'user-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled workspace projection commit kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn cancelled_projection_commit_rolls_back_large_legacy_table_deletes() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    seed_legacy_workspace_projection_rows(db.pool(), 511).await;
    let changed = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Changed",
        )],
    );

    let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        commit_cloudsync_workspace_projection_cancellable(db.pool(), &changed, true, || {
            cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 >= 7
        }),
    )
    .await
    .expect("legacy projection deletion did not drain after cancellation")
    .unwrap_err();
    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ProjectionCancelled
    ));
    assert_eq!(
        cancellation_checks.load(std::sync::atomic::Ordering::SeqCst),
        7
    );

    let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let membership_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let personal_name: String =
        sqlx::query_scalar("SELECT name FROM workspaces WHERE id = 'user-a'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(workspace_count, 513);
    assert_eq!(membership_count, 513);
    assert_eq!(personal_name, "Personal");
    assert!(
        cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .is_none()
    );

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-legacy-delete-cancel', 'user-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("cancelled legacy projection deletion kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn late_projection_cancellation_rolls_back_100k_rows_within_two_seconds() {
    const LEGACY_ROW_COUNT: usize = 100_000;

    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    seed_legacy_workspace_projection_rows(db.pool(), (LEGACY_ROW_COUNT - 1) as i64).await;
    let changed = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Changed",
        )],
    );

    let rows_per_table = LEGACY_ROW_COUNT + 1;
    let batches_per_table =
        rows_per_table.div_ceil(CLOUDSYNC_WORKSPACE_PROJECTION_BATCH_SIZE as usize);
    let cancellation_barrier = 3 + (batches_per_table * 4);
    let mut cancellation_checks = 0;
    let mut cancellation_started_at = None;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        commit_cloudsync_workspace_projection_cancellable(db.pool(), &changed, true, || {
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
    .expect("100k-row projection did not reach late cancellation")
    .unwrap_err();
    let rollback_elapsed = cancellation_started_at
        .expect("projection cancellation timestamp was not captured")
        .elapsed();
    assert!(matches!(
        error,
        CloudsyncWorkspaceError::ProjectionCancelled
    ));
    assert_eq!(cancellation_checks, cancellation_barrier);
    assert!(
        rollback_elapsed < std::time::Duration::from_secs(2),
        "100k-row projection rollback took {rollback_elapsed:?}"
    );

    let workspace_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let membership_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_memberships")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(workspace_count, rows_per_table as i64);
    assert_eq!(membership_count, rows_per_table as i64);

    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-100k-projection-cancel', 'user-a', 'user-a', 'Local write')",
        )
        .execute(db.pool()),
    )
    .await
    .expect("100k-row projection rollback kept the database busy")
    .unwrap();
}

#[tokio::test]
async fn workspace_projection_batches_have_composite_keyset_indexes() {
    let db = test_db().await;
    let session_index_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name
         FROM pragma_index_info('idx_sessions_workspace_id_id')
         ORDER BY seqno",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let eviction_index_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name
         FROM pragma_index_info(
           'idx_cloudsync_session_evictions_workspace_id_session_id'
         )
         ORDER BY seqno",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let membership_index_columns: Vec<String> = sqlx::query_scalar(
        "SELECT name
         FROM pragma_index_info(
           'idx_workspace_memberships_user_deleted_id'
         )
         ORDER BY seqno",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(
        session_index_columns,
        vec!["workspace_id".to_string(), "id".to_string()]
    );
    assert_eq!(
        eviction_index_columns,
        vec!["workspace_id".to_string(), "session_id".to_string()]
    );
    assert_eq!(
        membership_index_columns,
        vec![
            "user_id".to_string(),
            "deleted_at".to_string(),
            "id".to_string()
        ]
    );
}

#[tokio::test]
async fn reauthorized_workspace_cancels_staged_session_evictions() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    let current = projection(
        "user-a",
        vec![
            projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            ),
            projected_workspace(
                "workspace-shared",
                "user-b",
                "shared",
                "membership-shared",
                "member",
                "Shared",
            ),
        ],
    );
    replace_cloudsync_workspace_projection(db.pool(), &current)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title)
         VALUES ('session-shared', 'workspace-shared', 'Shared')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let personal_only = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    stage_cloudsync_workspace_reconciliation(db.pool(), &personal_only)
        .await
        .unwrap();

    commit_cloudsync_workspace_projection(db.pool(), &current, false)
        .await
        .unwrap();

    let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(queued_count, 0);
}

#[tokio::test]
async fn workspace_reconciliation_requires_the_claimed_account() {
    let db = test_db().await;
    let error = stage_cloudsync_workspace_reconciliation(
        db.pool(),
        &projection(
            "user-a",
            vec![projected_workspace(
                "user-a",
                "user-a",
                "personal",
                "membership-personal",
                "owner",
                "Personal",
            )],
        ),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, CloudsyncWorkspaceError::AccountMismatch));
    let queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloudsync_session_evictions")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(queue_count, 0);
}

#[tokio::test]
async fn cloudsync_write_filter_scope_is_local_and_versioned() {
    let db = test_db().await;
    assert!(
        !cloudsync_write_filter_installed(db.pool(), "user-a")
            .await
            .unwrap()
    );

    set_cloudsync_personal_write_scope(db.pool(), "user-a")
        .await
        .unwrap();
    mark_cloudsync_write_filter_installed(db.pool())
        .await
        .unwrap();

    let writable_workspace_ids: Vec<String> = sqlx::query_scalar(
        "SELECT allowed_workspace_id
             FROM cloudsync_writable_workspaces
             ORDER BY allowed_workspace_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(writable_workspace_ids, vec!["user-a".to_string()]);
    assert!(
        cloudsync_write_filter_installed(db.pool(), "user-a")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn invalid_workspace_projections_preserve_existing_rows() {
    let db = test_db().await;
    let valid = projection(
        "user-a",
        vec![projected_workspace(
            "user-a",
            "user-a",
            "personal",
            "membership-personal",
            "owner",
            "Personal",
        )],
    );
    replace_cloudsync_workspace_projection(db.pool(), &valid)
        .await
        .unwrap();

    let mut missing_personal = valid.clone();
    missing_personal.personal_workspace_id = "workspace-missing".to_string();
    let mut invalid_role = valid.clone();
    invalid_role.workspaces[0].role = "viewer".to_string();
    let mut invalid_membership_timestamp = valid.clone();
    invalid_membership_timestamp.workspaces[0].membership_created_at = String::new();
    let mut invalid_kind = valid.clone();
    invalid_kind.workspaces.push(projected_workspace(
        "workspace-shared",
        "user-b",
        "team",
        "membership-shared",
        "member",
        "Shared",
    ));
    let mut duplicate_personal = valid.clone();
    duplicate_personal.workspaces.push(projected_workspace(
        "workspace-personal-2",
        "user-b",
        "personal",
        "membership-personal-2",
        "owner",
        "Other personal",
    ));
    let mut duplicate_workspace = valid.clone();
    duplicate_workspace.workspaces.push(projected_workspace(
        "user-a",
        "user-b",
        "shared",
        "membership-shared",
        "member",
        "Shared",
    ));
    let mut duplicate_membership = valid.clone();
    duplicate_membership.workspaces.push(projected_workspace(
        "workspace-shared",
        "user-b",
        "shared",
        "membership-personal",
        "member",
        "Shared",
    ));

    for invalid in [
        missing_personal,
        invalid_role,
        invalid_membership_timestamp,
        invalid_kind,
        duplicate_personal,
        duplicate_workspace,
        duplicate_membership,
    ] {
        let error = replace_cloudsync_workspace_projection(db.pool(), &invalid)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            CloudsyncWorkspaceError::InvalidWorkspaceProjection
        ));
    }

    let workspaces: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM workspaces ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let memberships: Vec<(String, String)> =
        sqlx::query_as("SELECT id, workspace_id FROM workspace_memberships ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        workspaces,
        vec![("user-a".to_string(), "Personal".to_string())]
    );
    assert_eq!(
        memberships,
        vec![("membership-personal".to_string(), "user-a".to_string(),)]
    );
}
