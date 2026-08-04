use std::ffi::OsString;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::config::HooksConfig;
use crate::event::HookEvent;

const HOOK_TIMEOUT: Duration = Duration::from_secs(5);
const HOOK_OUTPUT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_HOOKS_PER_EVENT: usize = 32;
const MAX_CONCURRENT_HOOKS: usize = 4;
const MAX_HOOK_OUTPUT_BYTES: usize = 1024 * 1024;

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

    let commands = hooks
        .iter()
        .take(MAX_HOOKS_PER_EVENT)
        .map(|hook| hook.command.clone())
        .collect::<Vec<_>>();

    futures_util::stream::iter(commands.into_iter().map(|command| {
        let args = cli_args.clone();
        async move { execute_hook(&command, &args).await }
    }))
    .buffered(MAX_CONCURRENT_HOOKS)
    .collect()
    .await
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
        .kill_on_drop(true)
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

    let stdout_task = child.stdout.take().map(spawn_output_reader);
    let stderr_task = child.stderr.take().map(spawn_output_reader);

    match tokio::time::timeout(HOOK_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => {
            let stdout = collect_output(stdout_task).await;
            let stderr = collect_output(stderr_task).await;
            HookResult {
                command: command.to_string(),
                success: status.success(),
                exit_code: status.code(),
                stdout,
                stderr,
            }
        }
        Ok(Err(e)) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = collect_output(stdout_task).await;
            let _ = collect_output(stderr_task).await;
            HookResult {
                command: command.to_string(),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("failed to wait for command: {}", e),
            }
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = collect_output(stdout_task).await;
            let _ = collect_output(stderr_task).await;
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

fn spawn_output_reader(
    reader: impl AsyncRead + Unpin + Send + 'static,
) -> tokio::task::JoinHandle<String> {
    tokio::spawn(read_output_prefix(reader, MAX_HOOK_OUTPUT_BYTES))
}

async fn collect_output(task: Option<tokio::task::JoinHandle<String>>) -> String {
    let Some(mut task) = task else {
        return String::new();
    };

    match tokio::time::timeout(HOOK_OUTPUT_SHUTDOWN_TIMEOUT, &mut task).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => format!("[failed to collect hook output: {error}]"),
        Err(_) => {
            task.abort();
            "[hook output did not close]".to_string()
        }
    }
}

async fn read_output_prefix(mut reader: impl AsyncRead + Unpin, limit: usize) -> String {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    let mut truncated = false;

    loop {
        let read = match reader.read(&mut chunk).await {
            Ok(read) => read,
            Err(error) => {
                let mut output = String::from_utf8_lossy(&retained).into_owned();
                output.push_str(&format!("\n[failed to read hook output: {error}]"));
                return output;
            }
        };
        if read == 0 {
            break;
        }

        let remaining = limit.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&chunk[..keep]);
        truncated |= keep < read;
    }

    let mut output = String::from_utf8_lossy(&retained).into_owned();
    if truncated {
        output.push_str("\n[hook output truncated]");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hook_output_is_drained_but_retained_within_the_limit() {
        let output = read_output_prefix(&b"abcdefgh"[..], 4).await;
        assert_eq!(output, "abcd\n[hook output truncated]");
    }

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
