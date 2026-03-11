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
        let limiter = if auth.claims.is_pro() {
            &state.limiter_pro
        } else {
            &state.limiter_free
        };
        if let Err(not_until) = limiter.check_key(&auth.claims.sub) {
            let wait = not_until.wait_time_from(DefaultClock::default().now());
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
