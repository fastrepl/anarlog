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

#[tokio::test]
async fn confirmed_prefix_retry_sends_the_tail_without_recording_a_sync_failure() {
    use crate::cloudsync::ops::guarded_interruptible_network_send_changes;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    let db = super::db_with_cloudsync_items_table(
        "CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
    )
    .await;
    let mut connection =
        anlg_cloudsync::ReservedConnection::new(db.pool().acquire().await.unwrap());
    sqlx::query(
        "INSERT INTO items VALUES ('confirmed', 'uploaded before acknowledgement was lost')",
    )
    .execute(connection.connection().await.unwrap())
    .await
    .unwrap();
    let original = anlg_cloudsync::pending_payload_batch(
        connection.connection().await.unwrap(),
        8,
        4096,
        32 * 1024 * 1024,
    )
    .await
    .unwrap();
    let confirmed = original.watermark_db_version.unwrap();
    sqlx::query("INSERT INTO items VALUES ('tail', 'new edit before retry')")
        .execute(connection.connection().await.unwrap())
        .await
        .unwrap();
    let pending = anlg_cloudsync::pending_payload_batch(
        connection.connection().await.unwrap(),
        8,
        4096,
        32 * 1024 * 1024,
    )
    .await
    .unwrap();
    let newest = pending.watermark_db_version.unwrap();
    assert!(newest > confirmed);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    sqlx::query("SELECT cloudsync_network_init_custom(?, 'prefix-retry-test')")
        .bind(endpoint)
        .execute(connection.connection().await.unwrap())
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        for step in 0..3 {
            let (stream, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            assert!(line.starts_with(if step == 1 { "GET " } else { "POST " }));
            let mut length = 0;
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!(length < 128 * 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).await.unwrap();
            if step != 1 {
                let payload = String::from_utf8(body).unwrap().replace(' ', "");
                let start = if step == 0 {
                    original.start_db_version + 1
                } else {
                    confirmed + 1
                };
                assert!(
                    payload.contains(&format!("\"dbVersionMin\":{start},")),
                    "{payload}"
                );
                assert!(
                    payload.contains(&format!("\"dbVersionMax\":{newest},")),
                    "{payload}"
                );
            }
            let (status, body) = if step == 0 {
                ("409 Conflict", r#"{"errors":[{"status":"409","code":"already_exists","detail":"resource already exists"}]}"#.to_string())
            } else {
                let version = if step == 1 { confirmed } else { newest };
                (
                    "200 OK",
                    format!(
                        "{{\"lastOptimisticVersion\":{version},\"lastConfirmedVersion\":{version},\"gaps\":[]}}"
                    ),
                )
            };
            reader.get_mut().write_all(format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len(),
            ).as_bytes()).await.unwrap();
        }
    });
    let recovered = guarded_interruptible_network_send_changes(
        &mut connection,
        &db.cloudsync_interrupt,
        || false,
    )
    .await
    .unwrap();
    record_sync_result(
        &db.cloudsync_runtime,
        recovered,
        true,
        CloudsyncActivityTrigger::Manual,
    );
    let (last_error, failures, settled_at, activity) = {
        let state = db.cloudsync_runtime.lock().unwrap();
        (
            state.last_error.clone(),
            state.consecutive_failures,
            state.last_sync_at_ms,
            state.last_logged_activity,
        )
    };
    assert!(last_error.is_none(), "{last_error:?}");
    assert_eq!(failures, 0);
    assert!(settled_at.is_none());
    assert_eq!(
        activity,
        Some(crate::cloudsync::types::CloudsyncActivityStatus::Progress)
    );
    let tail = guarded_interruptible_network_send_changes(
        &mut connection,
        &db.cloudsync_interrupt,
        || false,
    )
    .await
    .unwrap();
    assert_eq!(tail.send.unwrap().status, "synced");
    assert!(
        !crate::cloudsync::ops::cloudsync_has_local_unsent_changes_on(
            connection.connection().await.unwrap(),
        )
        .await
        .unwrap()
    );
    server.await.unwrap();
}
