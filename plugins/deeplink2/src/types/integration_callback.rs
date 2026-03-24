use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct IntegrationCallbackSearch {
    pub integration_id: String,
    pub status: String,
    pub return_to: Option<String>,
}
