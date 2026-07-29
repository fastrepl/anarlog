use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use owhisper_client::Provider;

use crate::analytics::SttAnalyticsReporter;
use crate::anarlog_routing::{AnarlogRouter, AnarlogRoutingConfig};
use crate::env::{ApiKeys, Env};
use crate::provider_selector::ProviderSelector;

pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 7 * 1000;

#[derive(Clone)]
pub struct SupabaseConfig {
    pub url: Option<String>,
    pub service_role_key: Option<String>,
}

#[derive(Clone)]
pub struct CallbackConfig {
    pub api_base_url: Option<String>,
    pub secret: Option<String>,
}

#[derive(Clone)]
pub struct SttProxyConfig {
    pub api_keys: HashMap<Provider, String>,
    pub default_provider: Provider,
    pub connect_timeout: Duration,
    pub analytics: Option<Arc<dyn SttAnalyticsReporter>>,
    pub upstream_urls: HashMap<Provider, String>,
    pub anarlog_routing: Option<AnarlogRoutingConfig>,
    pub supabase: SupabaseConfig,
    pub callback: CallbackConfig,
}

impl SttProxyConfig {
    pub fn new(env: &Env, supabase: &anlg_api_env::SupabaseEnv) -> Self {
        Self {
            api_keys: ApiKeys::from(&env.stt).0,
            default_provider: Provider::Deepgram,
            connect_timeout: Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS),
            analytics: None,
            upstream_urls: HashMap::new(),
            anarlog_routing: None,
            supabase: SupabaseConfig {
                url: Some(supabase.supabase_url.clone()),
                service_role_key: Some(supabase.supabase_service_role_key.clone()),
            },
            callback: CallbackConfig {
                api_base_url: Some(env.callback.api_base_url.clone()),
                secret: env.callback.callback_secret.clone(),
            },
        }
    }

    pub fn with_default_provider(mut self, provider: Provider) -> Self {
        self.default_provider = provider;
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_analytics(mut self, analytics: Arc<dyn SttAnalyticsReporter>) -> Self {
        self.analytics = Some(analytics);
        self
    }

    pub fn with_upstream_url(mut self, provider: Provider, url: impl Into<String>) -> Self {
        self.upstream_urls.insert(provider, url.into());
        self
    }

    pub fn with_anarlog_routing(mut self, config: AnarlogRoutingConfig) -> Self {
        self.anarlog_routing = Some(config);
        self
    }

    pub fn provider_selector(&self) -> ProviderSelector {
        ProviderSelector::new(
            self.api_keys.clone(),
            self.default_provider,
            self.upstream_urls.clone(),
        )
    }

    pub fn anarlog_router(&self) -> Option<AnarlogRouter> {
        self.anarlog_routing.clone().map(AnarlogRouter::new)
    }
}
