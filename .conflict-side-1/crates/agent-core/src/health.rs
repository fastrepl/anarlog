use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    ProviderHealth,
    provider::{claude, codex, opencode},
};

#[derive(Debug, Clone, Default)]
pub struct HealthCheckOptions {
    pub codex_path_override: Option<PathBuf>,
    pub claude_path_override: Option<PathBuf>,
    pub opencode_path_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    pub providers: Vec<ProviderHealth>,
}

pub fn health_check() -> HealthCheckResponse {
    health_check_with_options(&HealthCheckOptions::default())
}

pub fn health_check_with_options(options: &HealthCheckOptions) -> HealthCheckResponse {
    let codex = codex::health(options);
    let claude = claude::health(options);
    let opencode = opencode::health(options);

    HealthCheckResponse {
        providers: vec![codex, claude, opencode],
    }
}
