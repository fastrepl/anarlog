use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Codex,
    Claude,
    Opencode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum ProviderHealthStatus {
    Ready,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthStatus {
    Authenticated,
    Unauthenticated,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub provider: ProviderKind,
    pub binary_path: PathBuf,
    pub installed: bool,
    pub integration_installed: bool,
    pub version: Option<String>,
    pub status: ProviderHealthStatus,
    pub auth_status: ProviderAuthStatus,
    pub message: Option<String>,
}

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
    let codex = hypr_codex::health_check_with_options(&hypr_codex::CodexOptions {
        codex_path_override: options.codex_path_override.clone(),
        ..Default::default()
    });
    let claude = hypr_claude::health_check_with_options(&hypr_claude::ClaudeOptions {
        claude_path_override: options.claude_path_override.clone(),
        ..Default::default()
    });
    let opencode = hypr_opencode::health_check_with_options(&hypr_opencode::OpencodeOptions {
        opencode_path_override: options.opencode_path_override.clone(),
        ..Default::default()
    });

    HealthCheckResponse {
        providers: vec![codex.into(), claude.into(), opencode.into()],
    }
}

impl From<hypr_codex::HealthCheck> for ProviderHealth {
    fn from(value: hypr_codex::HealthCheck) -> Self {
        let integration_installed = codex_integration_installed().unwrap_or(false);

        Self {
            provider: ProviderKind::Codex,
            binary_path: value.binary_path,
            installed: value.installed,
            integration_installed,
            version: value.version,
            status: value.status.into(),
            auth_status: value.auth_status.into(),
            message: value.message,
        }
    }
}

impl From<hypr_claude::HealthCheck> for ProviderHealth {
    fn from(value: hypr_claude::HealthCheck) -> Self {
        let integration_installed = claude_integration_installed().unwrap_or(false);

        Self {
            provider: ProviderKind::Claude,
            binary_path: value.binary_path,
            installed: value.installed,
            integration_installed,
            version: value.version,
            status: value.status.into(),
            auth_status: value.auth_status.into(),
            message: value.message,
        }
    }
}

impl From<hypr_opencode::HealthCheck> for ProviderHealth {
    fn from(value: hypr_opencode::HealthCheck) -> Self {
        let integration_installed = opencode_integration_installed().unwrap_or(false);

        Self {
            provider: ProviderKind::Opencode,
            binary_path: value.binary_path,
            installed: value.installed,
            integration_installed,
            version: value.version,
            status: value.status.into(),
            auth_status: value.auth_status.into(),
            message: value.message,
        }
    }
}

impl From<hypr_codex::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_codex::HealthStatus) -> Self {
        match value {
            hypr_codex::HealthStatus::Ready => Self::Ready,
            hypr_codex::HealthStatus::Warning => Self::Warning,
            hypr_codex::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_claude::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_claude::HealthStatus) -> Self {
        match value {
            hypr_claude::HealthStatus::Ready => Self::Ready,
            hypr_claude::HealthStatus::Warning => Self::Warning,
            hypr_claude::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_opencode::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_opencode::HealthStatus) -> Self {
        match value {
            hypr_opencode::HealthStatus::Ready => Self::Ready,
            hypr_opencode::HealthStatus::Warning => Self::Warning,
            hypr_opencode::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_codex::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_codex::HealthAuthStatus) -> Self {
        match value {
            hypr_codex::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_codex::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_codex::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<hypr_claude::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_claude::HealthAuthStatus) -> Self {
        match value {
            hypr_claude::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_claude::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_claude::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<hypr_opencode::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_opencode::HealthAuthStatus) -> Self {
        match value {
            hypr_opencode::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_opencode::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_opencode::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

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
        ProviderKind::Codex => install_codex_cli(),
        ProviderKind::Claude => install_claude_cli(),
        ProviderKind::Opencode => install_opencode_cli(),
    }
}

pub fn uninstall_cli(request: UninstallCliRequest) -> Result<UninstallCliResponse, String> {
    match request.provider {
        ProviderKind::Codex => uninstall_codex_cli(),
        ProviderKind::Claude => uninstall_claude_cli(),
        ProviderKind::Opencode => uninstall_opencode_cli(),
    }
}

fn install_codex_cli() -> Result<InstallCliResponse, String> {
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();

    let mut table = hypr_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !hypr_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to replace existing notify handler in {}",
            config_path.display()
        ));
    }

    hypr_codex::set_notify(&mut table, command);
    hypr_codex::write_config(&config_path, &table)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Installed char as Codex notify handler in {}",
            config_path.display()
        ),
    })
}

fn install_claude_cli() -> Result<InstallCliResponse, String> {
    const COMMAND: &str = "char claude notify";

    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::upsert_command_hook(&mut settings, "Stop", COMMAND)?;
    hypr_claude::write_settings(&settings_path, &settings)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Claude,
        target_path: settings_path.clone(),
        message: format!(
            "Installed char as Claude Code hook handler in {}",
            settings_path.display()
        ),
    })
}

fn install_opencode_cli() -> Result<InstallCliResponse, String> {
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists() && !hypr_opencode::has_char_plugin(&plugin_path)? {
        return Err(format!(
            "refusing to replace existing plugin at {}",
            plugin_path.display()
        ));
    }

    hypr_opencode::write_plugin(&plugin_path)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Opencode,
        target_path: plugin_path.clone(),
        message: format!(
            "Installed char as OpenCode plugin at {}",
            plugin_path.display()
        ),
    })
}

fn uninstall_codex_cli() -> Result<UninstallCliResponse, String> {
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();
    let mut table = hypr_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !hypr_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to remove existing notify handler in {}",
            config_path.display()
        ));
    }

    hypr_codex::remove_notify(&mut table);
    hypr_codex::write_config(&config_path, &table)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Removed char as Codex notify handler from {}",
            config_path.display()
        ),
    })
}

fn uninstall_claude_cli() -> Result<UninstallCliResponse, String> {
    const COMMAND: &str = "char claude notify";

    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::remove_command_hook(&mut settings, "Stop", COMMAND)?;
    hypr_claude::write_settings(&settings_path, &settings)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Claude,
        target_path: settings_path.clone(),
        message: format!(
            "Removed char as Claude Code hook handler from {}",
            settings_path.display()
        ),
    })
}

fn uninstall_opencode_cli() -> Result<UninstallCliResponse, String> {
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists() && !hypr_opencode::has_char_plugin(&plugin_path)? {
        return Err(format!(
            "refusing to remove existing plugin at {}",
            plugin_path.display()
        ));
    }

    hypr_opencode::remove_plugin(&plugin_path)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Opencode,
        target_path: plugin_path.clone(),
        message: format!(
            "Removed char as OpenCode plugin from {}",
            plugin_path.display()
        ),
    })
}

fn codex_integration_installed() -> Result<bool, String> {
    let config_path = hypr_codex::config_path();
    let table = hypr_codex::read_config(&config_path)?;
    Ok(hypr_codex::has_notify(
        &table,
        &hypr_codex::notify_command(),
    ))
}

fn claude_integration_installed() -> Result<bool, String> {
    let settings_path = hypr_claude::settings_path();
    let settings = hypr_claude::read_settings(&settings_path)?;
    Ok(hypr_claude::has_command_hook(
        &settings,
        "Stop",
        "char claude notify",
    ))
}

fn opencode_integration_installed() -> Result<bool, String> {
    let plugin_path = hypr_opencode::plugin_path();
    hypr_opencode::has_char_plugin(&plugin_path)
}
