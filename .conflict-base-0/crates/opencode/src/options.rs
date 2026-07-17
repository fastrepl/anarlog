use std::collections::BTreeMap;
use std::path::PathBuf;

use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct OpencodeOptions {
    pub opencode_path_override: Option<PathBuf>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    pub model: Option<String>,
    pub agent: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub fork: bool,
    pub continue_last: bool,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct TurnOptions {
    pub cancellation_token: Option<CancellationToken>,
}
