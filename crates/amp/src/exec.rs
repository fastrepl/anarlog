use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use hypr_cli_process::{spawn_streaming_lines, spawn_with_retry};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{EventStream, NormalizedInput, ThreadEvent};
use crate::options::AmpMode;
#[derive(Debug, Clone)]
pub(crate) struct AmpExec {
    executable_path: PathBuf,
    env_override: Option<BTreeMap<String, String>>,
}

pub(crate) struct AmpExecRun {
    pub events: EventStream,
    pub shutdown: CancellationToken,
}

pub(crate) struct AmpExecArgs {
    pub input: NormalizedInput,
    pub amp_api_key: Option<String>,
    pub thread_id: Option<String>,
    pub mode: Option<AmpMode>,
    pub working_directory: Option<PathBuf>,
    pub include_thinking_stream: bool,
    pub settings_file: Option<PathBuf>,
    pub cancellation_token: Option<CancellationToken>,
}

impl AmpExec {
    pub(crate) fn new(
        executable_path: Option<PathBuf>,
        env_override: Option<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            executable_path: executable_path.unwrap_or_else(|| PathBuf::from("amp")),
            env_override,
        }
    }

    pub(crate) fn run(&self, args: AmpExecArgs) -> Result<AmpExecRun, Error> {
        if args
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(Error::Cancelled);
        }

        let command_args = self.command_args(&args);
        let mut command = Command::new(&self.executable_path);
        command.args(command_args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);

        if let Some(env) = &self.env_override {
            command.env_clear();
            for (key, value) in env {
                command.env(key, value);
            }
        }

        if let Some(api_key) = &args.amp_api_key {
            command.env("AMP_API_KEY", api_key);
        }

        if let Some(working_directory) = &args.working_directory {
            command.current_dir(working_directory);
        }

        let child = spawn_with_retry(&mut command).map_err(Error::Spawn)?;
        let prompt = match args.input {
            NormalizedInput::Text(text) => text,
            NormalizedInput::StreamJson(text) => text,
        };
        let stream = spawn_streaming_lines(child, Some(prompt), args.cancellation_token, |line| {
            let value = serde_json::from_str::<serde_json::Value>(&line)?;
            Ok(ThreadEvent::try_from(value)?)
        })?;

        Ok(AmpExecRun {
            events: stream.events,
            shutdown: stream.shutdown,
        })
    }

    pub(crate) fn command_args(&self, args: &AmpExecArgs) -> Vec<String> {
        let mut command_args = Vec::new();

        if let Some(thread_id) = &args.thread_id {
            command_args.push("threads".to_string());
            command_args.push("continue".to_string());
            command_args.push(thread_id.clone());
        }

        command_args.push("--execute".to_string());

        if let Some(mode) = args.mode {
            command_args.push("--mode".to_string());
            command_args.push(serde_variant(&mode));
        }

        command_args.push("--stream-json".to_string());

        if args.include_thinking_stream {
            command_args.push("--stream-json-thinking".to_string());
        }

        if matches!(args.input, NormalizedInput::StreamJson(_)) {
            command_args.push("--stream-json-input".to_string());
        }

        if let Some(settings_file) = &args.settings_file {
            command_args.push("--settings-file".to_string());
            command_args.push(settings_file.display().to_string());
        }

        command_args
    }
}

fn serde_variant(value: &AmpMode) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "smart".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use tokio_util::sync::CancellationToken;

    use super::{AmpExec, AmpExecArgs};
    use crate::events::NormalizedInput;
    use crate::options::AmpMode;

    #[test]
    fn preserves_command_arg_shape() {
        let exec = AmpExec::new(Some(PathBuf::from("/bin/amp")), Some(BTreeMap::new()));
        let args = AmpExecArgs {
            input: NormalizedInput::Text("hello".to_string()),
            amp_api_key: Some("key".to_string()),
            thread_id: Some("T-1".to_string()),
            mode: Some(AmpMode::Deep),
            working_directory: Some(PathBuf::from("/tmp/workspace")),
            include_thinking_stream: true,
            settings_file: Some(PathBuf::from("/tmp/settings.json")),
            cancellation_token: Some(CancellationToken::new()),
        };

        assert_eq!(
            exec.command_args(&args),
            vec![
                "threads".to_string(),
                "continue".to_string(),
                "T-1".to_string(),
                "--execute".to_string(),
                "--mode".to_string(),
                "deep".to_string(),
                "--stream-json".to_string(),
                "--stream-json-thinking".to_string(),
                "--settings-file".to_string(),
                "/tmp/settings.json".to_string(),
            ]
        );
    }

    #[test]
    fn adds_stream_json_input_flag_for_structured_input() {
        let exec = AmpExec::new(Some(PathBuf::from("/bin/amp")), None);
        let args = AmpExecArgs {
            input: NormalizedInput::StreamJson("{}".to_string()),
            amp_api_key: None,
            thread_id: None,
            mode: Some(AmpMode::Smart),
            working_directory: None,
            include_thinking_stream: false,
            settings_file: None,
            cancellation_token: None,
        };

        assert_eq!(
            exec.command_args(&args),
            vec![
                "--execute".to_string(),
                "--mode".to_string(),
                "smart".to_string(),
                "--stream-json".to_string(),
                "--stream-json-input".to_string(),
            ]
        );
    }
}
