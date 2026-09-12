use super::subscription_aliases;
use crate::{
    auth::{self, AuthState},
    env::RuntimeConfig,
};
use axum::{Router, middleware};
use std::sync::Arc;

pub(crate) fn router(
    env: &RuntimeConfig,
    analytics: Arc<anlg_analytics::AnalyticsClient>,
) -> Router {
    let Some((stripe, loops)) = env.subscription.as_ref() else {
        return Router::new();
    };
    let config = anlg_api_subscription::SubscriptionConfig::new(&env.supabase, stripe, loops)
        .with_analytics(analytics);
    subscription_aliases(anlg_api_subscription::billing_router(config))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            AuthState::new(&env.supabase.supabase_url),
            auth::require_auth,
        ))
}
