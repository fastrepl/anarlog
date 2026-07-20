use std::io;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

const MAX_EVENTS_PER_BATCH: usize = 16;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_BATCH_BYTES: usize = 48 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_RATE_LIMIT_RETRIES: usize = 3;
const DEFAULT_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(30);
const MAX_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(60);
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Clone)]
pub(crate) struct E2eeWitnessClient {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    access_token: String,
    workspace_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishRequest<'a> {
    initialize: bool,
    events: Vec<PublishEvent<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishEvent<'a> {
    record_id: &'a str,
    payload_hash: &'a str,
    payload: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishResponse {
    initialized_at: String,
    head_sequence: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadPage {
    initialized: bool,
    initialized_at: Option<String>,
    head_sequence: u64,
    through_sequence: u64,
    next_after_sequence: u64,
    events: Vec<ReadEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadEvent {
    sequence: u64,
    record_id: String,
    payload_hash: String,
    payload: String,
}

impl E2eeWitnessClient {
    pub(crate) fn new(config: crate::CloudsyncE2eeWitness, workspace_id: &str) -> io::Result<Self> {
        let endpoint = reqwest::Url::parse(&config.endpoint)
            .map_err(|_| invalid_data("E2EE witness endpoint is invalid"))?;
        if !matches!(endpoint.scheme(), "https" | "http")
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path_segments().and_then(Iterator::last) != Some(workspace_id)
            || config.access_token.is_empty()
        {
            return Err(invalid_data("E2EE witness configuration is invalid"));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| io::Error::other(format!("E2EE witness client failed: {error}")))?;
        Ok(Self {
            client,
            endpoint,
            access_token: config.access_token,
            workspace_id: workspace_id.to_string(),
        })
    }

    pub(crate) fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub(crate) async fn initialize(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let status = self.read_page(cursor, None).await?;
        self.validate_page(&status, cursor, None)?;
        if status.head_sequence < cursor {
            return Err(rollback_error());
        }

        if status.initialized {
            self.publish_pending(pool, key, false).await?;
        } else {
            if !hypr_db_app::has_e2ee_local_state(pool, &self.workspace_id)
                .await
                .map_err(replica_error)?
            {
                return Err(io::Error::other(
                    "E2EE freshness witness must be initialized from an existing trusted device",
                ));
            }
            self.publish_pending(pool, key, true).await?;
        }

        self.refresh(pool, key).await
    }

    pub(crate) async fn publish_and_refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        self.publish_pending(pool, key, false).await?;
        self.refresh(pool, key).await
    }

    pub(crate) async fn refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let mut page = self.read_page(cursor, None).await?;
        self.validate_page(&page, cursor, None)?;
        if !page.initialized {
            return Err(io::Error::other(
                "E2EE freshness witness is not initialized",
            ));
        }
        if page.head_sequence < cursor {
            return Err(rollback_error());
        }

        let through = page.through_sequence;
        loop {
            let events = page
                .events
                .into_iter()
                .map(|event| hypr_db_app::E2eeWitnessEvent {
                    sequence: event.sequence,
                    record_id: event.record_id,
                    workspace_id: self.workspace_id.clone(),
                    payload_hash: event.payload_hash,
                    payload: event.payload,
                })
                .collect::<Vec<_>>();
            hypr_db_app::merge_e2ee_witness_events(pool, key, &self.workspace_id, &events)
                .await
                .map_err(replica_error)?;
            let after = page.next_after_sequence;
            hypr_db_app::advance_e2ee_witness_cursor(pool, &self.workspace_id, after)
                .await
                .map_err(replica_error)?;
            if after == through {
                break;
            }
            if after >= through {
                return Err(invalid_data("E2EE witness page cursor is invalid"));
            }
            page = self.read_page(after, Some(through)).await?;
            self.validate_page(&page, after, Some(through))?;
        }
        Ok(())
    }

    async fn publish_pending(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        initialize: bool,
    ) -> io::Result<()> {
        let uploads = hypr_db_app::pending_e2ee_witness_uploads(pool, &self.workspace_id, key)
            .await
            .map_err(replica_error)?;
        if initialize && uploads.is_empty() {
            return Err(io::Error::other(
                "E2EE freshness initialization requires established encrypted state",
            ));
        }
        let cursor = witness_cursor(pool, &self.workspace_id).await?;
        let mut start = 0;
        while start < uploads.len() {
            let mut end = start;
            let mut batch_bytes = 0usize;
            while end < uploads.len() && end - start < MAX_EVENTS_PER_BATCH {
                let event_bytes = uploads[end]
                    .payload
                    .len()
                    .saturating_add(uploads[end].record_id.len())
                    .saturating_add(uploads[end].payload_hash.len())
                    .saturating_add(256);
                if uploads[end].payload.len() > MAX_EVENT_BYTES {
                    return Err(invalid_data("E2EE witness event is too large"));
                }
                if end > start && batch_bytes.saturating_add(event_bytes) > MAX_BATCH_BYTES {
                    break;
                }
                batch_bytes = batch_bytes.saturating_add(event_bytes);
                end += 1;
            }
            let batch = &uploads[start..end];
            let response = self
                .send_with_rate_limit_retry(|| {
                    self.client
                        .post(self.endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .json(&PublishRequest {
                            initialize: initialize && start == 0,
                            events: batch
                                .iter()
                                .map(|upload| PublishEvent {
                                    record_id: &upload.record_id,
                                    payload_hash: &upload.payload_hash,
                                    payload: &upload.payload,
                                })
                                .collect(),
                        })
                })
                .await?;
            let status = response.status();
            let bytes = read_bounded(response).await?;
            if !status.is_success() {
                return Err(io::Error::other(format!(
                    "E2EE witness publication was rejected with status {status}"
                )));
            }
            let response: PublishResponse = serde_json::from_slice(&bytes)
                .map_err(|_| invalid_data("E2EE witness publication response is invalid"))?;
            if response.initialized_at.is_empty() || response.head_sequence < cursor {
                return Err(rollback_error());
            }
            hypr_db_app::acknowledge_e2ee_witness_uploads(pool, key, batch)
                .await
                .map_err(replica_error)?;
            start = end;
        }
        Ok(())
    }

    async fn read_page(&self, after: u64, through: Option<u64>) -> io::Result<ReadPage> {
        let response = self
            .send_with_rate_limit_retry(|| {
                let mut request = self
                    .client
                    .get(self.endpoint.clone())
                    .bearer_auth(&self.access_token)
                    .query(&[("afterSequence", after)]);
                if let Some(through) = through {
                    request = request.query(&[("throughSequence", through)]);
                }
                request
            })
            .await?;
        let status = response.status();
        let bytes = read_bounded(response).await?;
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE witness read was rejected with status {status}"
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("E2EE witness read response is invalid"))
    }

    async fn send_with_rate_limit_retry(
        &self,
        request: impl Fn() -> reqwest::RequestBuilder,
    ) -> io::Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let response = request().send().await.map_err(transport_error)?;
            if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS
                || retries == MAX_RATE_LIMIT_RETRIES
            {
                return Ok(response);
            }
            let delay = retry_after_delay(response.headers());
            read_bounded(response).await?;
            tokio::time::sleep(delay).await;
            retries += 1;
        }
    }

    fn validate_page(
        &self,
        page: &ReadPage,
        requested_after: u64,
        requested_through: Option<u64>,
    ) -> io::Result<()> {
        if page.initialized != page.initialized_at.is_some()
            || page.through_sequence > page.head_sequence
            || requested_after > page.through_sequence
            || requested_through.is_some_and(|through| through != page.through_sequence)
            || page.next_after_sequence < requested_after
            || page.next_after_sequence > page.through_sequence
            || (page.events.is_empty() && page.next_after_sequence != requested_after)
            || (page.events.is_empty() && requested_after != page.through_sequence)
            || page
                .events
                .last()
                .is_some_and(|event| event.sequence != page.next_after_sequence)
        {
            return Err(invalid_data("E2EE witness page is invalid"));
        }
        let mut previous = requested_after;
        for event in &page.events {
            if event.sequence <= previous
                || event.sequence > page.through_sequence
                || event.payload.is_empty()
                || event.payload.len() > MAX_EVENT_BYTES
            {
                return Err(invalid_data("E2EE witness event is invalid"));
            }
            previous = event.sequence;
        }
        Ok(())
    }
}

async fn witness_cursor(pool: &sqlx::SqlitePool, workspace_id: &str) -> io::Result<u64> {
    hypr_db_app::e2ee_witness_cursor(pool, workspace_id)
        .await
        .map_err(replica_error)
}

async fn read_bounded(response: reqwest::Response) -> io::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid_data("E2EE witness response is too large"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport_error)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(invalid_data("E2EE witness response is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn replica_error(error: hypr_db_app::E2eeReplicaError) -> io::Error {
    io::Error::other(format!("E2EE witness state failed: {error}"))
}

fn transport_error(error: reqwest::Error) -> io::Error {
    io::Error::other(format!("E2EE witness request failed: {error}"))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn rollback_error() -> io::Error {
    io::Error::other("E2EE freshness witness rollback was detected")
}

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> std::time::Duration {
    let seconds = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    match seconds {
        None => DEFAULT_RETRY_AFTER,
        Some(0) => std::time::Duration::ZERO,
        Some(seconds) => std::time::Duration::from_secs(seconds)
            .saturating_add(std::time::Duration::from_secs(1))
            .min(MAX_RETRY_AFTER),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
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
            crate::CloudsyncE2eeWitness {
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
    async fn stops_retrying_a_persistently_rate_limited_read() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .expect(4)
            .mount(&server)
            .await;
        let client = E2eeWitnessClient::new(
            crate::CloudsyncE2eeWitness {
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
        let db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&dir.path().join("app.db")),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', 'user-a', 'user-a', 'Session')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        let key = recovery_key.workspace_key("user-a").unwrap();
        hypr_db_app::encrypt_e2ee_replica_changes(
            db.pool(),
            &HashMap::from([("user-a".to_string(), key.clone())]),
        )
        .await
        .unwrap();
        let uploads = hypr_db_app::pending_e2ee_witness_uploads(db.pool(), "user-a", &key)
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
            crate::CloudsyncE2eeWitness {
                endpoint: format!("{}/sync/e2ee/witness/user-a", server.uri()),
                access_token: "access-token".to_string(),
            },
            "user-a",
        )
        .unwrap();

        assert!(client.refresh(db.pool(), &key).await.is_err());
        assert_eq!(
            hypr_db_app::e2ee_witness_cursor(db.pool(), "user-a")
                .await
                .unwrap(),
            3
        );

        client.refresh(db.pool(), &key).await.unwrap();

        assert_eq!(
            hypr_db_app::e2ee_witness_cursor(db.pool(), "user-a")
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
}
