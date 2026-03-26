use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use backon::{ConstantBuilder, Retryable};
use reqwest::StatusCode;

use crate::error::Error;

const MAX_RETRIES: usize = 14;
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(5);
const NO_RETRY_AFTER: u64 = u64::MAX;

pub(super) async fn post_with_retry(
    client: &reqwest::Client,
    url: url::Url,
    content_type: &str,
    audio_data: Vec<u8>,
) -> Result<reqwest::Response, Error> {
    let retry_after = Arc::new(AtomicU64::new(NO_RETRY_AFTER));
    let retry_after_ref = retry_after.clone();

    let result = (|| {
        let url = url.clone();
        let audio_data = audio_data.clone();
        let content_type = content_type.to_string();
        let retry_after_ref = retry_after_ref.clone();
        async move {
            let response = client
                .post(url.as_str())
                .header("Content-Type", &content_type)
                .header("Accept", "text/event-stream")
                .body(audio_data)
                .send()
                .await?;

            if response.status() == StatusCode::SERVICE_UNAVAILABLE {
                if let Some(val) = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    retry_after_ref.store(val, Ordering::SeqCst);
                }

                return Err(Error::UnexpectedStatus {
                    status: response.status(),
                    body: "service unavailable".to_string(),
                });
            }

            Ok(response)
        }
    })
    .retry(
        ConstantBuilder::default()
            .with_delay(DEFAULT_RETRY_DELAY)
            .with_max_times(MAX_RETRIES),
    )
    .when(|e: &Error| {
        matches!(
            e,
            Error::UnexpectedStatus { status, .. } if *status == StatusCode::SERVICE_UNAVAILABLE
        )
    })
    .adjust(|_e: &Error, dur| {
        let secs = retry_after.load(Ordering::SeqCst);
        if secs != NO_RETRY_AFTER {
            Some(Duration::from_secs(secs))
        } else {
            dur
        }
    })
    .notify(|e: &Error, dur| {
        tracing::warn!(delay_ms = dur.as_millis() as u64, "cactus_batch_retry: {e}");
    })
    .await?;

    let status = result.status();
    if !status.is_success() {
        let body = result.text().await.unwrap_or_default();
        tracing::error!(
            http.response.status_code = status.as_u16(),
            hyprnote.http.response.body = %body,
            "unexpected_response_status"
        );
        return Err(Error::UnexpectedStatus { status, body });
    }

    Ok(result)
}
