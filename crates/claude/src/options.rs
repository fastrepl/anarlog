use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct ClaudeOptions {
    pub claude_path_override: Option<PathBuf>,
    pub env: Option<BTreeMap<String, String>>,
    pub settings: Option<serde_json::Value>,
    pub settings_sources: Option<Vec<SettingSource>>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    pub model: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub additional_directories: Vec<PathBuf>,
    pub permission_mode: Option<PermissionMode>,
    pub append_system_prompt: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub include_partial_messages: bool,
    pub include_hook_events: bool,
    pub fork_session: bool,
    pub session_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TurnOptions {
    pub output_schema: Option<serde_json::Value>,
    pub cancellation_token: Option<CancellationToken>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Auto,
    DontAsk,
    BypassPermissions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingSource {
    User,
    Project,
    Local,
}
