use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = anlg_codex::health_check_with_options(&anlg_codex::CodexOptions {
        codex_path_override: options.codex_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Codex,
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
    let config_path = anlg_codex::config_path();
    let command = anlg_codex::notify_command();

    let mut table = anlg_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !anlg_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to replace existing notify handler in {}",
            config_path.display()
        ));
    }

    anlg_codex::set_notify(&mut table, command);
    anlg_codex::write_config(&config_path, &table)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Installed char as Codex notify handler in {}",
            config_path.display()
        ),
    })
}

pub fn upgrade() {
    upgrade_at(&anlg_codex::config_path());
}

fn upgrade_at(config_path: &std::path::Path) {
    let command = anlg_codex::notify_command();
    let Ok(mut table) = anlg_codex::read_config(config_path) else {
        return;
    };
    if !anlg_codex::has_notify(&table, &command) {
        return;
    }
    anlg_codex::set_notify(&mut table, command);
    let _ = anlg_codex::write_config(config_path, &table);
}

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
    let config_path = anlg_codex::config_path();
    let command = anlg_codex::notify_command();
    let mut table = anlg_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !anlg_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to remove existing notify handler in {}",
            config_path.display()
        ));
    }

    anlg_codex::remove_notify(&mut table);
    anlg_codex::write_config(&config_path, &table)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Removed char as Codex notify handler from {}",
            config_path.display()
        ),
    })
}

fn integration_installed() -> Result<bool, String> {
    let config_path = anlg_codex::config_path();
    let table = anlg_codex::read_config(&config_path)?;
    Ok(anlg_codex::has_notify(
        &table,
        &anlg_codex::notify_command(),
    ))
}

impl From<anlg_codex::HealthStatus> for ProviderHealthStatus {
    fn from(value: anlg_codex::HealthStatus) -> Self {
        match value {
            anlg_codex::HealthStatus::Ready => Self::Ready,
            anlg_codex::HealthStatus::Warning => Self::Warning,
            anlg_codex::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<anlg_codex::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: anlg_codex::HealthAuthStatus) -> Self {
        match value {
            anlg_codex::HealthAuthStatus::Authenticated => Self::Authenticated,
            anlg_codex::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            anlg_codex::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_does_not_create_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        upgrade_at(&path);

        assert!(!path.exists());
    }

    #[test]
    fn upgrade_does_not_add_hook_when_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();

        upgrade_at(&path);

        let table = anlg_codex::read_config(&path).unwrap();
        assert!(!anlg_codex::has_notify(
            &table,
            &anlg_codex::notify_command()
        ));
    }

    #[test]
    fn upgrade_refreshes_existing_hook() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut table = toml::Table::new();
        let command = anlg_codex::notify_command();
        anlg_codex::set_notify(&mut table, command.clone());
        anlg_codex::write_config(&path, &table).unwrap();

        upgrade_at(&path);

        let table = anlg_codex::read_config(&path).unwrap();
        assert!(anlg_codex::has_notify(&table, &command));
    }
}
