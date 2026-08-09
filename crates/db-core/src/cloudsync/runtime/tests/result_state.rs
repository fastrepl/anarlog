use super::super::*;

#[test]
fn embedded_sync_failures_update_runtime_error_state() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    runtime.lock().unwrap().last_sync_at_ms = Some(42);
    let result = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "failed".to_string(),
            local_version: 4,
            server_version: 3,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: Some("schema mismatch".to_string()),
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    let runtime = runtime.lock().unwrap();
    assert!(runtime.last_sync.is_some());
    assert_eq!(runtime.last_sync_at_ms, Some(42));
    assert_eq!(runtime.consecutive_failures, 1);
    assert_eq!(
        runtime.last_error_kind,
        Some(anlg_cloudsync::ErrorKind::Fatal)
    );
    assert!(
        runtime
            .last_error
            .as_deref()
            .unwrap()
            .contains("schema mismatch")
    );
}

#[test]
fn embedded_sqlite_contention_remains_retryable() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let result = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: false,
            error: Some("database is locked".to_string()),
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    let runtime = runtime.lock().unwrap();
    assert_eq!(
        runtime.last_error_kind,
        Some(anlg_cloudsync::ErrorKind::Transient)
    );
    assert_eq!(runtime.consecutive_failures, 1);
}

#[test]
fn embedded_sync_in_progress_preserves_last_successful_sync() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    runtime.lock().unwrap().last_sync_at_ms = Some(42);
    let result = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "syncing".to_string(),
            local_version: 4,
            server_version: 3,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    let runtime = runtime.lock().unwrap();
    assert_eq!(runtime.last_sync_at_ms, Some(42));
    assert!(runtime.last_error.is_none());
    assert_eq!(runtime.consecutive_failures, 0);
}

#[test]
fn initial_sync_stays_unsettled_while_receive_is_in_progress() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let result = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    let runtime = runtime.lock().unwrap();
    assert!(runtime.last_sync_at_ms.is_none());
    assert!(runtime.last_error.is_none());
}

#[test]
fn completed_receive_marks_sync_complete_without_a_send_result() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let result = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    assert!(runtime.lock().unwrap().last_sync_at_ms.is_some());
}

#[test]
fn settled_network_preserves_last_success_while_local_work_remains() {
    let runtime = Mutex::new(CloudsyncRuntimeState {
        last_sync_at_ms: Some(42),
        ..Default::default()
    });
    let result = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 2,
            server_version: 2,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(&runtime, result, true, CloudsyncActivityTrigger::Background);

    assert_eq!(runtime.lock().unwrap().last_sync_at_ms, Some(42));
}

#[test]
fn bounded_sync_combines_send_and_receive_results() {
    let send = CloudsyncNetworkResult {
        send: Some(anlg_cloudsync::NetworkSendResult {
            status: "synced".to_string(),
            local_version: 4,
            server_version: 4,
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: None,
    };
    let receive = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 2048,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };

    let result = merge_bounded_sync_results(send.clone(), receive.clone());

    assert_eq!(result.send, send.send);
    assert_eq!(result.receive, receive.receive);
    assert!(sync_result_needs_receive_progress(&result));
}

#[test]
fn manual_noop_sync_is_recorded() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let result = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(&runtime, result, false, CloudsyncActivityTrigger::Manual);

    let runtime = runtime.lock().unwrap();
    assert_eq!(runtime.activity_log.len(), 1);
    assert_eq!(
        runtime.activity_log.front().unwrap().status,
        crate::CloudsyncActivityStatus::Completed
    );
    assert_eq!(
        runtime.activity_log.front().unwrap().trigger,
        CloudsyncActivityTrigger::Manual
    );
}

#[test]
fn background_noop_sync_is_not_recorded() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let result = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        result,
        false,
        CloudsyncActivityTrigger::Background,
    );

    assert!(runtime.lock().unwrap().activity_log.is_empty());
}

#[test]
fn background_completion_closes_a_progress_entry() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());
    let progress = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 3,
            tables: vec!["sessions".to_string()],
            chunks: 1,
            bytes: 0,
            complete: false,
            error: None,
            last_failure: None,
        }),
    };
    let completed = CloudsyncNetworkResult {
        send: None,
        receive: Some(anlg_cloudsync::NetworkReceiveResult {
            rows: 0,
            tables: Vec::new(),
            chunks: 0,
            bytes: 0,
            complete: true,
            error: None,
            last_failure: None,
        }),
    };

    record_sync_result(
        &runtime,
        progress,
        false,
        CloudsyncActivityTrigger::Background,
    );
    record_sync_result(
        &runtime,
        completed,
        false,
        CloudsyncActivityTrigger::Background,
    );

    let runtime = runtime.lock().unwrap();
    assert_eq!(runtime.activity_log.len(), 2);
    assert_eq!(
        runtime.activity_log.back().unwrap().status,
        crate::CloudsyncActivityStatus::Completed
    );
}

#[test]
fn sync_activity_log_is_bounded() {
    let runtime = Mutex::new(CloudsyncRuntimeState::default());

    for _ in 0..=MAX_ACTIVITY_LOG_ENTRIES {
        record_sync_error(
            &runtime,
            &anlg_cloudsync::Error::Io(std::io::Error::other("offline")),
            CloudsyncActivityTrigger::Background,
        );
    }

    assert_eq!(
        runtime.lock().unwrap().activity_log.len(),
        MAX_ACTIVITY_LOG_ENTRIES
    );
}
