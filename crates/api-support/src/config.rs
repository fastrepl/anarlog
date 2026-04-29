use hypr_api_auth::AuthState;

use crate::env::{GitHubAppEnv, OpenRouterEnv, StripeEnv, SupabaseEnv, SupportDatabaseEnv};

#[derive(Clone)]
pub struct SupportConfig {
    pub github: GitHubAppEnv,
    pub openrouter: OpenRouterEnv,
    pub support_database: SupportDatabaseEnv,
    pub stripe: StripeEnv,
    pub supabase: SupabaseEnv,
    pub auth: AuthState,
}

impl SupportConfig {
    pub fn new(
        github: &GitHubAppEnv,
        openrouter: &OpenRouterEnv,
        support_database: &SupportDatabaseEnv,
        stripe: &StripeEnv,
        supabase: &SupabaseEnv,
        auth: AuthState,
    ) -> Self {
        Self {
            github: github.clone(),
            openrouter: openrouter.clone(),
            support_database: support_database.clone(),
            stripe: stripe.clone(),
            supabase: supabase.clone(),
            auth,
        }
    }
}
