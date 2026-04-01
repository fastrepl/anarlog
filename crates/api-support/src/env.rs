use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct GitHubAppEnv {
    #[serde(deserialize_with = "hypr_api_env::string_to_u64")]
    pub github_bot_app_id: u64,
    pub github_bot_private_key: String,
    #[serde(deserialize_with = "hypr_api_env::string_to_u64")]
    pub github_bot_installation_id: u64,
    pub github_repo_id: String,
    pub github_discussion_category_id: String,
}

#[derive(Clone, Deserialize)]
pub struct SupportDatabaseEnv {
    pub support_database_url: String,
}

pub use hypr_api_env::OpenRouterEnv;
pub use hypr_api_env::StripeEnv;
pub use hypr_api_env::SupabaseEnv;

fn default_chatwoot_base_url() -> String {
    "https://app.chatwoot.com".to_string()
}

#[derive(Clone, Deserialize)]
pub struct ChatwootEnv {
    #[serde(default = "default_chatwoot_base_url")]
    pub chatwoot_base_url: String,
    pub chatwoot_access_token: String,
    #[serde(deserialize_with = "hypr_api_env::string_to_u64")]
    pub chatwoot_account_id: u64,
    pub chatwoot_inbox_identifier: String,
}
