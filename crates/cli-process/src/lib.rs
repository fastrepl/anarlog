use std::pin::Pin;
use std::time::Duration;

use futures_util::Stream;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub type EventStream<T, E> = Pin<Box<dyn Stream<Item = Result<T, E>> + Send>>;

pub const MAX_PROCESS_STDOUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PROCESS_STDOUT_LINE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PROCESS_STDERR_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("process missing stdin")]
    MissingStdin,
    #[error("process missing stdout")]
    MissingStdout,
    #[error("failed to write process stdin: {0}")]
    StdinWrite(#[source] std::io::Error),
    #[error("failed to read process stdout: {0}")]
    StdoutRead(#[source] std::io::Error),
    #[error("failed to wait for process: {0}")]
    Wait(#[source] std::io::Error),
    #[error("failed to kill process: {0}")]
    Kill(#[source] std::io::Error),
    #[error("process exited unsuccessfully: {detail}")]
    ProcessFailed { detail: String },
    #[error("process cancelled")]
    Cancelled,
}

// One handle owns the spawned child and its event stream: polling yields the
// parsed events, cancel() requests shutdown explicitly, and dropping the
// handle also shuts the child down, so the child can never be orphaned.
pub struct StreamProcess<T, E> {
    events: EventStream<T, E>,
    shutdown: CancellationToken,
}

impl<T, E> StreamProcess<T, E> {
    pub fn cancel(&self) {
        self.shutdown.cancel();
    }
}

impl<T, E> Stream for StreamProcess<T, E> {
    type Item = Result<T, E>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.get_mut().events.as_mut().poll_next(cx)
    }
}

impl<T, E> Drop for StreamProcess<T, E> {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

pub fn spawn_with_retry(command: &mut tokio::process::Command) -> std::io::Result<Child> {
    const MAX_EXECUTABLE_BUSY_RETRIES: usize = 5;
    const EXECUTABLE_BUSY_RETRY_DELAY: Duration = Duration::from_millis(20);

    let mut attempts = 0;
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && attempts < MAX_EXECUTABLE_BUSY_RETRIES =>
            {
                attempts += 1;
                std::thread::sleep(EXECUTABLE_BUSY_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

pub async fn run_to_string(
    mut child: Child,
    cancellation_token: Option<CancellationToken>,
) -> Result<String, ProcessError> {
    if cancellation_token
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        kill_child(&mut child).await?;
        return Err(ProcessError::Cancelled);
    }

    let Some(stdout) = child.stdout.take() else {
        kill_child(&mut child).await?;
        return Err(ProcessError::MissingStdout);
    };
    let stderr_task = child.stderr.take().map(spawn_stderr_reader);

    let read_stdout = async {
        read_text_with_limit(stdout, MAX_PROCESS_STDOUT_BYTES)
            .await
            .map_err(ProcessError::StdoutRead)
    };

    let stdout_result = match cancellation_token.as_ref() {
        Some(token) => tokio::select! {
            _ = token.cancelled() => {
                kill_child(&mut child).await?;
                let _ = collect_stderr(stderr_task).await;
                return Err(ProcessError::Cancelled);
            }
            result = read_stdout => {
                result
            }
        },
        None => read_stdout.await,
    };
    let stdout_text = match stdout_result {
        Ok(output) => output,
        Err(error) => {
            kill_child(&mut child).await?;
            let _ = collect_stderr(stderr_task).await;
            return Err(error);
        }
    };

    let status_result = match cancellation_token.as_ref() {
        Some(token) => tokio::select! {
            _ = token.cancelled() => {
                kill_child(&mut child).await?;
                let _ = collect_stderr(stderr_task).await;
                return Err(ProcessError::Cancelled);
            }
            status = child.wait() => status,
        },
        None => child.wait().await,
    };
    let status = match status_result {
        Ok(status) => status,
        Err(error) => {
            let _ = kill_child(&mut child).await;
            let _ = collect_stderr(stderr_task).await;
            return Err(ProcessError::Wait(error));
        }
    };

    let stderr_output = collect_stderr(stderr_task).await;
    if !status.success() {
        let detail = if let Some(code) = status.code() {
            format!("code {code}: {}", stderr_output.trim())
        } else {
            stderr_output.trim().to_string()
        };
        return Err(ProcessError::ProcessFailed { detail });
    }

    Ok(stdout_text)
}

pub fn spawn_streaming_lines<T, E, F>(
    mut child: Child,
    stdin_input: Option<String>,
    cancellation_token: Option<CancellationToken>,
    mut parse_line: F,
) -> Result<StreamProcess<T, E>, E>
where
    T: Send + 'static,
    E: From<ProcessError> + Send + 'static,
    F: FnMut(String) -> Result<T, E> + Send + 'static,
{
    if cancellation_token
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        tokio::spawn(async move {
            let _ = kill_child(&mut child).await;
        });
        return Err(E::from(ProcessError::Cancelled));
    }

    let Some(stdout) = child.stdout.take() else {
        tokio::spawn(async move {
            let _ = kill_child(&mut child).await;
        });
        return Err(E::from(ProcessError::MissingStdout));
    };
    let mut stderr_task = child.stderr.take().map(spawn_stderr_reader);
    let shutdown = CancellationToken::new();
    let task_shutdown = shutdown.clone();

    let (tx, rx) = mpsc::channel(64);

    tokio::spawn(async move {
        let result: Result<(), E> = async {
            if let Some(input) = stdin_input {
                let mut stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| E::from(ProcessError::MissingStdin))?;
                stdin
                    .write_all(input.as_bytes())
                    .await
                    .map_err(ProcessError::StdinWrite)
                    .map_err(E::from)?;
                stdin
                    .shutdown()
                    .await
                    .map_err(ProcessError::StdinWrite)
                    .map_err(E::from)?;
            }

            let mut lines = BufReader::new(stdout);
            loop {
                let next_line = async {
                    read_line_with_limit(&mut lines, MAX_PROCESS_STDOUT_LINE_BYTES)
                        .await
                        .map_err(ProcessError::StdoutRead)
                };
                let line = match cancellation_token.as_ref() {
                    Some(token) => tokio::select! {
                        _ = token.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task.take()).await;
                            return Err(E::from(ProcessError::Cancelled));
                        }
                        _ = task_shutdown.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task.take()).await;
                            return Ok(());
                        }
                        line = next_line => match line {
                            Ok(line) => line,
                            Err(error) => {
                                kill_child(&mut child).await.map_err(E::from)?;
                                let _ = collect_stderr(stderr_task.take()).await;
                                return Err(E::from(error));
                            }
                        },
                    },
                    None => tokio::select! {
                        _ = task_shutdown.cancelled() => {
                            kill_child(&mut child).await.map_err(E::from)?;
                            let _ = collect_stderr(stderr_task.take()).await;
                            return Ok(());
                        }
                        line = next_line => match line {
                            Ok(line) => line,
                            Err(error) => {
                                kill_child(&mut child).await.map_err(E::from)?;
                                let _ = collect_stderr(stderr_task.take()).await;
                                return Err(E::from(error));
                            }
                        },
                    },
                };

                let Some(line) = line else {
                    break;
                };

                let event = match parse_line(line) {
                    Ok(event) => event,
                    Err(error) => {
                        kill_child(&mut child).await.map_err(E::from)?;
                        let _ = collect_stderr(stderr_task.take()).await;
                        return Err(error);
                    }
                };
                if tx.send(Ok(event)).await.is_err() {
                    kill_child(&mut child).await.map_err(E::from)?;
                    let _ = collect_stderr(stderr_task.take()).await;
                    return Ok(());
                }
            }

            let status = child
                .wait()
                .await
                .map_err(ProcessError::Wait)
                .map_err(E::from)?;
            let stderr_output = collect_stderr(stderr_task.take()).await;
            if !status.success() {
                let detail = if let Some(code) = status.code() {
                    format!("code {code}: {}", stderr_output.trim())
                } else {
                    stderr_output.trim().to_string()
                };
                return Err(E::from(ProcessError::ProcessFailed { detail }));
            }

            Ok(())
        }
        .await;

        if let Err(error) = result {
            let _ = kill_child(&mut child).await;
            let _ = collect_stderr(stderr_task.take()).await;
            let _ = tx.send(Err(error)).await;
        }
    });

    Ok(StreamProcess {
        events: Box::pin(ReceiverStream::new(rx)),
        shutdown,
    })
}

async fn kill_child(child: &mut Child) -> Result<(), ProcessError> {
    if child.try_wait().map_err(ProcessError::Wait)?.is_some() {
        return Ok(());
    }

    match child.kill().await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(ProcessError::Kill(error)),
    }

    child.wait().await.map_err(ProcessError::Wait)?;
    Ok(())
}

pub fn spawn_stderr_reader(stderr: tokio::process::ChildStderr) -> JoinHandle<String> {
    tokio::spawn(async move {
        read_text_prefix(stderr, MAX_PROCESS_STDERR_BYTES)
            .await
            .unwrap_or_default()
    })
}

pub async fn collect_stderr(stderr_task: Option<JoinHandle<String>>) -> String {
    match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    }
}

pub async fn read_text_with_limit(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> std::io::Result<String> {
    let mut reader = reader.take(limit.saturating_add(1) as u64);
    let mut output = String::new();
    reader.read_to_string(&mut output).await?;
    if output.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("process output exceeded {limit} bytes"),
        ));
    }
    Ok(output)
}

pub async fn read_line_with_limit(
    reader: &mut (impl AsyncBufRead + Unpin),
    limit: usize,
) -> std::io::Result<Option<String>> {
    let mut line = Vec::new();

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            break;
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        let content = newline.map_or(available, |index| &available[..index]);
        if line.len().saturating_add(content.len()) > limit.saturating_add(1) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("process output line exceeded {limit} bytes"),
            ));
        }
        line.extend_from_slice(content);
        reader.consume(consumed);

        if newline.is_some() {
            break;
        }
    }

    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if line.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("process output line exceeded {limit} bytes"),
        ));
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

async fn read_text_prefix(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> std::io::Result<String> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut chunk).await?;
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
        output.push_str("\n[process stderr truncated]");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use tokio::io::BufReader;
    use tokio_util::sync::CancellationToken;

    use super::{
        ProcessError, read_line_with_limit, read_text_prefix, read_text_with_limit, run_to_string,
        spawn_streaming_lines,
    };

    #[tokio::test]
    async fn bounded_text_rejects_oversized_output() {
        let error = read_text_with_limit(&b"12345"[..], 4)
            .await
            .expect_err("oversized output must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn bounded_line_handles_crlf_and_rejects_oversized_lines() {
        let mut reader = BufReader::new(&b"hello\r\ntoolong\n"[..]);
        assert_eq!(
            read_line_with_limit(&mut reader, 5).await.unwrap(),
            Some("hello".to_string())
        );
        let error = read_line_with_limit(&mut reader, 6)
            .await
            .expect_err("oversized line must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn stderr_reader_drains_but_only_retains_prefix() {
        let output = read_text_prefix(&b"abcdefgh"[..], 4).await.unwrap();
        assert_eq!(output, "abcd\n[process stderr truncated]");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn already_cancelled_processes_are_terminated() {
        let marker = std::env::temp_dir().join(format!(
            "cli-process-cancelled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = delayed_marker_child(&marker);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        assert!(matches!(
            run_to_string(child, Some(cancellation)).await,
            Err(ProcessError::Cancelled)
        ));
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        assert!(!marker.exists());

        let child = delayed_marker_child(&marker);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result =
            spawn_streaming_lines::<String, ProcessError, _>(child, None, Some(cancellation), Ok);
        assert!(matches!(result, Err(ProcessError::Cancelled)));
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        assert!(!marker.exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn streams_complete_on_eof_and_drain_all_output() {
        let mut command = tokio::process::Command::new("sh");
        command
            .args(["-c", "printf 'one\ntwo\nthree\n'"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();

        let stream =
            spawn_streaming_lines::<String, ProcessError, _>(child, None, None, Ok).unwrap();
        let lines: Vec<_> = futures_util::StreamExt::collect::<Vec<_>>(stream)
            .await
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert_eq!(lines, ["one", "two", "three"]);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn dropping_the_stream_handle_kills_the_child() {
        let marker = std::env::temp_dir().join(format!(
            "cli-process-drop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = delayed_marker_child(&marker);

        let stream =
            spawn_streaming_lines::<String, ProcessError, _>(child, None, None, Ok).unwrap();
        drop(stream);
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        assert!(!marker.exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn explicit_cancel_kills_the_child_and_ends_the_stream() {
        let marker = std::env::temp_dir().join(format!(
            "cli-process-explicit-cancel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = delayed_marker_child(&marker);

        let mut stream =
            spawn_streaming_lines::<String, ProcessError, _>(child, None, None, Ok).unwrap();
        stream.cancel();
        // Explicit shutdown ends the stream without an error event.
        while futures_util::StreamExt::next(&mut stream)
            .await
            .transpose()
            .unwrap()
            .is_some()
        {}
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    fn delayed_marker_child(marker: &std::path::Path) -> tokio::process::Child {
        let mut command = tokio::process::Command::new("sh");
        command
            .args(["-c", "sleep 0.2; touch \"$1\"", "_"])
            .arg(marker)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.spawn().unwrap()
    }
}
