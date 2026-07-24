use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

#[derive(Clone, Default)]
pub(crate) struct E2eeWitnessCancellation {
    state: Arc<E2eeWitnessCancellationState>,
}

#[derive(Default)]
struct E2eeWitnessCancellationState {
    cancelled: AtomicBool,
    changed: tokio::sync::Notify,
}

impl E2eeWitnessCancellation {
    pub(crate) fn cancel(&self) {
        if !self.state.cancelled.swap(true, Ordering::AcqRel) {
            self.state.changed.notify_waiters();
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        if self.is_cancelled() {
            Err(cancelled_error())
        } else {
            Ok(())
        }
    }

    async fn cancelled(&self) {
        loop {
            let changed = self.state.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            changed.await;
        }
    }

    async fn run_network<T>(&self, future: impl Future<Output = T>) -> io::Result<T> {
        self.check()?;
        tokio::pin!(future);
        tokio::select! {
            biased;
            _ = self.cancelled() => Err(cancelled_error()),
            result = &mut future => Ok(result),
        }
    }
}

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

    #[cfg(test)]
    pub(crate) async fn initialize(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<()> {
        self.initialize_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn initialize_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let status = self
            .read_page_cancellable(cursor, None, cancellation)
            .await?;
        self.validate_page(&status, cursor, None)?;
        if status.head_sequence < cursor {
            return Err(rollback_error());
        }

        if status.initialized {
            self.refresh_cancellable(pool, key, cancellation).await?;
            self.publish_pending(pool, key, false, cancellation).await?;
        } else {
            cancellation.check()?;
            let has_local_state = hypr_db_app::has_e2ee_local_state(pool, &self.workspace_id)
                .await
                .map_err(replica_error)?;
            cancellation.check()?;
            if !has_local_state {
                return Err(io::Error::other(
                    "E2EE freshness witness must be initialized from an existing trusted device",
                ));
            }
            self.publish_pending(pool, key, true, cancellation).await?;
        }

        self.refresh_cancellable(pool, key, cancellation)
            .await
            .map(|_| ())
    }

    #[cfg(test)]
    pub(crate) async fn publish_and_refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<usize> {
        self.publish_and_refresh_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn publish_and_refresh_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        self.publish_pending(pool, key, false, cancellation).await?;
        self.refresh_cancellable(pool, key, cancellation).await
    }

    pub(crate) async fn publish_and_refresh_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        self.publish_pending(pool, key, false, cancellation).await?;
        self.refresh_notifying_cancellable(pool, key, on_events, cancellation)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn refresh(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
    ) -> io::Result<usize> {
        self.refresh_cancellable(pool, key, &E2eeWitnessCancellation::default())
            .await
    }

    pub(crate) async fn refresh_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize> {
        self.refresh_notifying_cancellable(pool, key, || {}, cancellation)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn refresh_notifying<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        on_events: F,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        self.refresh_notifying_cancellable(
            pool,
            key,
            on_events,
            &E2eeWitnessCancellation::default(),
        )
        .await
    }

    pub(crate) async fn refresh_notifying_cancellable<F>(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        mut on_events: F,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<usize>
    where
        F: FnMut(),
    {
        let mut cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let mut page = self
            .read_page_cancellable(cursor, None, cancellation)
            .await?;
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
        let mut received_events = 0_usize;
        loop {
            received_events = received_events.saturating_add(page.events.len());
            if !page.events.is_empty() {
                on_events();
            }
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
            cancellation.check()?;
            hypr_db_app::merge_e2ee_witness_events_cancellable(
                pool,
                key,
                &self.workspace_id,
                &events,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            let after = page.next_after_sequence;
            if after != cursor {
                cancellation.check()?;
                hypr_db_app::advance_e2ee_witness_cursor(pool, &self.workspace_id, after)
                    .await
                    .map_err(replica_error)?;
                cancellation.check()?;
                cursor = after;
            }
            if after == through {
                break;
            }
            if after >= through {
                return Err(invalid_data("E2EE witness page cursor is invalid"));
            }
            page = self
                .read_page_cancellable(after, Some(through), cancellation)
                .await?;
            self.validate_page(&page, after, Some(through))?;
        }
        Ok(received_events)
    }

    async fn publish_pending(
        &self,
        pool: &sqlx::SqlitePool,
        key: &hypr_e2ee::WorkspaceKey,
        initialize: bool,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let mut first_batch = true;
        loop {
            cancellation.check()?;
            let uploads = hypr_db_app::pending_e2ee_witness_uploads_cancellable(
                pool,
                &self.workspace_id,
                key,
                MAX_EVENTS_PER_BATCH,
                MAX_BATCH_BYTES,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            if uploads.is_empty() {
                if initialize && first_batch {
                    return Err(io::Error::other(
                        "E2EE freshness initialization requires established encrypted state",
                    ));
                }
                return Ok(());
            }

            let mut batch_bytes = 0usize;
            for upload in &uploads {
                cancellation.check()?;
                let event_bytes = upload
                    .payload
                    .len()
                    .saturating_add(upload.record_id.len())
                    .saturating_add(upload.payload_hash.len())
                    .saturating_add(256);
                if upload.payload.len() > MAX_EVENT_BYTES {
                    return Err(invalid_data("E2EE witness event is too large"));
                }
                if batch_bytes.saturating_add(event_bytes) > MAX_BATCH_BYTES {
                    return Err(invalid_data("E2EE witness batch is too large"));
                }
                batch_bytes = batch_bytes.saturating_add(event_bytes);
            }
            let response = self
                .send_with_rate_limit_retry(
                    || {
                        self.client
                            .post(self.endpoint.clone())
                            .bearer_auth(&self.access_token)
                            .json(&PublishRequest {
                                initialize: initialize && first_batch,
                                events: uploads
                                    .iter()
                                    .map(|upload| PublishEvent {
                                        record_id: &upload.record_id,
                                        payload_hash: &upload.payload_hash,
                                        payload: &upload.payload,
                                    })
                                    .collect(),
                            })
                    },
                    cancellation,
                )
                .await?;
            let status = response.status();
            let bytes = cancellation.run_network(read_bounded(response)).await??;
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
            cancellation.check()?;
            hypr_db_app::acknowledge_e2ee_witness_uploads_cancellable(pool, key, &uploads, || {
                cancellation.is_cancelled()
            })
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            first_batch = false;
        }
    }

    #[cfg(test)]
    async fn read_page(&self, after: u64, through: Option<u64>) -> io::Result<ReadPage> {
        self.read_page_cancellable(after, through, &E2eeWitnessCancellation::default())
            .await
    }

    async fn read_page_cancellable(
        &self,
        after: u64,
        through: Option<u64>,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<ReadPage> {
        let response = self
            .send_with_rate_limit_retry(
                || {
                    let mut request = self
                        .client
                        .get(self.endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .query(&[("afterSequence", after)]);
                    if let Some(through) = through {
                        request = request.query(&[("throughSequence", through)]);
                    }
                    request
                },
                cancellation,
            )
            .await?;
        let status = response.status();
        let bytes = cancellation.run_network(read_bounded(response)).await??;
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
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let response = cancellation
                .run_network(request().send())
                .await?
                .map_err(transport_error)?;
            if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS
                || retries == MAX_RATE_LIMIT_RETRIES
            {
                return Ok(response);
            }
            let delay = retry_after_delay(response.headers());
            cancellation.run_network(read_bounded(response)).await??;
            cancellation.run_network(tokio::time::sleep(delay)).await?;
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

async fn witness_cursor_cancellable(
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    cancellation: &E2eeWitnessCancellation,
) -> io::Result<u64> {
    cancellation.check()?;
    let cursor = witness_cursor(pool, workspace_id).await?;
    cancellation.check()?;
    Ok(cursor)
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
    if matches!(&error, hypr_db_app::E2eeReplicaError::Cancelled) {
        cancelled_error()
    } else {
        io::Error::other(format!("E2EE witness state failed: {error}"))
    }
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

fn cancelled_error() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "E2EE witness request cancelled")
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
    fn cancelled_replica_work_is_reported_as_an_interrupted_witness_operation() {
        assert_eq!(
            replica_error(hypr_db_app::E2eeReplicaError::Cancelled).kind(),
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
    async fn cancellation_stops_a_stalled_witness_request_promptly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(witness_page(&[], 0, 0).set_delay(std::time::Duration::from_secs(120)))
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
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
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
            "payloadHash": hypr_e2ee::payload_hash(&sealed.payload),
            "payload": sealed.payload,
        });
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sync/e2ee/witness/user-a"))
            .respond_with(witness_page(&[event], 1, 1))
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
            hypr_db_app::e2ee_witness_cursor(db.pool(), "user-a")
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
            crate::CloudsyncE2eeWitness {
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
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
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
            crate::CloudsyncE2eeWitness {
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
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
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

        let server = MockServer::start().await;
        let responder = RequestOrder::default();
        Mock::given(path("/sync/e2ee/witness/user-a"))
            .respond_with(responder.clone())
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
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session', 'user-a', 'user-a', 'Before')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let recovery_key = hypr_e2ee::RecoveryKey::parse(
            "anarlog-e2ee-v1:BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc",
        )
        .unwrap();
        let key = recovery_key.workspace_key("user-a").unwrap();
        let keys = HashMap::from([("user-a".to_string(), key.clone())]);
        hypr_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
            .await
            .unwrap();
        loop {
            let uploads = hypr_db_app::pending_e2ee_witness_uploads(
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
            hypr_db_app::acknowledge_e2ee_witness_uploads(db.pool(), &key, &uploads)
                .await
                .unwrap();
        }

        sqlx::query("UPDATE sessions SET title = 'After' WHERE id = 'session'")
            .execute(db.pool())
            .await
            .unwrap();
        hypr_db_app::encrypt_e2ee_replica_changes(db.pool(), &keys)
            .await
            .unwrap();

        let server = MockServer::start().await;
        let responder = FailFirstPublish::default();
        Mock::given(path("/sync/e2ee/witness/user-a"))
            .respond_with(responder.clone())
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

        assert!(client.publish_and_refresh(db.pool(), &key).await.is_err());
        let queued_after_failure: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(queued_after_failure > 0);
        assert!(
            !hypr_db_app::pending_e2ee_witness_uploads(
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
        let queued_after_retry: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_witness_pending")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(queued_after_retry, 0);
        assert!(
            hypr_db_app::pending_e2ee_witness_uploads(
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
        let uploads = hypr_db_app::pending_e2ee_witness_uploads(
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
            crate::CloudsyncE2eeWitness {
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
            hypr_db_app::e2ee_witness_cursor(db.pool(), "user-a")
                .await
                .unwrap(),
            3
        );

        assert_eq!(client.refresh(db.pool(), &key).await.unwrap(), 1);

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
