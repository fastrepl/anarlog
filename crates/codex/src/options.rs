use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct CodexOptions {
    pub codex_path_override: Option<PathBuf>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub config: toml::Table,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadOptions {
    pub model: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub working_directory: Option<PathBuf>,
    pub skip_git_repo_check: bool,
    pub model_reasoning_effort: Option<ModelReasoningEffort>,
    pub network_access_enabled: Option<bool>,
    pub web_search_mode: Option<WebSearchMode>,
    pub approval_mode: Option<ApprovalMode>,
    pub additional_directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct TurnOptions {
    pub output_schema: Option<serde_json::Value>,
    pub cancellation_token: Option<CancellationToken>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    Never,
    OnRequest,
    OnFailure,
    Untrusted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchMode {
    Disabled,
    Cached,
    Live,
}
