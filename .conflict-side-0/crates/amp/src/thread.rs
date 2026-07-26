use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures_util::Stream;
use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{Input, RunStreamedResult, ThreadEvent, Turn};
use crate::exec::{AmpExec, AmpExecArgs};
use crate::options::{AmpOptions, ThreadOptions, TurnOptions};
use crate::settings::{SettingsFile, create_settings_file};

#[derive(Debug, Clone)]
pub struct Amp {
    exec: Arc<AmpExec>,
    options: AmpOptions,
}

impl Amp {
    pub fn new(options: AmpOptions) -> Self {
        let exec = AmpExec::new(options.amp_path_override.clone(), options.env.clone());
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
    exec: Arc<AmpExec>,
    options: AmpOptions,
    thread_options: ThreadOptions,
    id: Arc<Mutex<Option<String>>>,
}

impl Thread {
    pub(crate) fn new(
        exec: Arc<AmpExec>,
        options: AmpOptions,
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
        let input = input.into().normalize()?;
        let thread_id = self.id()?;
        let settings_file = create_settings_file(self.options.settings_overrides.as_ref())?;
        let settings_path = settings_file
            .as_ref()
            .map(|settings_file| settings_file.path().to_path_buf());

        let stream = self.exec.run(AmpExecArgs {
            input,
            amp_api_key: self.options.api_key.clone(),
            thread_id,
            mode: self.thread_options.mode,
            working_directory: self.thread_options.working_directory.clone(),
            include_thinking_stream: turn_options.include_thinking_stream,
            settings_file: settings_path,
            cancellation_token: turn_options.cancellation_token,
        })?;

        Ok(RunStreamedResult {
            events: Box::pin(ManagedEventStream {
                inner: stream.events,
                _settings_file: settings_file,
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
        let mut events = streamed.events;
        let mut items = Vec::new();
        let mut final_response = String::new();
        let mut usage = None;

        while let Some(event) = events.next().await {
            let event = event?;
            match &event {
                ThreadEvent::Assistant(message) => {
                    for content in &message.message.content {
                        if let crate::events::ContentBlock::Text { text } = content {
                            final_response = text.clone();
                        }
                    }
                    if let Some(message_usage) = &message.message.usage {
                        usage = Some(message_usage.clone());
                    }
                }
                ThreadEvent::Result(message) => {
                    final_response = message.result.clone();
                    if let Some(message_usage) = &message.base.usage {
                        usage = Some(message_usage.clone());
                    }
                }
                ThreadEvent::ErrorResult(message) => {
                    return Err(Error::TurnFailed(message.error.clone()));
                }
                ThreadEvent::SystemInit(_) | ThreadEvent::User(_) | ThreadEvent::Unknown(_) => {}
            }
            items.push(event);
        }

        Ok(Turn {
            events: items,
            final_response,
            usage,
        })
    }
}

struct ManagedEventStream {
    inner: crate::events::EventStream,
    _settings_file: Option<SettingsFile>,
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
            Poll::Ready(Some(Ok(event))) => {
                if let Some(thread_id) = event.session_id().map(ToOwned::to_owned) {
                    if let Ok(mut guard) = self.thread_id.lock() {
                        *guard = Some(thread_id);
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
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use futures_util::StreamExt;
    use tokio::runtime::Builder;
    use tokio_util::sync::CancellationToken;

    use super::{Amp, ManagedEventStream};
    use crate::error::Error;
    use crate::options::{AmpMode, AmpOptions, ThreadOptions, TurnOptions};

    #[test]
    fn run_without_turn_options_preserves_existing_behavior() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let args_file = temp_dir.path().join("args.txt");
        let stdin_file = temp_dir.path().join("stdin.txt");
        let script = write_fake_amp_script(
            &temp_dir,
            r#"
: > "$ARGS_FILE"
for arg in "$@"; do
  printf '%s\n' "$arg" >> "$ARGS_FILE"
done
cat > "$STDIN_FILE"
printf '%s\n' '{"type":"system","subtype":"init","cwd":"/tmp","session_id":"T-1","tools":[],"mcp_servers":[]}'
printf '%s\n' '{"type":"assistant","session_id":"T-1","message":{"type":"message","role":"assistant","content":[{"type":"text","text":"done"}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}},"parent_tool_use_id":null}'
printf '%s\n' '{"type":"result","subtype":"success","session_id":"T-1","is_error":false,"result":"done","duration_ms":1,"num_turns":1,"usage":{"input_tokens":1,"output_tokens":1}}'
"#,
        );

        let turn = runtime
            .block_on(
                test_amp(
                    script,
                    env_map([
                        ("ARGS_FILE", args_file.clone()),
                        ("STDIN_FILE", stdin_file.clone()),
                    ]),
                )
                .start_thread(ThreadOptions::default())
                .run("hello", TurnOptions::default()),
            )
            .expect("turn should succeed");

        assert_eq!(turn.final_response, "done");
        assert_eq!(turn.usage.expect("usage").output_tokens, 1);
        let args = fs::read_to_string(args_file).expect("args file");
        assert!(args.contains("--execute"));
        assert!(args.contains("--stream-json"));
        assert_eq!(fs::read_to_string(stdin_file).expect("stdin file"), "hello");
    }

    #[test]
    fn run_streamed_sets_thread_id_from_init_event() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let script = write_fake_amp_script(
            &temp_dir,
            r#"
printf '%s\n' '{"type":"system","subtype":"init","cwd":"/tmp","session_id":"T-99","tools":[],"mcp_servers":[]}'
printf '%s\n' '{"type":"result","subtype":"success","session_id":"T-99","is_error":false,"result":"ok","duration_ms":1,"num_turns":1}'
"#,
        );

        let thread = test_amp(script, BTreeMap::new()).start_thread(ThreadOptions::default());
        let streamed = runtime
            .block_on(thread.run_streamed("hello", TurnOptions::default()))
            .expect("stream");
        let events = runtime.block_on(async { streamed.events.collect::<Vec<_>>().await });

        assert_eq!(events.len(), 2);
        assert_eq!(thread.id().expect("id"), Some("T-99".to_string()));
    }

    #[test]
    fn dropping_managed_event_stream_cancels_shutdown_token() {
        let shutdown = CancellationToken::new();
        let stream = ManagedEventStream {
            inner: Box::pin(futures_util::stream::empty()),
            _settings_file: None,
            thread_id: Arc::new(Mutex::new(None)),
            shutdown: shutdown.clone(),
        };

        drop(stream);
        assert!(shutdown.is_cancelled());
    }

    #[test]
    fn cancellation_kills_child_process() {
        let runtime = runtime();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let pid_file = temp_dir.path().join("pid.txt");
        let script = write_fake_amp_script(
            &temp_dir,
            r#"
printf '%s' "$$" > "$PID_FILE"
printf '%s\n' '{"type":"system","subtype":"init","cwd":"/tmp","session_id":"T-cancel","tools":[],"mcp_servers":[]}'
while :; do sleep 1; done
"#,
        );

        let cancellation_token = CancellationToken::new();
        let thread = test_amp(script, env_map([("PID_FILE", pid_file.clone())])).start_thread(
            ThreadOptions {
                mode: Some(AmpMode::Rush),
                working_directory: None,
            },
        );

        let streamed = runtime
            .block_on(thread.run_streamed(
                "hello",
                TurnOptions {
                    cancellation_token: Some(cancellation_token.clone()),
                    include_thinking_stream: false,
                },
            ))
            .expect("stream");
        let mut events = streamed.events;
        let first = runtime.block_on(async { events.next().await.expect("first event") });
        assert!(first.is_ok());
        cancellation_token.cancel();
        let result = runtime.block_on(async { events.next().await.expect("event") });
        std::thread::sleep(Duration::from_millis(200));

        assert!(matches!(result, Err(Error::Cancelled)));
        let pid = fs::read_to_string(pid_file).expect("pid file");
        assert!(!process_exists(pid.trim()));
    }

    fn runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
    }

    fn test_amp(path: PathBuf, env: BTreeMap<String, String>) -> Amp {
        Amp::new(AmpOptions {
            amp_path_override: Some(path),
            api_key: None,
            settings_overrides: None,
            env: Some(env),
        })
    }

    fn env_map<const N: usize>(entries: [(&str, PathBuf); N]) -> BTreeMap<String, String> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.display().to_string()))
            .collect()
    }

    fn process_exists(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn write_fake_amp_script(temp_dir: &tempfile::TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_dir.path().join("amp");
        let script = format!("#!/bin/sh\nset -eu\n{body}");
        fs::write(&path, script).expect("write fake amp");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }
}
