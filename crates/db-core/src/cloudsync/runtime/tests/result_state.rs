use super::super::*;

fn network_result(
    send_status: Option<&str>,
    receive: Option<(i64, bool, Option<&str>)>,
) -> CloudsyncNetworkResult {
    CloudsyncNetworkResult {
        send: send_status.map(|status| anlg_cloudsync::NetworkSendResult {
            status: status.to_string(),
            local_version: 4,
            server_version: if status == "synced" { 4 } else { 3 },
            chunks: 1,
            bytes: 1024,
            last_failure: None,
        }),
        receive: receive.map(
            |(rows, complete, error)| anlg_cloudsync::NetworkReceiveResult {
                rows,
                tables: if rows > 0 {
                    vec!["sessions".to_string()]
                } else {
                    Vec::new()
                },
                chunks: if rows > 0 { 1 } else { 0 },
                bytes: if rows > 0 { 2048 } else { 0 },
                complete,
                error: error.map(str::to_string),
                last_failure: None,
            },
        ),
    }
}

#[test]
fn record_sync_result_tracks_settlement_and_failures() {
    struct StateCase {
        name: &'static str,
        initial_last_sync_at_ms: Option<u64>,
        result: CloudsyncNetworkResult,
        local_work_remaining: bool,
        expected_last_error: Option<&'static str>,
        expected_error_kind: Option<anlg_cloudsync::ErrorKind>,
        expected_consecutive_failures: u32,
        expected_last_sync_at_ms: Option<u64>,
        expected_last_sync_at_is_set: bool,
    }

    let cases = [
        StateCase {
            name: "embedded sync failures",
            initial_last_sync_at_ms: Some(42),
            result: network_result(Some("failed"), Some((0, true, Some("schema mismatch")))),
            local_work_remaining: false,
            expected_last_error: Some("send status: failed; receive error: schema mismatch"),
            expected_error_kind: Some(anlg_cloudsync::ErrorKind::Fatal),
            expected_consecutive_failures: 1,
            expected_last_sync_at_ms: Some(42),
            expected_last_sync_at_is_set: true,
        },
        StateCase {
            name: "sqlite contention",
            initial_last_sync_at_ms: None,
            result: network_result(None, Some((0, false, Some("database is locked")))),
            local_work_remaining: false,
            expected_last_error: Some("receive error: database is locked"),
            expected_error_kind: Some(anlg_cloudsync::ErrorKind::Transient),
            expected_consecutive_failures: 1,
            expected_last_sync_at_ms: None,
            expected_last_sync_at_is_set: false,
        },
        StateCase {
            name: "send in progress",
            initial_last_sync_at_ms: Some(42),
            result: network_result(Some("syncing"), Some((3, false, None))),
            local_work_remaining: false,
            expected_last_error: None,
            expected_error_kind: None,
            expected_consecutive_failures: 0,
            expected_last_sync_at_ms: Some(42),
            expected_last_sync_at_is_set: true,
        },
        StateCase {
            name: "initial receive in progress",
            initial_last_sync_at_ms: None,
            result: network_result(None, Some((3, false, None))),
            local_work_remaining: false,
            expected_last_error: None,
            expected_error_kind: None,
            expected_consecutive_failures: 0,
            expected_last_sync_at_ms: None,
            expected_last_sync_at_is_set: false,
        },
        StateCase {
            name: "completed receive without send result",
            initial_last_sync_at_ms: None,
            result: network_result(None, Some((0, true, None))),
            local_work_remaining: false,
            expected_last_error: None,
            expected_error_kind: None,
            expected_consecutive_failures: 0,
            expected_last_sync_at_ms: None,
            expected_last_sync_at_is_set: true,
        },
        StateCase {
            name: "settled network with local work remaining",
            initial_last_sync_at_ms: Some(42),
            result: network_result(Some("synced"), Some((0, true, None))),
            local_work_remaining: true,
            expected_last_error: None,
            expected_error_kind: None,
            expected_consecutive_failures: 0,
            expected_last_sync_at_ms: Some(42),
            expected_last_sync_at_is_set: true,
        },
    ];

    for case in cases {
        let runtime = Mutex::new(CloudsyncRuntimeState {
            last_sync_at_ms: case.initial_last_sync_at_ms,
            ..Default::default()
        });
        record_sync_result(
            &runtime,
            case.result,
            case.local_work_remaining,
            CloudsyncActivityTrigger::Background,
        );

        let runtime = runtime.lock().unwrap();
        assert!(runtime.last_sync.is_some(), "{}", case.name);
        assert_eq!(
            runtime.last_error.as_deref(),
            case.expected_last_error,
            "{}",
            case.name
        );
        assert_eq!(
            runtime.last_error_kind, case.expected_error_kind,
            "{}",
            case.name
        );
        assert_eq!(
            runtime.consecutive_failures, case.expected_consecutive_failures,
            "{}",
            case.name
        );
        assert_eq!(
            runtime.last_sync_at_ms.is_some(),
            case.expected_last_sync_at_is_set,
            "{}",
            case.name
        );
        if let Some(expected_last_sync_at_ms) = case.expected_last_sync_at_ms {
            assert_eq!(
                runtime.last_sync_at_ms,
                Some(expected_last_sync_at_ms),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn background_sync_uses_shared_receive_errors() {
    assert_eq!(
        crate::cloudsync_receive_error(&CloudsyncNetworkResult {
            send: None,
            receive: None,
        }),
        None
    );
    for (error, failure, expected) in [
        (None, None, None),
        (
            Some("later chunk failed"),
            None,
            Some("receive error: later chunk failed"),
        ),
        (
            None,
            Some("check_failed"),
            Some("receive failure: \"check_failed\""),
        ),
        (
            Some("later chunk failed"),
            Some("check_failed"),
            Some("receive error: later chunk failed; receive failure: \"check_failed\""),
        ),
    ] {
        for complete in [false, true] {
            let result = CloudsyncNetworkResult {
                send: None,
                receive: Some(anlg_cloudsync::NetworkReceiveResult {
                    rows: 0,
                    tables: Vec::new(),
                    chunks: 0,
                    bytes: 0,
                    complete,
                    error: error.map(str::to_string),
                    last_failure: failure.map(Into::into),
                }),
            };
            assert_eq!(crate::cloudsync_receive_error(&result).as_deref(), expected);

            let runtime = Mutex::new(CloudsyncRuntimeState::default());
            record_sync_result(
                &runtime,
                result,
                false,
                CloudsyncActivityTrigger::Background,
            );

            let runtime = runtime.lock().unwrap();
            assert_eq!(runtime.last_error.as_deref(), expected);
        }
    }
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
fn sync_logging_persists_transfer_start_and_end_but_not_each_progress_step() {
    use crate::cloudsync::types::CloudsyncActivityStatus::{Completed, Failed, Progress};

    let background = CloudsyncActivityTrigger::Background;
    let manual = CloudsyncActivityTrigger::Manual;

    assert_eq!(
        sync_result_log_level(background, Completed, false, None),
        None
    );
    assert_eq!(
        sync_result_log_level(manual, Completed, false, None),
        Some(SyncLogLevel::Info)
    );
    assert_eq!(
        sync_result_log_level(background, Progress, true, Some(Completed)),
        Some(SyncLogLevel::Info)
    );
    assert_eq!(
        sync_result_log_level(background, Progress, true, Some(Progress)),
        Some(SyncLogLevel::Debug)
    );
    assert_eq!(
        sync_result_log_level(background, Completed, false, Some(Progress)),
        Some(SyncLogLevel::Info)
    );
    assert_eq!(
        sync_result_log_level(background, Completed, false, Some(Failed)),
        Some(SyncLogLevel::Info)
    );
}
