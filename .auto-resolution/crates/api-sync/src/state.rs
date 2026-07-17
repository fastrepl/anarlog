use crate::config::SyncConfig;

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub config: SyncConfig,
    pub client: reqwest::Client,
    pub storage: hypr_supabase_storage::SupabaseStorage,
}

impl AppState {
    pub fn new(config: SyncConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(UPSTREAM_REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("CloudSync HTTP client must build");
        let storage = hypr_supabase_storage::SupabaseStorage::new(
            client.clone(),
            &config.supabase_url,
            &config.supabase_service_role_key,
        );

        Self {
            config,
            client,
            storage,
        }
    }
}
