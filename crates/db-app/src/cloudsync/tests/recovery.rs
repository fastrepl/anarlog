use super::*;

#[tokio::test]
async fn recovery_state_and_phase_transitions_are_crash_safe() {
    let db = test_db().await;
    let generation = "generation-1";
    let key = recovery_key("user-a");
    mark_full_resync_pending(db.pool(), generation).await;

    let state = ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
        .await
        .unwrap();
    assert_eq!(state.phase, CloudsyncRecoveryPhase::NeedFirstLogout);
    assert_eq!(
        ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
            .await
            .unwrap(),
        state
    );
    assert!(
        !advance_cloudsync_recovery_phase(
            db.pool(),
            "other-generation",
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedBarrierInsert,
        )
        .await
        .unwrap()
    );
    assert!(
        advance_cloudsync_recovery_phase(
            db.pool(),
            generation,
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedBarrierInsert,
        )
        .await
        .unwrap()
    );
    assert!(
        !advance_cloudsync_recovery_phase(
            db.pool(),
            generation,
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedBarrierInsert,
        )
        .await
        .unwrap()
    );
    assert_eq!(
        cloudsync_recovery_state(db.pool())
            .await
            .unwrap()
            .unwrap()
            .phase,
        CloudsyncRecoveryPhase::NeedBarrierInsert
    );
}

#[tokio::test]
async fn poison_recovery_generation_is_idempotent_and_restores_its_pending_marker() {
    let db = test_db().await;
    claim_cloudsync_workspace(db.pool(), "user-a")
        .await
        .unwrap();
    for id in [
        CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID,
        CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID,
    ] {
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(id)
            .bind(serde_json::to_string("stale-generation").unwrap())
            .execute(db.pool())
            .await
            .unwrap();
    }

    let generation = stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
        .await
        .unwrap();
    assert_eq!(
        stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
            .await
            .unwrap(),
        generation
    );
    let stale_markers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_settings
         WHERE id IN (?, ?)",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
    .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stale_markers, 0);

    let key = recovery_key("user-a");
    ensure_cloudsync_recovery_state(db.pool(), &generation, "user-a", "user-a", &key)
        .await
        .unwrap();
    sqlx::query("DELETE FROM app_settings WHERE id = ?")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();

    assert_eq!(
        stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a")
            .await
            .unwrap(),
        generation
    );
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        Some(generation.clone())
    );
    assert!(matches!(
        stage_cloudsync_poison_recovery(db.pool(), "user-a", "workspace-b").await,
        Err(CloudsyncWorkspaceError::RecoveryConflict)
    ));
    sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
        .bind(serde_json::to_string("conflicting-generation").unwrap())
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        stage_cloudsync_poison_recovery(db.pool(), "user-a", "user-a").await,
        Err(CloudsyncWorkspaceError::RecoveryConflict)
    ));
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        Some("conflicting-generation".to_string())
    );
}

#[tokio::test]
async fn active_recovery_generation_survives_restart_during_logout_windows() {
    for phase in [
        CloudsyncRecoveryPhase::NeedFirstLogout,
        CloudsyncRecoveryPhase::NeedCleanReceive,
    ] {
        let db = test_db().await;
        claim_cloudsync_workspace(db.pool(), "user-a")
            .await
            .unwrap();
        let projection = projection(
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
        let generation = commit_cloudsync_workspace_projection(db.pool(), &projection, true)
            .await
            .unwrap()
            .unwrap();
        let key = recovery_key("user-a");
        ensure_cloudsync_recovery_state(db.pool(), &generation, "user-a", "user-a", &key)
            .await
            .unwrap();

        if phase == CloudsyncRecoveryPhase::NeedCleanReceive {
            for (expected, next) in [
                (
                    CloudsyncRecoveryPhase::NeedFirstLogout,
                    CloudsyncRecoveryPhase::NeedBarrierInsert,
                ),
                (
                    CloudsyncRecoveryPhase::NeedBarrierInsert,
                    CloudsyncRecoveryPhase::NeedBarrierConfirm,
                ),
                (
                    CloudsyncRecoveryPhase::NeedBarrierConfirm,
                    CloudsyncRecoveryPhase::NeedCleanReceive,
                ),
            ] {
                assert!(
                    advance_cloudsync_recovery_phase(db.pool(), &generation, expected, next,)
                        .await
                        .unwrap()
                );
            }
        }

        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
            .bind(serde_json::to_string("superseding-generation").unwrap())
            .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
            .execute(db.pool())
            .await
            .unwrap();

        let resumed = commit_cloudsync_workspace_projection(db.pool(), &projection, true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resumed, generation);
        assert_eq!(
            cloudsync_full_resync_generation(db.pool()).await.unwrap(),
            Some(generation.clone())
        );
        let state = cloudsync_recovery_state(db.pool()).await.unwrap().unwrap();
        assert_eq!(state.generation, generation);
        assert_eq!(state.phase, phase);
    }
}

#[tokio::test]
async fn superseded_recovery_cannot_advance_or_clear_the_new_generation() {
    let db = test_db().await;
    let generation = "generation-1";
    let replacement = "generation-2";
    let key = recovery_key("user-a");
    mark_full_resync_pending(db.pool(), generation).await;
    ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
        .await
        .unwrap();

    sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
        .bind(serde_json::to_string(replacement).unwrap())
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        !advance_cloudsync_recovery_phase(
            db.pool(),
            generation,
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedBarrierInsert,
        )
        .await
        .unwrap()
    );

    sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
        .bind(serde_json::to_string(generation).unwrap())
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();
    for (expected, next) in [
        (
            CloudsyncRecoveryPhase::NeedFirstLogout,
            CloudsyncRecoveryPhase::NeedBarrierInsert,
        ),
        (
            CloudsyncRecoveryPhase::NeedBarrierInsert,
            CloudsyncRecoveryPhase::NeedBarrierConfirm,
        ),
        (
            CloudsyncRecoveryPhase::NeedBarrierConfirm,
            CloudsyncRecoveryPhase::NeedCleanReceive,
        ),
        (
            CloudsyncRecoveryPhase::NeedCleanReceive,
            CloudsyncRecoveryPhase::NeedWitnessRepair,
        ),
        (
            CloudsyncRecoveryPhase::NeedWitnessRepair,
            CloudsyncRecoveryPhase::NeedBarrierCleanup,
        ),
        (
            CloudsyncRecoveryPhase::NeedBarrierCleanup,
            CloudsyncRecoveryPhase::NeedTransportResume,
        ),
    ] {
        assert!(
            advance_cloudsync_recovery_phase(db.pool(), generation, expected, next)
                .await
                .unwrap()
        );
    }
    sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
        .bind(serde_json::to_string(replacement).unwrap())
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();

    assert!(
        !complete_cloudsync_recovery(db.pool(), generation)
            .await
            .unwrap()
    );
    assert_eq!(
        cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .as_deref(),
        Some(replacement)
    );
    assert!(cloudsync_recovery_state(db.pool()).await.unwrap().is_some());

    sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = ?")
        .bind(serde_json::to_string(generation).unwrap())
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        complete_cloudsync_recovery(db.pool(), generation)
            .await
            .unwrap()
    );
    assert!(
        cloudsync_full_resync_generation(db.pool())
            .await
            .unwrap()
            .is_none()
    );
    assert!(cloudsync_recovery_state(db.pool()).await.unwrap().is_none());
}

#[tokio::test]
async fn recovery_barrier_requires_exact_authenticated_content() {
    let db = test_db().await;
    let generation = "generation-1";
    let key = recovery_key("user-a");
    mark_full_resync_pending(db.pool(), generation).await;
    let state = ensure_cloudsync_recovery_state(db.pool(), generation, "user-a", "user-a", &key)
        .await
        .unwrap();

    assert!(
        insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
            .await
            .unwrap()
    );
    assert!(
        cloudsync_recovery_barrier_is_exact(db.pool(), &state, &key)
            .await
            .unwrap()
    );
    assert!(
        !insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
            .await
            .unwrap()
    );

    sqlx::query("UPDATE e2ee_records SET payload = 'wrong' WHERE id = ?")
        .bind(&state.barrier_id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        !cloudsync_recovery_barrier_is_exact(db.pool(), &state, &key)
            .await
            .unwrap()
    );
    assert!(matches!(
        insert_cloudsync_recovery_barrier(db.pool(), &state, &key).await,
        Err(CloudsyncWorkspaceError::RecoveryConflict)
    ));
    assert!(matches!(
        delete_cloudsync_recovery_barrier(db.pool(), &state, &key).await,
        Err(CloudsyncWorkspaceError::RecoveryConflict)
    ));

    sqlx::query("UPDATE e2ee_records SET workspace_id = ?, payload = ? WHERE id = ?")
        .bind(&state.workspace_id)
        .bind(&state.barrier_payload)
        .bind(&state.barrier_id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        delete_cloudsync_recovery_barrier(db.pool(), &state, &key)
            .await
            .unwrap()
    );
    assert!(
        !delete_cloudsync_recovery_barrier(db.pool(), &state, &key)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn detects_whether_the_encrypted_replica_has_local_records() {
    let db = test_db().await;

    assert!(
        cloudsync_encrypted_replica_is_empty(db.pool())
            .await
            .unwrap()
    );

    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
         VALUES ('record-a', 'workspace-a', 'payload-a')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        !cloudsync_encrypted_replica_is_empty(db.pool())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn full_resync_reset_markers_classify_legacy_poison_and_checked_resets() {
    let db = test_db().await;
    let generation = "generation-a";
    let value_json = serde_json::to_string(generation).unwrap();
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(&value_json)
        .execute(db.pool())
        .await
        .unwrap();

    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ResetRequired
    );
    assert!(
        !mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), "stale-generation")
            .await
            .unwrap()
    );

    mark_cloudsync_full_resync_reset_applied(db.pool(), generation)
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::PoisonRecoveryRequired
    );

    assert!(
        mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
            .await
            .unwrap()
    );
    assert!(
        mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
            .await
            .unwrap()
    );
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
    );

    clear_cloudsync_full_resync_pending(db.pool(), "stale-generation")
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
    );

    clear_cloudsync_full_resync_pending(db.pool(), generation)
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        None
    );
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ResetRequired
    );
    let remaining_markers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_settings
         WHERE id IN (?, ?, ?)",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
    .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining_markers, 0);
}

#[tokio::test]
async fn checked_reset_marker_is_atomic_idempotent_and_generation_scoped() {
    let db = test_db().await;
    let generation = "generation-a";
    let value_json = serde_json::to_string(generation).unwrap();
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(&value_json)
        .execute(db.pool())
        .await
        .unwrap();

    assert!(
        mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
            .await
            .unwrap()
    );
    assert!(
        mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
            .await
            .unwrap()
    );
    let applied_markers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_settings
         WHERE id IN (?, ?) AND value_json = ?",
    )
    .bind(CLOUDSYNC_FULL_RESYNC_RESET_APPLIED_ID)
    .bind(CLOUDSYNC_FULL_RESYNC_RECEIVE_ONLY_RESET_APPLIED_ID)
    .bind(&value_json)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(applied_markers, 2);

    let newer_generation = "generation-b";
    let newer_value_json = serde_json::to_string(newer_generation).unwrap();
    sqlx::query(
        "UPDATE app_settings
         SET value_json = ?
         WHERE id = ?",
    )
    .bind(&newer_value_json)
    .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        !mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), generation)
            .await
            .unwrap()
    );
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), newer_generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ResetRequired
    );

    assert!(
        mark_cloudsync_full_resync_receive_only_reset_applied(db.pool(), newer_generation)
            .await
            .unwrap()
    );
    clear_cloudsync_full_resync_pending(db.pool(), generation)
        .await
        .unwrap();
    assert_eq!(
        cloudsync_full_resync_generation(db.pool()).await.unwrap(),
        Some(newer_generation.to_string())
    );
    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), newer_generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::ReceiveOnlyResetApplied
    );
}

#[tokio::test]
async fn stale_legacy_only_reset_marker_requires_poison_recovery() {
    let db = test_db().await;
    let generation = "generation-a";
    let newer_generation = "generation-b";
    let newer_value_json = serde_json::to_string(newer_generation).unwrap();
    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
        .bind(CLOUDSYNC_FULL_RESYNC_PENDING_ID)
        .bind(newer_value_json)
        .execute(db.pool())
        .await
        .unwrap();
    mark_cloudsync_full_resync_reset_applied(db.pool(), generation)
        .await
        .unwrap();

    assert_eq!(
        cloudsync_full_resync_reset_state(db.pool(), newer_generation)
            .await
            .unwrap(),
        CloudsyncFullResyncResetState::PoisonRecoveryRequired
    );
}
