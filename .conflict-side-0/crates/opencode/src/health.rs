use std::path::PathBuf;
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

use crate::options::OpencodeOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ready,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
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
    pub auth_status: AuthStatus,
    pub message: Option<String>,
}

pub fn health_check() -> HealthCheck {
    health_check_with_options(&OpencodeOptions::default())
}

pub fn health_check_with_options(options: &OpencodeOptions) -> HealthCheck {
    let binary_path = options
        .opencode_path_override
        .clone()
        .unwrap_or_else(|| PathBuf::from("opencode"));

    let version_output = match run_command(&binary_path, options, &["--version"]) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return HealthCheck {
                binary_path,
                installed: false,
                version: None,
                status: HealthStatus::Error,
                auth_status: AuthStatus::Unknown,
                message: Some(
                    "OpenCode CLI (`opencode`) is not installed or not on PATH.".to_string(),
                ),
            };
        }
        Err(error) => {
            return HealthCheck {
                binary_path,
                installed: false,
                version: None,
                status: HealthStatus::Error,
                auth_status: AuthStatus::Unknown,
                message: Some(format!(
                    "Failed to execute OpenCode CLI health check: {error}."
                )),
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
            auth_status: AuthStatus::Unknown,
            message: Some(format!(
                "OpenCode CLI is installed but failed to run. {}",
                detail_from_output(&version_output)
            )),
        };
    }

    HealthCheck {
        binary_path,
        installed: true,
        version,
        status: HealthStatus::Ready,
        auth_status: AuthStatus::Unknown,
        message: None,
    }
}

fn run_command(
    binary_path: &PathBuf,
    options: &OpencodeOptions,
    args: &[&str],
) -> Result<Output, std::io::Error> {
    let mut command = Command::new(binary_path);
    command.args(args);

    if let Some(env) = &options.env {
        command.env_clear();
        command.envs(env);
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
