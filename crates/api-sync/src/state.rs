use crate::config::SyncConfig;

const UPSTREAM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub config: SyncConfig,
    pub client: reqwest::Client,
}

impl AppState {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(UPSTREAM_REQUEST_TIMEOUT)
                .build()
                .expect("CloudSync HTTP client must build"),
        }
    }
}
