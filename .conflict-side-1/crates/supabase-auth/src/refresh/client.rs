use std::sync::{Arc, Mutex};
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use chrono::NaiveDate;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Serialize;
use tokio::sync::{Mutex as AsyncMutex, Notify};

use crate::session::Session;

use super::response::{error_from_response, parse_api_version, parse_session};
use super::{Error, Result};

const AUTH_V1: &str = "/auth/v1";
const AUTO_REFRESH_TICK_DURATION_MS: u64 = 30_000;
const API_VERSION: &str = "2024-01-01";
const CLIENT_INFO: &str = concat!("hypr-supabase-auth/", env!("CARGO_PKG_VERSION"));
const API_VERSION_HEADER: &str = "x-supabase-api-version";

#[derive(Clone)]
pub struct AuthClient {
    client: reqwest::Client,
    supabase_url: String,
    anon_key: String,
    in_flight_refresh: Arc<AsyncMutex<Option<Arc<InFlightRefresh>>>>,
}

struct InFlightRefresh {
    notify: Notify,
    result: Mutex<Option<Result<Session>>>,
}

impl InFlightRefresh {
    fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
        }
    }

    fn finish(&self, result: Result<Session>) {
        let mut slot = self
            .result
            .lock()
            .expect("refresh result mutex should not be poisoned");
        *slot = Some(result);
        self.notify.notify_waiters();
    }

    async fn wait(&self) -> Result<Session> {
        loop {
            let notified = self.notify.notified();
            if let Some(result) = self
                .result
                .lock()
                .expect("refresh result mutex should not be poisoned")
                .clone()
            {
                return result;
            }
            notified.await;
        }
    }
}

#[derive(Serialize)]
struct RefreshSessionPayload<'a> {
    refresh_token: &'a str,
}

impl AuthClient {
    pub fn new(supabase_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            supabase_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            in_flight_refresh: Arc::new(AsyncMutex::new(None)),
        }
    }

    pub async fn refresh_session(&self, refresh_token: &str) -> Result<Session> {
        self.refresh_session_with_total_delay(
            refresh_token,
            Duration::from_millis(AUTO_REFRESH_TICK_DURATION_MS),
        )
        .await
    }

    pub(super) async fn refresh_session_with_total_delay(
        &self,
        refresh_token: &str,
        total_delay: Duration,
    ) -> Result<Session> {
        if refresh_token.is_empty() {
            return Err(Error::SessionMissing);
        }

        let existing = {
            let mut slot = self.in_flight_refresh.lock().await;
            if let Some(in_flight) = slot.as_ref() {
                Some(in_flight.clone())
            } else {
                let in_flight = Arc::new(InFlightRefresh::new());
                *slot = Some(in_flight);
                None
            }
        };
        if let Some(in_flight) = existing {
            return in_flight.wait().await;
        }

        let in_flight = self
            .in_flight_refresh
            .lock()
            .await
            .as_ref()
            .expect("refresh slot should exist for leader")
            .clone();

        let result = self
            .send_with_retry(refresh_token.to_string(), total_delay)
            .await;
        in_flight.finish(result.clone());

        let mut slot = self.in_flight_refresh.lock().await;
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &in_flight))
        {
            *slot = None;
        }

        result
    }

    async fn send_with_retry(
        &self,
        refresh_token: String,
        total_delay: Duration,
    ) -> Result<Session> {
        let client = self.clone();
        let backoff = refresh_backoff(total_delay);

        (move || {
            let client = client.clone();
            let refresh_token = refresh_token.clone();
            async move { client.send_once(&refresh_token).await }
        })
        .retry(backoff)
        .when(Error::is_retryable)
        .await
    }

    async fn send_once(&self, refresh_token: &str) -> Result<Session> {
        let response = self
            .client
            .post(self.refresh_url())
            .headers(self.headers()?)
            .json(&RefreshSessionPayload { refresh_token })
            .send()
            .await?;

        let status = response.status();
        let api_version = response_api_version(&response);
        let body = response.text().await?;

        if status.is_success() {
            return parse_session(&body);
        }

        Err(error_from_response(status.as_u16(), &body, api_version))
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert("apikey", HeaderValue::from_str(&self.anon_key)?);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.anon_key))?,
        );
        headers.insert(API_VERSION_HEADER, HeaderValue::from_static(API_VERSION));
        headers.insert("x-client-info", HeaderValue::from_static(CLIENT_INFO));
        Ok(headers)
    }

    fn refresh_url(&self) -> String {
        format!(
            "{}{AUTH_V1}/token?grant_type=refresh_token",
            self.supabase_url
        )
    }
}

fn response_api_version(response: &reqwest::Response) -> Option<NaiveDate> {
    response
        .headers()
        .get(API_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_api_version)
}

fn refresh_backoff(total_delay: Duration) -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(200))
        .with_factor(2.0)
        .without_max_times()
        .with_total_delay(Some(total_delay))
}
