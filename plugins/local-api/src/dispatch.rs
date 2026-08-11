use std::future::Future;

use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use sqlx::SqlitePool;

pub const EVENT_MEETING_COMPLETED: &str = "meeting.completed";
pub const EVENT_NOTE_ENHANCED: &str = "note.enhanced";
pub const EVENT_TEST: &str = "webhook.test";
pub const KNOWN_EVENTS: &[&str] = &[EVENT_MEETING_COMPLETED, EVENT_NOTE_ENHANCED];

const DELIVERY_TIMEOUT_SECS: u64 = 10;
const RETRY_DELAYS_SECS: &[u64] = &[0, 5, 30];
const MAX_STATUS_LEN: usize = 200;
const MAX_CONCURRENT_DELIVERIES: usize = 4;
const MAX_CONCURRENT_FANOUTS: usize = 4;
const MAX_PENDING_FANOUTS: usize = 32;
const MAX_PENDING_TEST_DELIVERIES: usize = 32;
const FANOUT_BUSY_ERROR: &str = "webhook fanout is busy; try again";
const DELIVERY_BUSY_ERROR: &str = "webhook delivery is busy; try again";

pub(crate) const MAX_WEBHOOK_ENDPOINTS: usize = 64;

static CREATE_ENDPOINT_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();

static DELIVERY_SLOTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();
static DELIVERY_WAITING_SLOTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();
static FANOUT_SLOTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();
static FANOUT_WAITING_SLOTS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();

fn delivery_slots() -> std::sync::Arc<tokio::sync::Semaphore> {
    DELIVERY_SLOTS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DELIVERIES)))
        .clone()
}

fn fanout_slots() -> std::sync::Arc<tokio::sync::Semaphore> {
    FANOUT_SLOTS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_FANOUTS)))
        .clone()
}

fn delivery_waiting_slots() -> std::sync::Arc<tokio::sync::Semaphore> {
    DELIVERY_WAITING_SLOTS
        .get_or_init(|| {
            std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_PENDING_TEST_DELIVERIES))
        })
        .clone()
}

fn fanout_waiting_slots() -> std::sync::Arc<tokio::sync::Semaphore> {
    FANOUT_WAITING_SLOTS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_PENDING_FANOUTS)))
        .clone()
}

async fn acquire_with_waiting_tier(
    active: std::sync::Arc<tokio::sync::Semaphore>,
    waiting: std::sync::Arc<tokio::sync::Semaphore>,
    busy_error: &'static str,
) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
    let waiting_permit = waiting
        .try_acquire_owned()
        .map_err(|_| busy_error.to_string())?;
    let active_permit = active
        .acquire_owned()
        .await
        .map_err(|_| busy_error.to_string())?;
    drop(waiting_permit);
    Ok(active_permit)
}

async fn acquire_fanout_slot() -> Result<tokio::sync::OwnedSemaphorePermit, String> {
    acquire_with_waiting_tier(fanout_slots(), fanout_waiting_slots(), FANOUT_BUSY_ERROR).await
}

async fn acquire_test_delivery_slot() -> Result<tokio::sync::OwnedSemaphorePermit, String> {
    acquire_with_waiting_tier(
        delivery_slots(),
        delivery_waiting_slots(),
        DELIVERY_BUSY_ERROR,
    )
    .await
}

#[cfg(test)]
pub fn sign_payload(secret: &str, body: &str) -> String {
    sign_payload_bytes(secret, body.as_bytes())
}

fn sign_payload_bytes(secret: &str, body: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("hmac accepts keys of any length");
    mac.update(body);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn subscribes(endpoint: &anlg_db_app::WebhookEndpointRow, event: &str) -> bool {
    let events: Vec<String> = serde_json::from_str(&endpoint.events_json).unwrap_or_default();
    events.is_empty() || events.iter().any(|subscribed| subscribed == event)
}

fn envelope(event: &str, data: serde_json::Value) -> String {
    serde_json::json!({
        "id": format!("evt_{}", uuid::Uuid::new_v4().simple()),
        "event": event,
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "data": data,
    })
    .to_string()
}

async fn meeting_payload(pool: &SqlitePool, meeting_id: &str) -> Result<serde_json::Value, String> {
    let export = anlg_agent_access::get_meeting_export(pool, meeting_id.to_string())
        .await
        .map_err(|error| error.to_string())?;
    let transcript_text = export
        .transcripts
        .iter()
        .map(|transcript| transcript.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(serde_json::json!({
        "meeting": export.meeting,
        "transcript_text": transcript_text,
    }))
}

pub async fn create_endpoint(
    pool: &SqlitePool,
    url: &str,
    events: &[String],
) -> Result<crate::CreatedWebhook, String> {
    let _guard = CREATE_ENDPOINT_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("url must start with http:// or https://".to_string());
    }
    if let Some(unknown) = events
        .iter()
        .find(|event| !KNOWN_EVENTS.contains(&event.as_str()))
    {
        return Err(format!(
            "unknown event '{unknown}'; known events: {}",
            KNOWN_EVENTS.join(", ")
        ));
    }
    let endpoint_count = anlg_db_app::list_webhook_endpoints(pool)
        .await
        .map_err(|error| error.to_string())?
        .len();
    if endpoint_count >= MAX_WEBHOOK_ENDPOINTS {
        return Err(format!(
            "at most {MAX_WEBHOOK_ENDPOINTS} webhook endpoints can be configured"
        ));
    }

    let events_json = serde_json::to_string(events).map_err(|error| error.to_string())?;
    let secret = anlg_db_app::generate_webhook_secret();
    let row = anlg_db_app::insert_webhook_endpoint(
        pool,
        &uuid::Uuid::new_v4().to_string(),
        url,
        &secret,
        &events_json,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(crate::CreatedWebhook {
        info: crate::WebhookInfo::from(row),
        secret,
    })
}

/// Fans an event out to every active endpoint subscribed to it. Deliveries run
/// in background tasks; the returned count is the number of endpoints targeted.
pub async fn dispatch_event(
    pool: &SqlitePool,
    event: &str,
    meeting_id: &str,
) -> Result<usize, String> {
    let fanout_permit = acquire_fanout_slot().await?;
    let mut endpoints = anlg_db_app::list_active_webhook_endpoints(pool)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|endpoint| subscribes(endpoint, event))
        .collect::<Vec<_>>();
    if endpoints.len() > MAX_WEBHOOK_ENDPOINTS {
        tracing::warn!(
            "[local-api] limiting webhook fanout from {} to {} endpoints",
            endpoints.len(),
            MAX_WEBHOOK_ENDPOINTS
        );
        endpoints.truncate(MAX_WEBHOOK_ENDPOINTS);
    }
    if endpoints.is_empty() {
        return Ok(0);
    }

    let body = Bytes::from(envelope(event, meeting_payload(pool, meeting_id).await?));
    let targeted = endpoints.len();
    let pool = pool.clone();
    let event = std::sync::Arc::<str>::from(event);
    let delivery_slots = delivery_slots();
    tokio::spawn(async move {
        let _fanout_permit = fanout_permit;
        run_bounded(endpoints, MAX_CONCURRENT_DELIVERIES, move |endpoint| {
            let pool = pool.clone();
            let event = event.clone();
            let body = body.clone();
            let delivery_slots = delivery_slots.clone();
            async move {
                let Ok(_permit) = delivery_slots.acquire_owned().await else {
                    return;
                };
                deliver_with_retry(&pool, &endpoint, &event, &body).await;
            }
        })
        .await;
    });
    Ok(targeted)
}

async fn run_bounded<T, F, Fut>(items: impl IntoIterator<Item = T>, limit: usize, run: F)
where
    T: Send + 'static,
    F: Fn(T) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut items = items.into_iter();
    let mut tasks = tokio::task::JoinSet::new();

    loop {
        while tasks.len() < limit {
            let Some(item) = items.next() else {
                break;
            };
            tasks.spawn(run.clone()(item));
        }

        let Some(result) = tasks.join_next().await else {
            break;
        };
        if let Err(error) = result {
            tracing::warn!("[local-api] webhook delivery task failed: {error}");
        }
    }
}

pub async fn send_test(
    pool: &SqlitePool,
    endpoint: &anlg_db_app::WebhookEndpointRow,
) -> Result<crate::WebhookDelivery, String> {
    let _permit = acquire_test_delivery_slot().await?;
    let body = Bytes::from(envelope(
        EVENT_TEST,
        serde_json::json!({ "message": "This is a test delivery from Anarlog." }),
    ));
    let delivery_id = format!("dlv_{}", uuid::Uuid::new_v4().simple());
    let status = deliver_once(endpoint, EVENT_TEST, &body, &delivery_id).await;
    let delivered = status.delivered;
    let status_text = truncate_status(&status.status);
    if let Err(error) = anlg_db_app::record_webhook_delivery(pool, &endpoint.id, &status_text).await
    {
        tracing::warn!("[local-api] failed to record webhook delivery: {error}");
    }
    Ok(crate::WebhookDelivery {
        delivered,
        status: status_text,
    })
}

struct DeliveryOutcome {
    delivered: bool,
    status: String,
}

async fn deliver_with_retry(
    pool: &SqlitePool,
    endpoint: &anlg_db_app::WebhookEndpointRow,
    event: &str,
    body: &Bytes,
) {
    let delivery_id = format!("dlv_{}", uuid::Uuid::new_v4().simple());
    let mut outcome = DeliveryOutcome {
        delivered: false,
        status: "not attempted".to_string(),
    };
    for delay in RETRY_DELAYS_SECS {
        if *delay > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(*delay)).await;
        }
        outcome = deliver_once(endpoint, event, body, &delivery_id).await;
        if outcome.delivered {
            break;
        }
    }

    if !outcome.delivered {
        tracing::warn!(
            "[local-api] webhook delivery to {} failed: {}",
            endpoint.url,
            outcome.status
        );
    }
    let status_text = truncate_status(&outcome.status);
    if let Err(error) = anlg_db_app::record_webhook_delivery(pool, &endpoint.id, &status_text).await
    {
        tracing::warn!("[local-api] failed to record webhook delivery: {error}");
    }
}

async fn deliver_once(
    endpoint: &anlg_db_app::WebhookEndpointRow,
    event: &str,
    body: &Bytes,
    delivery_id: &str,
) -> DeliveryOutcome {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(DELIVERY_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return DeliveryOutcome {
                delivered: false,
                status: format!("error: {error}"),
            };
        }
    };

    let response = client
        .post(&endpoint.url)
        .header("content-type", "application/json")
        .header("x-anarlog-event", event)
        .header("x-anarlog-delivery", delivery_id)
        .header(
            "x-anarlog-timestamp",
            chrono::Utc::now().timestamp().to_string(),
        )
        .header(
            "x-anarlog-signature",
            format!(
                "sha256={}",
                sign_payload_bytes(&endpoint.secret, body.as_ref())
            ),
        )
        .body(body.clone())
        .send()
        .await;

    match response {
        Ok(response) => DeliveryOutcome {
            delivered: response.status().is_success(),
            status: response.status().to_string(),
        },
        Err(error) => DeliveryOutcome {
            delivered: false,
            status: format!("error: {error}"),
        },
    }
}

fn truncate_status(status: &str) -> String {
    status.chars().take(MAX_STATUS_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn signature_matches_reference_hmac() {
        // Precomputed with python hmac: key=whsec_test, message={"a":1}
        assert_eq!(
            sign_payload("whsec_test", "{\"a\":1}"),
            "51426af50a41dd7ff2cd3f116594734766d4018d15d6fb07169aee5d2959adf5"
        );
    }

    #[test]
    fn empty_event_list_subscribes_to_everything() {
        let endpoint = anlg_db_app::WebhookEndpointRow {
            id: "webhook-1".to_string(),
            url: "https://example.com".to_string(),
            secret: "whsec_x".to_string(),
            events_json: "[]".to_string(),
            active: true,
            created_at: String::new(),
            last_delivery_at: None,
            last_delivery_status: String::new(),
        };
        assert!(subscribes(&endpoint, EVENT_NOTE_ENHANCED));

        let scoped = anlg_db_app::WebhookEndpointRow {
            events_json: "[\"meeting.completed\"]".to_string(),
            ..endpoint
        };
        assert!(subscribes(&scoped, EVENT_MEETING_COMPLETED));
        assert!(!subscribes(&scoped, EVENT_NOTE_ENHANCED));
    }

    #[tokio::test]
    async fn dispatch_without_subscribed_endpoints_is_a_no_op() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        anlg_db_app::insert_webhook_endpoint(
            db.pool(),
            "webhook-1",
            "https://example.com",
            "whsec_test",
            "[\"note.enhanced\"]",
        )
        .await
        .unwrap();

        let targeted = dispatch_event(db.pool(), EVENT_MEETING_COMPLETED, "missing-meeting")
            .await
            .unwrap();

        assert_eq!(targeted, 0);
    }

    #[tokio::test]
    async fn endpoint_creation_stops_at_the_fanout_limit() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();

        for index in 0..MAX_WEBHOOK_ENDPOINTS {
            anlg_db_app::insert_webhook_endpoint(
                db.pool(),
                &format!("webhook-{index}"),
                "https://example.com",
                "whsec_test",
                "[]",
            )
            .await
            .unwrap();
        }

        let error = create_endpoint(db.pool(), "https://example.com", &[])
            .await
            .unwrap_err();

        assert_eq!(
            error,
            format!("at most {MAX_WEBHOOK_ENDPOINTS} webhook endpoints can be configured")
        );
        assert_eq!(
            anlg_db_app::list_webhook_endpoints(db.pool())
                .await
                .unwrap()
                .len(),
            MAX_WEBHOOK_ENDPOINTS
        );
    }

    #[tokio::test]
    async fn endpoint_creation_rejects_bad_urls_and_unknown_events() {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();

        assert_eq!(
            create_endpoint(db.pool(), "example.com", &[])
                .await
                .unwrap_err(),
            "url must start with http:// or https://"
        );
        assert!(
            create_endpoint(db.pool(), "https://example.com", &["nope".to_string()])
                .await
                .unwrap_err()
                .starts_with("unknown event 'nope'")
        );
        assert!(
            anlg_db_app::list_webhook_endpoints(db.pool())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn bounded_runner_limits_concurrency_without_dropping_items() {
        let active = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        run_bounded(0..100, 4, {
            let active = active.clone();
            let completed = completed.clone();
            let peak = peak.clone();
            move |_| {
                let active = active.clone();
                let completed = completed.clone();
                let peak = peak.clone();
                async move {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .await;

        assert_eq!(completed.load(Ordering::SeqCst), 100);
        assert!(peak.load(Ordering::SeqCst) <= 4);
    }

    #[test]
    fn cloned_delivery_bodies_share_storage() {
        let body = Bytes::from(vec![b'x'; 1024 * 1024]);
        let clone = body.clone();

        assert_eq!(body.as_ptr(), clone.as_ptr());
        assert_eq!(body.len(), clone.len());
    }

    #[tokio::test]
    async fn bounded_admission_queues_a_limited_waiting_tier() {
        let active = Arc::new(tokio::sync::Semaphore::new(1));
        let waiting = Arc::new(tokio::sync::Semaphore::new(2));
        let active_guard = active.clone().acquire_owned().await.unwrap();

        let first = tokio::spawn(acquire_with_waiting_tier(
            active.clone(),
            waiting.clone(),
            "busy",
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while waiting.available_permits() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let second = tokio::spawn(acquire_with_waiting_tier(
            active.clone(),
            waiting.clone(),
            "busy",
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while waiting.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        assert_eq!(
            acquire_with_waiting_tier(active.clone(), waiting.clone(), "busy")
                .await
                .unwrap_err(),
            "busy"
        );

        drop(active_guard);
        let first_permit = tokio::time::timeout(std::time::Duration::from_secs(1), first)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        drop(first_permit);
        let second_permit = tokio::time::timeout(std::time::Duration::from_secs(1), second)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        drop(second_permit);

        assert_eq!(active.available_permits(), 1);
        assert_eq!(waiting.available_permits(), 2);
    }

    #[tokio::test]
    async fn cancelled_admission_releases_its_waiting_slot() {
        let active = Arc::new(tokio::sync::Semaphore::new(1));
        let waiting = Arc::new(tokio::sync::Semaphore::new(1));
        let active_guard = active.clone().acquire_owned().await.unwrap();
        let waiter = tokio::spawn(acquire_with_waiting_tier(
            active.clone(),
            waiting.clone(),
            "busy",
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while waiting.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert_eq!(waiting.available_permits(), 1);
        drop(active_guard);
    }
}
