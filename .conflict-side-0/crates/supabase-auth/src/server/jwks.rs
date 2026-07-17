use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use tokio::sync::RwLock;

const CACHE_DURATION: Duration = Duration::from_secs(600);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

struct Cache {
    jwks: Option<JwkSet>,
    fetched_at: Option<Instant>,
}

impl Cache {
    fn new() -> Self {
        Self {
            jwks: None,
            fetched_at: None,
        }
    }

    fn is_valid(&self) -> bool {
        self.jwks.is_some()
            && self
                .fetched_at
                .map(|t| t.elapsed() < CACHE_DURATION)
                .unwrap_or(false)
    }
}

#[derive(Clone)]
pub(super) struct CachedJwks {
    url: String,
    cache: Arc<RwLock<Cache>>,
    http_client: reqwest::Client,
    request_timeout: Duration,
}

impl CachedJwks {
    pub fn new(url: String) -> Self {
        Self::new_with_timeout(url, REQUEST_TIMEOUT)
    }

    fn new_with_timeout(url: String, request_timeout: Duration) -> Self {
        Self {
            url,
            cache: Arc::new(RwLock::new(Cache::new())),
            http_client: reqwest::Client::builder()
                .timeout(request_timeout)
                .build()
                .expect("JWKS HTTP client must build"),
            request_timeout,
        }
    }

    pub async fn get(&self) -> super::Result<JwkSet> {
        tokio::time::timeout(self.request_timeout, self.get_inner())
            .await
            .map_err(|_| super::Error::JwksFetchFailed)?
    }

    async fn get_inner(&self) -> super::Result<JwkSet> {
        {
            let cache = self.cache.read().await;
            if cache.is_valid() {
                return Ok(cache.jwks.clone().unwrap());
            }
        }

        let mut cache = self.cache.write().await;
        if cache.is_valid() {
            return Ok(cache.jwks.clone().unwrap());
        }

        let jwks: JwkSet = self
            .http_client
            .get(&self.url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|_| super::Error::JwksFetchFailed)?
            .json()
            .await
            .map_err(|_| super::Error::JwksFetchFailed)?;

        cache.jwks = Some(jwks.clone());
        cache.fetched_at = Some(Instant::now());

        Ok(jwks)
    }
}

#[cfg(test)]
mod tests {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn bounds_fetch_and_lock_waiters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jwks.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(1))
                    .set_body_json(serde_json::json!({ "keys": [] })),
            )
            .mount(&server)
            .await;
        let jwks = CachedJwks::new_with_timeout(
            format!("{}/jwks.json", server.uri()),
            Duration::from_millis(50),
        );

        let (first, second) = tokio::join!(jwks.get(), jwks.get());

        assert!(matches!(first, Err(super::super::Error::JwksFetchFailed)));
        assert!(matches!(second, Err(super::super::Error::JwksFetchFailed)));
    }
}
