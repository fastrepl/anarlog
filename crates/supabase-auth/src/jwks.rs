use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use tokio::sync::RwLock;

use crate::Error;

const CACHE_DURATION: Duration = Duration::from_secs(600);

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
pub(crate) struct CachedJwks {
    url: String,
    cache: Arc<RwLock<Cache>>,
    http_client: reqwest::Client,
}

impl CachedJwks {
    pub fn new(url: String) -> Self {
        Self {
            url,
            cache: Arc::new(RwLock::new(Cache::new())),
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn get(&self) -> Result<JwkSet, Error> {
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
            .map_err(|_| Error::JwksFetchFailed)?
            .json()
            .await
            .map_err(|_| Error::JwksFetchFailed)?;

        cache.jwks = Some(jwks.clone());
        cache.fetched_at = Some(Instant::now());

        Ok(jwks)
    }
}
