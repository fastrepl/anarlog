use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = hypr_opencode::health_check_with_options(&hypr_opencode::OpencodeOptions {
        opencode_path_override: options.opencode_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Opencode,
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
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists() && !hypr_opencode::is_char_plugin(&plugin_path)? {
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

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
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

pub fn upgrade() {
    upgrade_at(&hypr_opencode::plugin_path());
}

fn upgrade_at(plugin_path: &std::path::Path) {
    if hypr_opencode::is_char_plugin(plugin_path).unwrap_or(false) {
        let _ = hypr_opencode::write_plugin(plugin_path);
    }
}

fn integration_installed() -> Result<bool, String> {
    let plugin_path = hypr_opencode::plugin_path();
    hypr_opencode::is_char_plugin(&plugin_path)
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

impl From<hypr_opencode::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_opencode::HealthAuthStatus) -> Self {
        match value {
            hypr_opencode::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_opencode::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_opencode::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_does_not_create_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        upgrade_at(&path);

        assert!(!path.exists());
    }

    #[test]
    fn upgrade_does_not_add_hook_when_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");
        std::fs::write(&path, "export const plugin = {};\n").unwrap();

        upgrade_at(&path);

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "export const plugin = {};\n");
    }

    #[test]
    fn upgrade_refreshes_existing_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("char.ts");

        let old_plugin = r#"const child = Bun.spawn(["char", "opencode", "notify"]);"#;
        std::fs::write(&path, old_plugin).unwrap();

        upgrade_at(&path);

        assert!(hypr_opencode::has_char_plugin(&path).unwrap());
    }
}
