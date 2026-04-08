use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures_util::Stream;
use futures_util::StreamExt;

use crate::error::Error;
use crate::events::{Input, RunStreamedResult, ThreadEvent, Turn};
use crate::exec::{CodexExec, CodexExecArgs};
use crate::options::{CodexOptions, ThreadOptions, TurnOptions};
use crate::output_schema::{OutputSchemaFile, create_output_schema_file};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Codex {
    exec: Arc<CodexExec>,
    options: CodexOptions,
}

impl Codex {
    pub fn new(options: CodexOptions) -> Self {
        let exec = CodexExec::new(
            options.codex_path_override.clone(),
            options.env.clone(),
            options.config.clone(),
        );
        Self {
            exec: Arc::new(exec),
            options,
        }
    }

    pub fn start_thread(&self, options: ThreadOptions) -> Thread {
        Thread::new(self.exec.clone(), self.options.clone(), options, None)
    }

    pub fn resume_thread(&self, id: impl Into<String>, options: ThreadOptions) -> Thread {
        Thread::new(
            self.exec.clone(),
            self.options.clone(),
            options,
            Some(id.into()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Thread {
    exec: Arc<CodexExec>,
    options: CodexOptions,
    thread_options: ThreadOptions,
    id: Arc<Mutex<Option<String>>>,
}

impl Thread {
    pub(crate) fn new(
        exec: Arc<CodexExec>,
        options: CodexOptions,
        thread_options: ThreadOptions,
        id: Option<String>,
    ) -> Self {
        Self {
            exec,
            options,
            thread_options,
            id: Arc::new(Mutex::new(id)),
        }
    }

    pub fn id(&self) -> Result<Option<String>, Error> {
        self.id
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| Error::Poisoned)
    }

    pub async fn run_streamed<I>(
        &self,
        input: I,
        turn_options: TurnOptions,
    ) -> Result<RunStreamedResult, Error>
    where
        I: Into<Input>,
    {
        let input = input.into();
        let thread_id = self.id()?;
        let (prompt, images) = input.normalize();
        let output_schema = create_output_schema_file(turn_options.output_schema.as_ref())?;
        let output_schema_path = output_schema
            .as_ref()
            .map(|schema_file| schema_file.path().to_path_buf());
        let stream = self.exec.run(CodexExecArgs {
            input: prompt,
            base_url: self.options.base_url.clone(),
            api_key: self.options.api_key.clone(),
            thread_id,
            images,
            model: self.thread_options.model.clone(),
            sandbox_mode: self.thread_options.sandbox_mode,
            working_directory: self.thread_options.working_directory.clone(),
            additional_directories: self.thread_options.additional_directories.clone(),
            skip_git_repo_check: self.thread_options.skip_git_repo_check,
            output_schema_file: output_schema_path,
            model_reasoning_effort: self.thread_options.model_reasoning_effort,
            network_access_enabled: self.thread_options.network_access_enabled,
            web_search_mode: self.thread_options.web_search_mode,
            approval_mode: self.thread_options.approval_mode,
            cancellation_token: turn_options.cancellation_token,
        })?;
        Ok(RunStreamedResult {
            events: Box::pin(ManagedEventStream {
                inner: stream.events,
                _output_schema: output_schema,
                thread_id: self.id.clone(),
                shutdown: stream.shutdown,
            }),
        })
    }

    pub async fn run<I>(&self, input: I, turn_options: TurnOptions) -> Result<Turn, Error>
    where
        I: Into<Input>,
    {
        let streamed = self.run_streamed(input, turn_options).await?;
        let mut items = Vec::new();
        let mut final_response = String::new();
        let mut usage = None;
        let mut events = streamed.events;

        while let Some(event) = events.next().await {
            let event = event?;
            match event {
                ThreadEvent::ThreadStarted { thread_id } => {
                    let mut id = self.id.lock().map_err(|_| Error::Poisoned)?;
                    *id = Some(thread_id);
                }
                ThreadEvent::ItemCompleted { item } => {
                    if item.item_type == "agent_message" {
                        if let Some(text) = item.text() {
                            final_response = text.to_string();
                        }
                    }
                    items.push(item);
                }
                ThreadEvent::TurnCompleted { usage: turn_usage } => {
                    usage = Some(turn_usage);
                }
                ThreadEvent::TurnFailed { error } => {
                    return Err(Error::TurnFailed(error.message));
                }
                ThreadEvent::Error { message } => {
                    return Err(Error::ProcessFailed { detail: message });
                }
                ThreadEvent::TurnStarted
                | ThreadEvent::ItemStarted { .. }
                | ThreadEvent::ItemUpdated { .. } => {}
            }
        }

        Ok(Turn {
            items,
            final_response,
            usage,
        })
    }
}

struct ManagedEventStream {
    inner: crate::events::EventStream,
    _output_schema: Option<OutputSchemaFile>,
    thread_id: Arc<Mutex<Option<String>>>,
    shutdown: CancellationToken,
}

impl Stream for ManagedEventStream {
    type Item = Result<ThreadEvent, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(ThreadEvent::ThreadStarted { thread_id }))) => {
                let thread_id = thread_id.clone();
                if let Ok(mut guard) = self.thread_id.lock() {
                    *guard = Some(thread_id.clone());
                }
                Poll::Ready(Some(Ok(ThreadEvent::ThreadStarted { thread_id })))
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
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    use futures_util::StreamExt;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::runtime::Builder;
    use tokio_util::sync::CancellationToken;

    use super::Codex;
    use crate::error::Error;
    use crate::options::{CodexOptions, ThreadOptions, TurnOptions};

    #[test]
    fn run_without_turn_options_preserves_existing_behavior() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let args_file = temp_dir.path().join("args.txt");
        let script = write_fake_codex_script(
            &temp_dir,
            r#"
: > "$ARGS_FILE"
for arg in "$@"; do
  printf '%s\n' "$arg" >> "$ARGS_FILE"
done
printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item-1","type":"agent_message","text":"done"}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}'
"#,
        );

        let thread = test_codex(script, env_map([("ARGS_FILE", args_file.clone())]))
            .start_thread(ThreadOptions::default());

        let turn = runtime
            .block_on(thread.run("hello", TurnOptions::default()))
            .expect("turn should succeed");

        assert_eq!(turn.final_response, "done");
        assert_eq!(turn.usage.expect("usage").output_tokens, 1);
        let args = fs::read_to_string(args_file).expect("args file");
        assert!(!args.contains("--output-schema"));
    }

    #[test]
    fn run_streamed_rejects_non_object_output_schema() {
        let runtime = runtime();
        let thread = test_codex(PathBuf::from("/nonexistent-codex"), BTreeMap::new())
            .start_thread(ThreadOptions::default());

        let error = match runtime.block_on(thread.run_streamed(
            "hello",
            TurnOptions {
                output_schema: Some(json!(null)),
                cancellation_token: None,
            },
        )) {
            Ok(_) => panic!("invalid schema should fail"),
            Err(error) => error,
        };

        assert!(matches!(error, Error::InvalidOutputSchema));
    }

    #[test]
    fn run_streamed_passes_output_schema_flag_and_cleans_up_file() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let args_file = temp_dir.path().join("args.txt");
        let schema_status_file = temp_dir.path().join("schema-status.txt");
        let schema_path_file = temp_dir.path().join("schema-path.txt");
        let script = write_fake_codex_script(
            &temp_dir,
            r#"
: > "$ARGS_FILE"
schema_path=""
prev=""
for arg in "$@"; do
  printf '%s\n' "$arg" >> "$ARGS_FILE"
  if [ "$prev" = "--output-schema" ]; then
    schema_path="$arg"
  fi
  prev="$arg"
done
if [ -n "$schema_path" ]; then
  if [ -f "$schema_path" ]; then
    printf 'exists' > "$SCHEMA_STATUS_FILE"
  else
    printf 'missing' > "$SCHEMA_STATUS_FILE"
  fi
  printf '%s' "$schema_path" > "$SCHEMA_PATH_FILE"
fi
printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}'
"#,
        );

        let thread = test_codex(
            script,
            env_map([
                ("ARGS_FILE", args_file.clone()),
                ("SCHEMA_STATUS_FILE", schema_status_file.clone()),
                ("SCHEMA_PATH_FILE", schema_path_file.clone()),
            ]),
        )
        .start_thread(ThreadOptions::default());

        let mut events = runtime
            .block_on(thread.run_streamed(
                "hello",
                TurnOptions {
                    output_schema: Some(json!({
                        "type": "object",
                        "properties": { "answer": { "type": "string" } }
                    })),
                    cancellation_token: None,
                },
            ))
            .expect("stream should start")
            .events;

        runtime.block_on(async { while events.next().await.is_some() {} });
        drop(events);

        let args = fs::read_to_string(args_file).expect("args file");
        assert!(args.contains("--output-schema"));
        assert_eq!(
            fs::read_to_string(schema_status_file).expect("schema status"),
            "exists"
        );

        let schema_path = PathBuf::from(
            fs::read_to_string(schema_path_file)
                .expect("schema path")
                .trim()
                .to_string(),
        );
        assert!(
            !schema_path
                .parent()
                .expect("schema file should have a parent")
                .exists()
        );
    }

    #[test]
    fn run_streamed_updates_thread_id_after_thread_started_event() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_codex_script(
            &temp_dir,
            r#"
printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}'
"#,
        );

        let thread = test_codex(script, BTreeMap::new()).start_thread(ThreadOptions::default());
        let mut events = runtime
            .block_on(thread.run_streamed("hello", TurnOptions::default()))
            .expect("stream should start")
            .events;

        runtime.block_on(async { while events.next().await.is_some() {} });

        assert_eq!(
            thread.id().expect("thread id"),
            Some("thread-1".to_string())
        );
    }

    #[test]
    fn run_streamed_returns_cancelled_when_token_is_already_cancelled() {
        let runtime = runtime();
        let token = CancellationToken::new();
        token.cancel();

        let thread = test_codex(PathBuf::from("/nonexistent-codex"), BTreeMap::new())
            .start_thread(ThreadOptions::default());

        let error = match runtime.block_on(thread.run_streamed(
            "hello",
            TurnOptions {
                output_schema: None,
                cancellation_token: Some(token),
            },
        )) {
            Ok(_) => panic!("cancelled turn should fail"),
            Err(error) => error,
        };

        assert!(matches!(error, Error::Cancelled));
    }

    #[test]
    fn run_streamed_cancels_in_flight_execution() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_codex_script(
            &temp_dir,
            r#"
/bin/sleep 30
"#,
        );
        let token = CancellationToken::new();
        let cancel_token = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel_token.cancel();
        });

        let thread = test_codex(script, BTreeMap::new()).start_thread(ThreadOptions::default());
        let streamed = runtime
            .block_on(thread.run_streamed(
                "hello",
                TurnOptions {
                    output_schema: None,
                    cancellation_token: Some(token),
                },
            ))
            .expect("stream should start");

        let first = runtime.block_on(async {
            let mut events = streamed.events;
            events.next().await
        });

        assert!(matches!(first, Some(Err(Error::Cancelled))));
    }

    #[test]
    #[cfg(unix)]
    fn dropping_streamed_events_terminates_codex_process() {
        let (tx, rx) = mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let runtime = runtime();
            let temp_dir = tempfile::tempdir().expect("tempdir");
            let pid_file = temp_dir.path().join("pid.txt");
            let script = write_fake_codex_script(
                &temp_dir,
                r#"
printf '%s' "$$" > "$PID_FILE"
/bin/sleep 30
"#,
            );

            let thread = test_codex(script, env_map([("PID_FILE", pid_file.clone())]))
                .start_thread(ThreadOptions::default());
            let streamed = runtime
                .block_on(thread.run_streamed("hello", TurnOptions::default()))
                .expect("stream should start");

            let pid = wait_for_pid(&pid_file);
            drop(streamed.events);

            let exited = wait_for_process_exit(&runtime, pid, Duration::from_secs(2));
            tx.send(exited).expect("send result");
        });

        assert!(
            rx.recv_timeout(Duration::from_secs(3))
                .expect("cleanup result"),
            "dropping the event stream should terminate codex"
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_streamed_does_not_deadlock_when_codex_writes_large_stderr() {
        let (tx, rx) = mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let runtime = runtime();
            let temp_dir = tempfile::tempdir().expect("tempdir");
            let script = write_fake_codex_script(
                &temp_dir,
                r#"
python3 - <<'PY'
import sys
sys.stderr.write("x" * (1024 * 1024))
sys.stderr.flush()
print('{"type":"thread.started","thread_id":"thread-1"}')
print('{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}')
PY
"#,
            );

            let thread = test_codex(script, BTreeMap::new()).start_thread(ThreadOptions::default());
            let result = runtime.block_on(async {
                let mut events = thread
                    .run_streamed("hello", TurnOptions::default())
                    .await
                    .expect("stream should start")
                    .events;
                events.next().await
            });

            tx.send(result.map(|event| event.is_ok()))
                .expect("send result");
        });

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(3))
                .expect("stream result"),
            Some(true)
        );
    }

    fn runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn test_codex(codex_path_override: PathBuf, env: BTreeMap<String, String>) -> Codex {
        Codex::new(CodexOptions {
            codex_path_override: Some(codex_path_override),
            env: Some(env),
            ..CodexOptions::default()
        })
    }

    fn env_map<const N: usize>(entries: [(&str, PathBuf); N]) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        if let Some(path) = std::env::var_os("PATH") {
            env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
        }
        for (key, value) in entries {
            env.insert(key.to_string(), value.display().to_string());
        }
        env
    }

    #[cfg(unix)]
    fn write_fake_codex_script(temp_dir: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join("codex");
        let script = format!("#!/bin/sh\nset -eu\n{body}");
        fs::write(&path, script).expect("write fake codex");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }

    #[cfg(unix)]
    fn wait_for_pid(pid_file: &std::path::Path) -> u32 {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            if let Ok(pid) = fs::read_to_string(pid_file) {
                return pid.trim().parse().expect("pid");
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for pid file"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn wait_for_process_exit(
        runtime: &tokio::runtime::Runtime,
        pid: u32,
        timeout: Duration,
    ) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            runtime.block_on(async {
                tokio::task::yield_now().await;
            });
            if !process_is_alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        !process_is_alive(pid)
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        Command::new("sh")
            .args(["-c", &format!("kill -0 {pid}")])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    fn write_fake_codex_script(_temp_dir: &TempDir, _body: &str) -> PathBuf {
        panic!("fake codex script tests are only supported on unix")
    }
}
