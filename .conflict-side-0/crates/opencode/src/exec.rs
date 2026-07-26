use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use hypr_cli_process::spawn_streaming_lines;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{Event, EventStream};

#[derive(Debug, Clone)]
pub(crate) struct OpencodeExec {
    executable_path: PathBuf,
    env_override: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct OpencodeExecArgs {
    pub input: String,
    pub session_id: Option<String>,
    pub continue_last: bool,
    pub fork: bool,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub working_directory: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub cancellation_token: Option<CancellationToken>,
}

pub(crate) struct OpencodeExecRun {
    pub events: EventStream,
    pub shutdown: CancellationToken,
}

impl OpencodeExec {
    pub(crate) fn new(
        executable_path: Option<PathBuf>,
        env_override: Option<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            executable_path: executable_path.unwrap_or_else(|| PathBuf::from("opencode")),
            env_override,
        }
    }

    pub(crate) fn run(&self, args: OpencodeExecArgs) -> Result<OpencodeExecRun, Error> {
        let command_args = self.command_args(&args);

        let mut command = Command::new(&self.executable_path);
        command.args(command_args);
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

        let child = command.spawn().map_err(Error::Spawn)?;
        let stream = spawn_streaming_lines(child, None, args.cancellation_token, |line| {
            Ok(serde_json::from_str::<Event>(&line)?)
        })?;

        Ok(OpencodeExecRun {
            events: stream.events,
            shutdown: stream.shutdown,
        })
    }

    fn command_args(&self, args: &OpencodeExecArgs) -> Vec<String> {
        let mut command_args = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];

        if args.continue_last {
            command_args.push("--continue".to_string());
        }

        if let Some(session_id) = &args.session_id {
            command_args.push("--session".to_string());
            command_args.push(session_id.clone());
        }

        if args.fork {
            command_args.push("--fork".to_string());
        }

        if let Some(model) = &args.model {
            command_args.push("--model".to_string());
            command_args.push(model.clone());
        }

        if let Some(agent) = &args.agent {
            command_args.push("--agent".to_string());
            command_args.push(agent.clone());
        }

        if let Some(hostname) = &args.hostname {
            command_args.push("--hostname".to_string());
            command_args.push(hostname.clone());
        }

        if let Some(port) = args.port {
            command_args.push("--port".to_string());
            command_args.push(port.to_string());
        }

        for file in &args.files {
            command_args.push("--file".to_string());
            command_args.push(file.display().to_string());
        }

        command_args.push("--prompt".to_string());
        command_args.push(args.input.clone());

        command_args
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{OpencodeExec, OpencodeExecArgs};

    #[test]
    fn builds_run_command_args() {
        let exec = OpencodeExec::new(Some(PathBuf::from("opencode")), None);

        let args = exec.command_args(&OpencodeExecArgs {
            input: "hello".to_string(),
            session_id: Some("session-1".to_string()),
            continue_last: false,
            fork: true,
            model: Some("openai/gpt-5".to_string()),
            agent: Some("builder".to_string()),
            hostname: Some("127.0.0.1".to_string()),
            port: Some(4096),
            working_directory: None,
            files: vec![PathBuf::from("/tmp/note.md")],
            cancellation_token: None,
        });

        assert_eq!(
            args,
            vec![
                "run",
                "--format",
                "json",
                "--session",
                "session-1",
                "--fork",
                "--model",
                "openai/gpt-5",
                "--agent",
                "builder",
                "--hostname",
                "127.0.0.1",
                "--port",
                "4096",
                "--file",
                "/tmp/note.md",
                "--prompt",
                "hello",
            ]
        );
    }

    #[test]
    fn defaults_binary_path() {
        let exec = OpencodeExec::new(None, Some(BTreeMap::new()));
        assert_eq!(exec.executable_path, PathBuf::from("opencode"));
    }
}
