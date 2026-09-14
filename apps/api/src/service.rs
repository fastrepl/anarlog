use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    #[default]
    All,
    Ai,
    Sync,
    Core,
    Billing,
}

impl Service {
    pub fn includes(self, service: Self) -> bool {
        self == Self::All || self == service
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::All => "api",
            Self::Ai => "ai",
            Self::Sync => "sync",
            Self::Core => "core",
            Self::Billing => "billing-api",
        }
    }
}
