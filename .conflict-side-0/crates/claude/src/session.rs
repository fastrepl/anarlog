use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures_util::Stream;

use crate::error::Error;
use crate::events::{
    ClaudeEvent, RunStreamedResult, Turn, Usage, final_response_from_value, session_id_from_value,
};
use crate::exec::{ClaudeExec, ClaudeExecArgs, SessionMode};
use crate::options::{ClaudeOptions, SessionOptions, TurnOptions};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Claude {
    exec: Arc<ClaudeExec>,
}

impl Claude {
    pub fn new(options: ClaudeOptions) -> Self {
        let exec = ClaudeExec::new(
            options.claude_path_override,
            options.env,
            options.settings,
            options.settings_sources,
        );
        Self {
            exec: Arc::new(exec),
        }
    }

    pub fn start_session(&self, options: SessionOptions) -> Session {
        Session::new(self.exec.clone(), options, SessionMode::Start)
    }

    pub fn resume_session(
        &self,
        id_or_name: impl Into<String>,
        options: SessionOptions,
    ) -> Session {
        Session::new(
            self.exec.clone(),
            options,
            SessionMode::Resume(id_or_name.into()),
        )
    }

    pub fn continue_session(&self, options: SessionOptions) -> Session {
        Session::new(self.exec.clone(), options, SessionMode::Continue)
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    exec: Arc<ClaudeExec>,
    options: SessionOptions,
    mode: SessionMode,
    id: Arc<Mutex<Option<String>>>,
}

impl Session {
    fn new(exec: Arc<ClaudeExec>, options: SessionOptions, mode: SessionMode) -> Self {
        Self {
            exec,
            options,
            mode,
            id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn id(&self) -> Result<Option<String>, Error> {
        self.id
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| Error::Poisoned)
    }

    pub async fn run_streamed(
        &self,
        prompt: impl Into<String>,
        turn_options: TurnOptions,
    ) -> Result<RunStreamedResult, Error> {
        let stream = self
            .exec
            .run_streamed(self.exec_args(prompt.into(), turn_options))?;
        Ok(RunStreamedResult {
            events: Box::pin(ManagedEventStream {
                inner: stream.events,
                session_id: self.id.clone(),
                shutdown: stream.shutdown,
            }),
        })
    }

    pub async fn run(
        &self,
        prompt: impl Into<String>,
        turn_options: TurnOptions,
    ) -> Result<Turn, Error> {
        let result = self
            .exec
            .run_json(self.exec_args(prompt.into(), turn_options))
            .await?;

        let event = ClaudeEvent::from_value(result.clone());
        if result.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
            return Err(Error::TurnFailed(
                event
                    .error_message()
                    .or_else(|| final_response_from_value(&result))
                    .unwrap_or_else(|| result.to_string()),
            ));
        }

        if let Some(session_id) = session_id_from_value(&result) {
            let mut id = self.id.lock().map_err(|_| Error::Poisoned)?;
            *id = Some(session_id.clone());
        }

        Ok(Turn {
            events: vec![event],
            final_response: final_response_from_value(&result),
            session_id: session_id_from_value(&result),
            usage: Usage::from_value(&result),
            result,
        })
    }

    fn exec_args(&self, prompt: String, turn_options: TurnOptions) -> ClaudeExecArgs {
        ClaudeExecArgs {
            prompt,
            session_mode: self.mode.clone(),
            model: self.options.model.clone(),
            working_directory: self.options.working_directory.clone(),
            additional_directories: self.options.additional_directories.clone(),
            permission_mode: self.options.permission_mode,
            append_system_prompt: self.options.append_system_prompt.clone(),
            system_prompt: self.options.system_prompt.clone(),
            max_turns: self.options.max_turns,
            include_partial_messages: self.options.include_partial_messages,
            include_hook_events: self.options.include_hook_events,
            fork_session: self.options.fork_session,
            session_name: self.options.session_name.clone(),
            output_schema: turn_options.output_schema,
            cancellation_token: turn_options.cancellation_token,
        }
    }
}

struct ManagedEventStream {
    inner: crate::events::EventStream,
    session_id: Arc<Mutex<Option<String>>>,
    shutdown: CancellationToken,
}

impl Stream for ManagedEventStream {
    type Item = Result<crate::events::ClaudeEvent, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => {
                if let Some(session_id) = event.session_id.clone() {
                    if let Ok(mut guard) = self.session_id.lock() {
                        *guard = Some(session_id);
                    }
                }
                Poll::Ready(Some(Ok(event)))
            }
            other => other,
        }
    }
}

impl Drop for ManagedEventStream {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use tokio::runtime::Builder;
    use tokio_util::sync::CancellationToken;

    use super::Claude;
    use crate::error::Error;
    use crate::options::{ClaudeOptions, SessionOptions, TurnOptions};

    #[test]
    fn run_returns_json_result_and_session_id() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
if [ "${1-}" = "-p" ]; then
  printf '%s\n' '{"type":"result","subtype":"success","result":"done","session_id":"session-1","num_turns":2}'
  exit 0
fi
exit 11
"#,
        );

        let session = test_claude(script, BTreeMap::new()).start_session(SessionOptions::default());
        let turn = runtime
            .block_on(session.run("hello", TurnOptions::default()))
            .expect("turn");

        assert_eq!(turn.final_response.as_deref(), Some("done"));
        assert_eq!(turn.session_id.as_deref(), Some("session-1"));
        assert_eq!(session.id().expect("id").as_deref(), Some("session-1"));
        assert_eq!(
            turn.result.get("type").and_then(serde_json::Value::as_str),
            Some("result")
        );
    }

    #[test]
    fn run_streamed_captures_session_id_from_events() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
printf '%s\n' '{"type":"init","session_id":"session-2"}'
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"partial"}]}}'
printf '%s\n' '{"type":"result","subtype":"success","result":"done","session_id":"session-2"}'
"#,
        );

        let session = test_claude(script, BTreeMap::new()).start_session(SessionOptions::default());
        let streamed = runtime
            .block_on(session.run_streamed("hello", TurnOptions::default()))
            .expect("stream");
        let events = runtime
            .block_on(async { streamed.events.collect::<Vec<_>>().await })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("events");

        assert_eq!(events.len(), 3);
        assert_eq!(session.id().expect("id").as_deref(), Some("session-2"));
        assert_eq!(
            events[1].partial_assistant_text().as_deref(),
            Some("partial")
        );
    }

    #[test]
    fn run_rejects_non_object_output_schema() {
        let runtime = runtime();
        let session = test_claude(PathBuf::from("/nonexistent-claude"), BTreeMap::new())
            .start_session(SessionOptions::default());

        let error = runtime
            .block_on(session.run(
                "hello",
                TurnOptions {
                    output_schema: Some(json!(["not", "an", "object"])),
                    cancellation_token: None,
                },
            ))
            .expect_err("schema");

        assert!(matches!(error, Error::InvalidOutputSchema));
    }

    #[test]
    fn run_streamed_supports_cancellation() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
sleep 10
"#,
        );

        let session = test_claude(script, BTreeMap::new()).start_session(SessionOptions::default());
        let cancellation_token = CancellationToken::new();
        cancellation_token.cancel();

        let error = match runtime.block_on(session.run_streamed(
            "hello",
            TurnOptions {
                output_schema: None,
                cancellation_token: Some(cancellation_token),
            },
        )) {
            Ok(_) => panic!("expected cancellation"),
            Err(error) => error,
        };

        assert!(matches!(error, Error::Cancelled));
    }

    fn runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn test_claude(script: PathBuf, env: BTreeMap<String, String>) -> Claude {
        Claude::new(ClaudeOptions {
            claude_path_override: Some(script),
            env: Some(env),
            ..ClaudeOptions::default()
        })
    }

    #[cfg(unix)]
    fn write_fake_claude_script(temp_dir: &tempfile::TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join("claude");
        let script = format!("#!/bin/sh\nset -eu\n{body}");
        fs::write(&path, script).expect("write fake claude");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }

    #[test]
    fn run_json_process_failure_includes_stderr() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
echo 'permission denied' >&2
exit 23
"#,
        );

        let session = test_claude(script, BTreeMap::new()).start_session(SessionOptions::default());
        let error = runtime
            .block_on(session.run("hello", TurnOptions::default()))
            .expect_err("failure");

        match error {
            Error::ProcessFailed { detail } => assert!(detail.contains("permission denied")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn run_streamed_emits_parse_errors_for_invalid_json() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_claude_script(
            &temp_dir,
            r#"
printf '%s\n' 'not-json'
"#,
        );

        let session = test_claude(script, BTreeMap::new()).start_session(SessionOptions::default());
        let streamed = runtime
            .block_on(session.run_streamed("hello", TurnOptions::default()))
            .expect("stream");

        let result = runtime.block_on(async { streamed.events.collect::<Vec<_>>().await });
        let error = result
            .into_iter()
            .next()
            .expect("event")
            .expect_err("parse error");
        assert!(matches!(error, Error::ParseJson(_)));
    }
}
