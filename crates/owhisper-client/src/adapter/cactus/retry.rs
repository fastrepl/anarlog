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
    let retry_after_secs = AtomicU64::new(NO_RETRY_AFTER);

    let result = (|| {
        let url = url.clone();
        let audio_data = audio_data.clone();
        async {
            let resp = client
                .post(url)
                .header("Content-Type", content_type)
                .header("Accept", "text/event-stream")
                .body(audio_data)
                .send()
                .await?;

            if resp.status() == StatusCode::SERVICE_UNAVAILABLE {
                let secs = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(NO_RETRY_AFTER);
                retry_after_secs.store(secs, Ordering::SeqCst);

                let body = resp.text().await.unwrap_or_default();
                return Err(Error::UnexpectedStatus {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    body,
                });
            }

            Ok(resp)
        }
    })
    .retry(
        ConstantBuilder::default()
            .with_delay(DEFAULT_RETRY_DELAY)
            .with_max_times(MAX_RETRIES),
    )
    .when(|e| {
        matches!(
            e,
            Error::UnexpectedStatus { status, .. } if *status == StatusCode::SERVICE_UNAVAILABLE
        )
    })
    .adjust(|_err, dur| {
        let secs = retry_after_secs.swap(NO_RETRY_AFTER, Ordering::SeqCst);
        if secs == NO_RETRY_AFTER {
            dur
        } else {
            Some(Duration::from_secs(secs))
        }
    })
    .notify(|_err, dur| {
        tracing::info!(retry_after_secs = dur.as_secs(), "model_loading_retry");
    })
    .await;

    let response = result?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        tracing::error!(
            http.response.status_code = status.as_u16(),
            hyprnote.http.response.body = %body,
            "unexpected_response_status"
        );
        return Err(Error::UnexpectedStatus { status, body });
    }

    Ok(response)
}
