use super::*;

#[test]
fn recovery_waits_longer_without_progress() {
    assert_eq!(
        cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Progressed),
        Some(CLOUDSYNC_FULL_RESYNC_PROGRESS_INTERVAL)
    );
    assert_eq!(
        cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Waiting),
        Some(CLOUDSYNC_FULL_RESYNC_RETRY_INTERVAL)
    );
    assert_eq!(
        cloudsync_recovery_step_delay(CloudsyncRecoveryStep::Complete),
        None
    );
}

#[test]
fn cloudsync_completion_requires_confirmed_send_and_receive() {
    let completed: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "send": {
                "status": "synced",
                "localVersion": 2,
                "serverVersion": 2
            },
            "receive": {
                "rows": 0,
                "tables": [],
                "complete": true
            }
        }))
        .unwrap();
    let unconfirmed_send: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "send": {
                "status": "syncing",
                "localVersion": 2,
                "serverVersion": 1
            }
        }))
        .unwrap();
    let incomplete_receive: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "send": {
                "status": "synced",
                "localVersion": 2,
                "serverVersion": 2
            },
            "receive": {
                "rows": 1,
                "tables": ["sessions"],
                "complete": false
            }
        }))
        .unwrap();
    let send_only: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "send": {
                "status": "synced",
                "localVersion": 2,
                "serverVersion": 2
            }
        }))
        .unwrap();
    let receive_only: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": 0,
                "tables": [],
                "complete": true
            }
        }))
        .unwrap();
    let delivered_final: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": 1,
                "tables": ["e2ee_records"],
                "chunks": 1,
                "complete": true
            }
        }))
        .unwrap();
    let delivered_non_final: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": 1,
                "tables": ["e2ee_records"],
                "chunks": 1,
                "complete": false
            }
        }))
        .unwrap();
    let failed_receive: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "send": {
                "status": "synced",
                "localVersion": 2,
                "serverVersion": 2
            },
            "receive": {
                "rows": 0,
                "tables": [],
                "complete": true,
                "error": "database is locked"
            }
        }))
        .unwrap();
    let failed_after_chunks: anlg_db_core::CloudsyncNetworkResult =
        serde_json::from_value(serde_json::json!({
            "receive": {
                "rows": 1,
                "tables": ["e2ee_records"],
                "chunks": 1,
                "complete": false,
                "error": "later chunk failed"
            }
        }))
        .unwrap();

    assert!(cloudsync_send_completed(&completed));
    assert!(cloudsync_receive_completed(&completed));
    assert!(!cloudsync_send_completed(&unconfirmed_send));
    assert!(!cloudsync_receive_completed(&incomplete_receive));
    assert!(cloudsync_send_completed(&incomplete_receive));
    assert!(cloudsync_send_completed(&send_only));
    assert!(!cloudsync_receive_completed(&send_only));
    assert!(!cloudsync_send_completed(&receive_only));
    assert!(cloudsync_receive_completed(&receive_only));
    assert!(!cloudsync_receive_delivered_final(&receive_only));
    assert!(cloudsync_receive_delivered_final(&delivered_final));
    assert!(cloudsync_receive_delivered(&delivered_non_final));
    assert!(!cloudsync_recovery_snapshot_ready(
        true,
        &delivered_non_final
    ));
    assert!(cloudsync_recovery_snapshot_ready(true, &delivered_final));
    assert!(!cloudsync_recovery_snapshot_ready(true, &receive_only));
    assert!(!cloudsync_recovery_snapshot_ready(false, &delivered_final));
    assert!(cloudsync_send_completed(&failed_receive));
    assert!(!cloudsync_receive_completed(&failed_receive));
    assert!(!cloudsync_receive_delivered(&failed_after_chunks));
    assert!(cloudsync_receive_requires_reconciliation(
        &failed_after_chunks
    ));
    assert!(!cloudsync_send_completed(
        &anlg_db_core::CloudsyncNetworkResult::default()
    ));
}

#[test]
fn full_resync_schedule_tracks_generation_until_cancelled() {
    let mut schedule = CloudsyncFullResyncSchedule::default();

    schedule.claim("generation-1");
    assert!(!schedule.is_delayed("generation-1"));
    schedule.last_progress_at = Some(std::time::Instant::now() - CLOUDSYNC_RECOVERY_DELAYED_AFTER);
    assert!(schedule.is_delayed("generation-1"));
    schedule.mark_progress("generation-1");
    assert!(!schedule.is_delayed("generation-1"));
    schedule.mark_failure("generation-1");
    assert!(schedule.is_delayed("generation-1"));
    schedule.mark_progress("generation-1");
    assert!(!schedule.is_delayed("generation-1"));
    schedule.claim("generation-1");
    assert!(schedule.is_active("generation-1"));

    schedule.cancel();
    assert!(!schedule.is_active("generation-1"));
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tokio::test]
async fn witness_repair_refresh_is_reused_until_activity_invalidates_it() {
    use std::io::{Read, Write};

    let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    anlg_db_app::claim_cloudsync_workspace(db.pool(), "workspace-1")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE sync_probe (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("sync_probe", None, None).await.unwrap();
    let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    let workspace_key = runtime.workspace_key("workspace-1").unwrap();
    let witness_server = MockServer::start().await;
    Mock::given(path("/sync/e2ee/witness/workspace-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "initialized": true,
            "initializedAt": "2026-08-26T00:00:00Z",
            "headSequence": 130,
            "throughSequence": 130,
            "nextAfterSequence": 130,
            "events": [],
        })))
        .mount(&witness_server)
        .await;
    runtime.e2ee_sync_hook.set_witness(
        crate::e2ee_witness::E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/workspace-1", witness_server.uri()),
                access_token: "access-token".to_string(),
            },
            "workspace-1",
        )
        .unwrap(),
    );

    let cloudsync_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    cloudsync_listener.set_nonblocking(true).unwrap();
    let cloudsync_endpoint = format!("http://{}", cloudsync_listener.local_addr().unwrap());
    let (cloudsync_shutdown_tx, cloudsync_shutdown_rx) = std::sync::mpsc::sync_channel(1);
    let cloudsync_server = std::thread::spawn(move || {
        loop {
            if cloudsync_shutdown_rx.try_recv().is_ok() {
                return;
            }
            let (mut stream, _) = match cloudsync_listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("failed to accept CloudSync status request: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).unwrap();
            assert!(
                String::from_utf8_lossy(&request[..size]).contains("/status"),
                "recovery made an unexpected CloudSync request"
            );
            let body =
                r#"{"lastOptimisticVersion":9999999,"lastConfirmedVersion":9999999,"gaps":[]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            stream.flush().unwrap();
        }
    });
    let config = anlg_db_core::CloudsyncRuntimeConfig {
        connection_string: "test-database".to_string(),
        auth: anlg_db_core::CloudsyncAuth::None,
        tables: vec![],
        sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    };
    db.cloudsync_prepare_manual_transport(config.clone())
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(cloudsync_endpoint)
        .bind("witness-repair-refresh-test")
        .fetch_optional(db.pool())
        .await
        .unwrap();

    let generation =
        anlg_db_app::stage_cloudsync_poison_recovery(db.pool(), "workspace-1", "workspace-1")
            .await
            .unwrap();
    anlg_db_app::ensure_cloudsync_recovery_state(
        db.pool(),
        &generation,
        "workspace-1",
        "workspace-1",
        &workspace_key,
    )
    .await
    .unwrap();
    for (from, to) in [
        (
            anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout,
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
        ),
        (
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
        ),
        (
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
            anlg_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
        ),
        (
            anlg_db_app::CloudsyncRecoveryPhase::NeedCleanReceive,
            anlg_db_app::CloudsyncRecoveryPhase::NeedWitnessRepair,
        ),
    ] {
        assert!(
            anlg_db_app::advance_cloudsync_recovery_phase(db.pool(), &generation, from, to)
                .await
                .unwrap()
        );
    }

    let events = (0..130)
        .map(|index| {
            let sealed = workspace_key
                .seal_field(
                    "workspace-1",
                    "sessions",
                    &format!("session-{index:03}"),
                    "$row",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    serde_json::json!(true),
                )
                .unwrap();
            anlg_db_app::E2eeWitnessEvent {
                sequence: u64::try_from(index + 1).unwrap(),
                record_id: sealed.record_id,
                workspace_id: "workspace-1".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
                payload: sealed.payload,
            }
        })
        .collect::<Vec<_>>();
    anlg_db_app::merge_e2ee_witness_events(db.pool(), &workspace_key, "workspace-1", &events)
        .await
        .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(db.pool(), "workspace-1", 130)
        .await
        .unwrap();

    let auth_generation = runtime.cloudsync_auth_generation();
    runtime
        .schedule_cloudsync_full_resync(generation, config, auth_generation)
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let repaired: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap();
            if repaired >= 64 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("witness repair did not finish its first bounded batch");
    runtime
        .begin_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let drain = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let repaired: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
                .fetch_one(db.pool())
                .await
                .unwrap();
            if repaired == 130 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await;
    let failure = if drain.is_err() {
        let repaired: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_records")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_repair_pending")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let phase = anlg_db_app::cloudsync_recovery_state(db.pool())
            .await
            .unwrap()
            .map(|state| state.phase);
        let witness_requests = witness_server.received_requests().await.unwrap().len();
        Some(format!(
            "witness repair did not drain all bounded batches: repaired={repaired}, pending={pending}, phase={phase:?}, witness_requests={witness_requests}"
        ))
    } else {
        None
    };
    runtime.cancel_cloudsync_full_resync().await;
    cloudsync_shutdown_tx.send(()).unwrap();
    cloudsync_server.join().unwrap();
    if let Some(failure) = failure {
        panic!("{failure}");
    }

    let requests = witness_server.received_requests().await.unwrap();
    let refreshes = requests
        .iter()
        .filter(|request| request.method == wiremock::http::Method::GET)
        .count();
    assert!(
        refreshes >= 4,
        "witness freshness was not re-established after activity"
    );
    assert!(
        refreshes <= 5,
        "witness was refreshed {refreshes} times while draining three batches around one activity"
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn activity_drains_stalled_full_resync_before_immediate_local_write() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stalled_tx, stalled_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        let (mut status_stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        status_stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let size = status_stream.read(&mut request).unwrap();
        assert!(
            String::from_utf8_lossy(&request[..size]).contains("/status"),
            "recovery did not probe status before sending"
        );
        let body = r#"{"lastOptimisticVersion":0,"lastConfirmedVersion":0,"gaps":[]}"#;
        write!(
                status_stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        status_stream.flush().unwrap();
        drop(status_stream);

        let (stalled_stream, _) = listener.accept().unwrap();
        stalled_tx.send(()).unwrap();
        let _stalled_stream = stalled_stream;
        let _ = release_rx.recv_timeout(std::time::Duration::from_secs(5));
    });

    let db = std::sync::Arc::new(Db::connect_memory().await.unwrap());
    anlg_db_app::prepare_schema(db.as_ref()).await.unwrap();
    anlg_db_app::claim_cloudsync_workspace(db.pool(), "workspace-1")
        .await
        .unwrap();
    db.cloudsync_init(CLOUDSYNC_REPLICA_TABLE, None, None)
        .await
        .unwrap();
    let runtime = std::sync::Arc::new(PluginDbRuntime::new(std::sync::Arc::clone(&db)));
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    runtime
        .set_e2ee_recovery_key("workspace-1", &recovery_key)
        .unwrap();
    let key = runtime.workspace_key("workspace-1").unwrap();
    let (configured_hook, _witness_server) = configured_test_hook().await;
    runtime
        .e2ee_sync_hook
        .set_witness(configured_hook.witness().unwrap());

    let generation =
        anlg_db_app::stage_cloudsync_poison_recovery(db.pool(), "workspace-1", "workspace-1")
            .await
            .unwrap();
    let state = anlg_db_app::ensure_cloudsync_recovery_state(
        db.pool(),
        &generation,
        "workspace-1",
        "workspace-1",
        &key,
    )
    .await
    .unwrap();
    assert!(
        anlg_db_app::advance_cloudsync_recovery_phase(
            db.pool(),
            &generation,
            anlg_db_app::CloudsyncRecoveryPhase::NeedFirstLogout,
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
        )
        .await
        .unwrap()
    );
    assert!(
        anlg_db_app::insert_cloudsync_recovery_barrier(db.pool(), &state, &key)
            .await
            .unwrap()
    );
    assert!(
        anlg_db_app::advance_cloudsync_recovery_phase(
            db.pool(),
            &generation,
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierInsert,
            anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm,
        )
        .await
        .unwrap()
    );

    let config = anlg_db_core::CloudsyncRuntimeConfig {
        connection_string: "test-database".to_string(),
        auth: anlg_db_core::CloudsyncAuth::None,
        tables: vec![],
        sync_interval_ms: DEFAULT_CLOUDSYNC_INTERVAL_MS,
        wait_ms: Some(5_000),
        max_retries: Some(3),
    };
    db.cloudsync_prepare_manual_transport(config.clone())
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(endpoint)
        .bind("full-resync-interrupt-test")
        .fetch_optional(db.pool())
        .await
        .unwrap();
    let auth_generation = runtime.cloudsync_auth_generation();
    runtime
        .schedule_cloudsync_full_resync(generation.clone(), config, auth_generation)
        .await;
    tokio::task::spawn_blocking(move || {
        stalled_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("full resync did not reach the stalled manual send");
    })
    .await
    .unwrap();

    tokio::time::timeout(
        std::time::Duration::from_millis(2_500),
        runtime.begin_cloudsync_activity_with_timeout(
            "capture".to_string(),
            "session-1".to_string(),
            std::time::Duration::from_millis(1_800),
        ),
    )
    .await
    .expect("activity did not drain the stalled full resync")
    .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_millis(250),
        runtime.execute(
            "INSERT INTO sessions (id, workspace_id, title)
                 VALUES ('local-after-recovery', 'workspace-1', 'Local')"
                .to_string(),
            vec![],
        ),
    )
    .await
    .expect("local write remained blocked after full-resync cancellation")
    .unwrap();

    assert_eq!(
        anlg_db_app::cloudsync_recovery_state(db.pool())
            .await
            .unwrap()
            .unwrap()
            .phase,
        anlg_db_app::CloudsyncRecoveryPhase::NeedBarrierConfirm
    );
    assert!(
        runtime
            .scheduled_cloudsync_full_resync
            .lock()
            .unwrap()
            .is_active(&generation)
    );

    runtime.cancel_cloudsync_full_resync().await;
    runtime
        .end_cloudsync_activity("capture".to_string(), "session-1".to_string())
        .await
        .unwrap();
    let _ = release_tx.send(());
    server.join().unwrap();
}
