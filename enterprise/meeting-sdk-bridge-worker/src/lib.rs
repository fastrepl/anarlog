use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};

use anlg_meeting_capture::{
    BotState, CaptureEvent, MeetingSdkBridgeCommand, MeetingSdkBridgeError, MeetingSdkBridgeEvent,
    MeetingSdkBridgeEventPayload, MeetingSdkBridgeNormalizer, MeetingSdkBridgeStart,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{mpsc, watch},
};

const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_LINE_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct MeetingSdkBridgeProcessConfig {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub environment: Vec<(OsString, OsString)>,
    pub startup_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub max_line_bytes: usize,
}

impl MeetingSdkBridgeProcessConfig {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            environment: Vec::new(),
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        }
    }

    pub fn validate(self) -> Result<Self, MeetingSdkBridgeProcessConfigError> {
        if !self.executable.is_absolute() {
            return Err(MeetingSdkBridgeProcessConfigError::ExecutableMustBeAbsolute);
        }
        if self.startup_timeout.is_zero() || self.startup_timeout > Duration::from_secs(120) {
            return Err(MeetingSdkBridgeProcessConfigError::InvalidStartupTimeout);
        }
        if self.shutdown_timeout.is_zero() || self.shutdown_timeout > Duration::from_secs(60) {
            return Err(MeetingSdkBridgeProcessConfigError::InvalidShutdownTimeout);
        }
        if !(1024..=4 * 1024 * 1024).contains(&self.max_line_bytes) {
            return Err(MeetingSdkBridgeProcessConfigError::InvalidMaxLineBytes);
        }
        Ok(self)
    }
}

impl std::fmt::Debug for MeetingSdkBridgeProcessConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MeetingSdkBridgeProcessConfig")
            .field("executable", &self.executable)
            .field("argument_count", &self.args.len())
            .field(
                "environment",
                &self
                    .environment
                    .iter()
                    .map(|(key, _)| key)
                    .collect::<Vec<_>>(),
            )
            .field("startup_timeout", &self.startup_timeout)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("max_line_bytes", &self.max_line_bytes)
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MeetingSdkBridgeProcessConfigError {
    #[error("meeting SDK bridge executable path must be absolute")]
    ExecutableMustBeAbsolute,
    #[error("meeting SDK bridge startup timeout must be between one nanosecond and 120 seconds")]
    InvalidStartupTimeout,
    #[error("meeting SDK bridge shutdown timeout must be between one nanosecond and 60 seconds")]
    InvalidShutdownTimeout,
    #[error("meeting SDK bridge line limit must be between 1 KiB and 4 MiB")]
    InvalidMaxLineBytes,
}

pub struct MeetingSdkBridgeWorker {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    normalizer: MeetingSdkBridgeNormalizer,
    job_id: String,
    shutdown_timeout: Duration,
    max_line_bytes: usize,
}

impl MeetingSdkBridgeWorker {
    pub async fn launch(
        config: MeetingSdkBridgeProcessConfig,
        start: MeetingSdkBridgeStart,
    ) -> Result<(Self, CaptureEvent), MeetingSdkBridgeWorkerError> {
        let config = config.validate()?;
        start.validate()?;
        let normalizer = MeetingSdkBridgeNormalizer::new(&start.checkpoint)?;
        let job_id = start.checkpoint.job_id.clone();
        let mut child = Command::new(&config.executable)
            .args(&config.args)
            .env_clear()
            .envs(config.environment.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or(MeetingSdkBridgeWorkerError::MissingStdin)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(MeetingSdkBridgeWorkerError::MissingStdout)?;
        let mut worker = Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            normalizer,
            job_id,
            shutdown_timeout: config.shutdown_timeout,
            max_line_bytes: config.max_line_bytes,
        };
        worker
            .send_command(&MeetingSdkBridgeCommand::Start(start))
            .await?;
        let ready = tokio::time::timeout(config.startup_timeout, worker.read_event())
            .await
            .map_err(|_| MeetingSdkBridgeWorkerError::StartupTimeout)??;
        if !matches!(ready.payload, MeetingSdkBridgeEventPayload::Ready) {
            worker.terminate().await;
            return Err(MeetingSdkBridgeWorkerError::ReadyRequired);
        }
        let ready = worker.normalizer.accept(ready, chrono::Utc::now())?;
        Ok((worker, ready))
    }

    pub async fn run(
        mut self,
        events: mpsc::Sender<CaptureEvent>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<MeetingSdkBridgeWorkerOutcome, MeetingSdkBridgeWorkerError> {
        if *shutdown.borrow() {
            self.send_command(&MeetingSdkBridgeCommand::Stop {
                job_id: self.job_id.clone(),
            })
            .await?;
            return self.drain_shutdown(events).await;
        }
        loop {
            tokio::select! {
                event = self.read_event() => {
                    let event = self.normalizer.accept(event?, chrono::Utc::now())?;
                    let terminal = event_is_terminal(&event);
                    events.send(event).await.map_err(|_| MeetingSdkBridgeWorkerError::EventReceiverClosed)?;
                    if terminal {
                        let state = self.normalizer.state();
                        self.wait_or_terminate().await?;
                        return Ok(MeetingSdkBridgeWorkerOutcome::Terminal(state));
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.send_command(&MeetingSdkBridgeCommand::Stop {
                            job_id: self.job_id.clone(),
                        }).await?;
                        return self.drain_shutdown(events).await;
                    }
                }
            }
        }
    }

    async fn drain_shutdown(
        &mut self,
        events: mpsc::Sender<CaptureEvent>,
    ) -> Result<MeetingSdkBridgeWorkerOutcome, MeetingSdkBridgeWorkerError> {
        let result = tokio::time::timeout(self.shutdown_timeout, async {
            loop {
                let event = self.read_event().await?;
                let event = self.normalizer.accept(event, chrono::Utc::now())?;
                let terminal = event_is_terminal(&event);
                events
                    .send(event)
                    .await
                    .map_err(|_| MeetingSdkBridgeWorkerError::EventReceiverClosed)?;
                if terminal {
                    return Ok::<_, MeetingSdkBridgeWorkerError>(
                        MeetingSdkBridgeWorkerOutcome::Terminal(self.normalizer.state()),
                    );
                }
            }
        })
        .await;
        match result {
            Ok(result) => {
                self.wait_or_terminate().await?;
                result
            }
            Err(_) => {
                self.terminate().await;
                Err(MeetingSdkBridgeWorkerError::ShutdownTimeout)
            }
        }
    }

    async fn send_command(
        &mut self,
        command: &MeetingSdkBridgeCommand,
    ) -> Result<(), MeetingSdkBridgeWorkerError> {
        let mut encoded = serde_json::to_vec(command)?;
        if encoded.len() > self.max_line_bytes {
            return Err(MeetingSdkBridgeWorkerError::LineTooLarge);
        }
        encoded.push(b'\n');
        self.stdin.write_all(&encoded).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_event(&mut self) -> Result<MeetingSdkBridgeEvent, MeetingSdkBridgeWorkerError> {
        let line = read_line_bounded(&mut self.stdout, self.max_line_bytes).await?;
        serde_json::from_slice(&line).map_err(Into::into)
    }

    async fn wait_or_terminate(&mut self) -> Result<(), MeetingSdkBridgeWorkerError> {
        match tokio::time::timeout(self.shutdown_timeout, self.child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => Err(MeetingSdkBridgeWorkerError::SidecarFailed(status.code())),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => {
                self.terminate().await;
                Err(MeetingSdkBridgeWorkerError::ShutdownTimeout)
            }
        }
    }

    async fn terminate(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

fn event_is_terminal(event: &CaptureEvent) -> bool {
    matches!(
        &event.payload,
        anlg_meeting_capture::CaptureEventPayload::Lifecycle(transition)
            if transition.to.is_terminal()
    )
}

async fn read_line_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    max_line_bytes: usize,
) -> Result<Vec<u8>, MeetingSdkBridgeWorkerError> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Err(MeetingSdkBridgeWorkerError::SidecarExited);
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(take) > max_line_bytes.saturating_add(1) {
            return Err(MeetingSdkBridgeWorkerError::LineTooLarge);
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                return Err(MeetingSdkBridgeWorkerError::EmptyLine);
            }
            return Ok(line);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingSdkBridgeWorkerOutcome {
    Terminal(BotState),
}

#[derive(Debug, thiserror::Error)]
pub enum MeetingSdkBridgeWorkerError {
    #[error(transparent)]
    Config(#[from] MeetingSdkBridgeProcessConfigError),
    #[error(transparent)]
    Protocol(#[from] MeetingSdkBridgeError),
    #[error("meeting SDK bridge I/O failed")]
    Io(#[from] std::io::Error),
    #[error("meeting SDK bridge JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("meeting SDK bridge process did not expose stdin")]
    MissingStdin,
    #[error("meeting SDK bridge process did not expose stdout")]
    MissingStdout,
    #[error("meeting SDK bridge process did not become ready before the deadline")]
    StartupTimeout,
    #[error("meeting SDK bridge first event must be ready")]
    ReadyRequired,
    #[error("meeting SDK bridge process exited before a complete event")]
    SidecarExited,
    #[error("meeting SDK bridge process exited unsuccessfully with code {0:?}")]
    SidecarFailed(Option<i32>),
    #[error("meeting SDK bridge process did not stop before the deadline")]
    ShutdownTimeout,
    #[error("meeting SDK bridge line exceeded the configured size limit")]
    LineTooLarge,
    #[error("meeting SDK bridge emitted an empty line")]
    EmptyLine,
    #[error("capture event receiver closed")]
    EventReceiverClosed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_meeting_capture::{
        CaptureProviderKind, CaptureWorkerCheckpoint, MeetingPlatform, MeetingReference,
    };
    use tokio::io::AsyncWriteExt;

    #[test]
    fn requires_an_absolute_executable_and_bounded_limits() {
        assert!(matches!(
            MeetingSdkBridgeProcessConfig::new("relative-sidecar").validate(),
            Err(MeetingSdkBridgeProcessConfigError::ExecutableMustBeAbsolute)
        ));
        assert!(
            MeetingSdkBridgeProcessConfig::new("/opt/anarlog/teams-bridge")
                .validate()
                .is_ok()
        );

        let mut config = MeetingSdkBridgeProcessConfig::new("/opt/anarlog/teams-bridge");
        config.args.push("argument-secret".into());
        config
            .environment
            .push(("TEAMS_CLIENT_SECRET".into(), "environment-secret".into()));
        let debug = format!("{config:?}");
        assert!(debug.contains("TEAMS_CLIENT_SECRET"));
        assert!(!debug.contains("argument-secret"));
        assert!(!debug.contains("environment-secret"));
    }

    #[tokio::test]
    async fn reads_only_complete_bounded_ndjson_records() {
        let (mut writer, reader) = tokio::io::duplex(64);
        writer.write_all(b"{\"ok\":true}\n").await.unwrap();
        let mut reader = BufReader::new(reader);
        assert_eq!(
            read_line_bounded(&mut reader, 32).await.unwrap(),
            br#"{"ok":true}"#
        );

        let (mut writer, reader) = tokio::io::duplex(64);
        writer.write_all(b"0123456789\n").await.unwrap();
        let mut reader = BufReader::new(reader);
        assert!(matches!(
            read_line_bounded(&mut reader, 4).await,
            Err(MeetingSdkBridgeWorkerError::LineTooLarge)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervises_a_versioned_teams_sdk_sidecar_to_terminal_state() {
        let checkpoint = CaptureWorkerCheckpoint {
            job_id: "job-1".into(),
            bot_id: "bot-1".into(),
            provider: CaptureProviderKind::MicrosoftGraph,
            meeting: MeetingReference {
                platform: MeetingPlatform::MicrosoftTeams,
                url: "https://teams.microsoft.com/l/meetup-join/test".into(),
                external_id: None,
                calendar_event_id: None,
            },
            state: BotState::Queued,
            next_sequence: 0,
        };
        let start = MeetingSdkBridgeStart::new(checkpoint, "Anarlog Notetaker").unwrap();
        let mut config = MeetingSdkBridgeProcessConfig::new("/bin/sh");
        config.args = vec![
            "-c".into(),
            r#"read -r start
printf '%s\n' \
'{"protocolVersion":1,"sequence":0,"platform":"microsoft_teams","provider":"microsoft_graph","payload":{"type":"ready"}}' \
'{"protocolVersion":1,"sequence":1,"platform":"microsoft_teams","provider":"microsoft_graph","payload":{"type":"joined"}}' \
'{"protocolVersion":1,"sequence":2,"platform":"microsoft_teams","provider":"microsoft_graph","payload":{"type":"capturing"}}' \
'{"protocolVersion":1,"sequence":3,"platform":"microsoft_teams","provider":"microsoft_graph","payload":{"type":"terminal","data":{"state":"completed","reason":{"kind":"meeting_ended","retryable":false}}}}'"#.into(),
        ];

        let (worker, ready) = MeetingSdkBridgeWorker::launch(config, start).await.unwrap();
        assert!(matches!(
            ready.payload,
            anlg_meeting_capture::CaptureEventPayload::Lifecycle(ref transition)
                if transition.to == BotState::Launching
        ));
        let (events_tx, mut events_rx) = mpsc::channel(8);
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        assert_eq!(
            worker.run(events_tx, shutdown_rx).await.unwrap(),
            MeetingSdkBridgeWorkerOutcome::Terminal(BotState::Completed)
        );
        let mut states = Vec::new();
        while let Ok(event) = events_rx.try_recv() {
            if let anlg_meeting_capture::CaptureEventPayload::Lifecycle(transition) = event.payload
            {
                states.push(transition.to);
            }
        }
        assert_eq!(
            states,
            vec![BotState::Joined, BotState::Capturing, BotState::Completed]
        );
    }
}
