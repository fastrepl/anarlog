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
