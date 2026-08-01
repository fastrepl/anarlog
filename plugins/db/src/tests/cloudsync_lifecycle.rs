use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;

use crate::{commands, runtime};

use super::support::{setup_enabled_cloudsync_runtime, setup_runtime, unreachable_witness};

#[tokio::test]
async fn sign_out_suspend_command_preserves_activity_leases() {
    let (_dir, runtime) = setup_runtime().await;
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let app = tauri::test::mock_builder()
        .manage(Arc::clone(&runtime))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();

    commands::suspend_cloudsync_for_sign_out(app.state())
        .await
        .unwrap();

    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["activity_paused"], true);
    assert_eq!(status["deferred_for_capture"], true);
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn sign_out_suspend_command_defers_busy_pool_teardown() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(2),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db.cloudsync_init("sessions", None, None).await.unwrap();
    let runtime = Arc::new(runtime::PluginDbRuntime::new(Arc::new(db)));
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let held_connection = runtime.pool().acquire().await.unwrap();
    let app = tauri::test::mock_builder()
        .manage(Arc::clone(&runtime))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();

    tokio::time::timeout(
        Duration::from_millis(1_500),
        commands::suspend_cloudsync_for_sign_out(app.state()),
    )
    .await
    .expect("sign-out suspension exceeded its pool-drain deadline")
    .unwrap();

    drop(held_connection);
    let mut replacement =
        tokio::time::timeout(Duration::from_millis(1_500), runtime.pool().acquire())
            .await
            .expect("pool did not open a replacement after deferred teardown")
            .unwrap();
    replacement.return_to_pool().await;
    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["activity_paused"], true);
    assert_eq!(status["deferred_for_capture"], true);

    runtime.suspend_cloudsync().await.unwrap();
    tokio::time::timeout(
        Duration::from_millis(1_500),
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('after-sign-out-retry', '', '', 'Note')",
        )
        .execute(runtime.pool()),
    )
    .await
    .expect("local write could not reuse the pool after deferred teardown retry")
    .unwrap();
}

#[tokio::test]
async fn cloudsync_transport_stays_inert_until_configured() {
    let (_dir, runtime) = setup_runtime().await;

    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["cloudsync_enabled"], false);
    assert_eq!(status["configured"], false);

    runtime
        .configure_cloudsync(
            serde_json::json!({
                "connection_string": "managed-database-id",
                "auth": { "type": "token", "token": "test-token" },
                "tables": anlg_db_app::cloudsync_table_registry(),
                "sync_interval_ms": 30_000,
                "wait_ms": 5_000,
                "max_retries": 3
            })
            .to_string(),
        )
        .await
        .unwrap();
    runtime.start_cloudsync().await.unwrap();

    let status = runtime.cloudsync_status().await.unwrap();
    assert_eq!(status["configured"], true);
    assert_eq!(status["running"], false);
    assert_eq!(status["network_initialized"], false);

    runtime.logout_cloudsync(false).await.unwrap();
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}

#[tokio::test]
async fn cloudsync_waits_for_legacy_migration_verification() {
    let (_dir, runtime) = setup_enabled_cloudsync_runtime().await;
    sqlx::query(
        "UPDATE storage_migration_state
         SET parity_verified = 0, last_error = 'completed_with_issues'
         WHERE id = 'legacy_v1'",
    )
    .execute(runtime.pool())
    .await
    .unwrap();

    let error = runtime
        .configure_cloudsync_token(
            "managed-database-id".to_string(),
            "token".to_string(),
            "user-a".to_string(),
            unreachable_witness("user-a"),
        )
        .await
        .unwrap_err();

    assert!(matches!(&error, crate::Error::Io(_)));
    assert!(
        error
            .to_string()
            .contains("migration needs attention before CloudSync can start")
    );
    assert!(runtime.start_cloudsync().await.is_err());
    assert!(runtime.sync_cloudsync_now().await.is_err());
    assert_eq!(
        runtime.cloudsync_status().await.unwrap()["configured"],
        false
    );
}
