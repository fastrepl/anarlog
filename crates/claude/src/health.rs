use std::path::PathBuf;
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

use crate::options::ClaudeOptions;

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
    health_check_with_options(&ClaudeOptions::default())
}

pub fn health_check_with_options(options: &ClaudeOptions) -> HealthCheck {
    let binary_path = options
        .claude_path_override
        .clone()
        .unwrap_or_else(|| PathBuf::from("claude"));

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
                    "Claude Agent CLI (`claude`) is not installed or not on PATH.".to_string(),
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
                    "Failed to execute Claude Agent CLI health check: {error}."
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
                "Claude Agent CLI is installed but failed to run. {}",
                detail_from_output(&version_output)
            )),
        };
    }

    let auth_output = match run_command(&binary_path, options, &["auth", "status"]) {
        Ok(output) => output,
        Err(error) => {
            return HealthCheck {
                binary_path,
                installed: true,
                version,
                status: HealthStatus::Warning,
                auth_status: AuthStatus::Unknown,
                message: Some(format!(
                    "Could not verify Claude authentication status: {error}."
                )),
            };
        }
    };

    let (status, auth_status, message) = parse_auth_status(&auth_output);
    HealthCheck {
        binary_path,
        installed: true,
        version,
        status,
        auth_status,
        message,
    }
}

fn run_command(
    binary_path: &PathBuf,
    options: &ClaudeOptions,
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
        .find_map(|token| normalize_semver_token(token))
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

fn parse_auth_status(output: &Output) -> (HealthStatus, AuthStatus, Option<String>) {
    let text = combined_output(output);
    let lower = text.to_lowercase();

    if lower.contains("not logged in")
        || lower.contains("login required")
        || lower.contains("authentication required")
        || lower.contains("run `claude login`")
        || lower.contains("run `claude auth login`")
        || lower.contains("run claude login")
        || lower.contains("run claude auth login")
    {
        return (
            HealthStatus::Error,
            AuthStatus::Unauthenticated,
            Some("Claude is not authenticated. Run `claude auth login` and try again.".to_string()),
        );
    }

    if let Some(authenticated) = extract_auth_boolean(text.trim()) {
        return if authenticated {
            (HealthStatus::Ready, AuthStatus::Authenticated, None)
        } else {
            (
                HealthStatus::Error,
                AuthStatus::Unauthenticated,
                Some(
                    "Claude is not authenticated. Run `claude auth login` and try again."
                        .to_string(),
                ),
            )
        };
    }

    if output.status.success() {
        return (HealthStatus::Ready, AuthStatus::Authenticated, None);
    }

    (
        HealthStatus::Warning,
        AuthStatus::Unknown,
        Some(format!(
            "Could not verify Claude authentication status. {}",
            detail_from_output(output)
        )),
    )
}

fn extract_auth_boolean(text: &str) -> Option<bool> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    extract_auth_boolean_from_value(&value)
}

fn extract_auth_boolean_from_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Array(items) => items.iter().find_map(extract_auth_boolean_from_value),
        serde_json::Value::Object(map) => {
            for key in ["authenticated", "isAuthenticated", "loggedIn", "isLoggedIn"] {
                if let Some(value) = map.get(key).and_then(serde_json::Value::as_bool) {
                    return Some(value);
                }
            }

            for key in ["auth", "status", "session", "account"] {
                if let Some(value) = map.get(key) {
                    if let Some(nested) = extract_auth_boolean_from_value(value) {
                        return Some(nested);
                    }
                }
            }

            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::{ExitStatus, Output};
    use std::sync::Mutex;

    use tempfile::TempDir;

    use super::{
        AuthStatus, HealthStatus, extract_auth_boolean, health_check_with_options,
        normalize_semver_token, parse_auth_status,
    };
    use crate::options::ClaudeOptions;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_semver_tokens() {
        assert_eq!(normalize_semver_token("claude-2.1.92"), None);
        assert_eq!(
            normalize_semver_token("v2.1.92"),
            Some("2.1.92".to_string())
        );
        assert_eq!(normalize_semver_token("2.1.92"), Some("2.1.92".to_string()));
    }

    #[test]
    fn extracts_auth_boolean_from_json() {
        assert_eq!(
            extract_auth_boolean(r#"{"authenticated":true}"#),
            Some(true)
        );
        assert_eq!(
            extract_auth_boolean(r#"{"status":{"loggedIn":false}}"#),
            Some(false)
        );
    }

    #[test]
    fn marks_unauthenticated_output_as_error() {
        let output = Output {
            status: ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: b"Not logged in. Run `claude auth login`".to_vec(),
        };

        let (status, auth_status, message) = parse_auth_status(&output);
        assert_eq!(status, HealthStatus::Error);
        assert_eq!(auth_status, AuthStatus::Unauthenticated);
        assert!(message.is_some());
    }

    #[test]
    #[cfg(unix)]
    fn health_check_respects_env_override_without_leaking_parent_env() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
case "${1-} ${2-}" in
  "--version " )
    [ -z "${PARENT_ONLY:-}" ] || exit 11
    [ "${CUSTOM_ENV:-}" = "custom" ] || exit 12
    printf '2.1.92 (Claude Code)\n'
    ;;
  "auth status" )
    [ -z "${PARENT_ONLY:-}" ] || exit 13
    [ "${CUSTOM_ENV:-}" = "custom" ] || exit 14
    printf '{"authenticated":true}\n'
    ;;
  * )
    exit 15
    ;;
esac
"#,
        );

        unsafe {
            std::env::set_var("PARENT_ONLY", "leak");
        }

        let health = health_check_with_options(&ClaudeOptions {
            claude_path_override: Some(script),
            env: Some(BTreeMap::from([(
                "CUSTOM_ENV".to_string(),
                "custom".to_string(),
            )])),
            ..ClaudeOptions::default()
        });

        unsafe {
            std::env::remove_var("PARENT_ONLY");
        }

        assert_eq!(health.status, HealthStatus::Ready);
        assert_eq!(health.auth_status, AuthStatus::Authenticated);
        assert_eq!(health.version.as_deref(), Some("2.1.92"));
    }

    #[cfg(unix)]
    fn write_fake_claude_script(temp_dir: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join("claude");
        let script = format!("#!/bin/sh\nset -eu\n{body}");
        fs::write(&path, script).expect("write fake claude");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }
}
