use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Default, Serialize, Deserialize, Type)]
pub struct AuthCallbackSearch {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

impl fmt::Debug for AuthCallbackSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthCallbackSearch")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("code", &self.code.as_ref().map(|_| "[REDACTED]"))
            .field("state", &self.state)
            .finish()
    }
}
