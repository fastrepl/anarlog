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
    pub durable_cleanup_enabled: bool,
    pub cloudsync_cleanup: Option<CloudsyncCleanupConfig>,
}

#[derive(Clone)]
pub struct CloudsyncCleanupConfig {
    pub(crate) project_url: String,
    pub(crate) token_issuer_api_key: String,
    pub(crate) managed_database_id: String,
    pub(crate) management_api_key: String,
    pub(crate) management_url: String,
    pub(crate) project_id: String,
}

impl SubscriptionConfig {
    pub fn new(supabase: &SupabaseEnv, stripe: &StripeEnv, loops: &LoopsEnv) -> Self {
        Self {
            supabase: supabase.clone(),
            stripe: stripe.clone(),
            loops: loops.clone(),
            analytics: None,
            durable_cleanup_enabled: false,
            cloudsync_cleanup: None,
        }
    }

    pub fn with_analytics(mut self, analytics: Arc<AnalyticsClient>) -> Self {
        self.analytics = Some(analytics);
        self
    }

    pub fn with_durable_cleanup_enabled(mut self, enabled: bool) -> Self {
        self.durable_cleanup_enabled = enabled;
        self
    }

    pub fn with_cloudsync_cleanup(mut self, cloudsync_cleanup: CloudsyncCleanupConfig) -> Self {
        self.cloudsync_cleanup = Some(cloudsync_cleanup);
        self
    }
}

impl CloudsyncCleanupConfig {
    pub fn new(
        project_url: impl Into<String>,
        token_issuer_api_key: impl Into<String>,
        managed_database_id: impl Into<String>,
        management_api_key: impl Into<String>,
    ) -> Result<Self, String> {
        let project_url = project_url.into();
        let url = reqwest::Url::parse(&project_url)
            .map_err(|_| "SQLITECLOUD_PROJECT_URL must be a valid URL".to_string())?;
        let host = url
            .host_str()
            .ok_or_else(|| "SQLITECLOUD_PROJECT_URL must include a host".to_string())?;
        if url.scheme() != "https"
            || !host.ends_with(".sqlite.cloud")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "SQLITECLOUD_PROJECT_URL must be an HTTPS SQLite Cloud project origin".to_string(),
            );
        }

        let project_id = host
            .split('.')
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "SQLITECLOUD_PROJECT_URL must include a project ID".to_string())?
            .to_string();
        let token_issuer_api_key = required_secret(
            token_issuer_api_key.into(),
            "SQLITECLOUD_TOKEN_ISSUER_API_KEY",
        )?;
        let management_api_key = required_secret(
            management_api_key.into(),
            "SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY",
        )?;
        let managed_database_id = managed_database_id.into();
        if managed_database_id.is_empty()
            || managed_database_id.len() > 128
            || !managed_database_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID is invalid".to_string());
        }

        Ok(Self {
            project_url: url.origin().ascii_serialization(),
            token_issuer_api_key,
            managed_database_id,
            management_api_key,
            management_url: "https://cloudsync.sqlite.ai".to_string(),
            project_id,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(origin: &str) -> Self {
        Self {
            project_url: origin.to_string(),
            token_issuer_api_key: "issuer-key".to_string(),
            managed_database_id: "managed-e2ee".to_string(),
            management_api_key: "management-key".to_string(),
            management_url: origin.to_string(),
            project_id: "test-project".to_string(),
        }
    }
}

fn required_secret(value: String, name: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(format!("{name} is required for CloudSync account cleanup"));
    }
    Ok(value)
}
