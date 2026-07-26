use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use hypr_cli_process::{spawn_streaming_lines, spawn_with_retry};
use serde::Serialize;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{EventStream, ThreadEvent};
use crate::options::{ApprovalMode, ModelReasoningEffort, SandboxMode, WebSearchMode};

const INTERNAL_ORIGINATOR_ENV: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
const RUST_SDK_ORIGINATOR: &str = "codex_sdk_rs";

#[derive(Debug, Clone)]
pub(crate) struct CodexExec {
    executable_path: PathBuf,
    env_override: Option<BTreeMap<String, String>>,
    config_overrides: toml::Table,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexExecArgs {
    pub input: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub thread_id: Option<String>,
    pub images: Vec<PathBuf>,
    pub model: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub working_directory: Option<PathBuf>,
    pub additional_directories: Vec<PathBuf>,
    pub skip_git_repo_check: bool,
    pub output_schema_file: Option<PathBuf>,
    pub model_reasoning_effort: Option<ModelReasoningEffort>,
    pub network_access_enabled: Option<bool>,
    pub web_search_mode: Option<WebSearchMode>,
    pub approval_mode: Option<ApprovalMode>,
    pub cancellation_token: Option<CancellationToken>,
}

pub(crate) struct CodexExecRun {
    pub events: EventStream,
    pub shutdown: CancellationToken,
}

impl CodexExec {
    pub(crate) fn new(
        executable_path: Option<PathBuf>,
        env_override: Option<BTreeMap<String, String>>,
        config_overrides: toml::Table,
    ) -> Self {
        Self {
            executable_path: executable_path.unwrap_or_else(|| PathBuf::from("codex")),
            env_override,
            config_overrides,
        }
    }

    pub(crate) fn run(&self, args: CodexExecArgs) -> Result<CodexExecRun, Error> {
        if args
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(Error::Cancelled);
        }

        let command_args = self.command_args(&args)?;

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

        command.env(INTERNAL_ORIGINATOR_ENV, RUST_SDK_ORIGINATOR);
        if let Some(api_key) = &args.api_key {
            command.env("CODEX_API_KEY", api_key);
        }

        let child = spawn_with_retry(&mut command).map_err(Error::Spawn)?;
        let prompt = args.input;
        let stream = spawn_streaming_lines(child, Some(prompt), args.cancellation_token, |line| {
            Ok(serde_json::from_str::<ThreadEvent>(&line)?)
        })?;

        Ok(CodexExecRun {
            events: stream.events,
            shutdown: stream.shutdown,
        })
    }

    fn command_args(&self, args: &CodexExecArgs) -> Result<Vec<String>, Error> {
        let mut command_args = vec!["exec".to_string(), "--experimental-json".to_string()];

        append_flagged_values(
            &mut command_args,
            "--config",
            serialize_config_overrides(&self.config_overrides)?,
        );

        if let Some(base_url) = &args.base_url {
            push_flagged_value(
                &mut command_args,
                "--config",
                format!("openai_base_url={}", toml_string(base_url)),
            );
        }

        if let Some(model) = &args.model {
            push_flagged_value(&mut command_args, "--model", model.clone());
        }

        if let Some(sandbox_mode) = args.sandbox_mode {
            push_flagged_value(
                &mut command_args,
                "--sandbox",
                serde_variant(&sandbox_mode)?,
            );
        }

        if let Some(working_directory) = &args.working_directory {
            push_flagged_value(
                &mut command_args,
                "--cd",
                working_directory.display().to_string(),
            );
        }

        append_flagged_values(
            &mut command_args,
            "--add-dir",
            args.additional_directories
                .iter()
                .map(|dir| dir.display().to_string()),
        );

        if args.skip_git_repo_check {
            command_args.push("--skip-git-repo-check".to_string());
        }

        if let Some(output_schema_file) = &args.output_schema_file {
            push_flagged_value(
                &mut command_args,
                "--output-schema",
                output_schema_file.display().to_string(),
            );
        }

        if let Some(reasoning_effort) = args.model_reasoning_effort {
            push_flagged_value(
                &mut command_args,
                "--config",
                format!(
                    "model_reasoning_effort={}",
                    toml_string(&serde_variant(&reasoning_effort)?)
                ),
            );
        }

        if let Some(network_access_enabled) = args.network_access_enabled {
            push_flagged_value(
                &mut command_args,
                "--config",
                format!("sandbox_workspace_write.network_access={network_access_enabled}"),
            );
        }

        if let Some(web_search_mode) = args.web_search_mode {
            push_flagged_value(
                &mut command_args,
                "--config",
                format!(
                    "web_search={}",
                    toml_string(&serde_variant(&web_search_mode)?)
                ),
            );
        }

        if let Some(approval_mode) = args.approval_mode {
            push_flagged_value(
                &mut command_args,
                "--config",
                format!(
                    "approval_policy={}",
                    toml_string(&serde_variant(&approval_mode)?)
                ),
            );
        }

        if let Some(thread_id) = &args.thread_id {
            command_args.push("resume".to_string());
            command_args.push(thread_id.clone());
        }

        append_flagged_values(
            &mut command_args,
            "--image",
            args.images.iter().map(|image| image.display().to_string()),
        );

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

fn serde_variant<T: Serialize>(value: &T) -> Result<String, Error> {
    let serialized = serde_json::to_value(value)?;
    match serialized {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(Error::ProcessFailed {
            detail: "enum failed to serialize to string".to_string(),
        }),
    }
}

fn serialize_config_overrides(table: &toml::Table) -> Result<Vec<String>, Error> {
    let mut overrides = Vec::new();
    flatten_config_table(table, String::new(), &mut overrides)?;
    Ok(overrides)
}

fn flatten_config_table(
    table: &toml::Table,
    prefix: String,
    overrides: &mut Vec<String>,
) -> Result<(), Error> {
    if !prefix.is_empty() && table.is_empty() {
        overrides.push(format!("{prefix}={{}}"));
        return Ok(());
    }

    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match value {
            toml::Value::Table(child) => flatten_config_table(child, path, overrides)?,
            _ => overrides.push(format!("{path}={}", toml_value(value)?)),
        }
    }
    Ok(())
}

fn toml_value(value: &toml::Value) -> Result<String, Error> {
    Ok(match value {
        toml::Value::String(value) => toml_string(value),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Array(values) => {
            let parts = values
                .iter()
                .map(toml_value)
                .collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", parts.join(", "))
        }
        toml::Value::Table(table) => {
            let parts = table
                .iter()
                .map(|(key, value)| toml_value(value).map(|value| format!("{key} = {value}")))
                .collect::<Result<Vec<_>, _>>()?;
            format!("{{{}}}", parts.join(", "))
        }
        toml::Value::Datetime(value) => toml_string(&value.to_string()),
    })
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use tokio_util::sync::CancellationToken;

    use super::{CodexExec, CodexExecArgs, serialize_config_overrides};
    use crate::options::{ApprovalMode, ModelReasoningEffort, SandboxMode, WebSearchMode};

    #[test]
    fn preserves_empty_nested_table_overrides() {
        let mut nested = toml::Table::new();
        nested.insert("child".to_string(), toml::Value::Table(toml::Table::new()));

        let mut config = toml::Table::new();
        config.insert("parent".to_string(), toml::Value::Table(nested));

        assert_eq!(
            serialize_config_overrides(&config).expect("overrides"),
            vec!["parent.child={}".to_string()]
        );
    }

    #[test]
    fn preserves_command_arg_shape() {
        let mut config_overrides = toml::Table::new();
        config_overrides.insert(
            "provider".to_string(),
            toml::Value::String("openai".to_string()),
        );

        let exec = CodexExec::new(
            Some(PathBuf::from("/bin/codex")),
            Some(BTreeMap::new()),
            config_overrides,
        );

        let args = CodexExecArgs {
            input: "hello".to_string(),
            base_url: Some("https://example.com/v1".to_string()),
            api_key: Some("key".to_string()),
            thread_id: Some("thread-1".to_string()),
            images: vec![PathBuf::from("/tmp/image.png")],
            model: Some("gpt-5".to_string()),
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            working_directory: Some(PathBuf::from("/tmp/workspace")),
            additional_directories: vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
            skip_git_repo_check: true,
            output_schema_file: Some(PathBuf::from("/tmp/schema.json")),
            model_reasoning_effort: Some(ModelReasoningEffort::High),
            network_access_enabled: Some(true),
            web_search_mode: Some(WebSearchMode::Live),
            approval_mode: Some(ApprovalMode::OnRequest),
            cancellation_token: Some(CancellationToken::new()),
        };

        assert_eq!(
            exec.command_args(&args).expect("command args"),
            vec![
                "exec".to_string(),
                "--experimental-json".to_string(),
                "--config".to_string(),
                "provider=\"openai\"".to_string(),
                "--config".to_string(),
                "openai_base_url=\"https://example.com/v1\"".to_string(),
                "--model".to_string(),
                "gpt-5".to_string(),
                "--sandbox".to_string(),
                "workspace-write".to_string(),
                "--cd".to_string(),
                "/tmp/workspace".to_string(),
                "--add-dir".to_string(),
                "/tmp/a".to_string(),
                "--add-dir".to_string(),
                "/tmp/b".to_string(),
                "--skip-git-repo-check".to_string(),
                "--output-schema".to_string(),
                "/tmp/schema.json".to_string(),
                "--config".to_string(),
                "model_reasoning_effort=\"high\"".to_string(),
                "--config".to_string(),
                "sandbox_workspace_write.network_access=true".to_string(),
                "--config".to_string(),
                "web_search=\"live\"".to_string(),
                "--config".to_string(),
                "approval_policy=\"on-request\"".to_string(),
                "resume".to_string(),
                "thread-1".to_string(),
                "--image".to_string(),
                "/tmp/image.png".to_string(),
            ]
        );
    }
}
