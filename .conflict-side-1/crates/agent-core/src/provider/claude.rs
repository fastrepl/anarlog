use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

const STOP_EVENT: &str = "Stop";
const COMMAND: &str = "char claude notify";

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = hypr_claude::health_check_with_options(&hypr_claude::ClaudeOptions {
        claude_path_override: options.claude_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Claude,
        binary_path: health.binary_path,
        installed: health.installed,
        integration_installed: integration_installed().unwrap_or(false),
        version: health.version,
        status: health.status.into(),
        auth_status: health.auth_status.into(),
        message: health.message,
    }
}

pub fn install_cli() -> Result<InstallCliResponse, String> {
    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::upsert_command_hook(&mut settings, STOP_EVENT, COMMAND)?;
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

pub fn upgrade() {
    upgrade_at(&hypr_claude::settings_path());
}

fn upgrade_at(settings_path: &std::path::Path) {
    let Ok(mut settings) = hypr_claude::read_settings(settings_path) else {
        return;
    };
    if !hypr_claude::has_command_hook(&settings, STOP_EVENT, COMMAND) {
        return;
    }
    let _ = hypr_claude::remove_command_hook(&mut settings, STOP_EVENT, COMMAND);
    let _ = hypr_claude::upsert_command_hook(&mut settings, STOP_EVENT, COMMAND);
    let _ = hypr_claude::write_settings(settings_path, &settings);
}

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::remove_command_hook(&mut settings, STOP_EVENT, COMMAND)?;
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

fn integration_installed() -> Result<bool, String> {
    let settings_path = hypr_claude::settings_path();
    let settings = hypr_claude::read_settings(&settings_path)?;
    Ok(hypr_claude::has_command_hook(
        &settings, STOP_EVENT, COMMAND,
    ))
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

impl From<hypr_claude::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_claude::HealthAuthStatus) -> Self {
        match value {
            hypr_claude::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_claude::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_claude::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_does_not_create_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        upgrade_at(&path);

        assert!(!path.exists());
    }

    #[test]
    fn upgrade_does_not_add_hook_when_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();

        upgrade_at(&path);

        let settings = hypr_claude::read_settings(&path).unwrap();
        assert!(!hypr_claude::has_command_hook(
            &settings, STOP_EVENT, COMMAND
        ));
    }

    #[test]
    fn upgrade_refreshes_existing_hook() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = serde_json::json!({});
        hypr_claude::upsert_command_hook(&mut settings, STOP_EVENT, COMMAND).unwrap();
        hypr_claude::write_settings(&path, &settings).unwrap();

        upgrade_at(&path);

        let settings = hypr_claude::read_settings(&path).unwrap();
        assert!(hypr_claude::has_command_hook(
            &settings, STOP_EVENT, COMMAND
        ));
    }
}
