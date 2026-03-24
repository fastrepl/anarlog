use serde::{Deserialize, Serialize};
use strum::{EnumString, VariantNames};

#[derive(Debug, Clone, Copy)]
pub enum FlagStrategy {
    Debug,
    Posthog(&'static str),
    Hardcoded(bool),
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
    EnumString,
    VariantNames,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum Feature {
    Chat,
}

impl Feature {
    pub fn strategy(&self) -> FlagStrategy {
        match self {
            Feature::Chat => FlagStrategy::Hardcoded(true),
        }
    }
}
