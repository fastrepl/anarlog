use super::*;

#[tokio::test]
async fn cloudsync_activity_leases_are_idempotent_and_preserved_by_identity_clear() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);

    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-2".to_string())
        .await
        .unwrap();
    runtime.e2ee_sync_hook.clear();

    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["activity_paused"], true);
    assert_eq!(status["deferred_for_capture"], true);
    assert_eq!(status["has_unsent_changes"], serde_json::Value::Null);

    runtime
        .end_cloudsync_activity("capture".to_string(), "missing".to_string())
        .await
        .unwrap();
    assert!(runtime.e2ee_sync_hook.activity_paused());

    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    assert!(runtime.e2ee_sync_hook.activity_paused());
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-2".to_string())
        .await
        .unwrap();
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    assert!(!runtime.e2ee_sync_hook.activity_paused());
}

#[tokio::test]
async fn cloudsync_activity_pause_is_process_scoped() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    assert!(runtime.e2ee_sync_hook.activity_paused());
    drop(runtime);

    let restarted_runtime = PluginDbRuntime::new(db);
    let status = restarted_runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["activity_paused"], false);
    assert_eq!(status["deferred_for_capture"], false);
}

#[tokio::test]
async fn activity_resume_wait_cannot_lose_its_notification() {
    let hook = E2eeSyncHook::default();
    hook.begin_activity("capture".to_string(), "session-1".to_string());
    let mut resumed = Box::pin(hook.wait_until_activity_resumed());

    tokio::select! {
        biased;
        () = &mut resumed => panic!("activity resume wait completed while paused"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(hook.end_activity("capture", "session-1"));
    hook.notify_activity_changed();
    tokio::time::timeout(std::time::Duration::from_millis(100), resumed)
        .await
        .expect("activity resume notification was lost");

    hook.wait_until_activity_resumed().await;
}

#[tokio::test]
async fn clearing_activities_wakes_resume_waiters() {
    let hook = E2eeSyncHook::default();
    hook.begin_activity("capture".to_string(), "session-1".to_string());
    hook.begin_activity("chat".to_string(), "chat-1".to_string());
    let mut resumed = Box::pin(hook.wait_until_activity_resumed());

    tokio::select! {
        biased;
        () = &mut resumed => panic!("activity resume wait completed while paused"),
        _ = tokio::task::yield_now() => {}
    }
    hook.clear_activities();
    tokio::time::timeout(std::time::Duration::from_millis(100), resumed)
        .await
        .expect("activity clear notification was lost");
    assert!(!hook.activity_paused());
}

#[tokio::test]
async fn paused_cloudsync_status_does_not_probe_the_busy_database() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let _held_connection = db.pool().acquire().await.unwrap();

    let status = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.cloudsync_status(),
    )
    .await
    .expect("paused CloudSync status queried the busy database")
    .unwrap();

    assert_eq!(status["activity_paused"], true);
    assert_eq!(status["deferred_for_capture"], true);
}

#[tokio::test]
async fn activity_begin_waits_for_an_in_flight_cloudsync_control_operation() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let control_operation = runtime.cloudsync_control_operation.lock().await;
    let mut begin =
        Box::pin(runtime.begin_cloudsync_activity("capture".to_string(), "session-1".to_string()));

    tokio::select! {
        biased;
        result = &mut begin => panic!("activity begin bypassed recovery: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(runtime.e2ee_sync_hook.activity_paused());

    drop(control_operation);
    tokio::time::timeout(std::time::Duration::from_millis(100), begin)
        .await
        .expect("activity begin did not finish after CloudSync control became idle")
        .unwrap();
}

#[tokio::test]
async fn activity_begin_timeout_fails_closed_and_releases_the_new_lease() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let control_operation = runtime.cloudsync_control_operation.lock().await;

    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.begin_cloudsync_activity_with_timeout(
            "capture".to_string(),
            "session-1".to_string(),
            std::time::Duration::from_millis(10),
        ),
    )
    .await
    .expect("activity begin exceeded its drain deadline")
    .unwrap_err();

    assert!(matches!(error, crate::Error::CloudsyncActivityDrainTimeout));
    assert!(!runtime.e2ee_sync_hook.activity_paused());
    assert!(
        !runtime
            .e2ee_sync_hook
            .has_activity_lease("capture", "session-1")
    );
    drop(control_operation);
}

#[tokio::test]
async fn duplicate_activity_begin_timeout_preserves_the_existing_lease() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let control_operation = runtime.cloudsync_control_operation.lock().await;

    runtime
        .begin_cloudsync_activity_with_timeout(
            "capture".to_string(),
            "session-1".to_string(),
            std::time::Duration::from_millis(10),
        )
        .await
        .unwrap();

    assert!(runtime.e2ee_sync_hook.activity_paused());
    assert!(
        runtime
            .e2ee_sync_hook
            .has_activity_lease("capture", "session-1")
    );
    drop(control_operation);
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
}

#[tokio::test]
async fn activity_end_cancels_a_begin_waiting_for_cloudsync_control() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let control_operation = runtime.cloudsync_control_operation.lock().await;
    let mut begin =
        Box::pin(runtime.begin_cloudsync_activity("capture".to_string(), "session-1".to_string()));

    tokio::select! {
        biased;
        result = &mut begin => panic!("activity begin bypassed CloudSync control: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    drop(control_operation);

    let error = tokio::time::timeout(std::time::Duration::from_millis(100), begin)
        .await
        .expect("cancelled activity begin did not finish")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ended before synchronization became idle")
    );
}

#[tokio::test]
async fn cloudsync_configuration_and_start_fail_promptly_during_activity() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let config = serde_json::to_string(&anlg_db_core::CloudsyncRuntimeConfig {
        connection_string: "managed-database-id".to_string(),
        auth: anlg_db_core::CloudsyncAuth::None,
        tables: Vec::new(),
        sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    })
    .unwrap();

    let configure_error = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.configure_cloudsync(config),
    )
    .await
    .expect("CloudSync configuration waited for the activity to finish")
    .unwrap_err();
    assert!(matches!(
        configure_error,
        crate::Error::CloudsyncActivityDeferred
    ));
    assert_eq!(configure_error.to_string(), "cloudsync_activity_deferred");

    let token_error = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.configure_cloudsync_token(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            crate::CloudsyncE2eeWitness {
                endpoint: "https://example.com/witness".to_string(),
                access_token: "access-token".to_string(),
            },
        ),
    )
    .await
    .expect("CloudSync token configuration waited for the activity to finish")
    .unwrap_err();
    assert!(matches!(
        token_error,
        crate::Error::CloudsyncActivityDeferred
    ));

    let start_error = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.start_cloudsync(),
    )
    .await
    .expect("CloudSync start waited for the activity to finish")
    .unwrap_err();
    assert!(matches!(
        start_error,
        crate::Error::CloudsyncActivityDeferred
    ));
}

#[tokio::test]
async fn cloudsync_start_prearms_reconciliation() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    sqlx::query(
        "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .execute(db.pool())
    .await
    .unwrap();
    let runtime = PluginDbRuntime::new(db);
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    let setup_epoch = runtime
        .e2ee_sync_hook
        .reconciliation_request_epoch()
        .unwrap();
    runtime.e2ee_sync_hook.complete_reconciliation(setup_epoch);

    let _ = runtime.start_cloudsync().await;

    assert!(runtime.e2ee_sync_hook.reconciliation_requested());
}

#[tokio::test]
async fn cloudsync_control_rechecks_activity_after_acquiring_serialization() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let control_operation = runtime.cloudsync_control_operation.lock().await;
    let mut start = Box::pin(runtime.start_cloudsync());

    tokio::select! {
        biased;
        result = &mut start => panic!("CloudSync start bypassed control serialization: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    runtime
        .e2ee_sync_hook
        .begin_activity("capture".to_string(), "session-1".to_string());

    let error = tokio::time::timeout(std::time::Duration::from_millis(100), start)
        .await
        .expect("CloudSync start remained queued after activity began")
        .unwrap_err();
    assert!(matches!(error, crate::Error::CloudsyncActivityDeferred));
    drop(control_operation);
}

#[tokio::test]
async fn local_account_binding_is_not_deferred_by_cloudsync_activity() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();

    let bound = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.bind_cloudsync_account("user-a".to_string()),
    )
    .await
    .expect("local account binding waited for the activity to finish")
    .unwrap();
    assert!(bound);
    assert!(
        runtime
            .e2ee_sync_hook
            .has_activity_lease("capture", "session-1")
    );
}

#[tokio::test]
async fn local_account_binding_times_out_before_mutation_and_remains_fail_closed() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    let control = runtime.cloudsync_control_operation.lock().await;

    let error = runtime
        .bind_cloudsync_account_with_lock_timeout(
            "user-a".to_string(),
            std::time::Duration::from_millis(10),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "CloudSync account binding lock preflight timed out"
    );
    drop(control);

    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    assert!(
        runtime
            .bind_cloudsync_account("user-a".to_string())
            .await
            .unwrap()
    );
    assert!(
        !runtime
            .bind_cloudsync_account("user-b".to_string())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn stop_suspend_and_logout_are_not_deferred_by_cloudsync_activity() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();

    let stop = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.stop_cloudsync(),
    )
    .await
    .expect("CloudSync stop waited for the activity to finish");
    assert!(!matches!(
        stop,
        Err(crate::Error::CloudsyncActivityDeferred)
    ));

    let suspend = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.suspend_cloudsync(),
    )
    .await
    .expect("CloudSync suspension waited for the activity to finish");
    assert!(!matches!(
        suspend,
        Err(crate::Error::CloudsyncActivityDeferred)
    ));
    assert!(runtime.e2ee_sync_hook.activity_paused());

    let logout = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.logout_cloudsync(false),
    )
    .await
    .expect("CloudSync logout waited for the activity to finish");
    assert!(!matches!(
        logout,
        Err(crate::Error::CloudsyncActivityDeferred)
    ));
    assert!(!runtime.e2ee_sync_hook.activity_paused());
}

#[tokio::test]
async fn sign_out_suspend_preserves_activity_leases() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    runtime
        .begin_cloudsync_activity("chat".to_string(), "chat-1".to_string())
        .await
        .unwrap();

    runtime.suspend_cloudsync_for_sign_out().await.unwrap();

    assert!(runtime.e2ee_sync_hook.activity_paused());
    assert!(
        runtime
            .e2ee_sync_hook
            .has_activity_lease("capture", "session-1")
    );
    assert!(runtime.e2ee_sync_hook.has_activity_lease("chat", "chat-1"));
}

#[tokio::test]
async fn sign_out_suspend_preserves_activity_leases_after_non_busy_error() {
    let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    db.pool().close().await;

    let error = runtime.suspend_cloudsync_for_sign_out().await.unwrap_err();

    assert!(!matches!(
        error,
        crate::Error::Cloudsync(anlg_db_core::CloudsyncRuntimeError::LocalStatusBusy)
    ));
    assert!(runtime.e2ee_sync_hook.activity_paused());
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    assert!(!runtime.e2ee_sync_hook.activity_paused());
}

#[tokio::test]
async fn auth_loss_suspend_preserves_activity_leases_after_non_busy_error() {
    let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
    let runtime = PluginDbRuntime::new(std::sync::Arc::clone(&db));
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    db.pool().close().await;

    let error = runtime
        .suspend_cloudsync_after_auth_loss()
        .await
        .unwrap_err();

    assert!(!matches!(
        error,
        crate::Error::Cloudsync(anlg_db_core::CloudsyncRuntimeError::LocalStatusBusy)
    ));
    assert!(runtime.e2ee_sync_hook.activity_paused());
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    assert!(!runtime.e2ee_sync_hook.activity_paused());
}

#[tokio::test]
async fn auth_suspension_bypasses_activity_acquisition_without_clearing_leases() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .e2ee_sync_hook
        .begin_activity("capture".to_string(), "session-1".to_string());
    let acquisition = runtime.cloudsync_activity_acquisition.lock().await;
    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.suspend_cloudsync_after_auth_loss(),
    )
    .await
    .expect("auth-loss suspension waited for activity acquisition")
    .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.suspend_cloudsync_for_sign_out(),
    )
    .await
    .expect("sign-out suspension waited for activity acquisition")
    .unwrap();
    assert!(runtime.e2ee_sync_hook.activity_paused());
    drop(acquisition);
}

#[tokio::test]
async fn raw_suspend_bypasses_activity_acquisition_and_preserves_leases() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .e2ee_sync_hook
        .begin_activity("chat".to_string(), "chat-1".to_string());
    let acquisition = runtime.cloudsync_activity_acquisition.lock().await;
    let mut queued_activity =
        Box::pin(runtime.begin_cloudsync_activity("capture".to_string(), "session-1".to_string()));
    tokio::select! {
        biased;
        result = &mut queued_activity => panic!("queued activity unexpectedly completed: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }

    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        runtime.suspend_cloudsync(),
    )
    .await
    .expect("forced auth cleanup waited for activity acquisition")
    .unwrap();
    assert!(runtime.e2ee_sync_hook.activity_paused());
    drop(acquisition);
    tokio::time::timeout(std::time::Duration::from_millis(100), queued_activity)
        .await
        .expect("queued activity remained blocked")
        .unwrap();
    assert!(
        runtime
            .e2ee_sync_hook
            .has_activity_lease("capture", "session-1")
    );
    assert!(runtime.e2ee_sync_hook.has_activity_lease("chat", "chat-1"));
}

#[tokio::test]
async fn final_activity_release_resets_the_recovery_delay_clock() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    {
        let mut schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
        schedule.claim("generation-1");
        schedule.last_progress_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(61));
    }

    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();

    let schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
    assert!(!schedule.recovery_delayed);
    assert!(!schedule.is_delayed("generation-1"));
}

#[tokio::test]
async fn activity_release_does_not_hide_a_recovery_failure() {
    let db = std::sync::Arc::new(Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    let runtime = PluginDbRuntime::new(db);
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    {
        let mut schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
        schedule.claim("generation-1");
        schedule.mark_failure("generation-1");
    }

    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();

    let schedule = runtime.scheduled_cloudsync_full_resync.lock().unwrap();
    assert!(schedule.recovery_delayed);
    assert!(schedule.is_delayed("generation-1"));
}

#[test]
fn cloudsync_activity_identifiers_are_bounded() {
    assert!(normalized_cloudsync_activity_part(" ".to_string(), "activity").is_err());
    assert!(
        normalized_cloudsync_activity_part("capture\n".to_string(), "activity").is_ok(),
        "outer whitespace is intentionally trimmed"
    );
    assert!(normalized_cloudsync_activity_part("cap\nture".to_string(), "activity").is_err());
    assert!(normalized_cloudsync_activity_part("a".repeat(33), "activity").is_err());
    assert!(normalized_cloudsync_activity_part("k".repeat(129), "key").is_err());
}

#[test]
fn focus_nudges_are_throttled() {
    let base = std::time::Instant::now();

    assert!(focus_nudge_due(None, base));
    assert!(!focus_nudge_due(
        Some(base),
        base + std::time::Duration::from_secs(1)
    ));
    assert!(focus_nudge_due(
        Some(base),
        base + CLOUDSYNC_FOCUS_NUDGE_THROTTLE
    ));
}
