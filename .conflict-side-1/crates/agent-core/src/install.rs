use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    ProviderKind,
    provider::{claude, codex, opencode},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct InstallCliRequest {
    pub provider: ProviderKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct InstallCliResponse {
    pub provider: ProviderKind,
    pub target_path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UninstallCliRequest {
    pub provider: ProviderKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UninstallCliResponse {
    pub provider: ProviderKind,
    pub target_path: PathBuf,
    pub message: String,
}

pub fn install_cli(request: InstallCliRequest) -> Result<InstallCliResponse, String> {
    match request.provider {
        ProviderKind::Codex => codex::install_cli(),
        ProviderKind::Claude => claude::install_cli(),
        ProviderKind::Opencode => opencode::install_cli(),
    }
}

pub fn upgrade_hooks() {
    claude::upgrade();
    codex::upgrade();
    opencode::upgrade();
}

pub fn uninstall_cli(request: UninstallCliRequest) -> Result<UninstallCliResponse, String> {
    match request.provider {
        ProviderKind::Codex => codex::uninstall_cli(),
        ProviderKind::Claude => claude::uninstall_cli(),
        ProviderKind::Opencode => opencode::uninstall_cli(),
    }
}
