use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use serde_json::json;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{method, path},
};

use super::*;

#[derive(Clone, Default)]
struct RateLimitedOnce {
    requests: Arc<AtomicUsize>,
}

impl Respond for RateLimitedOnce {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.requests.fetch_add(1, Ordering::Relaxed) == 0 {
            return ResponseTemplate::new(429).insert_header("retry-after", "0");
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-07-17T00:00:00Z",
            "headSequence": 0,
            "throughSequence": 0,
            "nextAfterSequence": 0,
            "events": [],
        }))
    }
}

#[derive(Clone, Default)]
struct RequestOrder {
    methods: Arc<Mutex<Vec<String>>>,
}

impl Respond for RequestOrder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        self.methods
            .lock()
            .unwrap()
            .push(request.method.as_str().to_string());
        if request.method.as_str() == "GET" {
            return witness_page(&[], 0, 0);
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "initializedAt": "2026-07-17T00:00:00Z",
            "headSequence": 0,
        }))
    }
}

#[derive(Clone, Default)]
struct FailFirstPublish {
    publishes: Arc<AtomicUsize>,
}

impl Respond for FailFirstPublish {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.method.as_str() == "GET" {
            return witness_page(&[], 0, 0);
        }
        if self.publishes.fetch_add(1, Ordering::Relaxed) == 0 {
            return ResponseTemplate::new(500);
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "initializedAt": "2026-07-17T00:00:00Z",
            "headSequence": 0,
        }))
    }
}

#[derive(Clone)]
struct InterruptedPage {
    events: Vec<serde_json::Value>,
    requests: Arc<AtomicUsize>,
    after_sequences: Arc<Mutex<Vec<u64>>>,
}

impl Respond for InterruptedPage {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let after = request
            .url
            .query_pairs()
            .find_map(|(key, value)| (key == "afterSequence").then(|| value.parse().unwrap()))
            .unwrap_or(0);
        self.after_sequences.lock().unwrap().push(after);
        match self.requests.fetch_add(1, Ordering::Relaxed) {
            0 => witness_page(&self.events[..3], 4, 4),
            1 => ResponseTemplate::new(500),
            _ => witness_page(&self.events[3..], 4, 4),
        }
    }
}

fn witness_page(
    events: &[serde_json::Value],
    head_sequence: u64,
    through_sequence: u64,
) -> ResponseTemplate {
    let next_after_sequence = events
        .last()
        .and_then(|event| event["sequence"].as_u64())
        .unwrap_or(through_sequence);
    ResponseTemplate::new(200).set_body_json(json!({
        "initialized": true,
        "initializedAt": "2026-07-17T00:00:00Z",
        "headSequence": head_sequence,
        "throughSequence": through_sequence,
        "nextAfterSequence": next_after_sequence,
        "events": events,
    }))
}

#[test]
fn derives_a_sibling_witness_endpoint_for_a_shared_workspace() {
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: "https://api.example.com/sync/e2ee/witness/personal".to_string(),
            access_token: "access-token".to_string(),
        },
        "personal",
    )
    .unwrap();

    let shared = client.for_workspace("shared-workspace").unwrap();

    assert_eq!(
        shared.endpoint.as_str(),
        "https://api.example.com/sync/e2ee/witness/shared-workspace"
    );
    assert_eq!(shared.workspace_id(), "shared-workspace");
}

#[test]
fn cancelled_replica_work_is_reported_as_an_interrupted_witness_operation() {
    assert_eq!(
        replica_error(anlg_db_app::E2eeReplicaError::Cancelled).kind(),
        io::ErrorKind::Interrupted
    );
}

#[tokio::test]
async fn retries_a_rate_limited_witness_read() {
    let server = MockServer::start().await;
    let responder = RateLimitedOnce::default();
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(responder.clone())
        .expect(2)
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    let page = client.read_page(0, None).await.unwrap();

    assert_eq!(page.head_sequence, 0);
    assert_eq!(responder.requests.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn cancellation_stops_a_stalled_witness_request_promptly() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(witness_page(&[], 0, 0).set_delay(std::time::Duration::from_secs(120)))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    let cancellation = E2eeWitnessCancellation::default();
    let request_client = client.clone();
    let request_cancellation = cancellation.clone();
    let request = tokio::spawn(async move {
        request_client
            .read_page_cancellable(0, None, &request_cancellation)
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if !server.received_requests().await.unwrap().is_empty() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("witness request did not reach the stalled endpoint");

    cancellation.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_millis(500), request)
        .await
        .expect("witness cancellation waited for the HTTP timeout")
        .unwrap();
    let Err(error) = result else {
        panic!("cancelled witness request unexpectedly succeeded");
    };

    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

#[tokio::test]
async fn cancelled_witness_merge_does_not_advance_the_authenticated_cursor() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let sealed = key
        .seal_field(
            "user-a",
            "sessions",
            "session-1",
            "title",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            false,
            json!("Remote"),
        )
        .unwrap();
    let event = json!({
        "sequence": 1,
        "recordId": sealed.record_id,
        "payloadHash": anlg_e2ee::payload_hash(&sealed.payload),
        "payload": sealed.payload,
    });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(witness_page(&[event], 1, 1))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    let cancellation = E2eeWitnessCancellation::default();
    let cancel_on_events = cancellation.clone();

    let error = client
        .refresh_notifying_cancellable(
            db.pool(),
            &key,
            move || cancel_on_events.cancel(),
            &cancellation,
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_records")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn cancellation_stops_a_rate_limit_retry_sleep() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    let cancellation = E2eeWitnessCancellation::default();
    let request_client = client.clone();
    let request_cancellation = cancellation.clone();
    let request = tokio::spawn(async move {
        request_client
            .read_page_cancellable(0, None, &request_cancellation)
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if server.received_requests().await.unwrap().len() == 1 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("witness request did not enter rate-limit backoff");

    cancellation.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_millis(500), request)
        .await
        .expect("witness cancellation waited for retry-after")
        .unwrap();
    let Err(error) = result else {
        panic!("cancelled witness retry unexpectedly succeeded");
    };

    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn empty_refresh_does_not_write_an_unchanged_cursor() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(witness_page(&[], 0, 0))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    assert_eq!(client.refresh(db.pool(), &key).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_witness_state")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn initialized_witness_refreshes_before_publishing_pending_state() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', 'user-a', 'user-a', 'Session')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(
        db.pool(),
        &HashMap::from([("user-a".to_string(), key.clone().into())]),
    )
    .await
    .unwrap();

    let server = MockServer::start().await;
    let responder = RequestOrder::default();
    Mock::given(path("/sync/e2ee/witness/user-a"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    client.initialize(db.pool(), &key).await.unwrap();

    let methods = responder.methods.lock().unwrap().clone();
    assert_eq!(&methods[..2], ["GET", "GET"]);
    assert_eq!(methods.last().map(String::as_str), Some("GET"));
    assert!(
        methods[2..methods.len() - 1]
            .iter()
            .all(|method| method == "POST")
    );
}

#[tokio::test]
async fn pending_local_state_is_retryable_after_a_failed_publish() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', 'user-a', 'user-a', 'Before')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
    anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    loop {
        let uploads = anlg_db_app::pending_e2ee_witness_uploads(
            db.pool(),
            "user-a",
            &key,
            MAX_EVENTS_PER_BATCH,
            MAX_BATCH_BYTES,
        )
        .await
        .unwrap();
        if uploads.is_empty() {
            break;
        }
        anlg_db_app::acknowledge_e2ee_witness_uploads(db.pool(), &key, &uploads)
            .await
            .unwrap();
    }

    sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session'")
        .execute(db.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();

    let server = MockServer::start().await;
    let responder = FailFirstPublish::default();
    Mock::given(path("/sync/e2ee/witness/user-a"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    assert!(client.publish_and_refresh(db.pool(), &key).await.is_err());
    let queued_after_failure: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(queued_after_failure > 0);
    assert!(
        !anlg_db_app::pending_e2ee_witness_uploads(
            db.pool(),
            "user-a",
            &key,
            MAX_EVENTS_PER_BATCH,
            MAX_BATCH_BYTES,
        )
        .await
        .unwrap()
        .is_empty()
    );

    client.publish_and_refresh(db.pool(), &key).await.unwrap();

    assert_eq!(responder.publishes.load(Ordering::Relaxed), 2);
    let queued_after_retry: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(queued_after_retry, 0);
    assert!(
        anlg_db_app::pending_e2ee_witness_uploads(
            db.pool(),
            "user-a",
            &key,
            MAX_EVENTS_PER_BATCH,
            MAX_BATCH_BYTES,
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn stops_retrying_a_persistently_rate_limited_read() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .expect(4)
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    let error = client
        .read_page(0, None)
        .await
        .err()
        .expect("persistent throttling should fail");

    assert!(error.to_string().contains("429 Too Many Requests"));
}

#[tokio::test]
async fn resumes_refresh_from_the_last_authenticated_page() {
    let dir = tempfile::tempdir().unwrap();
    let db = anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(&dir.path().join("app.db")),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session', 'user-a', 'user-a', 'Session')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(
        db.pool(),
        &HashMap::from([("user-a".to_string(), key.clone().into())]),
    )
    .await
    .unwrap();
    let uploads = anlg_db_app::pending_e2ee_witness_uploads(
        db.pool(),
        "user-a",
        &key,
        MAX_EVENTS_PER_BATCH,
        MAX_BATCH_BYTES,
    )
    .await
    .unwrap();
    assert!(uploads.len() >= 4);
    let events = uploads
        .iter()
        .take(4)
        .enumerate()
        .map(|(index, upload)| {
            json!({
                "sequence": index + 1,
                "recordId": upload.record_id,
                "payloadHash": upload.payload_hash,
                "payload": upload.payload,
            })
        })
        .collect::<Vec<_>>();
    let server = MockServer::start().await;
    let responder = InterruptedPage {
        events,
        requests: Arc::new(AtomicUsize::new(0)),
        after_sequences: Arc::new(Mutex::new(Vec::new())),
    };
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(responder.clone())
        .expect(3)
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    let reconciliation_requested = Arc::new(AtomicBool::new(false));
    let reconciliation_requested_for_refresh = Arc::clone(&reconciliation_requested);
    assert!(
        client
            .refresh_notifying(db.pool(), &key, move || {
                reconciliation_requested_for_refresh.store(true, Ordering::SeqCst);
            })
            .await
            .is_err()
    );
    assert!(reconciliation_requested.load(Ordering::SeqCst));
    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        3
    );

    assert_eq!(client.refresh(db.pool(), &key).await.unwrap(), 1);

    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        4
    );
    assert_eq!(*responder.after_sequences.lock().unwrap(), vec![0, 3, 3]);
}

#[test]
fn retry_after_delays_are_bounded_and_allow_immediate_test_retries() {
    let mut headers = reqwest::header::HeaderMap::new();
    assert_eq!(retry_after_delay(&headers), DEFAULT_RETRY_AFTER);

    headers.insert(reqwest::header::RETRY_AFTER, "0".parse().unwrap());
    assert!(retry_after_delay(&headers).is_zero());

    headers.insert(reqwest::header::RETRY_AFTER, "later".parse().unwrap());
    assert_eq!(retry_after_delay(&headers), DEFAULT_RETRY_AFTER);

    headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
    assert_eq!(retry_after_delay(&headers), MAX_RETRY_AFTER);
}

#[tokio::test]
async fn wait_for_remote_head_reports_only_advanced_heads() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/wait"))
        .and(wiremock::matchers::query_param("afterSequence", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "headSequence": 5,
        })))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    let head = client
        .wait_for_remote_head(3, &E2eeWitnessCancellation::default())
        .await
        .unwrap();

    assert_eq!(head, Some(5));
}

#[tokio::test]
async fn wait_for_remote_head_ignores_stale_and_uninitialized_heads() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/wait"))
        .and(wiremock::matchers::query_param("afterSequence", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "headSequence": 5,
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/wait"))
        .and(wiremock::matchers::query_param("afterSequence", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": false,
            "headSequence": 0,
        })))
        .mount(&server)
        .await;
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();

    let stale = client
        .wait_for_remote_head(5, &E2eeWitnessCancellation::default())
        .await
        .unwrap();
    let uninitialized = client
        .wait_for_remote_head(0, &E2eeWitnessCancellation::default())
        .await
        .unwrap();

    assert_eq!(stale, None);
    assert_eq!(uninitialized, None);
}
