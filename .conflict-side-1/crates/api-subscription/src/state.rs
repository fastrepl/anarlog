use hypr_loops::LoopClient;
use stripe::Client as StripeClient;

use crate::config::SubscriptionConfig;
use crate::supabase::SupabaseClient;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: SubscriptionConfig,
    pub(crate) supabase: SupabaseClient,
    pub(crate) stripe: StripeClient,
    pub(crate) loops: LoopClient,
}

impl AppState {
    pub(crate) fn new(config: SubscriptionConfig) -> Self {
        let supabase = SupabaseClient::new(
            config.supabase.supabase_url.clone(),
            config.supabase.supabase_anon_key.clone(),
            config.supabase.supabase_service_role_key.clone(),
        );

        let stripe = StripeClient::new(&config.stripe.stripe_secret_key);

        let loops = LoopClient::builder()
            .api_key(&config.loops.loops_key)
            .build();

        Self {
            config,
            supabase,
            stripe,
            loops,
        }
    }
}
