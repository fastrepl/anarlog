use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use hypr_cli_process::spawn_with_retry;

use crate::error::Error;
use crate::events::{ClaudeEvent, EventStream};
use crate::options::{PermissionMode, SettingSource};

#[derive(Debug, Clone)]
pub(crate) struct ClaudeExec {
    executable_path: PathBuf,
    env_override: Option<BTreeMap<String, String>>,
    settings: Option<serde_json::Value>,
    settings_sources: Option<Vec<SettingSource>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeExecArgs {
    pub prompt: String,
    pub session_mode: SessionMode,
    pub model: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub additional_directories: Vec<PathBuf>,
    pub permission_mode: Option<PermissionMode>,
    pub append_system_prompt: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub include_partial_messages: bool,
    pub include_hook_events: bool,
    pub fork_session: bool,
    pub session_name: Option<String>,
    pub output_schema: Option<serde_json::Value>,
    pub cancellation_token: Option<CancellationToken>,
}

#[derive(Debug, Clone)]
pub(crate) enum SessionMode {
    Start,
    Resume(String),
    Continue,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Json,
    StreamJson,
}

pub(crate) struct ClaudeExecStream {
    pub events: EventStream,
    pub shutdown: CancellationToken,
}

impl ClaudeExec {
    pub(crate) fn new(
        executable_path: Option<PathBuf>,
        env_override: Option<BTreeMap<String, String>>,
        settings: Option<serde_json::Value>,
        settings_sources: Option<Vec<SettingSource>>,
    ) -> Self {
        Self {
            executable_path: executable_path.unwrap_or_else(|| PathBuf::from("claude")),
            env_override,
            settings,
            settings_sources,
        }
    }

    pub(crate) async fn run_json(&self, args: ClaudeExecArgs) -> Result<serde_json::Value, Error> {
        if args
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(Error::Cancelled);
        }

        let mut command = self.build_command(&args, OutputFormat::Json)?;
        let mut child = spawn_with_retry(&mut command).map_err(Error::Spawn)?;
        let stdout = child.stdout.take().ok_or(Error::MissingStdout)?;
        let stderr = child.stderr.take();
        let cancellation_token = args.cancellation_token;
        let stderr_task = stderr.map(spawn_stderr_reader);
        let mut stdout_text = String::new();

        let read_stdout = async {
            let mut reader = BufReader::new(stdout);
            reader
                .read_to_string(&mut stdout_text)
                .await
                .map_err(Error::StdoutRead)
        };

        match cancellation_token.as_ref() {
            Some(token) => tokio::select! {
                _ = token.cancelled() => {
                    kill_child(&mut child).await?;
                    let _ = collect_stderr(stderr_task).await;
                    return Err(Error::Cancelled);
                }
                result = read_stdout => {
                    result?;
                }
            },
            None => {
                read_stdout.await?;
            }
        }

        let status = match cancellation_token.as_ref() {
            Some(token) => tokio::select! {
                _ = token.cancelled() => {
                    kill_child(&mut child).await?;
                    let _ = collect_stderr(stderr_task).await;
                    return Err(Error::Cancelled);
                }
                status = child.wait() => status.map_err(Error::Wait)?,
            },
            None => child.wait().await.map_err(Error::Wait)?,
        };

        let stderr_output = collect_stderr(stderr_task).await;
        if !status.success() {
            let detail = if let Some(code) = status.code() {
                format!("code {code}: {}", stderr_output.trim())
            } else {
                stderr_output.trim().to_string()
            };
            return Err(Error::ProcessFailed { detail });
        }

        Ok(serde_json::from_str(stdout_text.trim())?)
    }

    pub(crate) fn run_streamed(&self, args: ClaudeExecArgs) -> Result<ClaudeExecStream, Error> {
        if args
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(Error::Cancelled);
        }

        let mut command = self.build_command(&args, OutputFormat::StreamJson)?;
        let mut child = spawn_with_retry(&mut command).map_err(Error::Spawn)?;
        let stdout = child.stdout.take().ok_or(Error::MissingStdout)?;
        let stderr = child.stderr.take();
        let cancellation_token = args.cancellation_token;
        let shutdown = CancellationToken::new();
        let task_shutdown = shutdown.clone();
        let stderr_task = stderr.map(spawn_stderr_reader);
        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let result = async {
                let mut lines = BufReader::new(stdout).lines();
                loop {
                    let next_line = async { lines.next_line().await.map_err(Error::StdoutRead) };
                    let line = match cancellation_token.as_ref() {
                        Some(token) => tokio::select! {
                            _ = token.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Err(Error::Cancelled);
                            }
                            _ = task_shutdown.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Ok(());
                            }
                            line = next_line => line?,
                        },
                        None => tokio::select! {
                            _ = task_shutdown.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Ok(());
                            }
                            line = next_line => line?,
                        },
                    };

                    let Some(line) = line else {
                        break;
                    };

                    let event = ClaudeEvent::from_value(serde_json::from_str(&line)?);
                    if tx.send(Ok(event)).await.is_err() {
                        kill_child(&mut child).await?;
                        let _ = collect_stderr(stderr_task).await;
                        return Ok(());
                    }
                }

                let status = child.wait().await.map_err(Error::Wait)?;
                let stderr_output = collect_stderr(stderr_task).await;
                if !status.success() {
                    let detail = if let Some(code) = status.code() {
                        format!("code {code}: {}", stderr_output.trim())
                    } else {
                        stderr_output.trim().to_string()
                    };
                    return Err(Error::ProcessFailed { detail });
                }

                Ok(())
            }
            .await;

            if let Err(error) = result {
                let _ = tx.send(Err(error)).await;
            }
        });

        Ok(ClaudeExecStream {
            events: Box::pin(ReceiverStream::new(rx)),
            shutdown,
        })
    }

    fn build_command(
        &self,
        args: &ClaudeExecArgs,
        output_format: OutputFormat,
    ) -> Result<Command, Error> {
        let mut command = Command::new(&self.executable_path);
        command.args(self.command_args(args, output_format)?);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);

        if let Some(working_directory) = &args.working_directory {
            command.current_dir(working_directory);
        }

        if let Some(env) = &self.env_override {
            command.env_clear();
            for (key, value) in env {
                command.env(key, value);
            }
        }

        Ok(command)
    }

    fn command_args(
        &self,
        args: &ClaudeExecArgs,
        output_format: OutputFormat,
    ) -> Result<Vec<String>, Error> {
        let mut command_args = vec!["-p".to_string(), "--output-format".to_string()];
        command_args.push(match output_format {
            OutputFormat::Json => "json".to_string(),
            OutputFormat::StreamJson => "stream-json".to_string(),
        });

        match &args.session_mode {
            SessionMode::Start => {}
            SessionMode::Resume(session) => {
                push_flagged_value(&mut command_args, "--resume", session.clone());
            }
            SessionMode::Continue => command_args.push("--continue".to_string()),
        }

        if args.fork_session {
            command_args.push("--fork-session".to_string());
        }

        if let Some(settings) = &self.settings {
            push_flagged_value(
                &mut command_args,
                "--settings",
                serde_json::to_string(settings)?,
            );
        }

        if let Some(sources) = &self.settings_sources {
            push_flagged_value(
                &mut command_args,
                "--setting-sources",
                sources
                    .iter()
                    .map(|source| serde_variant(source))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        if let Some(model) = &args.model {
            push_flagged_value(&mut command_args, "--model", model.clone());
        }

        append_flagged_values(
            &mut command_args,
            "--add-dir",
            args.additional_directories
                .iter()
                .map(|dir| dir.display().to_string()),
        );

        if let Some(permission_mode) = args.permission_mode {
            push_flagged_value(
                &mut command_args,
                "--permission-mode",
                serde_variant(&permission_mode),
            );
        }

        if let Some(system_prompt) = &args.system_prompt {
            push_flagged_value(&mut command_args, "--system-prompt", system_prompt.clone());
        }

        if let Some(append_system_prompt) = &args.append_system_prompt {
            push_flagged_value(
                &mut command_args,
                "--append-system-prompt",
                append_system_prompt.clone(),
            );
        }

        if let Some(max_turns) = args.max_turns {
            push_flagged_value(&mut command_args, "--max-turns", max_turns.to_string());
        }

        if args.include_partial_messages && matches!(output_format, OutputFormat::StreamJson) {
            command_args.push("--include-partial-messages".to_string());
        }

        if args.include_hook_events && matches!(output_format, OutputFormat::StreamJson) {
            command_args.push("--include-hook-events".to_string());
        }

        if let Some(session_name) = &args.session_name {
            push_flagged_value(&mut command_args, "--name", session_name.clone());
        }

        if let Some(output_schema) = &args.output_schema {
            if !matches!(output_schema, serde_json::Value::Object(_)) {
                return Err(Error::InvalidOutputSchema);
            }
            push_flagged_value(
                &mut command_args,
                "--json-schema",
                serde_json::to_string(output_schema)?,
            );
        }

        command_args.push(args.prompt.clone());

        Ok(command_args)
    }
}

fn push_flagged_value(command_args: &mut Vec<String>, flag: &str, value: String) {
    command_args.push(flag.to_string());
    command_args.push(value);
}

fn append_flagged_values<I>(command_args: &mut Vec<String>, flag: &str, values: I)
where
    I: IntoIterator<Item = String>,
{
    for value in values {
        push_flagged_value(command_args, flag, value);
    }
}

fn serde_variant<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

async fn kill_child(child: &mut tokio::process::Child) -> Result<(), Error> {
    if child.try_wait().map_err(Error::Wait)?.is_some() {
        return Ok(());
    }

    match child.kill().await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(Error::Kill(error)),
    }

    child.wait().await.map_err(Error::Wait)?;
    Ok(())
}

fn spawn_stderr_reader(stderr: tokio::process::ChildStderr) -> JoinHandle<String> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).await.ok();
        buf
    })
}

async fn collect_stderr(stderr_task: Option<JoinHandle<String>>) -> String {
    match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::{ClaudeExec, ClaudeExecArgs, OutputFormat, SessionMode};
    use crate::options::{PermissionMode, SettingSource};

    fn exec() -> ClaudeExec {
        ClaudeExec::new(
            Some(PathBuf::from("claude")),
            None,
            Some(json!({ "foo": "bar" })),
            Some(vec![SettingSource::User, SettingSource::Project]),
        )
    }

    fn args() -> ClaudeExecArgs {
        ClaudeExecArgs {
            prompt: "hello".to_string(),
            session_mode: SessionMode::Resume("session-123".to_string()),
            model: Some("sonnet".to_string()),
            working_directory: Some(PathBuf::from("/tmp/project")),
            additional_directories: vec![PathBuf::from("../apps"), PathBuf::from("../lib")],
            permission_mode: Some(PermissionMode::Plan),
            append_system_prompt: Some("Append".to_string()),
            system_prompt: Some("System".to_string()),
            max_turns: Some(3),
            include_partial_messages: true,
            include_hook_events: true,
            fork_session: true,
            session_name: Some("named-session".to_string()),
            output_schema: Some(json!({ "type": "object" })),
            cancellation_token: None,
        }
    }

    #[test]
    fn command_args_include_headless_flags() {
        let command_args = exec()
            .command_args(&args(), OutputFormat::StreamJson)
            .expect("args");

        assert!(
            command_args
                .windows(2)
                .any(|pair| pair == ["--resume", "session-123"])
        );
        assert!(
            command_args
                .windows(2)
                .any(|pair| pair == ["--model", "sonnet"])
        );
        assert!(
            command_args
                .windows(2)
                .any(|pair| pair == ["--permission-mode", "plan"])
        );
        assert!(command_args.contains(&"--fork-session".to_string()));
        assert!(command_args.contains(&"--include-partial-messages".to_string()));
        assert!(command_args.contains(&"--include-hook-events".to_string()));
        assert!(
            command_args
                .windows(2)
                .any(|pair| pair == ["--setting-sources", "user,project"])
        );
        assert_eq!(command_args.last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn command_args_support_continue_mode() {
        let mut args = args();
        args.session_mode = SessionMode::Continue;

        let command_args = exec()
            .command_args(&args, OutputFormat::Json)
            .expect("args");

        assert!(command_args.contains(&"--continue".to_string()));
        assert!(!command_args.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn command_args_reject_non_object_schema() {
        let mut args = args();
        args.output_schema = Some(json!(["not", "an", "object"]));

        let error = exec()
            .command_args(&args, OutputFormat::Json)
            .expect_err("schema");

        assert!(matches!(error, crate::Error::InvalidOutputSchema));
    }
}
