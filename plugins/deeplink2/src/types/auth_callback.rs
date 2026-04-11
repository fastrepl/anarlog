use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct AuthCallbackSearch {
    pub access_token: String,
    pub refresh_token: String,
    pub web_distinct_id: Option<String>,
}

impl fmt::Debug for AuthCallbackSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthCallbackSearch")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("web_distinct_id", &self.web_distinct_id)
            .finish()
    }
}
