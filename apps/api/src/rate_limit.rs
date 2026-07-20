use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash, Hasher},
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    Quota, RateLimiter,
    clock::{Clock, DefaultClock},
    state::keyed::DefaultKeyedStateStore,
};
use hypr_api_auth::AuthContext;

type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;
type IpKeyedLimiter = RateLimiter<u16, DefaultKeyedStateStore<u16>, DefaultClock>;

const IP_RATE_LIMIT_CLEANUP_EVERY: usize = 1024;
const IP_RATE_LIMIT_BUCKETS: u64 = 4096;
const UNKNOWN_CLIENT_IP_BUCKET: u16 = IP_RATE_LIMIT_BUCKETS as u16;
const MAX_RATE_LIMIT_QUEUE_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Clone)]
pub struct IpRateLimitState {
    limiter: Arc<IpKeyedLimiter>,
    bucket_hasher: Arc<RandomState>,
    checks: Arc<AtomicUsize>,
    cleanup_every: usize,
}

impl IpRateLimitState {
    pub fn new(quota: Quota) -> Self {
        Self::with_cleanup_every(quota, IP_RATE_LIMIT_CLEANUP_EVERY)
    }

    fn with_cleanup_every(quota: Quota, cleanup_every: usize) -> Self {
        assert!(cleanup_every > 0);
        Self {
            limiter: Arc::new(RateLimiter::keyed(quota)),
            bucket_hasher: Arc::new(RandomState::new()),
            checks: Arc::new(AtomicUsize::new(0)),
            cleanup_every,
        }
    }

    fn bucket(&self, client_ip: Option<IpAddr>) -> u16 {
        let Some(client_ip) = client_ip else {
            return UNKNOWN_CLIENT_IP_BUCKET;
        };
        let mut hasher = self.bucket_hasher.build_hasher();
        client_ip.hash(&mut hasher);
        (hasher.finish() % IP_RATE_LIMIT_BUCKETS) as u16
    }

    fn check(&self, client_ip: Option<IpAddr>) -> Result<(), std::time::Duration> {
        let bucket = self.bucket(client_ip);
        let result = self
            .limiter
            .check_key(&bucket)
            .map_err(|not_until| not_until.wait_time_from(DefaultClock::default().now()));

        let previous_checks = self.checks.fetch_add(1, Ordering::Relaxed);
        if previous_checks % self.cleanup_every == self.cleanup_every - 1 {
            self.limiter.retain_recent();
            self.limiter.shrink_to_fit();
        }

        result
    }
}

#[derive(Clone)]
pub struct RateLimitState {
    limiter_pro: Arc<KeyedLimiter>,
    limiter_free: Arc<KeyedLimiter>,
}

impl RateLimitState {
    pub fn builder() -> RateLimitStateBuilder {
        RateLimitStateBuilder {
            pro: None,
            free: None,
        }
    }

    fn check(&self, auth: &AuthContext) -> Result<(), std::time::Duration> {
        let limiter = if auth.claims.is_paid() {
            &self.limiter_pro
        } else {
            &self.limiter_free
        };
        limiter
            .check_key(&auth.claims.sub)
            .map_err(|not_until| not_until.wait_time_from(DefaultClock::default().now()))
    }

    async fn wait(&self, auth: &AuthContext) {
        let limiter = if auth.claims.is_paid() {
            &self.limiter_pro
        } else {
            &self.limiter_free
        };
        limiter.until_key_ready(&auth.claims.sub).await;
    }
}

pub struct RateLimitStateBuilder {
    pro: Option<Quota>,
    free: Option<Quota>,
}

impl RateLimitStateBuilder {
    pub fn pro(mut self, quota: Quota) -> Self {
        self.pro = Some(quota);
        self
    }

    pub fn free(mut self, quota: Quota) -> Self {
        self.free = Some(quota);
        self
    }

    pub fn build(self) -> RateLimitState {
        RateLimitState {
            limiter_pro: Arc::new(RateLimiter::keyed(self.pro.expect("pro quota is required"))),
            limiter_free: Arc::new(RateLimiter::keyed(
                self.free.expect("free quota is required"),
            )),
        }
    }
}

pub async fn rate_limit(
    axum::extract::State(state): axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    if cfg!(debug_assertions) {
        return Ok(next.run(request).await);
    }

    if let Some(auth) = request.extensions().get::<AuthContext>() {
        if let Err(wait) = state.check(auth) {
            let retry_after = wait.as_secs().max(1).to_string();
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after)],
                "rate limit exceeded",
            )
                .into_response());
        }
    }

    Ok(next.run(request).await)
}

pub async fn wait_for_rate_limit(
    axum::extract::State(state): axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Response {
    if !cfg!(debug_assertions)
        && let Some(auth) = request.extensions().get::<AuthContext>()
        && tokio::time::timeout(MAX_RATE_LIMIT_QUEUE_WAIT, state.wait(auth))
            .await
            .is_err()
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "1"), ("cache-control", "no-store")],
            "rate limit exceeded",
        )
            .into_response();
    }

    next.run(request).await
}

pub async fn rate_limit_by_ip(
    axum::extract::State(state): axum::extract::State<IpRateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    if cfg!(debug_assertions) {
        return Ok(next.run(request).await);
    }

    if let Err(wait) = state.check(trusted_client_ip(request.headers())) {
        let retry_after = wait.as_secs().max(1).to_string();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("retry-after", retry_after.as_str()),
                ("cache-control", "no-store"),
            ],
            "rate limit exceeded",
        )
            .into_response());
    }

    Ok(next.run(request).await)
}

fn trusted_client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    // Fly Proxy supplies the address it observed. Falling back to forwarded headers would let
    // clients manufacture unbounded limiter keys when the app is not behind a trusted proxy.
    headers
        .get("fly-client-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_repeated_requests_per_user() {
        let quota = Quota::with_period(std::time::Duration::from_secs(60))
            .unwrap()
            .allow_burst(std::num::NonZeroU32::new(1).unwrap());
        let state = RateLimitState::builder().pro(quota).free(quota).build();
        let auth = AuthContext {
            token: "token".to_string(),
            claims: hypr_api_auth::Claims {
                sub: "user-a".to_string(),
                email: None,
                entitlements: Vec::new(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
            },
        };

        assert!(state.check(&auth).is_ok());
        assert!(state.check(&auth).is_err());
    }

    #[test]
    fn limits_repeated_requests_per_normalized_fly_client_ip() {
        let quota = Quota::with_period(std::time::Duration::from_secs(60))
            .unwrap()
            .allow_burst(std::num::NonZeroU32::new(1).unwrap());
        let state = IpRateLimitState::new(quota);
        let headers = HeaderMap::from_iter([(
            "fly-client-ip".parse().unwrap(),
            "2001:0db8::1".parse().unwrap(),
        )]);
        let client_ip = trusted_client_ip(&headers);

        assert_eq!(client_ip, Some("2001:db8::1".parse().unwrap()));
        assert!(state.check(client_ip).is_ok());
        assert!(state.check(client_ip).is_err());
    }

    #[test]
    fn ignores_spoofable_forwarded_headers_and_uses_a_bounded_fallback_key() {
        let headers = HeaderMap::from_iter([
            (
                "x-forwarded-for".parse().unwrap(),
                "203.0.113.1".parse().unwrap(),
            ),
            ("x-real-ip".parse().unwrap(), "203.0.113.2".parse().unwrap()),
        ]);

        assert_eq!(trusted_client_ip(&headers), None);
        assert_eq!(trusted_client_ip(&HeaderMap::new()), None);
    }

    #[test]
    fn fly_client_ip_takes_precedence_over_forwarded_headers() {
        let headers = HeaderMap::from_iter([
            (
                "fly-client-ip".parse().unwrap(),
                "198.51.100.10".parse().unwrap(),
            ),
            (
                "x-forwarded-for".parse().unwrap(),
                "203.0.113.1".parse().unwrap(),
            ),
        ]);

        assert_eq!(
            trusted_client_ip(&headers),
            Some("198.51.100.10".parse().unwrap())
        );
    }

    #[test]
    fn periodically_discards_stale_ip_keys() {
        let quota = Quota::with_period(std::time::Duration::from_millis(1))
            .unwrap()
            .allow_burst(std::num::NonZeroU32::new(1).unwrap());
        let state = IpRateLimitState::with_cleanup_every(quota, 2);

        let first_ip = "198.51.100.1".parse().unwrap();
        let first_bucket = state.bucket(Some(first_ip));
        let second_ip = (2..=254)
            .map(|last_octet| format!("198.51.100.{last_octet}").parse().unwrap())
            .find(|address| state.bucket(Some(*address)) != first_bucket)
            .unwrap();

        assert!(state.check(Some(first_ip)).is_ok());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(state.check(Some(second_ip)).is_ok());

        assert_eq!(state.limiter.len(), 1);
    }

    #[test]
    fn distinct_fly_addresses_cannot_create_more_than_the_fixed_bucket_count() {
        let quota = Quota::with_period(std::time::Duration::from_secs(60))
            .unwrap()
            .allow_burst(std::num::NonZeroU32::new(1).unwrap());
        let state = IpRateLimitState::with_cleanup_every(quota, usize::MAX);

        for suffix in 0..10_000_u128 {
            let _ = state.check(Some(IpAddr::V6(std::net::Ipv6Addr::from(suffix))));
        }

        assert!(state.limiter.len() <= IP_RATE_LIMIT_BUCKETS as usize);
    }
}
