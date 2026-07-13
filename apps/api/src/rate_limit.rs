use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
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
}
