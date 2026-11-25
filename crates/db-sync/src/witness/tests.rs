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
    status: u16,
    requests: Arc<AtomicUsize>,
}

impl Respond for RateLimitedOnce {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.requests.fetch_add(1, Ordering::Relaxed) == 0 {
            return ResponseTemplate::new(if self.status == 0 { 429 } else { self.status })
                .insert_header("retry-after", "0");
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
    accepted: bool,
    legacy_first: bool,
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
        if self.legacy_first {
            if request.url.path().ends_with("/accepted") && after == 0 {
                return ResponseTemplate::new(404);
            }
            if !request.url.path().ends_with("/accepted") && after > 0 {
                return ResponseTemplate::new(426);
            }
        }
        self.after_sequences.lock().unwrap().push(after);
        let events = match (
            self.legacy_first,
            self.requests.fetch_add(1, Ordering::Relaxed),
        ) {
            (_, 0) => &self.events[..3],
            (true, 1) => &self.events[3..6],
            (false, 1) | (true, 2) => return ResponseTemplate::new(500),
            (false, _) => &self.events[3..],
            (true, _) => &self.events[6..],
        };
        ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-07-17T00:00:00Z",
            "headSequence": self.events.len(),
            "throughSequence": self.events.len(),
            "nextAfterSequence": events.last().unwrap()["sequence"],
            "cloudAuthorityAfter": self.accepted.then_some(0),
            "events": events,
        }))
    }
}

#[derive(Clone)]
struct PagedWitness {
    events: Vec<serde_json::Value>,
    page_size: usize,
}

impl Respond for PagedWitness {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let after = request
            .url
            .query_pairs()
            .find_map(|(key, value)| (key == "afterSequence").then(|| value.parse().unwrap()))
            .unwrap_or(0);
        let start = usize::try_from(after).unwrap();
        let end = start.saturating_add(self.page_size).min(self.events.len());
        witness_page(
            &self.events[start..end],
            self.events.len() as u64,
            self.events.len() as u64,
        )
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
async fn retries_rate_limited_and_busy_witness_reads() {
    for status in [429, 503] {
        let server = MockServer::start().await;
        let responder = RateLimitedOnce {
            status,
            ..Default::default()
        };
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
}

#[tokio::test]
async fn witness_timeout_diagnostics_do_not_expose_the_workspace_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(1)))
        .mount(&server)
        .await;
    let mut client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/private-workspace", server.uri()),
            access_token: "private-token".to_string(),
        },
        "private-workspace",
    )
    .unwrap();
    client.client = http_client(std::time::Duration::from_millis(50)).unwrap();
    let error = client.read_page(0, None).await.err().unwrap().to_string();
    assert!(error.contains("timed out"));
    assert!(!error.contains("private-workspace"));
    assert!(!error.contains("private-token"));
    assert!(!error.contains(&server.uri()));
}

#[tokio::test]
async fn headerless_service_outages_fail_without_a_rate_limit_retry_delay() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
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
    let error = tokio::time::timeout(std::time::Duration::from_secs(1), client.read_page(0, None))
        .await
        .expect("ordinary outages must not enter the busy retry sleep")
        .err()
        .expect("service outage must fail");
    assert!(error.to_string().contains("503"));
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
async fn merged_witness_pages_can_materialize_rows_before_refresh_completes() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let writer_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = key
        .seal_field(
            "user-a",
            "sessions",
            "session-1",
            "$row",
            writer_id,
            1,
            false,
            json!(true),
        )
        .unwrap();
    let title = key
        .seal_field(
            "user-a",
            "sessions",
            "session-1",
            "title",
            writer_id,
            1,
            false,
            json!("Remote"),
        )
        .unwrap();
    let events = [manifest, title]
        .into_iter()
        .enumerate()
        .map(|(index, sealed)| {
            json!({
                "sequence": index + 1,
                "recordId": sealed.record_id,
                "payloadHash": anlg_e2ee::payload_hash(&sealed.payload),
                "payload": sealed.payload,
            })
        })
        .collect::<Vec<_>>();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(PagedWitness {
            events,
            page_size: 1,
        })
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
    let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
    let observed_titles = Arc::new(Mutex::new(Vec::new()));
    let observed_titles_for_handler = Arc::clone(&observed_titles);

    client
        .refresh_keyring_with_page_handler_cancellable(
            db.pool(),
            &keys["user-a"],
            || {
                let observed_titles = Arc::clone(&observed_titles_for_handler);
                let pool = db.pool().clone();
                let keys = keys.clone();
                async move {
                    anlg_db_app::apply_received_e2ee_replica_changes_with_witness(
                        &pool, &keys, true,
                    )
                    .await
                    .map_err(replica_error)?;
                    let title = sqlx::query_scalar::<_, String>(
                        "SELECT title FROM sessions WHERE id = 'session-1'",
                    )
                    .fetch_one(&pool)
                    .await
                    .map_err(|error| io::Error::other(error.to_string()))?;
                    observed_titles.lock().unwrap().push(title);
                    Ok(())
                }
            },
            &E2eeWitnessCancellation::default(),
        )
        .await
        .unwrap();

    assert_eq!(*observed_titles.lock().unwrap(), ["", "Remote"]);
}

#[tokio::test]
async fn replica_transport_reads_the_next_page_for_incomplete_transcripts() {
    check_replica_transport_hydrates_transcripts(false).await;
}

#[tokio::test]
async fn replica_transport_drains_ready_rows_and_preserves_incomplete_transcripts() {
    check_replica_transport_hydrates_transcripts(true).await;
}

async fn check_replica_transport_hydrates_transcripts(many_rows: bool) {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let db = Arc::new(db);
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let words = json!([{ "text": "restored", "start_ms": 0, "end_ms": 500 }]);
    let mut events: Vec<_> = [
        ("$row", json!(true)),
        ("words_json#n", json!(1)),
        ("words_json#0", words.clone()),
    ]
    .into_iter()
    .take(if many_rows { 2 } else { 3 })
    .enumerate()
    .map(|(index, (field, value))| {
        let sealed = key
            .seal_field(
                "user-a",
                "transcripts",
                "transcript-1",
                field,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                value,
            )
            .unwrap();
        json!({
            "sequence": index + 1,
            "recordId": sealed.record_id,
            "payloadHash": anlg_e2ee::payload_hash(&sealed.payload),
            "payload": sealed.payload,
        })
    })
    .collect();
    if many_rows {
        for index in 0..40 {
            let sealed = key
                .seal_field(
                    "user-a",
                    "transcripts",
                    &format!("transcript-a-{index:02}"),
                    "$row",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    json!(true),
                )
                .unwrap();
            events.push(json!({
                "sequence": events.len() + 1,
                "recordId": sealed.record_id,
                "payloadHash": anlg_e2ee::payload_hash(&sealed.payload),
                "payload": sealed.payload,
            }));
        }
    }
    let mut head_sequence = events.len() as u64;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(PagedWitness {
            events: events.clone(),
            page_size: if many_rows { 100 } else { 2 },
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initializedAt": "2026-07-17T00:00:00Z", "headSequence": head_sequence,
        })))
        .mount(&server)
        .await;
    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );

    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        hook.sync_replica_transport(db.pool()),
    )
    .await
    .expect("replica sync blocked the next witness page on an incomplete transcript")
    .unwrap();
    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        head_sequence
    );
    if many_rows {
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transcripts")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            41
        );
        sqlx::query("UPDATE transcripts SET created_at = 'local edit' WHERE id = 'transcript-1'")
            .execute(db.pool())
            .await
            .unwrap();
        for index in 0..16 {
            let row_id = format!("session-conflict-{index:02}");
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, title) VALUES (?, 'user-a', 'local')",
            )
            .bind(&row_id)
            .execute(db.pool())
            .await
            .unwrap();
            let deleted = key
                .seal_field(
                    "user-a",
                    "sessions",
                    &row_id,
                    "$row",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    true,
                    json!(null),
                )
                .unwrap();
            events.push(json!({
                "sequence": events.len() + 1,
                "recordId": deleted.record_id,
                "payloadHash": anlg_e2ee::payload_hash(&deleted.payload),
                "payload": deleted.payload,
            }));
        }
        head_sequence = events.len() as u64;
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(PagedWitness {
                events: events.clone(),
                page_size: 100,
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "initializedAt": "2026-07-17T00:00:00Z", "headSequence": head_sequence,
            })))
            .mount(&server)
            .await;
        let mut outcome = hook.sync_replica_transport(db.pool()).await.unwrap();
        let created_at_record_id = key.blind_field_id("transcripts", "transcript-1", "created_at");
        let count_record_id = key.blind_field_id("transcripts", "transcript-1", "words_json#n");
        let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&count_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        let count = key
            .open_field("user-a", &count_record_id, &payload)
            .unwrap();
        assert_eq!(count.value, json!(1));
        assert_eq!(count.revision, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_records WHERE id = ?")
                .bind(&created_at_record_id)
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        for _ in 0..4 {
            if outcome != crate::ReplicaSyncOutcome::MoreWork {
                break;
            }
            outcome = hook.sync_replica_transport(db.pool()).await.unwrap();
        }
        assert_eq!(outcome, crate::ReplicaSyncOutcome::WaitingForRemote);
        let task = crate::spawn_replica_sync(Arc::clone(&db), Arc::clone(&hook));
        hook.request_replica_sync();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !hook.replica_status().pending_changes {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("missing chunks were reported as successfully synced");
        assert!(hook.replica_status().last_sync_at_ms.is_none());
        let requests = server.received_requests().await.unwrap().len();
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        assert_eq!(server.received_requests().await.unwrap().len(), requests);
        let chunk = key
            .seal_field(
                "user-a",
                "transcripts",
                "transcript-1",
                "words_json#0",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                1,
                false,
                words.clone(),
            )
            .unwrap();
        events.push(json!({
            "sequence": head_sequence + 1,
            "recordId": chunk.record_id,
            "payloadHash": anlg_e2ee::payload_hash(&chunk.payload),
            "payload": chunk.payload,
        }));
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(PagedWitness {
                events,
                page_size: 100,
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "initializedAt": "2026-07-17T00:00:00Z", "headSequence": head_sequence + 1,
            })))
            .mount(&server)
            .await;
        hook.request_replica_sync();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while hook.replica_status().pending_changes {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("replica sync did not resume after the missing chunk arrived");
        assert!(hook.replica_status().last_sync_at_ms.is_some());
        drop(task);
        let created_at_record_id = key.blind_field_id("transcripts", "transcript-1", "created_at");
        let payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&created_at_record_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            key.open_field("user-a", &created_at_record_id, &payload)
                .unwrap()
                .value,
            json!("local edit")
        );
    }
    let actual: String =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
        words
    );
}

#[tokio::test]
async fn initialization_publishes_local_edits_before_hydrating_conflicting_history() {
    for publish_status in [200, 500] {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'user-a', 'user-a', 'Local edit')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let recovery_key = anlg_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        let key = recovery_key.workspace_key("user-a").unwrap();
        let hook = crate::E2eeSyncHook::default();
        hook.set_personal_workspace("user-a", &recovery_key)
            .unwrap();
        hook.prepare_local_snapshot(db.pool(), &E2eeWitnessCancellation::default())
            .await
            .unwrap();
        let uploads = anlg_db_app::pending_e2ee_witness_uploads(
            db.pool(),
            "user-a",
            &key,
            100,
            MAX_BATCH_BYTES,
        )
        .await
        .unwrap();
        let title_id = key.blind_field_id("sessions", "session-1", "title");
        let local_title = uploads
            .iter()
            .find(|upload| upload.record_id == title_id)
            .unwrap();
        let remote_title = key
            .seal_field(
                "user-a",
                "sessions",
                "session-1",
                "title",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                100,
                false,
                json!("Remote edit"),
            )
            .unwrap();
        let events = vec![json!({
            "sequence": 1,
            "recordId": remote_title.record_id,
            "payloadHash": anlg_e2ee::payload_hash(&remote_title.payload),
            "payload": remote_title.payload,
        })];
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(PagedWitness {
                events,
                page_size: 1,
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(ResponseTemplate::new(publish_status).set_body_json(json!({
                "initializedAt": "2026-07-17T00:00:00Z", "headSequence": 1,
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
        let keyring = key.clone().into();
        let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
        let cancellation = E2eeWitnessCancellation::default();
        let result = client
            .initialize_keyring_with_page_handler_cancellable(
                db.pool(),
                &keyring,
                || async {
                    anlg_db_app::apply_received_e2ee_replica_changes_with_witness(
                        db.pool(),
                        &keys,
                        true,
                    )
                    .await
                    .map(|_| ())
                    .map_err(replica_error)
                },
                &cancellation,
            )
            .await;
        if publish_status == 200 {
            result.unwrap();
        } else {
            assert!(result.is_err());
            let title: String =
                sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(title, "Local edit");
            let pending = anlg_db_app::pending_e2ee_witness_uploads(
                db.pool(),
                "user-a",
                &key,
                100,
                MAX_BATCH_BYTES,
            )
            .await
            .unwrap();
            assert!(
                pending
                    .iter()
                    .any(|upload| upload.payload_hash == local_title.payload_hash)
            );
        }

        let requests = server.received_requests().await.unwrap();
        assert!(
            requests
                .iter()
                .filter(|request| request.method.as_str() == "POST")
                .any(|request| {
                    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                    body["events"].as_array().unwrap().iter().any(|event| {
                        event["recordId"] == local_title.record_id
                            && event["payloadHash"] == local_title.payload_hash
                    })
                }),
            "the unpublished local title must reach the witness before remote hydration replaces it"
        );
    }
}

#[tokio::test]
async fn initialized_witness_checks_status_then_publishes_before_refreshing() {
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
    assert_eq!(methods.first().map(String::as_str), Some("GET"));
    assert_eq!(methods.last().map(String::as_str), Some("GET"));
    assert!(
        methods[1..methods.len() - 1]
            .iter()
            .all(|method| method == "POST")
    );
}

#[tokio::test]
async fn replica_startup_materializes_notes_when_a_later_history_page_fails() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let key = recovery_key.workspace_key("user-a").unwrap();
    let events = [("$row", json!(true)), ("title", json!("Remote"))]
        .into_iter()
        .enumerate()
        .map(|(index, (field, value))| {
            let sealed = key
                .seal_field(
                    "user-a",
                    "sessions",
                    "session-1",
                    field,
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    1,
                    false,
                    value,
                )
                .unwrap();
            json!({
                "sequence": index + 1,
                "recordId": sealed.record_id,
                "payloadHash": anlg_e2ee::payload_hash(&sealed.payload),
                "payload": sealed.payload,
            })
        })
        .collect::<Vec<_>>();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .and(wiremock::matchers::query_param("afterSequence", "0"))
        .respond_with(witness_page(&events, 3, 3))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a"))
        .and(wiremock::matchers::query_param("afterSequence", "2"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let hook = crate::E2eeSyncHook::default();
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );

    assert!(hook.sync_replica_transport(db.pool()).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "Remote"
    );
    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        2
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
        accepted: false,
        legacy_first: false,
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

#[tokio::test]
async fn accepted_refresh_resumes_after_restart_without_exposing_partial_rows() {
    for legacy_first in [false, true] {
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
        let mut uploads = anlg_db_app::pending_e2ee_witness_uploads(
            db.pool(),
            "user-a",
            &key,
            128,
            MAX_BATCH_BYTES,
        )
        .await
        .unwrap();
        // Put the row identity and title on the page after a possible legacy cutover.
        uploads.sort_by_key(|upload| {
            let field = key
                .open_field("user-a", &upload.record_id, &upload.payload)
                .unwrap();
            match field.field.as_str() {
                "created_at" => 0,
                "updated_at" => 1,
                "kind" => 2,
                "$row" => 3,
                "title" => 4,
                _ => 5,
            }
        });
        assert!(uploads.len() > 6);
        let events = uploads
            .iter()
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
        let head = events.len() as u64;
        let responder = InterruptedPage {
            events,
            accepted: true,
            legacy_first,
            requests: Arc::new(AtomicUsize::new(0)),
            after_sequences: Arc::new(Mutex::new(Vec::new())),
        };
        Mock::given(method("GET"))
            .respond_with(responder.clone())
            .expect(if legacy_first { 6 } else { 3 })
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

        let receiver_path = dir.path().join("receiver.db");
        let open_receiver = || {
            anlg_db_core::Db::open(anlg_db_core::DbOpenOptions {
                storage: anlg_db_core::DbStorage::Local(&receiver_path),
                cloudsync_enabled: false,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(1),
            })
        };
        let db = open_receiver().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        assert!(client.refresh(db.pool(), &key).await.is_err());
        let cursor = if legacy_first { 6 } else { 3 };
        assert_eq!(
            anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
                .await
                .unwrap(),
            cursor
        );

        db.pool().close().await;
        let db = open_receiver().await.unwrap();
        let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
        anlg_db_app::apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions WHERE id = 'session'")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        let restarted = E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".into(),
            },
            "user-a",
        )
        .unwrap();
        assert_eq!(
            restarted.refresh(db.pool(), &key).await.unwrap(),
            (head - cursor) as usize
        );
        anlg_db_app::apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'session'")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            "Session"
        );

        assert_eq!(
            anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
                .await
                .unwrap(),
            head
        );
        assert_eq!(
            *responder.after_sequences.lock().unwrap(),
            if legacy_first {
                vec![0, 3, 6, 6]
            } else {
                vec![0, 3, 3]
            }
        );
    }
}

#[test]
fn retry_after_delays_are_bounded_and_allow_immediate_test_retries() {
    let mut headers = reqwest::header::HeaderMap::new();
    assert_eq!(retry_after_delay(&headers), None);

    headers.insert(reqwest::header::RETRY_AFTER, "0".parse().unwrap());
    assert_eq!(retry_after_delay(&headers), Some(std::time::Duration::ZERO));

    headers.insert(reqwest::header::RETRY_AFTER, "later".parse().unwrap());
    assert_eq!(retry_after_delay(&headers), None);

    headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
    assert_eq!(retry_after_delay(&headers), Some(MAX_RETRY_AFTER));
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

#[derive(Default)]
struct AcceptedServer {
    events: Vec<serde_json::Value>,
    receipts: HashMap<String, (serde_json::Value, serde_json::Value)>,
    lose_response: bool,
}

#[derive(Clone)]
struct AcceptedResponder(Arc<Mutex<AcceptedServer>>);

impl Respond for AcceptedResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut server = self.0.lock().unwrap();
        if request.method.as_str() == "GET" {
            let after = request
                .url
                .query_pairs()
                .find_map(|(k, v)| (k == "afterSequence").then(|| v.parse::<u64>().unwrap()))
                .unwrap_or(0);
            let head = server.events.len() as u64;
            return ResponseTemplate::new(200).set_body_json(json!({
                "initialized": true, "initializedAt": "2026-10-05T00:00:00Z", "cloudAuthorityAfter": 0,
                "headSequence": head, "throughSequence": head, "nextAfterSequence": head,
                "events": server.events.iter().filter(|e| e["sequence"].as_u64().unwrap() > after).collect::<Vec<_>>(),
            }));
        }
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let id = body["mutationId"].as_str().unwrap().to_string();
        if let Some((original, receipt)) = server.receipts.get(&id) {
            assert_eq!(original, &body, "A retry must retain the immutable request");
            return ResponseTemplate::new(200).set_body_json(receipt);
        }
        if body["baseSequence"] != server.events.len() {
            return ResponseTemplate::new(409)
                .set_body_json(json!({"error":{"code":"e2ee_replica_base_changed"}}));
        }
        let mut receipts = Vec::new();
        for event in body["events"].as_array().unwrap() {
            let sequence = server.events.len() + 1;
            let mut event = event.clone();
            event["sequence"] = json!(sequence);
            receipts.push(json!({"sequence": sequence, "recordId": event["recordId"], "payloadHash": event["payloadHash"]}));
            server.events.push(event);
        }
        let receipt = json!({"initializedAt":"2026-10-05T00:00:00Z", "headSequence": server.events.len(),
            "cloudAuthorityAfter":0, "mutationId":id, "receipts": receipts});
        server.receipts.insert(id, (body, receipt.clone()));
        if server.lose_response {
            server.lose_response = false;
            ResponseTemplate::new(500)
        } else {
            ResponseTemplate::new(200).set_body_json(receipt)
        }
    }
}

#[tokio::test]
async fn accepted_sync_recovers_a_lost_receipt_without_losing_a_newer_offline_edit() {
    let server = MockServer::start().await;
    let state = Arc::new(Mutex::new(AcceptedServer {
        lose_response: true,
        ..Default::default()
    }));
    Mock::given(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(AcceptedResponder(state.clone()))
        .mount(&server)
        .await;
    let make_client = || {
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "token".into(),
            },
            "user-a",
        )
        .unwrap()
    };
    let key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap()
    .workspace_key("user-a")
    .unwrap();
    let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s','user-a','user-a','First')").execute(db.pool()).await.unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    assert!(make_client().initialize(db.pool(), &key).await.is_err());
    let frozen = anlg_db_app::pending_e2ee_cloud_batch(db.pool(), "user-a", false)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Newer offline edit' WHERE id = 's'")
        .execute(db.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
        .await
        .unwrap();
    assert_eq!(
        anlg_db_app::pending_e2ee_cloud_batch(db.pool(), "user-a", false)
            .await
            .unwrap()
            .unwrap()
            .mutation_id,
        frozen.mutation_id
    );
    // A newly constructed client must recover solely from durable SQLite state.
    make_client().initialize(db.pool(), &key).await.unwrap();
    assert!(
        anlg_db_app::pending_e2ee_cloud_batch(db.pool(), "user-a", false)
            .await
            .unwrap()
            .is_none()
    );
    let fresh = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&fresh).await.unwrap();
    make_client().initialize(fresh.pool(), &key).await.unwrap();
    anlg_db_app::apply_received_e2ee_replica_changes_with_witness(fresh.pool(), &keys, true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 's'")
            .fetch_one(fresh.pool())
            .await
            .unwrap(),
        "Newer offline edit"
    );
    // Receipts can be durable before the paginated read cursor advances.
    sqlx::query("UPDATE e2ee_witness_state SET last_sequence = 0 WHERE workspace_id = 'user-a'")
        .execute(db.pool())
        .await
        .unwrap();
    state.lock().unwrap().events.clear();
    assert!(make_client().refresh(db.pool(), &key).await.is_err());
}

#[tokio::test]
async fn cloud_acceptance_preserves_offline_notes_and_speaker_conflicts_in_either_reconnect_order()
{
    for order in [[0, 1], [1, 0]] {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(AcceptedServer::default()));
        Mock::given(path("/sync/e2ee/witness/user-a/accepted"))
            .respond_with(AcceptedResponder(state))
            .mount(&server)
            .await;
        let client = E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "token".into(),
            },
            "user-a",
        )
        .unwrap();
        let key = anlg_e2ee::RecoveryKey::generate()
            .unwrap()
            .workspace_key("user-a")
            .unwrap();
        let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
        let devices = [
            anlg_db_core::Db::connect_memory_plain().await.unwrap(),
            anlg_db_core::Db::connect_memory_plain().await.unwrap(),
        ];
        for db in &devices {
            anlg_db_app::prepare_schema(db).await.unwrap();
        }
        let document = |first: &str, second: &str| {
            json!({"type":"doc","content":
            ([first, second].map(|text| json!({"type":"paragraph","content":[{"type":"text","text":text}]})))}).to_string()
        };
        let hints = |human: &str| {
            json!([{"id":"assignment","word_id":"word","type":"user_speaker_assignment","value":{"human_id":human,"scope":"segment","word_ids":["word"]}}]).to_string()
        };
        let words =
            json!([{"id":"word","text":"Keep this","start_ms":100,"end_ms":300,"channel":2}])
                .to_string();
        sqlx::query("INSERT INTO sessions(id,workspace_id,owner_user_id,title) VALUES('s','user-a','user-a','Meeting')").execute(devices[0].pool()).await.unwrap();
        sqlx::query("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body) VALUES('s','user-a','s','note','prosemirror_json',?)")
            .bind(document("first", "second")).execute(devices[0].pool()).await.unwrap();
        sqlx::query("INSERT INTO transcripts(id,workspace_id,session_id,words_json,speaker_hints_json) VALUES('t','user-a','s',?,?)")
            .bind(&words).bind(hints("original")).execute(devices[0].pool()).await.unwrap();
        anlg_db_app::encrypt_e2ee_replica_changes(devices[0].pool(), &keys)
            .await
            .unwrap();
        for db in &devices {
            client.initialize(db.pool(), &key).await.unwrap();
            anlg_db_app::apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
                .await
                .unwrap();
        }
        // Both devices edit the same accepted baseline while disconnected.
        for (index, db) in devices.iter().enumerate() {
            let body = if index == 0 {
                document("desktop edit", "second")
            } else {
                document("first", "phone edit")
            };
            sqlx::query("UPDATE session_documents SET body = ? WHERE id = 's'")
                .bind(body)
                .execute(db.pool())
                .await
                .unwrap();
            sqlx::query("UPDATE transcripts SET speaker_hints_json = ? WHERE id = 't'")
                .bind(hints(if index == 0 { "alice" } else { "bob" }))
                .execute(db.pool())
                .await
                .unwrap();
            anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
                .await
                .unwrap();
        }
        for index in order {
            client
                .publish_and_refresh(devices[index].pool(), &key)
                .await
                .unwrap();
        }
        let winner = if order[1] == 0 { "alice" } else { "bob" };
        let loser = if order[0] == 0 { "alice" } else { "bob" };
        // A fresh replica must reconstruct the same accepted state without device clocks.
        let fresh = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&fresh).await.unwrap();
        client.initialize(fresh.pool(), &key).await.unwrap();
        for db in [&devices[0], &devices[1], &fresh] {
            client.refresh(db.pool(), &key).await.unwrap();
            anlg_db_app::apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
                .await
                .unwrap();
            let (body, stored_words, stored_hints): (String, String, String) = sqlx::query_as(
                "SELECT d.body,t.words_json,t.speaker_hints_json FROM session_documents d JOIN transcripts t ON t.session_id=d.session_id WHERE d.id='s' AND d.deleted_at IS NULL AND t.deleted_at IS NULL")
                .fetch_one(db.pool()).await.unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&body).unwrap(),
                serde_json::from_str::<serde_json::Value>(&document("desktop edit", "phone edit"))
                    .unwrap()
            );
            assert_eq!(stored_words, words);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&stored_hints).unwrap(),
                serde_json::from_str::<serde_json::Value>(&hints(winner)).unwrap()
            );
            assert!(
                anlg_db_app::pending_e2ee_cloud_batch(db.pool(), "user-a", false)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        let conflicts = anlg_db_app::list_e2ee_field_conflicts(
            devices[order[1]].pool(),
            "transcripts",
            "t",
            false,
        )
        .await
        .unwrap();
        assert!(
            conflicts
                .iter()
                .any(|c| c.field_name == "speaker_hints_json"
                    && serde_json::from_str::<serde_json::Value>(&c.value_json).unwrap()
                        == json!(hints(loser))),
            "the displaced assignment must remain recoverable"
        );
    }
}

#[tokio::test]
async fn running_legacy_clients_reprobe_after_cloud_cutover() {
    for pending_edit in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(witness_page(&[], 0, 0))
            .mount(&server)
            .await;
        let client = E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "token".into(),
            },
            "user-a",
        )
        .unwrap();
        let key = anlg_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap()
        .workspace_key("user-a")
        .unwrap();
        let keys = HashMap::from([("user-a".to_string(), key.clone().into())]);
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        client.initialize(db.pool(), &key).await.unwrap();
        if pending_edit {
            sqlx::query("INSERT INTO sessions(id,workspace_id,owner_user_id,title) VALUES('s','user-a','user-a','Offline')")
                .execute(db.pool()).await.unwrap();
            anlg_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
                .await
                .unwrap();
        }
        server.reset().await;
        Mock::given(path("/sync/e2ee/witness/user-a"))
            .respond_with(
                ResponseTemplate::new(426)
                    .set_body_json(json!({"error":{"code":"cloudsync_upgrade_required"}})),
            )
            .mount(&server)
            .await;
        Mock::given(path("/sync/e2ee/witness/user-a/accepted"))
            .respond_with(AcceptedResponder(Arc::new(Mutex::new(
                AcceptedServer::default(),
            ))))
            .mount(&server)
            .await;
        client.publish_and_refresh(db.pool(), &key).await.unwrap();
        assert!(
            anlg_db_app::e2ee_cloud_authority_enabled(db.pool(), "user-a")
                .await
                .unwrap()
        );
        assert!(
            anlg_db_app::pending_e2ee_cloud_batch(db.pool(), "user-a", false)
                .await
                .unwrap()
                .is_none()
        );
        if pending_edit {
            let fresh = anlg_db_core::Db::connect_memory_plain().await.unwrap();
            anlg_db_app::prepare_schema(&fresh).await.unwrap();
            client.initialize(fresh.pool(), &key).await.unwrap();
            anlg_db_app::apply_received_e2ee_replica_changes_with_witness(
                fresh.pool(),
                &keys,
                true,
            )
            .await
            .unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id='s'")
                    .fetch_one(fresh.pool())
                    .await
                    .unwrap(),
                "Offline"
            );
        }
    }
}

#[tokio::test]
async fn a_slow_witness_page_outlasts_the_idle_timeout_while_bytes_arrive() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let body = serde_json::to_vec(&json!({
        "initialized": true,
        "initializedAt": "2026-07-17T00:00:00Z",
        "headSequence": 0,
        "throughSequence": 0,
        "nextAfterSequence": 0,
        "events": [],
    }))
    .unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        for chunk in body.chunks(body.len().div_ceil(8)) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            socket.write_all(chunk).await.unwrap();
        }
    });
    let mut client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("http://{address}/sync/e2ee/witness/user-a"),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    client.client = http_client(std::time::Duration::from_millis(300)).unwrap();
    client.accepted_support.store(1, Ordering::Release);

    let page = client.read_page(0, None).await.unwrap();

    assert_eq!(page.head_sequence, 0);
}

#[tokio::test]
async fn idle_accepted_sync_round_performs_single_witness_read() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-10-05T00:00:00Z",
            "headSequence": 1,
            "throughSequence": 1,
            "nextAfterSequence": 1,
            "cloudAuthorityAfter": 0,
            "events": [],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    anlg_db_app::configure_e2ee_cloud_authority(db.pool(), "user-a", Some(0))
        .await
        .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(db.pool(), "user-a", 1)
        .await
        .unwrap();

    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );

    let outcome = hook.sync_replica_transport(db.pool()).await.unwrap();
    assert_eq!(outcome, crate::ReplicaSyncOutcome::Settled);
    server.verify().await;
}

#[tokio::test]
async fn accepted_sync_batches_multiple_rows_and_skips_redundant_get_after_publish() {
    #[derive(Clone, Default)]
    struct AcceptedBatchServer {
        gets: Arc<AtomicUsize>,
        posts: Arc<AtomicUsize>,
        head: Arc<Mutex<u64>>,
    }

    impl Respond for AcceptedBatchServer {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            if request.method.as_str() == "GET" {
                self.gets.fetch_add(1, Ordering::SeqCst);
                let head = *self.head.lock().unwrap();
                return ResponseTemplate::new(200).set_body_json(json!({
                    "initialized": true,
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": head,
                    "throughSequence": head,
                    "nextAfterSequence": head,
                    "cloudAuthorityAfter": 0,
                    "events": [],
                }));
            }
            self.posts.fetch_add(1, Ordering::SeqCst);
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let events = body["events"].as_array().unwrap();
            let mut head = self.head.lock().unwrap();
            let receipts: Vec<_> = events
                .iter()
                .map(|event| {
                    *head += 1;
                    json!({
                        "sequence": *head,
                        "recordId": event["recordId"],
                        "payloadHash": event["payloadHash"],
                    })
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "initializedAt": "2026-10-05T00:00:00Z",
                "headSequence": *head,
                "cloudAuthorityAfter": 0,
                "mutationId": body["mutationId"],
                "receipts": receipts,
            }))
        }
    }

    let server = MockServer::start().await;
    let responder = AcceptedBatchServer::default();
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    for id in ["s1", "s2"] {
        sqlx::query(
            "INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES(?, 'user-a', 'user-a', 'Session')",
        )
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    }

    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );

    let outcome = hook.sync_replica_transport(db.pool()).await.unwrap();
    assert_eq!(outcome, crate::ReplicaSyncOutcome::Settled);
    assert_eq!(responder.gets.load(Ordering::SeqCst), 1);
    assert_eq!(responder.posts.load(Ordering::SeqCst), 1);
    assert_eq!(
        anlg_db_app::e2ee_witness_cursor(db.pool(), "user-a")
            .await
            .unwrap(),
        *responder.head.lock().unwrap()
    );
}

#[tokio::test]
async fn adaptive_replica_sync_relaxes_to_heartbeat_while_watch_healthy() {
    #[derive(Clone)]
    struct AdaptiveServer {
        accepted_gets: Arc<AtomicUsize>,
        wait_healthy: Arc<AtomicBool>,
        wait_calls: Arc<AtomicUsize>,
    }

    impl Respond for AdaptiveServer {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            if request.url.path().ends_with("/accepted") {
                self.accepted_gets.fetch_add(1, Ordering::SeqCst);
                return ResponseTemplate::new(200).set_body_json(json!({
                    "initialized": true,
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": 1,
                    "throughSequence": 1,
                    "nextAfterSequence": 1,
                    "cloudAuthorityAfter": 0,
                    "events": [],
                }));
            }
            let call = self.wait_calls.fetch_add(1, Ordering::SeqCst);
            if !self.wait_healthy.load(Ordering::Acquire) {
                return ResponseTemplate::new(500);
            }
            let response = ResponseTemplate::new(200).set_body_json(json!({
                "initialized": true,
                "headSequence": 1,
            }));
            if call == 0 {
                response
            } else {
                response.set_delay(std::time::Duration::from_millis(250))
            }
        }
    }

    let server = MockServer::start().await;
    let responder = AdaptiveServer {
        accepted_gets: Arc::new(AtomicUsize::new(0)),
        wait_healthy: Arc::new(AtomicBool::new(true)),
        wait_calls: Arc::new(AtomicUsize::new(0)),
    };
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/wait"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let db = Arc::new(anlg_db_core::Db::connect_memory_plain().await.unwrap());
    anlg_db_app::prepare_schema(&db).await.unwrap();
    anlg_db_app::configure_e2ee_cloud_authority(db.pool(), "user-a", Some(0))
        .await
        .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(db.pool(), "user-a", 1)
        .await
        .unwrap();

    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );

    let _watch = crate::spawn_witness_watch(Arc::clone(&db), Arc::clone(&hook));
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while !hook.witness_watch_healthy() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let _task = crate::replica_sync::spawn_replica_sync_with_intervals(
        Arc::clone(&db),
        Arc::clone(&hook),
        std::time::Duration::from_millis(50),
        std::time::Duration::from_millis(280),
    );
    // `set_replica_witness` queues one immediate startup sync; drain it before timing the intervals.
    let initial_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    while responder.accepted_gets.load(Ordering::SeqCst) == 0 {
        assert!(tokio::time::Instant::now() < initial_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    responder.accepted_gets.store(0, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(140)).await;
    assert_eq!(
        responder.accepted_gets.load(Ordering::SeqCst),
        0,
        "healthy /wait should suppress the 50ms fallback timer"
    );

    let heartbeat_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    while responder.accepted_gets.load(Ordering::SeqCst) == 0 {
        assert!(tokio::time::Instant::now() < heartbeat_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    }

    responder.wait_healthy.store(false, Ordering::Release);
    let unhealthy_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while hook.witness_watch_healthy() {
        assert!(tokio::time::Instant::now() < unhealthy_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    responder.accepted_gets.store(0, Ordering::SeqCst);
    let fallback_deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(220);
    while responder.accepted_gets.load(Ordering::SeqCst) == 0 {
        assert!(tokio::time::Instant::now() < fallback_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn bootstrap_from_snapshot_and_410_recovery_preserves_unsent_local_edits() {
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let workspace_key = recovery_key.workspace_key("user-a").unwrap();
    let keyring = anlg_e2ee::WorkspaceKeyring::new(workspace_key.clone());

    // Build canonical witness history 1..=N in a seed DB using accepted replica encryption.
    let seed_db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&seed_db).await.unwrap();
    anlg_db_app::configure_e2ee_cloud_authority(seed_db.pool(), "user-a", Some(0))
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s1', 'user-a', 'user-a', 'Initial Title')")
        .execute(seed_db.pool())
        .await
        .unwrap();
    let keys_map = HashMap::from([("user-a".to_string(), keyring.clone())]);
    anlg_db_app::encrypt_e2ee_replica_changes(seed_db.pool(), &keys_map)
        .await
        .unwrap();
    let batch1 = anlg_db_app::pending_e2ee_cloud_batch(seed_db.pool(), "user-a", false)
        .await
        .unwrap()
        .unwrap();
    let mut seq = 0_u64;
    let mut initial_events = Vec::new();
    for event in &batch1.events {
        seq += 1;
        initial_events.push(anlg_db_app::E2eeWitnessEvent {
            sequence: seq,
            record_id: event.record_id.clone(),
            workspace_id: "user-a".to_string(),
            payload_hash: event.payload_hash.clone(),
            payload: event.payload.clone(),
        });
    }
    anlg_db_app::merge_e2ee_witness_events_with_keyring(
        seed_db.pool(),
        &keyring,
        "user-a",
        &initial_events,
    )
    .await
    .unwrap();
    anlg_db_app::acknowledge_e2ee_cloud_batch(seed_db.pool(), "user-a", &batch1)
        .await
        .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(seed_db.pool(), "user-a", seq)
        .await
        .unwrap();
    let initial_seq = seq;

    // Update s1 title on cloud before creating the compacted snapshot so it competes with dormant_db.
    sqlx::query("UPDATE sessions SET title = 'Remote Snapshot Title' WHERE id = 's1'")
        .execute(seed_db.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(seed_db.pool(), &keys_map)
        .await
        .unwrap();
    let batch1b = anlg_db_app::pending_e2ee_cloud_batch(seed_db.pool(), "user-a", false)
        .await
        .unwrap()
        .unwrap();
    let mut update_events = Vec::new();
    for event in &batch1b.events {
        seq += 1;
        update_events.push(anlg_db_app::E2eeWitnessEvent {
            sequence: seq,
            record_id: event.record_id.clone(),
            workspace_id: "user-a".to_string(),
            payload_hash: event.payload_hash.clone(),
            payload: event.payload.clone(),
        });
    }
    anlg_db_app::merge_e2ee_witness_events_with_keyring(
        seed_db.pool(),
        &keyring,
        "user-a",
        &update_events,
    )
    .await
    .unwrap();
    anlg_db_app::acknowledge_e2ee_cloud_batch(seed_db.pool(), "user-a", &batch1b)
        .await
        .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(seed_db.pool(), "user-a", seq)
        .await
        .unwrap();
    let snapshot_seq = seq;
    let snapshot_payload = anlg_db_app::export_e2ee_workspace_snapshot(seed_db.pool(), "user-a")
        .await
        .unwrap()
        .unwrap();
    let snapshot_bytes = anlg_e2ee::seal_snapshot(&workspace_key, &snapshot_payload).unwrap();
    let snapshot_sha = anlg_e2ee::bytes_sha256(&snapshot_bytes);

    // Append one tail session s2 after the compacted snapshot sequence.
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s2', 'user-a', 'user-a', 'Tail Session')")
        .execute(seed_db.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(seed_db.pool(), &keys_map)
        .await
        .unwrap();
    let batch2 = anlg_db_app::pending_e2ee_cloud_batch(seed_db.pool(), "user-a", false)
        .await
        .unwrap()
        .unwrap();
    let mut tail_events = Vec::new();
    for event in &batch2.events {
        seq += 1;
        tail_events.push(json!({
            "sequence": seq,
            "recordId": event.record_id,
            "payloadHash": event.payload_hash,
            "payload": event.payload,
        }));
    }
    let head_after_tail = seq;

    #[derive(Clone)]
    struct CompactedServer {
        server_uri: String,
        compacted_through: u64,
        snapshot_seq: u64,
        snapshot_bytes: Vec<u8>,
        snapshot_sha: String,
        snapshot_records: usize,
        key_id: String,
        head: Arc<Mutex<u64>>,
        tail_events: Vec<serde_json::Value>,
        uploaded_batches: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    impl Respond for CompactedServer {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let req_path = request.url.path();
            if req_path.ends_with("/accepted") && request.method.as_str() == "GET" {
                let after = request
                    .url
                    .query_pairs()
                    .find_map(|(k, v)| (k == "afterSequence").then(|| v.parse::<u64>().unwrap()))
                    .unwrap_or(0);
                if after < self.compacted_through {
                    return ResponseTemplate::new(410).set_body_json(json!({
                        "error": { "code": "e2ee_history_compacted" }
                    }));
                }
                let head = *self.head.lock().unwrap();
                let events: Vec<_> = self
                    .tail_events
                    .iter()
                    .filter(|ev| ev["sequence"].as_u64().unwrap() > after)
                    .cloned()
                    .collect();
                return ResponseTemplate::new(200).set_body_json(json!({
                    "initialized": true,
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": head,
                    "throughSequence": head,
                    "nextAfterSequence": head,
                    "cloudAuthorityAfter": 0,
                    "events": events,
                }));
            }
            if req_path.ends_with("/accepted") && request.method.as_str() == "POST" {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                self.uploaded_batches.lock().unwrap().push(body.clone());
                let events = body["events"].as_array().unwrap();
                let mut head = self.head.lock().unwrap();
                let receipts: Vec<_> = events
                    .iter()
                    .map(|event| {
                        *head += 1;
                        json!({
                            "sequence": *head,
                            "recordId": event["recordId"],
                            "payloadHash": event["payloadHash"],
                        })
                    })
                    .collect();
                return ResponseTemplate::new(200).set_body_json(json!({
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": *head,
                    "cloudAuthorityAfter": 0,
                    "mutationId": body["mutationId"],
                    "receipts": receipts,
                }));
            }
            if req_path.ends_with("/snapshot") && request.method.as_str() == "GET" {
                let head = *self.head.lock().unwrap();
                return ResponseTemplate::new(200).set_body_json(json!({
                    "snapshot": {
                        "snapshotSequence": self.snapshot_seq,
                        "formatVersion": 1,
                        "payloadBytes": self.snapshot_bytes.len(),
                        "payloadSha256": self.snapshot_sha,
                        "recordCount": self.snapshot_records,
                        "keyId": self.key_id,
                        "headSequence": head,
                        "cloudAuthorityAfter": 0,
                        "downloadUrl": format!("{}/download/snap.ans1", self.server_uri),
                    }
                }));
            }
            if req_path == "/download/snap.ans1" {
                return ResponseTemplate::new(200).set_body_bytes(self.snapshot_bytes.clone());
            }
            ResponseTemplate::new(404)
        }
    }

    let server = MockServer::start().await;
    let responder = CompactedServer {
        server_uri: server.uri(),
        compacted_through: snapshot_seq,
        snapshot_seq,
        snapshot_bytes,
        snapshot_sha,
        snapshot_records: snapshot_payload.records.len(),
        key_id: workspace_key.key_id().to_string(),
        head: Arc::new(Mutex::new(head_after_tail)),
        tail_events,
        uploaded_batches: Arc::new(Mutex::new(Vec::new())),
    };
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/snapshot"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/download/snap.ans1"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    // Fresh device at cursor=0 hits 410, bootstraps snapshot, applies tail s2, and hydrates both rows.
    let fresh_db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&fresh_db).await.unwrap();
    let fresh_hook = Arc::new(crate::E2eeSyncHook::default());
    fresh_hook
        .set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    fresh_hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    let outcome = fresh_hook
        .sync_replica_transport(fresh_db.pool())
        .await
        .unwrap();
    assert_eq!(outcome, crate::ReplicaSyncOutcome::Settled);
    let s1_title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's1'")
        .fetch_one(fresh_db.pool())
        .await
        .unwrap();
    let s2_title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's2'")
        .fetch_one(fresh_db.pool())
        .await
        .unwrap();
    assert_eq!(s1_title, "Remote Snapshot Title");
    assert_eq!(s2_title, "Tail Session");

    // Dormant device had synced older event 1 (title 'Initial Title'), fell behind before compaction,
    // and created an offline session 's-offline' + updated 's1' title. It must recover via 410
    // snapshot without losing 's-offline' or its offline edit on 's1'.
    let dormant_db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&dormant_db).await.unwrap();
    anlg_db_app::configure_e2ee_cloud_authority(dormant_db.pool(), "user-a", Some(0))
        .await
        .unwrap();
    anlg_db_app::merge_e2ee_witness_events_with_keyring(
        dormant_db.pool(),
        &keyring,
        "user-a",
        &initial_events,
    )
    .await
    .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(dormant_db.pool(), "user-a", initial_seq)
        .await
        .unwrap();
    anlg_db_app::apply_received_e2ee_replica_changes_with_witness(
        dormant_db.pool(),
        &keys_map,
        true,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Dormant Offline Title' WHERE id = 's1'")
        .execute(dormant_db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s-offline', 'user-a', 'user-a', 'Offline Note')")
        .execute(dormant_db.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(dormant_db.pool(), &keys_map)
        .await
        .unwrap();
    let dormant_hook = Arc::new(crate::E2eeSyncHook::default());
    dormant_hook
        .set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    dormant_hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    let outcome = dormant_hook
        .sync_replica_transport(dormant_db.pool())
        .await
        .unwrap();
    assert_eq!(outcome, crate::ReplicaSyncOutcome::Settled);
    assert!(!responder.uploaded_batches.lock().unwrap().is_empty());
    let dormant_s1: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's1'")
        .fetch_one(dormant_db.pool())
        .await
        .unwrap();
    assert_eq!(dormant_s1, "Dormant Offline Title");
    let dormant_s2: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's2'")
        .fetch_one(dormant_db.pool())
        .await
        .unwrap();
    assert_eq!(dormant_s2, "Tail Session");
    let dormant_offline: String =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's-offline'")
            .fetch_one(dormant_db.pool())
            .await
            .unwrap();
    assert_eq!(dormant_offline, "Offline Note");
    let conflicts =
        anlg_db_app::list_e2ee_field_conflicts(dormant_db.pool(), "sessions", "s1", false)
            .await
            .unwrap();
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.field_name == "title"
                && conflict.value_json.contains("Remote Snapshot Title"))
    );
}

#[tokio::test]
async fn opportunistic_snapshot_publish_uploads_and_commits_above_threshold() {
    let server = MockServer::start().await;
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let workspace_key = recovery_key.workspace_key("user-a").unwrap();
    let keyring = anlg_e2ee::WorkspaceKeyring::new(workspace_key.clone());

    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    anlg_db_app::configure_e2ee_cloud_authority(db.pool(), "user-a", Some(0))
        .await
        .unwrap();
    let sealed = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s1",
            "title",
            "00000000000000000000000000000001",
            1,
            false,
            Some(10),
            json!("Snapshotted"),
        )
        .unwrap();
    anlg_db_app::merge_e2ee_witness_events_with_keyring(
        db.pool(),
        &keyring,
        "user-a",
        &[anlg_db_app::E2eeWitnessEvent {
            sequence: 5,
            record_id: sealed.record_id,
            workspace_id: "user-a".to_string(),
            payload_hash: anlg_e2ee::payload_hash(&sealed.payload),
            payload: sealed.payload,
        }],
    )
    .await
    .unwrap();
    anlg_db_app::advance_e2ee_witness_cursor(db.pool(), "user-a", 5)
        .await
        .unwrap();

    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "access-token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    let cancel = E2eeWitnessCancellation::default();

    // Below threshold: returns Ok(None) without issuing any HTTP request.
    assert_eq!(
        client
            .maybe_publish_snapshot_cancellable(db.pool(), &keyring, 2000, None, &cancel)
            .await
            .unwrap(),
        None
    );

    let uploaded_bytes: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let uploaded_ref = Arc::clone(&uploaded_bytes);
    let remote_snapshot: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
    let remote_snapshot_ref = Arc::clone(&remote_snapshot);
    let server_uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/snapshot"))
        .respond_with(move |_request: &Request| match *remote_snapshot_ref.lock().unwrap() {
            Some(seq) => ResponseTemplate::new(200).set_body_json(json!({
                "snapshot": {
                    "snapshotSequence": seq,
                    "formatVersion": 1,
                    "payloadBytes": 128,
                    "payloadSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "recordCount": 1,
                    "keyId": workspace_key.key_id(),
                    "headSequence": 5,
                    "cloudAuthorityAfter": 0,
                    "downloadUrl": format!("{server_uri}/unused"),
                }
            })),
            None => ResponseTemplate::new(200).set_body_json(json!({ "snapshot": null })),
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a/snapshot/upload"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "objectKey": "user-a/5-11111111-1111-4111-8111-111111111111.ans1",
            "uploadUrl": format!("{}/upload-target", server.uri()),
            "token": "tok"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload-target"))
        .respond_with(move |request: &Request| {
            *uploaded_ref.lock().unwrap() = request.body.clone();
            ResponseTemplate::new(200)
        })
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a/snapshot/commit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "committed": true,
            "snapshotSequence": 5
        })))
        .expect(1)
        .mount(&server)
        .await;

    let seq = client
        .maybe_publish_snapshot_cancellable(db.pool(), &keyring, 2, None, &cancel)
        .await
        .unwrap();
    assert_eq!(seq, Some(5));
    // After restart (`last_published_sequence == None`), an up-to-date remote snapshot skips upload.
    *remote_snapshot.lock().unwrap() = Some(5);
    let skipped = client
        .maybe_publish_snapshot_cancellable(db.pool(), &keyring, 2, None, &cancel)
        .await
        .unwrap();
    assert_eq!(skipped, Some(5));
    server.verify().await;

    let bytes = uploaded_bytes.lock().unwrap().clone();
    let decoded = anlg_e2ee::open_snapshot(&keyring, "user-a", 5, &bytes).unwrap();
    assert_eq!(decoded.records.len(), 1);
    assert_eq!(decoded.snapshot_sequence, 5);
}

#[derive(Clone)]
struct ClusterEvent {
    sequence: u64,
    record_id: String,
    payload_hash: String,
    payload: String,
}

#[derive(Clone)]
struct ClusterSnapshot {
    snapshot_sequence: u64,
    bytes: Vec<u8>,
    sha256: String,
    record_count: u32,
    key_id: String,
}

#[derive(Default)]
struct ClusterWorkspaceState {
    events: Vec<ClusterEvent>,
    receipts: HashMap<String, (u64, Vec<serde_json::Value>)>,
    cloud_authority_after: Option<u64>,
    compacted_through: u64,
    snapshot: Option<ClusterSnapshot>,
    pending_snapshot_uploads: HashMap<String, Vec<u8>>,
}

struct PeerClusterState {
    workspaces: Mutex<HashMap<String, ClusterWorkspaceState>>,
    peer_wakes: Vec<Arc<tokio::sync::Notify>>,
}

impl PeerClusterState {
    fn new(num_peers: usize) -> Arc<Self> {
        Arc::new(Self {
            workspaces: Mutex::new(HashMap::new()),
            peer_wakes: (0..num_peers)
                .map(|_| Arc::new(tokio::sync::Notify::new()))
                .collect(),
        })
    }

    fn notify_all(&self) {
        for wake in &self.peer_wakes {
            wake.notify_waiters();
        }
    }
}

#[derive(Clone)]
struct PeerReplicaEndpoint {
    base_url: String,
    cluster: Arc<PeerClusterState>,
}

impl Respond for PeerReplicaEndpoint {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let req_path = request.url.path();
        if let Some(object_key) = req_path.strip_prefix("/storage/put/") {
            let ws_id = object_key.split('/').next().unwrap_or_default().to_string();
            let mut map = self.cluster.workspaces.lock().unwrap();
            let ws = map.entry(ws_id).or_default();
            ws.pending_snapshot_uploads
                .insert(object_key.to_string(), request.body.clone());
            return ResponseTemplate::new(200);
        }
        if let Some(object_key) = req_path.strip_prefix("/storage/get/") {
            let ws_id = object_key.split('/').next().unwrap_or_default().to_string();
            let map = self.cluster.workspaces.lock().unwrap();
            if let Some(snapshot) = map.get(&ws_id).and_then(|ws| ws.snapshot.as_ref()) {
                return ResponseTemplate::new(200).set_body_bytes(snapshot.bytes.clone());
            }
            return ResponseTemplate::new(404);
        }

        let Some(rest) = req_path.strip_prefix("/sync/e2ee/witness/") else {
            return ResponseTemplate::new(404);
        };
        let mut parts = rest.split('/');
        let workspace_id = parts.next().unwrap_or_default().to_string();
        let sub1 = parts.next().unwrap_or_default();
        let sub2 = parts.next().unwrap_or_default();

        match (request.method.as_str(), sub1, sub2) {
            ("GET", "accepted", "") => {
                let after = request
                    .url
                    .query_pairs()
                    .find_map(|(k, v)| (k == "afterSequence").then(|| v.parse::<u64>().unwrap()))
                    .unwrap_or(0);
                let through_opt = request
                    .url
                    .query_pairs()
                    .find_map(|(k, v)| (k == "throughSequence").then(|| v.parse::<u64>().unwrap()));
                let map = self.cluster.workspaces.lock().unwrap();
                let ws = map.get(&workspace_id);
                let compacted = ws.map_or(0, |w| w.compacted_through);
                if compacted > 0 && after < compacted {
                    return ResponseTemplate::new(410).set_body_json(json!({
                        "error": { "code": "e2ee_history_compacted" }
                    }));
                }
                let head = ws
                    .and_then(|w| w.events.last().map(|e| e.sequence))
                    .unwrap_or(compacted)
                    .max(compacted);
                let through = through_opt.unwrap_or(head);
                let events: Vec<_> = ws
                    .map(|w| {
                        w.events
                            .iter()
                            .filter(|e| e.sequence > after && e.sequence <= through)
                            .map(|e| {
                                json!({
                                    "sequence": e.sequence,
                                    "recordId": e.record_id,
                                    "payloadHash": e.payload_hash,
                                    "payload": e.payload,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let next_after = events
                    .last()
                    .and_then(|e| e["sequence"].as_u64())
                    .unwrap_or(through);
                ResponseTemplate::new(200).set_body_json(json!({
                    "initialized": head > 0,
                    "initializedAt": (head > 0).then_some("2026-10-05T00:00:00Z"),
                    "headSequence": head,
                    "throughSequence": through,
                    "nextAfterSequence": next_after,
                    "cloudAuthorityAfter": ws.and_then(|w| w.cloud_authority_after).unwrap_or(0),
                    "events": events,
                }))
            }
            ("POST", "accepted", "") => {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let mutation_id = body["mutationId"].as_str().unwrap().to_string();
                let base_seq = body["baseSequence"].as_u64().unwrap();
                let events = body["events"].as_array().unwrap();
                let mut map = self.cluster.workspaces.lock().unwrap();
                let ws = map.entry(workspace_id).or_default();
                if let Some((accepted_head, receipts)) = ws.receipts.get(&mutation_id).cloned() {
                    return ResponseTemplate::new(200).set_body_json(json!({
                        "initializedAt": "2026-10-05T00:00:00Z",
                        "headSequence": accepted_head,
                        "cloudAuthorityAfter": ws.cloud_authority_after.unwrap_or(0),
                        "mutationId": mutation_id,
                        "receipts": receipts,
                    }));
                }
                let current_head = ws
                    .events
                    .last()
                    .map(|e| e.sequence)
                    .unwrap_or(ws.compacted_through)
                    .max(ws.compacted_through);
                if base_seq != current_head {
                    return ResponseTemplate::new(409).set_body_json(json!({
                        "error": { "code": "e2ee_replica_base_changed" }
                    }));
                }
                if ws.cloud_authority_after.is_none() {
                    ws.cloud_authority_after = Some(0);
                }
                let mut next_seq = current_head;
                let mut receipts = Vec::new();
                for event in events {
                    next_seq += 1;
                    let record_id = event["recordId"].as_str().unwrap().to_string();
                    let payload_hash = event["payloadHash"].as_str().unwrap().to_string();
                    let payload = event["payload"].as_str().unwrap().to_string();
                    ws.events.push(ClusterEvent {
                        sequence: next_seq,
                        record_id: record_id.clone(),
                        payload_hash: payload_hash.clone(),
                        payload,
                    });
                    receipts.push(json!({
                        "sequence": next_seq,
                        "recordId": record_id,
                        "payloadHash": payload_hash,
                    }));
                }
                ws.receipts
                    .insert(mutation_id.clone(), (next_seq, receipts.clone()));
                drop(map);
                // Fan out wakes across all simulated API instances.
                self.cluster.notify_all();
                ResponseTemplate::new(200).set_body_json(json!({
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": next_seq,
                    "cloudAuthorityAfter": 0,
                    "mutationId": mutation_id,
                    "receipts": receipts,
                }))
            }
            ("GET", "snapshot", "") => {
                let map = self.cluster.workspaces.lock().unwrap();
                let Some(ws) = map.get(&workspace_id) else {
                    return ResponseTemplate::new(200).set_body_json(json!({ "snapshot": null }));
                };
                let head = ws
                    .events
                    .last()
                    .map(|e| e.sequence)
                    .unwrap_or(ws.compacted_through)
                    .max(ws.compacted_through);
                let Some(snapshot) = &ws.snapshot else {
                    return ResponseTemplate::new(200).set_body_json(json!({ "snapshot": null }));
                };
                ResponseTemplate::new(200).set_body_json(json!({
                    "snapshot": {
                        "snapshotSequence": snapshot.snapshot_sequence,
                        "formatVersion": 1,
                        "payloadBytes": snapshot.bytes.len(),
                        "payloadSha256": snapshot.sha256,
                        "recordCount": snapshot.record_count,
                        "keyId": snapshot.key_id,
                        "headSequence": head,
                        "cloudAuthorityAfter": ws.cloud_authority_after,
                        "downloadUrl": format!("{}/storage/get/{}/snap.ans1", self.base_url, workspace_id),
                    }
                }))
            }
            ("POST", "snapshot", "upload") => {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let seq = body["snapshotSequence"].as_u64().unwrap();
                let key = format!("{workspace_id}/{seq}-11111111-1111-4111-8111-111111111111.ans1");
                ResponseTemplate::new(200).set_body_json(json!({
                    "objectKey": key,
                    "uploadUrl": format!("{}/storage/put/{key}", self.base_url),
                    "token": "tok",
                }))
            }
            ("POST", "snapshot", "commit") => {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let seq = body["snapshotSequence"].as_u64().unwrap();
                let object_key = body["objectKey"].as_str().unwrap().to_string();
                let sha = body["payloadSha256"].as_str().unwrap().to_string();
                let record_count = body["recordCount"].as_u64().unwrap() as u32;
                let key_id = body["keyId"].as_str().unwrap().to_string();
                let mut map = self.cluster.workspaces.lock().unwrap();
                let ws = map.entry(workspace_id).or_default();
                let bytes = ws.pending_snapshot_uploads.remove(&object_key).unwrap();
                assert_eq!(anlg_e2ee::bytes_sha256(&bytes), sha);
                ws.snapshot = Some(ClusterSnapshot {
                    snapshot_sequence: seq,
                    bytes,
                    sha256: sha,
                    record_count,
                    key_id,
                });
                ResponseTemplate::new(200).set_body_json(json!({
                    "committed": true,
                    "snapshotSequence": seq,
                }))
            }
            _ => ResponseTemplate::new(404),
        }
    }
}

async fn start_wait_http_peer(
    peer_index: usize,
    cluster: Arc<PeerClusterState>,
    api_server: &MockServer,
) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let api_uri = api_server.uri();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let cluster = Arc::clone(&cluster);
            let api_uri = api_uri.clone();
            tokio::spawn(async move {
                let mut buf = [0_u8; 4096];
                let Ok(n) = socket.read(&mut buf).await else {
                    return;
                };
                let req_text = String::from_utf8_lossy(&buf[..n]);
                let first_line = req_text.lines().next().unwrap_or_default().to_string();
                let path_query = first_line.split_whitespace().nth(1).unwrap_or_default();
                if path_query.contains("/wait") {
                    let workspace_id = path_query
                        .strip_prefix("/sync/e2ee/witness/")
                        .and_then(|s| s.split('/').next())
                        .unwrap_or_default()
                        .to_string();
                    let after = path_query
                        .split("afterSequence=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    let wake = Arc::clone(&cluster.peer_wakes[peer_index]);
                    let (initialized, head) = loop {
                        let notified = wake.notified();
                        tokio::pin!(notified);
                        notified.as_mut().enable();
                        let (init, head) = {
                            let map = cluster.workspaces.lock().unwrap();
                            let ws = map.get(&workspace_id);
                            let compacted = ws.map_or(0, |w| w.compacted_through);
                            let h = ws
                                .and_then(|w| w.events.last().map(|e| e.sequence))
                                .unwrap_or(compacted)
                                .max(compacted);
                            (h > 0, h)
                        };
                        if !init || head > after {
                            break (init, head);
                        }
                        if tokio::time::timeout(std::time::Duration::from_millis(50), notified)
                            .await
                            .is_err()
                        {
                            // Match the server's bounded idle hold so healthy /wait watchers can
                            // report health cleanly even when no further writes arrive immediately.
                            break (init, head);
                        }
                    };
                    let body =
                        json!({ "initialized": initialized, "headSequence": head }).to_string();
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                    return;
                }
                // Proxy any non-/wait call straight to the backing MockServer!
                let body_split = req_text.split("\r\n\r\n").nth(1).unwrap_or_default();
                let method = first_line.split_whitespace().next().unwrap_or("GET");
                let mut total_body = body_split.as_bytes().to_vec();
                if let Some(cl_line) = req_text
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                    && let Ok(cl) = cl_line
                        .split(':')
                        .nth(1)
                        .unwrap_or("0")
                        .trim()
                        .parse::<usize>()
                {
                    while total_body.len() < cl {
                        let mut extra = vec![0_u8; cl - total_body.len()];
                        let Ok(k) = socket.read(&mut extra).await else {
                            break;
                        };
                        if k == 0 {
                            break;
                        }
                        total_body.extend_from_slice(&extra[..k]);
                    }
                }
                let client = reqwest::Client::new();
                let builder = match method {
                    "POST" => client
                        .post(format!("{api_uri}{path_query}"))
                        .body(total_body),
                    "PUT" => client
                        .put(format!("{api_uri}{path_query}"))
                        .body(total_body),
                    _ => client.get(format!("{api_uri}{path_query}")),
                };
                let Ok(resp) = builder.send().await else {
                    return;
                };
                let status = resp.status();
                let bytes = resp.bytes().await.unwrap_or_default();
                let ct = if path_query.contains("/storage/") {
                    "application/octet-stream"
                } else {
                    "application/json"
                };
                let reply = format!(
                    "HTTP/1.1 {} {}\r\ncontent-type: {ct}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or("OK"),
                    bytes.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.write_all(&bytes).await;
            });
        }
    });
    addr
}

async fn mount_peer_mock(
    cluster: Arc<PeerClusterState>,
    peer_index: usize,
) -> std::net::SocketAddr {
    let mock = MockServer::start().await;
    let responder = PeerReplicaEndpoint {
        base_url: mock.uri(),
        cluster: Arc::clone(&cluster),
    };
    Mock::given(wiremock::matchers::any())
        .respond_with(responder)
        .mount(&mock)
        .await;
    let addr = start_wait_http_peer(peer_index, cluster, &mock).await;
    // Keep MockServer alive for test duration via leak? Better: return MockServer or spawn wait proxy using peer's base URL.
    std::mem::forget(mock);
    addr
}

#[tokio::test]
async fn e2e_two_devices_converge_live_across_peer_wakes_and_offline_restarts() {
    use sqlx::Connection as _;

    let recovery = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let cluster = PeerClusterState::new(2);
    let peer1_addr = mount_peer_mock(Arc::clone(&cluster), 0).await;
    let peer2_addr = mount_peer_mock(Arc::clone(&cluster), 1).await;

    let dir = tempfile::tempdir().unwrap();
    let path_a = dir.path().join("device-a.db");
    let path_b = dir.path().join("device-b.db");

    let doc = |p1: &str, p2: &str| {
        json!({
            "type": "doc",
            "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": p1 }] },
                { "type": "paragraph", "content": [{ "type": "text", "text": p2 }] }
            ]
        })
        .to_string()
    };

    // Part A: Live convergence across peer instances while Device B's /wait is parked.
    let db_a = Arc::new(
        anlg_db_core::Db::connect_local_plain(&path_a)
            .await
            .unwrap(),
    );
    anlg_db_app::prepare_schema(&db_a).await.unwrap();
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s-live', 'user-a', 'user-a', 'Live Init')")
        .execute(db_a.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_documents(id, workspace_id, session_id, kind, body_format, body) VALUES('d-live', 'user-a', 's-live', 'note', 'prosemirror_json', ?)")
        .bind(doc("Hello", "World"))
        .execute(db_a.pool())
        .await
        .unwrap();

    let hook_a = Arc::new(crate::E2eeSyncHook::default());
    hook_a.set_personal_workspace("user-a", &recovery).unwrap();
    hook_a.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("http://{peer1_addr}/sync/e2ee/witness/user-a"),
                access_token: "token-a".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    assert_eq!(
        hook_a.sync_replica_transport(db_a.pool()).await.unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    let db_b = Arc::new(
        anlg_db_core::Db::connect_local_plain(&path_b)
            .await
            .unwrap(),
    );
    anlg_db_app::prepare_schema(&db_b).await.unwrap();
    let hook_b = Arc::new(crate::E2eeSyncHook::default());
    hook_b.set_personal_workspace("user-a", &recovery).unwrap();
    hook_b.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("http://{peer2_addr}/sync/e2ee/witness/user-a"),
                access_token: "token-b".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    let _task_b = crate::replica_sync::spawn_replica_sync(Arc::clone(&db_b), Arc::clone(&hook_b));
    let _watch_b = crate::spawn_witness_watch(Arc::clone(&db_b), Arc::clone(&hook_b));

    let init_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Ok(title) =
            sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 's-live'")
                .fetch_one(db_b.pool())
                .await
            && title == "Live Init"
        {
            break;
        }
        assert!(tokio::time::Instant::now() < init_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Wait for watch_b to park on /wait before Peer 1 receives an update.
    let healthy_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    while !hook_b.witness_watch_healthy() {
        assert!(tokio::time::Instant::now() < healthy_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    sqlx::query("UPDATE sessions SET title = 'Live Updated' WHERE id = 's-live'")
        .execute(db_a.pool())
        .await
        .unwrap();
    assert_eq!(
        hook_a.sync_replica_transport(db_a.pool()).await.unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    let wake_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Ok(title) =
            sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 's-live'")
                .fetch_one(db_b.pool())
                .await
            && title == "Live Updated"
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < wake_deadline,
            "peer wake must trigger Device B sync well before heartbeat timer"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Part B: Offline edits + full disk DB restart + document/title convergence + zero tx leaks.
    drop(_watch_b);
    drop(_task_b);
    sqlx::query("UPDATE session_documents SET body = ? WHERE id = 'd-live'")
        .bind(doc("Hello from A", "World"))
        .execute(db_a.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Title A' WHERE id = 's-live'")
        .execute(db_a.pool())
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    sqlx::query("UPDATE session_documents SET body = ? WHERE id = 'd-live'")
        .bind(doc("Hello", "World from B"))
        .execute(db_b.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'Title B' WHERE id = 's-live'")
        .execute(db_b.pool())
        .await
        .unwrap();

    db_a.pool().close().await;
    db_b.pool().close().await;
    drop(db_a);
    drop(db_b);

    let db_a = Arc::new(
        anlg_db_core::Db::connect_local_plain(&path_a)
            .await
            .unwrap(),
    );
    anlg_db_app::prepare_schema(&db_a).await.unwrap();
    let db_b = Arc::new(
        anlg_db_core::Db::connect_local_plain(&path_b)
            .await
            .unwrap(),
    );
    anlg_db_app::prepare_schema(&db_b).await.unwrap();

    assert_eq!(
        hook_a.sync_replica_transport(db_a.pool()).await.unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    assert_eq!(
        hook_b.sync_replica_transport(db_b.pool()).await.unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    assert_eq!(
        hook_a.sync_replica_transport(db_a.pool()).await.unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    let body_a: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'd-live'")
            .fetch_one(db_a.pool())
            .await
            .unwrap();
    let body_b: String =
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'd-live'")
            .fetch_one(db_b.pool())
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body_a).unwrap(),
        serde_json::from_str::<serde_json::Value>(&doc("Hello from A", "World from B")).unwrap()
    );
    assert_eq!(body_a, body_b);

    let title_a: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's-live'")
        .fetch_one(db_a.pool())
        .await
        .unwrap();
    let title_b: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's-live'")
        .fetch_one(db_b.pool())
        .await
        .unwrap();
    assert_eq!(title_a, "Title B");
    assert_eq!(title_a, title_b);

    for pool in [db_a.pool(), db_b.pool()] {
        let conn = pool.acquire().await.unwrap();
        assert!(!conn.is_in_transaction());
        conn.close().await.unwrap();
    }
}

#[tokio::test]
async fn e2e_shared_workspace_snapshot_bootstrap_and_dormant_410_recovery() {
    let ws = "ws-shared-1";
    let retired_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap()
    .workspace_key(ws)
    .unwrap();
    let active_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg",
    )
    .unwrap()
    .workspace_key(ws)
    .unwrap();
    let mut member_keyring = anlg_e2ee::WorkspaceKeyring::new(active_key.clone());
    member_keyring.insert_retired(retired_key.clone());
    let mut owner_keyring = anlg_e2ee::WorkspaceKeyring::new(retired_key.clone());

    let cluster = PeerClusterState::new(1);
    let peer_addr = mount_peer_mock(Arc::clone(&cluster), 0).await;
    let endpoint = format!("http://{peer_addr}/sync/e2ee/witness/{ws}");

    let dir = tempfile::tempdir().unwrap();
    let db_owner = anlg_db_core::Db::connect_local_plain(dir.path().join("owner.db"))
        .await
        .unwrap();
    anlg_db_app::prepare_schema(&db_owner).await.unwrap();
    let db_dormant = anlg_db_core::Db::connect_local_plain(dir.path().join("dormant.db"))
        .await
        .unwrap();
    anlg_db_app::prepare_schema(&db_dormant).await.unwrap();
    let db_fresh = anlg_db_core::Db::connect_local_plain(dir.path().join("fresh.db"))
        .await
        .unwrap();
    anlg_db_app::prepare_schema(&db_fresh).await.unwrap();

    for (db, personal_ws) in [
        (&db_owner, "owner"),
        (&db_dormant, "member-d"),
        (&db_fresh, "member-c"),
    ] {
        sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES(?, ?, ?, 'Personal')")
            .bind(format!("p-{personal_ws}"))
            .bind(personal_ws)
            .bind(personal_ws)
            .execute(db.pool())
            .await
            .unwrap();
    }

    let make_witnesses = |personal_ws: &str, token: &str| {
        HashMap::from([
            (
                personal_ws.to_string(),
                E2eeWitnessClient::new(
                    E2eeWitnessConfig {
                        endpoint: format!("http://{peer_addr}/sync/e2ee/witness/{personal_ws}"),
                        access_token: token.to_string(),
                    },
                    personal_ws,
                )
                .unwrap(),
            ),
            (
                ws.to_string(),
                E2eeWitnessClient::new(
                    E2eeWitnessConfig {
                        endpoint: endpoint.clone(),
                        access_token: token.to_string(),
                    },
                    ws,
                )
                .unwrap(),
            ),
        ])
    };

    // Owner writes s1 under retired key and syncs. Dormant Device D syncs s1 before going offline.
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s1', ?, 'owner', 'Shared 1')")
        .bind(ws)
        .execute(db_owner.pool())
        .await
        .unwrap();
    let hook_owner = Arc::new(crate::E2eeSyncHook::default());
    hook_owner
        .set_workspaces(
            "owner",
            &anlg_e2ee::RecoveryKey::parse(
                "anarlog-e2ee-v1:CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg",
            )
            .unwrap(),
            HashMap::from([(ws.to_string(), owner_keyring.clone())]),
        )
        .unwrap();
    let owner_client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: endpoint.clone(),
            access_token: "tok-o".to_string(),
        },
        ws,
    )
    .unwrap();
    hook_owner.set_replica_witnesses(make_witnesses("owner", "tok-o"));
    assert_eq!(
        hook_owner
            .sync_replica_transport(db_owner.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    let hook_dormant = Arc::new(crate::E2eeSyncHook::default());
    hook_dormant
        .set_workspaces(
            "member-d",
            &anlg_e2ee::RecoveryKey::parse(
                "anarlog-e2ee-v1:CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg",
            )
            .unwrap(),
            HashMap::from([(ws.to_string(), member_keyring.clone())]),
        )
        .unwrap();
    hook_dormant.set_replica_witnesses(make_witnesses("member-d", "tok-d"));
    assert_eq!(
        hook_dormant
            .sync_replica_transport(db_dormant.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    // Owner rotates to active key, adds s2 + s3, syncs, publishes snapshot, and server compacts!
    owner_keyring = member_keyring.clone();
    hook_owner
        .set_workspaces(
            "owner",
            &anlg_e2ee::RecoveryKey::parse(
                "anarlog-e2ee-v1:CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg",
            )
            .unwrap(),
            HashMap::from([(ws.to_string(), owner_keyring.clone())]),
        )
        .unwrap();
    hook_owner.set_replica_witnesses(make_witnesses("owner", "tok-o"));
    for id in ["s2", "s3"] {
        sqlx::query(
            "INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES(?, ?, 'owner', ?)",
        )
        .bind(id)
        .bind(ws)
        .bind(format!("Shared {id}"))
        .execute(db_owner.pool())
        .await
        .unwrap();
    }
    assert_eq!(
        hook_owner
            .sync_replica_transport(db_owner.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    let cancel = E2eeWitnessCancellation::default();
    let snap_seq = owner_client
        .maybe_publish_snapshot_cancellable(db_owner.pool(), &owner_keyring, 1, None, &cancel)
        .await
        .unwrap()
        .expect("snapshot published");

    // Compact server through `snap_seq`, then append s4 after compaction.
    {
        let mut map = cluster.workspaces.lock().unwrap();
        let ws_state = map.get_mut(ws).unwrap();
        ws_state.compacted_through = snap_seq;
        ws_state.events.retain(|e| e.sequence > snap_seq);
    }
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s4', ?, 'owner', 'Post Compact')")
        .bind(ws)
        .execute(db_owner.pool())
        .await
        .unwrap();
    assert_eq!(
        hook_owner
            .sync_replica_transport(db_owner.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );

    // Fresh Member C bootstraps from snapshot + pulls tail s4.
    let hook_fresh = Arc::new(crate::E2eeSyncHook::default());
    hook_fresh
        .set_workspaces(
            "member-c",
            &anlg_e2ee::RecoveryKey::parse(
                "anarlog-e2ee-v1:CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg",
            )
            .unwrap(),
            HashMap::from([(ws.to_string(), member_keyring.clone())]),
        )
        .unwrap();
    hook_fresh.set_replica_witnesses(make_witnesses("member-c", "tok-c"));
    assert_eq!(
        hook_fresh
            .sync_replica_transport(db_fresh.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    let fresh_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE workspace_id = ?")
            .bind(ws)
            .fetch_one(db_fresh.pool())
            .await
            .unwrap();
    assert_eq!(fresh_count, 4);

    // Dormant Member D created offline note s-dormant before hitting 410 compaction.
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s-dormant', ?, 'member-d', 'Dormant Offline')")
        .bind(ws)
        .execute(db_dormant.pool())
        .await
        .unwrap();
    assert_eq!(
        hook_dormant
            .sync_replica_transport(db_dormant.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    assert_eq!(
        hook_fresh
            .sync_replica_transport(db_fresh.pool())
            .await
            .unwrap(),
        crate::ReplicaSyncOutcome::Settled
    );
    let final_fresh_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE workspace_id = ?")
            .bind(ws)
            .fetch_one(db_fresh.pool())
            .await
            .unwrap();
    assert_eq!(final_fresh_count, 5);
}

#[tokio::test]
async fn multi_page_history_replay_survives_fast_merge_before_delayed_prefetch() {
    let server = MockServer::start().await;
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let workspace_key = recovery_key.workspace_key("user-a").unwrap();
    let keyring = anlg_e2ee::WorkspaceKeyring::new(workspace_key.clone());

    let m1 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s1",
            "$row",
            "00000000000000000000000000000001",
            1,
            false,
            Some(10),
            json!(true),
        )
        .unwrap();
    let e1 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s1",
            "title",
            "00000000000000000000000000000001",
            1,
            false,
            Some(10),
            json!("Page One Title"),
        )
        .unwrap();
    let m2 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s2",
            "$row",
            "00000000000000000000000000000001",
            1,
            false,
            Some(11),
            json!(true),
        )
        .unwrap();
    let e2 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s2",
            "title",
            "00000000000000000000000000000001",
            1,
            false,
            Some(11),
            json!("Page Two Title"),
        )
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .and(wiremock::matchers::query_param("afterSequence", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-10-05T00:00:00Z",
            "headSequence": 4,
            "throughSequence": 4,
            "nextAfterSequence": 2,
            "cloudAuthorityAfter": 0,
            "events": [
                {
                    "sequence": 1,
                    "recordId": m1.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&m1.payload),
                    "payload": m1.payload,
                },
                {
                    "sequence": 2,
                    "recordId": e1.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&e1.payload),
                    "payload": e1.payload,
                }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .and(wiremock::matchers::query_param("afterSequence", "2"))
        .and(wiremock::matchers::query_param("throughSequence", "4"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(150))
                .set_body_json(json!({
                    "initialized": true,
                    "initializedAt": "2026-10-05T00:00:00Z",
                    "headSequence": 4,
                    "throughSequence": 4,
                    "nextAfterSequence": 4,
                    "cloudAuthorityAfter": 0,
                    "events": [
                        {
                            "sequence": 3,
                            "recordId": m2.record_id,
                            "payloadHash": anlg_e2ee::payload_hash(&m2.payload),
                            "payload": m2.payload,
                        },
                        {
                            "sequence": 4,
                            "recordId": e2.record_id,
                            "payloadHash": anlg_e2ee::payload_hash(&e2.payload),
                            "payload": e2.payload,
                        }
                    ]
                })),
        )
        .mount(&server)
        .await;

    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let client = E2eeWitnessClient::new(
        E2eeWitnessConfig {
            endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
            access_token: "token".to_string(),
        },
        "user-a",
    )
    .unwrap();
    let cancel = E2eeWitnessCancellation::default();
    let merged = client
        .refresh_keyring_notifying_cancellable(db.pool(), &keyring, || {}, &cancel)
        .await
        .unwrap();
    assert_eq!(merged, 4);

    let keys = HashMap::from([("user-a".to_string(), keyring)]);
    anlg_db_app::apply_received_e2ee_replica_changes_with_witness(db.pool(), &keys, true)
        .await
        .unwrap();
    let s1: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let s2: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's2'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(s1, "Page One Title");
    assert_eq!(s2, "Page Two Title");
}

#[tokio::test]
async fn initial_sync_falls_back_to_history_when_optional_snapshot_fails() {
    let server = MockServer::start().await;
    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let workspace_key = recovery_key.workspace_key("user-a").unwrap();

    let m1 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s1",
            "$row",
            "00000000000000000000000000000001",
            1,
            false,
            Some(10),
            json!(true),
        )
        .unwrap();
    let e1 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s1",
            "title",
            "00000000000000000000000000000001",
            1,
            false,
            Some(10),
            json!("Fallback One"),
        )
        .unwrap();
    let m2 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s2",
            "$row",
            "00000000000000000000000000000001",
            1,
            false,
            Some(11),
            json!(true),
        )
        .unwrap();
    let e2 = workspace_key
        .seal_field_at(
            "user-a",
            "sessions",
            "s2",
            "title",
            "00000000000000000000000000000001",
            1,
            false,
            Some(11),
            json!("Fallback Tail"),
        )
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .and(wiremock::matchers::query_param("afterSequence", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-10-05T00:00:00Z",
            "headSequence": 2000,
            "throughSequence": 2000,
            "nextAfterSequence": 2,
            "cloudAuthorityAfter": 0,
            "events": [
                {
                    "sequence": 1,
                    "recordId": m1.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&m1.payload),
                    "payload": m1.payload,
                },
                {
                    "sequence": 2,
                    "recordId": e1.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&e1.payload),
                    "payload": e1.payload,
                }
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/snapshot"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1..=2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .and(wiremock::matchers::query_param("afterSequence", "2"))
        .and(wiremock::matchers::query_param("throughSequence", "2000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "initialized": true,
            "initializedAt": "2026-10-05T00:00:00Z",
            "headSequence": 2000,
            "throughSequence": 2000,
            "nextAfterSequence": 2000,
            "cloudAuthorityAfter": 0,
            "events": [
                {
                    "sequence": 1999,
                    "recordId": m2.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&m2.payload),
                    "payload": m2.payload,
                },
                {
                    "sequence": 2000,
                    "recordId": e2.record_id,
                    "payloadHash": anlg_e2ee::payload_hash(&e2.payload),
                    "payload": e2.payload,
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    let outcome = hook.sync_replica_transport(db.pool()).await.unwrap();
    assert_eq!(outcome, crate::ReplicaSyncOutcome::Settled);
    server.verify().await;
    let s1: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let s2: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 's2'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(s1, "Fallback One");
    assert_eq!(s2, "Fallback Tail");
}

#[tokio::test]
async fn replica_sync_with_healthy_wait_uploads_local_edit_before_heartbeat() {
    let server = MockServer::start().await;
    let get_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let post_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let get_calls_ref = Arc::clone(&get_calls);
    Mock::given(method("GET"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(move |_req: &Request| {
            get_calls_ref.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            ResponseTemplate::new(200).set_body_json(json!({
                "initialized": false,
                "initializedAt": null,
                "headSequence": 0,
                "throughSequence": 0,
                "nextAfterSequence": 0,
                "cloudAuthorityAfter": null,
                "events": []
            }))
        })
        .mount(&server)
        .await;
    let post_calls_ref = Arc::clone(&post_calls);
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/witness/user-a/accepted"))
        .respond_with(move |req: &Request| {
            post_calls_ref.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let events = body["events"].as_array().unwrap();
            let receipts: Vec<_> = events
                .iter()
                .enumerate()
                .map(|(i, event)| {
                    json!({
                        "sequence": i as u64 + 1,
                        "recordId": event["recordId"],
                        "payloadHash": event["payloadHash"],
                    })
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "initializedAt": "2026-10-05T00:00:00Z",
                "headSequence": receipts.len() as u64,
                "cloudAuthorityAfter": 0,
                "mutationId": body["mutationId"],
                "receipts": receipts,
            }))
        })
        .mount(&server)
        .await;

    let recovery_key = anlg_e2ee::RecoveryKey::parse(
        "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(
        anlg_db_core::Db::connect_local_plain(&dir.path().join("local.db"))
            .await
            .unwrap(),
    );
    anlg_db_app::prepare_schema(&db).await.unwrap();
    let hook = Arc::new(crate::E2eeSyncHook::default());
    hook.set_personal_workspace("user-a", &recovery_key)
        .unwrap();
    hook.set_replica_witness(
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "token".to_string(),
            },
            "user-a",
        )
        .unwrap(),
    );
    hook.set_witness_watch_healthy(true);

    let _task = crate::replica_sync::spawn_replica_sync_with_intervals(
        Arc::clone(&db),
        Arc::clone(&hook),
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(300),
    );

    // Wait for initial sync request triggered by `set_replica_witness`.
    let initial_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    while get_calls.load(std::sync::atomic::Ordering::Acquire) == 0 {
        assert!(tokio::time::Instant::now() < initial_deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let settled_gets = get_calls.load(std::sync::atomic::Ordering::Acquire);
    assert_eq!(post_calls.load(std::sync::atomic::Ordering::Acquire), 0);

    // Confirm idle heartbeat emits zero network calls over 400ms.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(
        get_calls.load(std::sync::atomic::Ordering::Acquire),
        settled_gets
    );

    // Insert a local edit WITHOUT calling `request_replica_sync()`. Local-work check must
    // trigger an upload well before the 300s heartbeat.
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES('s-fast', 'user-a', 'user-a', 'Fast Upload')")
        .execute(db.pool())
        .await
        .unwrap();

    let upload_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(6);
    while post_calls.load(std::sync::atomic::Ordering::Acquire) == 0 {
        assert!(
            tokio::time::Instant::now() < upload_deadline,
            "local edit should upload well before 300s heartbeat"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
