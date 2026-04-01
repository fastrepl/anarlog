use std::sync::Arc;

use hypr_analytics::AnalyticsClient;

use crate::StripeEnv;
use hypr_api_env::{LoopsEnv, SupabaseEnv};

#[derive(Clone)]
pub struct SubscriptionConfig {
    pub supabase: SupabaseEnv,
    pub stripe: StripeEnv,
    pub loops: LoopsEnv,
    pub analytics: Option<Arc<AnalyticsClient>>,
}

impl SubscriptionConfig {
    pub fn new(supabase: &SupabaseEnv, stripe: &StripeEnv, loops: &LoopsEnv) -> Self {
        Self {
            supabase: supabase.clone(),
            stripe: stripe.clone(),
            loops: loops.clone(),
            analytics: None,
        }
    }

    pub fn with_analytics(mut self, analytics: Arc<AnalyticsClient>) -> Self {
        self.analytics = Some(analytics);
        self
    }
}
