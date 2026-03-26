use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrationCallbackSearch {
    pub integration_id: String,
    pub status: String,
    pub return_to: Option<String>,
}
