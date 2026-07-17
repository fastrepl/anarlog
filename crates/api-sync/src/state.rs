use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::config::SyncConfig;

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const ATTACHMENT_VERIFICATION_CONCURRENCY: usize = 1;

#[derive(Clone)]
pub struct AppState {
    pub config: SyncConfig,
    pub client: reqwest::Client,
    pub storage: hypr_supabase_storage::SupabaseStorage,
    pub attachment_verification_slots: Arc<Semaphore>,
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
            attachment_verification_slots: Arc::new(Semaphore::new(
                ATTACHMENT_VERIFICATION_CONCURRENCY,
            )),
        }
    }
}
