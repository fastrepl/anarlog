use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct AmpOptions {
    pub amp_path_override: Option<PathBuf>,
    pub api_key: Option<String>,
    pub settings_overrides: Option<serde_json::Value>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadOptions {
    pub mode: Option<AmpMode>,
    pub working_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct TurnOptions {
    pub cancellation_token: Option<CancellationToken>,
    pub include_thinking_stream: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AmpMode {
    Smart,
    Rush,
    Deep,
}
