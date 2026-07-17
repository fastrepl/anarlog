use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct HooksConfig {
    pub version: u8,
    #[serde(default)]
    pub on: HashMap<String, Vec<HookDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct HookDefinition {
    pub command: String,
}

impl HooksConfig {
    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        let config: HooksConfig =
            serde_json::from_value(value).map_err(|e| Error::ConfigParse(e.to_string()))?;
        if config.version != 0 {
            return Err(Error::UnsupportedVersion(config.version));
        }
        Ok(config)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        let value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| Error::ConfigParse(e.to_string()))?;
        Self::from_value(value)
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).map_err(|e| Error::ConfigLoad(e.to_string()))?;
        Self::from_str(&content)
    }

    pub fn empty() -> Self {
        Self {
            version: 0,
            on: HashMap::new(),
        }
    }
}
