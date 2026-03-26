use crate::config::SyncConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: SyncConfig,
}

impl AppState {
    pub fn new(config: SyncConfig) -> Self {
        Self { config }
    }
}
