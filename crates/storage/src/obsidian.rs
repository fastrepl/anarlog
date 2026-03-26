use std::path::PathBuf;
use std::{collections::HashMap, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ObsidianConfig {
    vaults: HashMap<String, ObsidianVault>,
}

impl FromStr for ObsidianConfig {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(serde_json::from_str(s)?)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ObsidianVault {
    pub path: PathBuf,
}

impl From<ObsidianConfig> for Vec<ObsidianVault> {
    fn from(config: ObsidianConfig) -> Self {
        config.vaults.into_values().collect()
    }
}

fn config_path() -> Result<PathBuf, crate::Error> {
    let data_dir = dirs::data_dir().ok_or(crate::Error::DataDirUnavailable)?;
    Ok(data_dir.join("obsidian").join("obsidian.json"))
}

pub fn list_vaults() -> Result<Vec<ObsidianVault>, crate::Error> {
    let config: ObsidianConfig = {
        let config_path = config_path()?;
        let content = std::fs::read_to_string(&config_path)?;
        content.parse()?
    };

    Ok(config.into())
}
