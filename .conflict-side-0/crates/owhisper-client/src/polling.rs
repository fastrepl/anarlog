use std::future::Future;
use std::time::Duration;

use crate::error::Error;

pub struct PollingConfig {
    pub interval: Duration,
    pub max_attempts: usize,
    pub timeout_error: String,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(2),
            max_attempts: 300,
            timeout_error: "polling timed out".to_string(),
        }
    }
}

impl PollingConfig {
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn with_timeout_error(mut self, timeout_error: impl Into<String>) -> Self {
        self.timeout_error = timeout_error.into();
        self
    }
}

pub enum PollingResult<T> {
    Complete(T),
    Continue,
    Failed { message: String, retryable: bool },
}

pub async fn poll_until<T, Fut, F>(poll_fn: F, config: PollingConfig) -> Result<T, Error>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<PollingResult<T>, Error>>,
{
    for _ in 0..config.max_attempts {
        match poll_fn().await? {
            PollingResult::Complete(result) => return Ok(result),
            PollingResult::Continue => {
                tokio::time::sleep(config.interval).await;
            }
            PollingResult::Failed { message, retryable } => {
                return Err(Error::provider_failure(message, retryable));
            }
        }
    }

    Err(Error::provider_failure(config.timeout_error, true))
}
