use std::ffi::OsString;
use std::time::Duration;

use crate::config::HooksConfig;
use crate::event::HookEvent;

const HOOK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct HookResult {
    pub command: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_hooks_for_event(config: &HooksConfig, event: HookEvent) -> Vec<HookResult> {
    let condition_key = event.condition_key();
    let cli_args = event.cli_args();

    let Some(hooks) = config.on.get(condition_key) else {
        return vec![];
    };

    let futures: Vec<_> = hooks
        .iter()
        .map(|hook_def| {
            let command = hook_def.command.clone();
            let args = cli_args.clone();
            async move { execute_hook(&command, &args).await }
        })
        .collect();

    futures_util::future::join_all(futures).await
}

async fn execute_hook(command: &str, args: &[OsString]) -> HookResult {
    let expanded = shellexpand::full(command)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| command.to_string());

    let parts: Vec<&str> = expanded.split_whitespace().collect();

    if parts.is_empty() {
        return HookResult {
            command: command.to_string(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: "empty command".to_string(),
        };
    }

    let mut cmd = tokio::process::Command::new(parts[0]);

    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    cmd.args(args);

    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return HookResult {
                command: command.to_string(),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("failed to spawn command: {}", e),
            };
        }
    };

    match tokio::time::timeout(HOOK_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => {
            let stdout = match child.stdout.take() {
                Some(mut stdout) => {
                    let mut buf = Vec::new();
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut buf).await;
                    String::from_utf8_lossy(&buf).to_string()
                }
                None => String::new(),
            };
            let stderr = match child.stderr.take() {
                Some(mut stderr) => {
                    let mut buf = Vec::new();
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buf).await;
                    String::from_utf8_lossy(&buf).to_string()
                }
                None => String::new(),
            };
            HookResult {
                command: command.to_string(),
                success: status.success(),
                exit_code: status.code(),
                stdout,
                stderr,
            }
        }
        Ok(Err(e)) => HookResult {
            command: command.to_string(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("failed to wait for command: {}", e),
        },
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            HookResult {
                command: command.to_string(),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("hook timed out after {} seconds", HOOK_TIMEOUT.as_secs()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_command() {
        let result = execute_hook("", &[]).await;
        assert!(!result.success);
        assert_eq!(result.stderr, "empty command");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn successful_command() {
        let result = execute_hook("echo hello", &[]).await;
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn failed_command() {
        let result = execute_hook("false", &[]).await;
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn with_cli_args() {
        let args = vec![OsString::from("world")];
        let result = execute_hook("echo", &args).await;
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "world");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn expands_home_env_var() {
        let home = std::env::var("HOME").unwrap();
        let result = execute_hook("echo $HOME", &[]).await;
        assert!(result.success);
        assert_eq!(result.stdout.trim(), home);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn expands_tilde_in_command_path() {
        let result = execute_hook("~/../../bin/echo tilde_works", &[]).await;
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "tilde_works");
    }

    #[tokio::test]
    async fn nonexistent_command() {
        let result = execute_hook("nonexistent_command_12345", &[]).await;
        assert!(!result.success);
        assert!(result.stderr.contains("failed to spawn command"));
    }
}
