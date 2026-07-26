use std::path::PathBuf;
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

use crate::options::AmpOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ready,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmpAuthStatus {
    Authenticated,
    Unauthenticated,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub binary_path: PathBuf,
    pub installed: bool,
    pub version: Option<String>,
    pub status: HealthStatus,
    pub auth_status: AmpAuthStatus,
    pub message: Option<String>,
}

pub fn health_check() -> HealthCheck {
    health_check_with_options(&AmpOptions::default())
}

pub fn health_check_with_options(options: &AmpOptions) -> HealthCheck {
    let binary_path = options
        .amp_path_override
        .clone()
        .unwrap_or_else(|| PathBuf::from("amp"));

    let version_output = match run_command(&binary_path, options, &["--version"]) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return HealthCheck {
                binary_path,
                installed: false,
                version: None,
                status: HealthStatus::Error,
                auth_status: AmpAuthStatus::Unknown,
                message: Some("Amp CLI (`amp`) is not installed or not on PATH.".to_string()),
            };
        }
        Err(error) => {
            return HealthCheck {
                binary_path,
                installed: false,
                version: None,
                status: HealthStatus::Error,
                auth_status: AmpAuthStatus::Unknown,
                message: Some(format!("Failed to execute Amp CLI health check: {error}.")),
            };
        }
    };

    let version_text = combined_output(&version_output);
    let version = parse_version(&version_text);

    if !version_output.status.success() {
        return HealthCheck {
            binary_path,
            installed: true,
            version,
            status: HealthStatus::Error,
            auth_status: AmpAuthStatus::Unknown,
            message: Some(format!(
                "Amp CLI is installed but failed to run. {}",
                detail_from_output(&version_output)
            )),
        };
    }

    HealthCheck {
        binary_path,
        installed: true,
        version,
        status: HealthStatus::Ready,
        auth_status: AmpAuthStatus::Unknown,
        message: Some(
            "Amp authentication status could not be verified non-interactively.".to_string(),
        ),
    }
}

fn run_command(
    binary_path: &PathBuf,
    options: &AmpOptions,
    args: &[&str],
) -> Result<Output, std::io::Error> {
    let mut command = Command::new(binary_path);
    command.args(args);

    if let Some(env) = &options.env {
        command.env_clear();
        command.envs(env);
    }

    if let Some(api_key) = &options.api_key {
        command.env("AMP_API_KEY", api_key);
    }

    command.output()
}

fn combined_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.is_empty() {
        stdout.into_owned()
    } else if stdout.is_empty() {
        stderr.into_owned()
    } else {
        format!("{stdout}\n{stderr}")
    }
}

fn detail_from_output(output: &Output) -> String {
    let detail = combined_output(output).trim().to_string();
    if detail.is_empty() {
        match output.status.code() {
            Some(code) => format!("Command exited with code {code}."),
            None => "Command exited unsuccessfully.".to_string(),
        }
    } else {
        detail
    }
}

fn parse_version(output: &str) -> Option<String> {
    output
        .split(|c: char| c.is_whitespace())
        .find_map(normalize_semver_token)
}

fn normalize_semver_token(token: &str) -> Option<String> {
    let trimmed =
        token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'));
    let trimmed = trimmed.trim_start_matches('v');
    let mut parts = trimmed.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;

    if !(major.chars().all(|c| c.is_ascii_digit())
        && minor.chars().all(|c| c.is_ascii_digit())
        && patch
            .chars()
            .take_while(|c| *c != '-')
            .all(|c| c.is_ascii_digit()))
    {
        return None;
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::{AmpAuthStatus, HealthStatus, health_check_with_options, normalize_semver_token};
    use crate::options::AmpOptions;

    #[test]
    fn parses_semver_tokens() {
        assert_eq!(normalize_semver_token("amp-cli-0.23.1"), None);
        assert_eq!(
            normalize_semver_token("v0.23.1"),
            Some("0.23.1".to_string())
        );
        assert_eq!(normalize_semver_token("0.23.1"), Some("0.23.1".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn health_check_respects_env_override_without_leaking_parent_env() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_amp_script(
            &temp_dir,
            r#"
[ -z "${PARENT_ONLY:-}" ] || exit 11
[ "${CUSTOM_ENV:-}" = "custom" ] || exit 12
printf 'amp 0.23.1\n'
"#,
        );

        unsafe {
            std::env::set_var("PARENT_ONLY", "leak");
        }

        let health = health_check_with_options(&AmpOptions {
            amp_path_override: Some(script),
            api_key: None,
            settings_overrides: None,
            env: Some(BTreeMap::from([(
                "CUSTOM_ENV".to_string(),
                "custom".to_string(),
            )])),
        });

        unsafe {
            std::env::remove_var("PARENT_ONLY");
        }

        assert_eq!(health.status, HealthStatus::Ready);
        assert_eq!(health.auth_status, AmpAuthStatus::Unknown);
        assert_eq!(health.version.as_deref(), Some("0.23.1"));
    }

    #[cfg(unix)]
    fn write_fake_amp_script(temp_dir: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join("amp");
        let script = format!("#!/bin/sh\nset -eu\n{body}");
        fs::write(&path, script).expect("write fake amp");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }
}
