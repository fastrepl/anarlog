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

pub fn sign_payload(secret: &str, body: &str) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("hmac accepts keys of any length");
    mac.update(body.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn subscribes(endpoint: &hypr_db_app::WebhookEndpointRow, event: &str) -> bool {
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
    let export = hypr_agent_access::get_meeting_export(pool, meeting_id.to_string())
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

/// Fans an event out to every active endpoint subscribed to it. Deliveries run
/// in background tasks; the returned count is the number of endpoints targeted.
pub async fn dispatch_event(
    pool: &SqlitePool,
    event: &str,
    meeting_id: &str,
) -> Result<usize, String> {
    if !crate::settings::load(pool)
        .await
        .map_err(|error| error.to_string())?
        .enabled
    {
        return Ok(0);
    }

    let endpoints = hypr_db_app::list_active_webhook_endpoints(pool)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|endpoint| subscribes(endpoint, event))
        .collect::<Vec<_>>();
    if endpoints.is_empty() {
        return Ok(0);
    }

    let body = envelope(event, meeting_payload(pool, meeting_id).await?);
    let targeted = endpoints.len();
    for endpoint in endpoints {
        let pool = pool.clone();
        let body = body.clone();
        let event = event.to_string();
        tokio::spawn(async move {
            deliver_with_retry(&pool, &endpoint, &event, &body).await;
        });
    }
    Ok(targeted)
}

pub async fn send_test(
    pool: &SqlitePool,
    endpoint: &hypr_db_app::WebhookEndpointRow,
) -> crate::WebhookDelivery {
    let body = envelope(
        EVENT_TEST,
        serde_json::json!({ "message": "This is a test delivery from Anarlog." }),
    );
    let delivery_id = format!("dlv_{}", uuid::Uuid::new_v4().simple());
    let status = deliver_once(endpoint, EVENT_TEST, &body, &delivery_id).await;
    let delivered = status.delivered;
    let status_text = truncate_status(&status.status);
    if let Err(error) = hypr_db_app::record_webhook_delivery(pool, &endpoint.id, &status_text).await
    {
        tracing::warn!("[local-api] failed to record webhook delivery: {error}");
    }
    crate::WebhookDelivery {
        delivered,
        status: status_text,
    }
}

struct DeliveryOutcome {
    delivered: bool,
    status: String,
}

async fn deliver_with_retry(
    pool: &SqlitePool,
    endpoint: &hypr_db_app::WebhookEndpointRow,
    event: &str,
    body: &str,
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
    if let Err(error) = hypr_db_app::record_webhook_delivery(pool, &endpoint.id, &status_text).await
    {
        tracing::warn!("[local-api] failed to record webhook delivery: {error}");
    }
}

async fn deliver_once(
    endpoint: &hypr_db_app::WebhookEndpointRow,
    event: &str,
    body: &str,
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
            format!("sha256={}", sign_payload(&endpoint.secret, body)),
        )
        .body(body.to_string())
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
        let endpoint = hypr_db_app::WebhookEndpointRow {
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

        let scoped = hypr_db_app::WebhookEndpointRow {
            events_json: "[\"meeting.completed\"]".to_string(),
            ..endpoint
        };
        assert!(subscribes(&scoped, EVENT_MEETING_COMPLETED));
        assert!(!subscribes(&scoped, EVENT_NOTE_ENHANCED));
    }

    #[tokio::test]
    async fn disabled_api_skips_webhook_dispatch() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        hypr_db_app::insert_webhook_endpoint(
            db.pool(),
            "webhook-1",
            "https://example.com",
            "whsec_test",
            "[]",
        )
        .await
        .unwrap();

        let targeted = dispatch_event(db.pool(), EVENT_MEETING_COMPLETED, "missing-meeting")
            .await
            .unwrap();

        assert_eq!(targeted, 0);
    }
}
