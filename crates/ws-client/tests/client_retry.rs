mod common;

use common::{
    TestIO, collect_messages, dropping_server, flaky_echo_server, http_error_server,
    message_stream, start_test_client, test_client,
};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use ws_client::client::{WebSocketConnectPolicy, WebSocketRetryEvent};

#[tokio::test]
async fn test_retry() {
    let (addr, attempt_count) = flaky_echo_server(1).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = test_client(addr);
    let (output, _handle) = start_test_client(client, message_stream("retry_test"))
        .await
        .unwrap();

    let received = collect_messages::<TestIO>(output, 1).await;
    assert_eq!(received[0].text, "retry_test");
    assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_retry_exhausted_returns_explicit_error() {
    let (addr, _attempts) = dropping_server().await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(1),
        max_attempts: 2,
        retry_delay: Duration::from_millis(10),
    });

    let error = match start_test_client(client, message_stream("nope")).await {
        Ok(_) => panic!("expected connect retries to be exhausted"),
        Err(error) => error,
    };

    match error {
        ws_client::Error::ConnectRetriesExhausted {
            attempts,
            last_error,
            ..
        } => {
            assert_eq!(attempts, 2);
            assert!(!last_error.is_empty());
        }
        other => panic!("expected explicit retries exhausted error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_non_retryable_http_handshake_error_fails_fast() {
    let (addr, attempts) = http_error_server("400 Bad Request", "bad request", "").await;
    let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(1),
        max_attempts: 3,
        retry_delay: Duration::from_millis(10),
    });

    let error = start_test_client(client, message_stream("bad-http")).await;
    let error = match error {
        Ok(_) => panic!("http 400 should fail fast"),
        Err(error) => error,
    };

    match error {
        ws_client::Error::ConnectFailed {
            attempt,
            max_attempts,
            ..
        } => {
            assert_eq!(attempt, 1);
            assert_eq!(max_attempts, 3);
        }
        other => panic!("expected connect failure, got {other:?}"),
    }

    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_single_attempt_preserves_retry_after() {
    let (addr, attempts) =
        http_error_server("503 Service Unavailable", "draining", "Retry-After: 7\r\n").await;
    let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(1),
        max_attempts: 1,
        retry_delay: Duration::from_millis(10),
    });

    let error = match start_test_client(client, message_stream("retry-after")).await {
        Ok(_) => panic!("expected connection failure"),
        Err(error) => error,
    };

    assert_eq!(error.retry_after_secs(), Some(7));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_retryable_http_statuses_use_the_connection_budget() {
    for status in ["429 Too Many Requests", "503 Service Unavailable"] {
        let (addr, attempts) = http_error_server(status, "temporarily unavailable", "").await;
        let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
            connect_timeout: Duration::from_secs(1),
            max_attempts: 2,
            retry_delay: Duration::from_millis(1),
        });

        let error = start_test_client(client, message_stream("retryable-http")).await;
        assert!(matches!(
            error,
            Err(ws_client::Error::ConnectRetriesExhausted { attempts: 2, .. })
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn test_retry_after_is_capped() {
    let (addr, attempts) = http_error_server(
        "503 Service Unavailable",
        "draining",
        "Retry-After: 3600\r\n",
    )
    .await;
    let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(1),
        max_attempts: 1,
        retry_delay: Duration::from_millis(10),
    });

    let error = match start_test_client(client, message_stream("capped-retry-after")).await {
        Ok(_) => panic!("expected connection failure"),
        Err(error) => error,
    };

    assert_eq!(error.retry_after_secs(), Some(30));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_zero_retry_after_uses_a_nonzero_minimum() {
    let (addr, attempts) = http_error_server(
        "429 Too Many Requests",
        "rate limited",
        "Retry-After: 0\r\n",
    )
    .await;
    let client = test_client(addr).with_connect_policy(WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(1),
        max_attempts: 1,
        retry_delay: Duration::from_millis(10),
    });

    let error = match start_test_client(client, message_stream("minimum-retry-after")).await {
        Ok(_) => panic!("expected connection failure"),
        Err(error) => error,
    };

    assert_eq!(error.retry_after_secs(), Some(1));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_on_retry_callback_reports_upcoming_attempts() {
    let (addr, attempts) = dropping_server().await;
    let retry_events = Arc::new(Mutex::new(Vec::<WebSocketRetryEvent>::new()));
    let retry_events_for_callback = retry_events.clone();

    let client = test_client(addr)
        .with_connect_policy(WebSocketConnectPolicy {
            connect_timeout: Duration::from_secs(1),
            max_attempts: 3,
            retry_delay: Duration::from_millis(10),
        })
        .on_retry(Arc::new(move |event| {
            retry_events_for_callback.lock().unwrap().push(event);
        }));

    let error = start_test_client(client, message_stream("retry-callback")).await;
    let error = match error {
        Ok(_) => panic!("expected connection failure"),
        Err(error) => error,
    };

    match error {
        ws_client::Error::ConnectRetriesExhausted { attempts, .. } => {
            assert_eq!(attempts, 3);
        }
        other => panic!("expected retries exhausted error, got {other:?}"),
    }

    assert_eq!(attempts.load(Ordering::SeqCst), 3);

    let retry_events = retry_events.lock().unwrap();
    assert_eq!(retry_events.len(), 2);
    assert_eq!(retry_events[0].attempt, 2);
    assert_eq!(retry_events[1].attempt, 3);
    assert!(retry_events.iter().all(|event| event.max_attempts == 3));
    assert!(retry_events.iter().all(|event| !event.error.is_empty()));
}
