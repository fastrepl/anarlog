//! Outbound meeting webhooks, ported from shipping's `plugins/local-api` (`commands.rs` and
//! `dispatch.rs`): endpoints live in the library's `webhook_endpoints` table and each delivery
//! carries an HMAC-SHA256 signature of its body.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{Result, RuntimeHandle, SessionId};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use super::failure;
use crate::workspace::automation_runner::Trigger;

const EVENT_MEETING_COMPLETED: &str = "meeting.completed";
const EVENT_NOTE_ENHANCED: &str = "note.enhanced";
const EVENT_TEST: &str = "webhook.test";
const KNOWN_EVENTS: &[&str] = &[EVENT_MEETING_COMPLETED, EVENT_NOTE_ENHANCED];
const MAX_ENDPOINTS: usize = 64;
const MAX_STATUS_LEN: usize = 200;
const RETRY_DELAYS_SECS: &[u64] = &[0, 5, 30];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookInfo {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub last_delivery_at: Option<String>,
    pub last_delivery_status: String,
}

impl From<anlg_db_app::WebhookEndpointRow> for WebhookInfo {
    fn from(row: anlg_db_app::WebhookEndpointRow) -> Self {
        Self {
            id: row.id,
            url: row.url,
            events: serde_json::from_str(&row.events_json).unwrap_or_default(),
            active: row.active,
            last_delivery_at: row.last_delivery_at,
            last_delivery_status: row.last_delivery_status,
        }
    }
}

/// A new endpoint with its signing secret, which is only shown once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedWebhook {
    pub info: WebhookInfo,
    pub secret: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookDelivery {
    pub delivered: bool,
    pub status: String,
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<WebhookInfo>> {
    Ok(anlg_db_app::list_webhook_endpoints(pool)
        .await
        .map_err(failure)?
        .into_iter()
        .map(WebhookInfo::from)
        .collect())
}

pub async fn create(pool: &SqlitePool, url: &str, events: &[String]) -> Result<CreatedWebhook> {
    static CREATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = CREATE.lock().await;
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(failure("url must start with http:// or https://"));
    }
    if let Some(unknown) = events
        .iter()
        .find(|event| !KNOWN_EVENTS.contains(&event.as_str()))
    {
        return Err(failure(format!(
            "unknown event '{unknown}'; known events: {}",
            KNOWN_EVENTS.join(", ")
        )));
    }
    if anlg_db_app::list_webhook_endpoints(pool)
        .await
        .map_err(failure)?
        .len()
        >= MAX_ENDPOINTS
    {
        return Err(failure(format!(
            "at most {MAX_ENDPOINTS} webhook endpoints can be configured"
        )));
    }
    let secret = anlg_db_app::generate_webhook_secret();
    let row = anlg_db_app::insert_webhook_endpoint(
        pool,
        &uuid::Uuid::new_v4().to_string(),
        url,
        &secret,
        &serde_json::to_string(events).map_err(failure)?,
    )
    .await
    .map_err(failure)?;
    Ok(CreatedWebhook {
        info: row.into(),
        secret,
    })
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<()> {
    anlg_db_app::delete_webhook_endpoint(pool, id)
        .await
        .map_err(failure)?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: &str, active: bool) -> Result<WebhookInfo> {
    anlg_db_app::set_webhook_endpoint_active(pool, id, active)
        .await
        .map_err(failure)?
        .map(WebhookInfo::from)
        .ok_or_else(|| failure(format!("webhook '{id}' not found")))
}

pub async fn test(pool: &SqlitePool, id: &str) -> Result<WebhookDelivery> {
    let endpoint = anlg_db_app::get_webhook_endpoint(pool, id)
        .await
        .map_err(failure)?
        .ok_or_else(|| failure(format!("webhook '{id}' not found")))?;
    let _permit = deliveries().acquire().await.map_err(failure)?;
    let body = envelope(
        EVENT_TEST,
        serde_json::json!({ "message": "This is a test delivery from Anarlog." }),
    );
    let delivery = deliver(&endpoint, EVENT_TEST, &body, &delivery_id()).await;
    record(pool, &endpoint.id, &delivery.status).await;
    Ok(delivery)
}

/// Fans the trigger's event out to every active subscribed endpoint in the background,
/// retrying failures after 5 and 30 seconds like shipping's dispatcher.
pub fn dispatch(runtime: &RuntimeHandle, trigger: Trigger, session: SessionId) {
    let event = match trigger {
        Trigger::MeetingCompleted => EVENT_MEETING_COMPLETED,
        Trigger::NoteEnhanced => EVENT_NOTE_ENHANCED,
    };
    let reply = runtime.service(move |services| async move {
        let pool = services.db.pool().clone();
        let endpoints = anlg_db_app::list_active_webhook_endpoints(&pool)
            .await
            .map_err(failure)?
            .into_iter()
            .filter(|endpoint| subscribes(endpoint, event))
            .take(MAX_ENDPOINTS)
            .collect::<Vec<_>>();
        if endpoints.is_empty() {
            return Ok(());
        }
        let export = anlg_agent_access::get_meeting_export(&pool, session.0.to_string())
            .await
            .map_err(failure)?;
        let transcript = export
            .transcripts
            .iter()
            .map(|transcript| transcript.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let body: Arc<str> = envelope(
            event,
            serde_json::json!({ "meeting": export.meeting, "transcript_text": transcript }),
        )
        .into();
        for endpoint in endpoints {
            let (pool, body) = (pool.clone(), body.clone());
            tokio::spawn(async move {
                let Ok(_permit) = deliveries().acquire().await else {
                    return;
                };
                let id = delivery_id();
                let mut delivery = WebhookDelivery {
                    delivered: false,
                    status: "not attempted".into(),
                };
                for delay in RETRY_DELAYS_SECS {
                    tokio::time::sleep(Duration::from_secs(*delay)).await;
                    delivery = deliver(&endpoint, event, &body, &id).await;
                    if delivery.delivered {
                        break;
                    }
                }
                if !delivery.delivered {
                    tracing::warn!(status = %delivery.status, "webhook delivery failed");
                }
                record(&pool, &endpoint.id, &delivery.status).await;
            });
        }
        Ok(())
    });
    if let Err(error) = reply {
        tracing::warn!(%error, "webhook dispatch was not admitted");
    }
}

fn deliveries() -> &'static Semaphore {
    static SLOTS: Semaphore = Semaphore::const_new(4);
    &SLOTS
}

fn subscribes(endpoint: &anlg_db_app::WebhookEndpointRow, event: &str) -> bool {
    let events: Vec<String> = serde_json::from_str(&endpoint.events_json).unwrap_or_default();
    events.is_empty() || events.iter().any(|subscribed| subscribed == event)
}

fn delivery_id() -> String {
    format!("dlv_{}", uuid::Uuid::new_v4().simple())
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

async fn deliver(
    endpoint: &anlg_db_app::WebhookEndpointRow,
    event: &str,
    body: &str,
    delivery_id: &str,
) -> WebhookDelivery {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return WebhookDelivery {
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
            format!("sha256={}", signature(&endpoint.secret, body.as_bytes())),
        )
        .body(body.to_owned())
        .send()
        .await;
    let (delivered, status) = match response {
        Ok(response) => (
            response.status().is_success(),
            response.status().to_string(),
        ),
        Err(error) => (false, format!("error: {error}")),
    };
    WebhookDelivery {
        delivered,
        status: status.chars().take(MAX_STATUS_LEN).collect(),
    }
}

async fn record(pool: &SqlitePool, id: &str, status: &str) {
    if let Err(error) = anlg_db_app::record_webhook_delivery(pool, id, status).await {
        tracing::warn!(%error, "could not record the webhook delivery");
    }
}

/// HMAC-SHA256 (RFC 2104) as lowercase hex.
fn signature(secret: &str, body: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut key = [0u8; BLOCK];
    if secret.len() > BLOCK {
        key[..32].copy_from_slice(&Sha256::digest(secret.as_bytes()));
    } else {
        key[..secret.len()].copy_from_slice(secret.as_bytes());
    }
    let pad = |byte: u8| key.map(|k| k ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(body)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, header_exists, method},
    };

    #[test]
    fn signatures_match_shipping_receivers() {
        // Shipping's reference vector, precomputed with Python's hmac module.
        assert_eq!(
            signature("whsec_test", b"{\"a\":1}"),
            "51426af50a41dd7ff2cd3f116594734766d4018d15d6fb07169aee5d2959adf5"
        );
    }

    #[tokio::test]
    async fn test_delivery_is_signed_and_recorded_on_the_endpoint() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("webhooks.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("x-anarlog-event", EVENT_TEST))
            .and(header_exists("x-anarlog-signature"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let url = server.uri();
        let listed = runtime
            .service(move |services| async move {
                let pool = services.db.pool();
                assert!(create(pool, "ftp://example.com", &[]).await.is_err());
                let created = create(pool, &url, &[]).await?;
                assert!(created.secret.starts_with("whsec_"));
                let delivery = test(pool, &created.info.id).await?;
                assert!(delivery.delivered, "{}", delivery.status);
                set_active(pool, &created.info.id, false).await?;
                list(pool).await
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].active);
        assert!(listed[0].last_delivery_at.is_some());
        assert_eq!(listed[0].last_delivery_status, "204 No Content");
        runtime.shutdown().await.unwrap();
    }
}
