//! Anarlog's native meeting engine: the `anarlog-meeting-capture` helper (tools/meeting-capture)
//! records the microphone and the call's process tap on one host clock, journals both 48 kHz
//! tracks to the runtime's staging directory, and streams them here. This adapter converts the
//! audio once to the live rate, pads pauses and gaps with `Gap`, cancels a raw microphone's echo
//! against the call track, and reports the call, provenance, track health and roster observations
//! through `anlg_audio::meeting`.

mod convert;
mod dispatch;
mod echo;
mod recovery;
mod wire;

pub use recovery::{RecoveredTracks, StagedJournal, read_journal, recover_tracks};

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc as std_mpsc};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use anlg_audio::meeting::{
    CallApp, CallPlatform, CallStatus, CaptureEngine, CaptureProvenance, CaptureScope,
    EngineAvailability, MeetingCapture, MeetingCaptureControl, MeetingCaptureEngine,
    MeetingCaptureRequest, NATIVE_SAMPLE_RATE, ScopeRequest,
};
use anlg_audio::{CaptureEvent, CaptureStream, Error};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, Stream};
use tokio::sync::{mpsc, oneshot, watch};

use convert::Converter;
use dispatch::{Ack, Dispatcher, Output};
use echo::EchoCanceller;
use wire::Message;

pub const HELPER_NAME: &str = "anarlog-meeting-capture";
pub const HELPER_ENV: &str = "ANARLOG_MEETING_CAPTURE_HELPER";
/// Qualification only: `voice`, `voice-advanced-ducking` or `plain` instead of `automatic`.
pub const MIC_PROCESSING_ENV: &str = "ANARLOG_MEETING_MIC_PROCESSING";
const PROTOCOL_VERSION: &str = "1";
const STREAM_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    /// Microphone settling and one accessibility read happen before `started`.
    pub start: Duration,
    pub control: Duration,
    /// Finishing tracks can include padding a long pause to the stop time.
    pub stop: Duration,
    pub probe: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            start: Duration::from_secs(30),
            control: Duration::from_secs(8),
            stop: Duration::from_secs(120),
            probe: Duration::from_secs(5),
        }
    }
}

/// The helper executable, if this install has one. The environment override is for
/// qualification; packaged apps carry it beside the main executable (Tauri `externalBin`).
pub fn helper_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(HELPER_ENV) {
        return Some(PathBuf::from(path)).filter(|path| path.is_file());
    }
    let beside = std::env::current_exe().ok()?.parent()?.join(HELPER_NAME);
    if beside.is_file() {
        return Some(beside);
    }
    None
}

/// The process-wide engine `ActualAudio` hands out, so the helper probe runs once.
pub fn shared_engine() -> Arc<NativeMeetingEngine> {
    static ENGINE: OnceLock<Arc<NativeMeetingEngine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| Arc::new(NativeMeetingEngine::new(helper_path())))
        .clone()
}

pub struct NativeMeetingEngine {
    supported: bool,
    helper: Option<PathBuf>,
    timeouts: Timeouts,
    probe: OnceLock<Result<(), String>>,
}

impl NativeMeetingEngine {
    pub fn new(helper: Option<PathBuf>) -> Self {
        Self::with_timeouts(helper, Timeouts::default())
    }

    pub fn with_timeouts(helper: Option<PathBuf>, timeouts: Timeouts) -> Self {
        Self {
            supported: cfg!(target_os = "macos"),
            helper,
            timeouts,
            probe: OnceLock::new(),
        }
    }

    fn helper(&self) -> Result<&Path, Error> {
        if let EngineAvailability::Unavailable { reason } = self.availability() {
            return Err(Error::MeetingCaptureUnavailable(reason));
        }
        Ok(self.helper.as_deref().expect("availability checked"))
    }

    /// Runs the helper once to check it starts on this macOS and speaks this protocol.
    fn probe(&self, helper: &Path) -> Result<(), String> {
        let messages = run_to_end(helper, &["version"], self.timeouts.probe)?;
        let version = messages.iter().find_map(|message| match message {
            Message::Event(line) if line.event() == "version" => line.get("protocol"),
            _ => None,
        });
        match version {
            Some(PROTOCOL_VERSION) => Ok(()),
            Some(other) => Err(format!("the meeting helper speaks protocol {other}")),
            None => Err("the meeting helper did not report its version".into()),
        }
    }
}

impl MeetingCaptureEngine for NativeMeetingEngine {
    fn availability(&self) -> EngineAvailability {
        if !self.supported {
            return EngineAvailability::Unavailable {
                reason: "the native meeting engine is macOS-only".into(),
            };
        }
        let Some(helper) = &self.helper else {
            return EngineAvailability::Unavailable {
                reason: format!("{HELPER_NAME} is not installed"),
            };
        };
        match self.probe.get_or_init(|| self.probe(helper)) {
            // The helper records only the Mac's default input; it never switches it.
            Ok(()) => EngineAvailability::Available {
                selects_microphone: false,
            },
            Err(reason) => EngineAvailability::Unavailable {
                reason: reason.clone(),
            },
        }
    }

    fn calls_in_progress(&self) -> Result<Vec<CallApp>, Error> {
        let helper = self.helper()?;
        let messages = run_to_end(helper, &["calls"], self.timeouts.probe)
            .map_err(Error::MeetingCaptureFailed)?;
        Ok(messages
            .into_iter()
            .filter_map(|message| match message {
                Message::Event(line) if line.event() == "call" => Some(CallApp {
                    bundle_id: line.get("bundleId")?.to_string(),
                    platform: CallPlatform::from_helper(line.get("platform")?)?,
                    status: match line.get("status")? {
                        "in-call" => CallStatus::InCall,
                        "not-in-call" => CallStatus::NotInCall,
                        _ => CallStatus::Unknown,
                    },
                    call_url: line.get("callUrl").map(str::to_string),
                }),
                _ => None,
            })
            .collect())
    }

    fn open(&self, request: MeetingCaptureRequest) -> Result<MeetingCapture, Error> {
        let helper = self.helper()?.to_path_buf();
        let converter = Converter::new(request.config.sample_rate, request.config.chunk_size)
            .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))?;
        let mut child = Command::new(&helper)
            .args(record_arguments(&request))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Error::MeetingCaptureUnavailable(error.to_string()))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let stdin = child.stdin.take();
        let (stream_tx, stream_rx) = mpsc::channel(STREAM_CAPACITY);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (finished_tx, finished_rx) = watch::channel(None);
        let shared = Arc::new(Shared {
            stdin: Mutex::new(stdin),
            child: Mutex::new(Some(child)),
            pending: Mutex::new(None),
            parked: Mutex::new(None),
            finished: finished_rx,
            staging_dir: request.staging_dir.clone(),
            timeouts: self.timeouts,
        });
        let stderr_tail = Arc::new(Mutex::new(String::new()));
        spawn_stderr(stderr, stderr_tail.clone());
        let (started_tx, started_rx) = std_mpsc::channel();
        let cancel_echo = Arc::new(AtomicBool::new(request.config.enable_aec));
        let echo = EchoCanceller::new(request.config.sample_rate, cancel_echo.clone());
        let dispatcher = Dispatcher::new(request.capture.clone(), converter, echo);
        let reader = Reader {
            shared: shared.clone(),
            dispatcher,
            stream: Some(stream_tx),
            events: events_tx,
            started: Some(started_tx),
            finished: finished_tx,
            stderr_tail,
        };
        std::thread::Builder::new()
            .name("native-meeting-reader".into())
            .spawn(move || reader.run(stdout))
            .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))?;

        let started = wait_blocking(|| started_rx.recv_timeout(self.timeouts.start));
        let started = match started {
            Ok(Ok(started)) => started,
            Ok(Err(error)) => {
                shared.kill();
                return Err(error);
            }
            Err(_) => {
                shared.kill();
                return Err(Error::MeetingCaptureFailed(
                    "the native helper did not start in time".into(),
                ));
            }
        };
        if let ScopeRequest::Exactly(expected) = &request.scope
            && *expected != started.scope
        {
            shared.kill();
            return Err(Error::MeetingCaptureFailed(
                "the native helper opened a different scope".into(),
            ));
        }
        tracing::info!(
            scope = ?started.scope,
            mic = ?started.mic,
            "native_meeting_capture_started"
        );
        Ok(MeetingCapture {
            // The listener turns cancellation off while every output is headphones.
            stream: CaptureStream::with_events(NativeStream {
                rx: stream_rx,
                shared: shared.clone(),
            })
            .with_output_change(move |enable_aec| cancel_echo.store(enable_aec, Ordering::Release)),
            scope: started.scope,
            provenance: CaptureProvenance {
                engine: CaptureEngine::NativeMeeting,
                source_sample_rate: NATIVE_SAMPLE_RATE,
                mic: started.mic,
            },
            events: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(
                events_rx,
            )),
            control: Arc::new(NativeControl { shared }),
        })
    }
}

fn record_arguments(request: &MeetingCaptureRequest) -> Vec<String> {
    let mut args = vec![
        "record".to_string(),
        "--output".into(),
        request.staging_dir.to_string_lossy().into_owned(),
        "--capture-id".into(),
        request.capture.capture_id.clone(),
    ];
    match &request.scope {
        ScopeRequest::Discover => args.extend(["--scope".into(), "discover".into()]),
        ScopeRequest::Exactly(CaptureScope::System) => {
            args.extend(["--scope".into(), "system".into()])
        }
        ScopeRequest::Exactly(CaptureScope::Call(call)) => {
            args.extend([
                "--scope".into(),
                "call".into(),
                "--pid".into(),
                call.pid.to_string(),
                "--bundle-id".into(),
                call.bundle_id.clone(),
                "--platform".into(),
                call.platform.helper_name().into(),
            ]);
            if let Some(url) = &call.call_url {
                args.extend(["--call-url".into(), url.clone()]);
            }
        }
    }
    if let Some(device) = &request.config.mic_device {
        args.extend(["--microphone".into(), device.clone()]);
    }
    if let Ok(mode) = std::env::var(MIC_PROCESSING_ENV) {
        args.extend(["--microphone-processing".into(), mode]);
    }
    args
}

/// `open` is synchronous and usually called from a Tokio worker: let the runtime move other
/// tasks off this thread while the helper settles the microphone.
fn wait_blocking<T>(wait: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(wait)
        }
        _ => wait(),
    }
}

/// One-shot helper commands (`version`, `calls`), bounded in time.
fn run_to_end(helper: &Path, args: &[&str], timeout: Duration) -> Result<Vec<Message>, String> {
    let mut child = Command::new(helper)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot run {HELPER_NAME}: {error}"))?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std_mpsc::channel();
    std::thread::spawn(move || {
        let mut messages = Vec::new();
        let result = loop {
            match wire::read_message(&mut stdout) {
                Ok(Some(message)) => messages.push(message),
                Ok(None) => break Ok(messages),
                Err(error) => break Err(error.to_string()),
            }
        };
        let _ = tx.send(result);
    });
    let result = wait_blocking(|| rx.recv_timeout(timeout));
    let _ = child.kill();
    let status = child.wait().ok();
    let messages = result.map_err(|_| format!("{HELPER_NAME} did not answer in time"))??;
    if status.is_some_and(|status| status.success()) {
        Ok(messages)
    } else {
        Err(messages
            .iter()
            .find_map(|message| match message {
                Message::Event(line) if line.event() == "failed" => {
                    line.get("message").map(str::to_string)
                }
                _ => None,
            })
            .unwrap_or_else(|| format!("{HELPER_NAME} failed")))
    }
}

fn spawn_stderr(stderr: std::process::ChildStderr, tail: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            tracing::debug!(%line, "native_meeting_helper_stderr");
            let mut tail = tail.lock().unwrap_or_else(|poison| poison.into_inner());
            tail.clear();
            tail.push_str(&line);
        }
    });
}

type Finished = Option<Result<(), String>>;
type AckWaiter = (Ack, oneshot::Sender<Result<(), Error>>);

struct Shared {
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Option<Child>>,
    pending: Mutex<Option<AckWaiter>>,
    /// Keeps the stream open after a user stop, so the listener sees no false failure.
    parked: Mutex<Option<mpsc::Sender<Result<CaptureEvent, Error>>>>,
    finished: watch::Receiver<Finished>,
    staging_dir: PathBuf,
    timeouts: Timeouts,
}

impl Shared {
    fn send(&self, command: &str) -> Result<(), Error> {
        let mut stdin = self
            .stdin
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let pipe = stdin
            .as_mut()
            .ok_or_else(|| Error::MeetingCaptureFailed("the native capture is stopping".into()))?;
        pipe.write_all(format!("{command}\n").as_bytes())
            .and_then(|()| pipe.flush())
            .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))
    }

    /// Asks the helper to finish its tracks. Idempotent; closing stdin is the same request.
    fn request_stop(&self) {
        let mut stdin = self
            .stdin
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(mut pipe) = stdin.take() {
            let _ = pipe.write_all(b"stop\n");
        }
    }

    fn kill(&self) {
        self.request_stop();
        if let Some(child) = self
            .child
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
    }

    fn is_finished(&self) -> bool {
        self.finished.borrow().is_some()
    }

    async fn wait_finished(&self, timeout: Duration) -> Result<(), Error> {
        let mut finished = self.finished.clone();
        let outcome = tokio::time::timeout(timeout, finished.wait_for(Option::is_some)).await;
        match outcome {
            Ok(Ok(state)) => state
                .clone()
                .expect("waited for an outcome")
                .map_err(Error::MeetingCaptureFailed),
            Ok(Err(_)) => Err(Error::MeetingCaptureFailed(
                "the native helper went away".into(),
            )),
            Err(_) => {
                self.kill();
                Err(Error::MeetingCaptureFailed(
                    "the native helper did not finish in time".into(),
                ))
            }
        }
    }
}

struct Reader {
    shared: Arc<Shared>,
    dispatcher: Dispatcher,
    stream: Option<mpsc::Sender<Result<CaptureEvent, Error>>>,
    events: mpsc::UnboundedSender<anlg_audio::meeting::MeetingCaptureEvent>,
    started: Option<std_mpsc::Sender<Result<dispatch::Started, Error>>>,
    finished: watch::Sender<Finished>,
    stderr_tail: Arc<Mutex<String>>,
}

impl Reader {
    fn run(mut self, stdout: std::process::ChildStdout) {
        let mut stdout = BufReader::with_capacity(1 << 16, stdout);
        let protocol_error = loop {
            match wire::read_message(&mut stdout) {
                Ok(Some(message)) => {
                    let outputs = self.dispatcher.handle(message);
                    self.deliver(outputs);
                }
                Ok(None) => break None,
                Err(error) => break Some(error.to_string()),
            }
        };
        if let Some(error) = &protocol_error {
            tracing::error!(%error, "native_meeting_protocol_error");
            self.shared.kill();
        }
        let status = self.wait_exit();
        let clean =
            protocol_error.is_none() && status.as_ref().is_ok_and(|status| status.success());
        let detail = protocol_error.or_else(|| {
            let tail = self
                .stderr_tail
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
            match &status {
                Ok(status) if !status.success() => Some(if tail.is_empty() {
                    format!("the native helper exited ({status})")
                } else {
                    tail
                }),
                Err(error) => Some(error.clone()),
                _ => None,
            }
        });
        let outputs = self.dispatcher.close(clean, detail.clone());
        let user_stopped = clean
            && !outputs
                .iter()
                .any(|output| matches!(output, Output::Stream(Err(_))));
        self.deliver(outputs);
        if let Some((_, waiter)) = self.shared.pending.lock().unwrap().take() {
            let _ = waiter.send(Err(Error::MeetingCaptureFailed(
                "the native helper ended".into(),
            )));
        }
        let failure = self.dispatcher.failure().map(str::to_string);
        let _ = self.finished.send(Some(match (clean, failure) {
            (true, None) => Ok(()),
            (_, Some(failure)) => Err(failure),
            (false, None) => Err(detail.unwrap_or_else(|| "the native helper failed".into())),
        }));
        if user_stopped {
            *self.shared.parked.lock().unwrap() = self.stream.take();
        }
    }

    /// Bounded: a helper that closed stdout is exiting; one that does not is killed.
    fn wait_exit(&self) -> Result<std::process::ExitStatus, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut child = self.shared.child.lock().unwrap_or_else(|p| p.into_inner());
            let Some(process) = child.as_mut() else {
                return Err("the native helper is gone".into());
            };
            match process.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < deadline => {}
                Ok(None) => {
                    let _ = process.kill();
                    return process.wait().map_err(|error| error.to_string());
                }
                Err(error) => return Err(error.to_string()),
            }
            drop(child);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn deliver(&mut self, outputs: Vec<Output>) {
        for output in outputs {
            match output {
                Output::Started(result) => {
                    if let Some(started) = self.started.take() {
                        let _ = started.send(result);
                    }
                }
                // Bounded: a listener that stops reading backs up into the helper, which then
                // fails the capture loudly instead of dropping audio.
                Output::Stream(item) => {
                    if let Some(stream) = &self.stream
                        && stream.blocking_send(item).is_err()
                    {
                        self.stream = None;
                    }
                }
                Output::Event(event) => {
                    let _ = self.events.send(event);
                }
                Output::Ack(result) => {
                    let pending = self.shared.pending.lock().unwrap().take();
                    if let Some((expected, waiter)) = pending {
                        let _ = waiter.send(match result {
                            Ok(ack) if ack == expected => Ok(()),
                            Ok(ack) => Err(Error::MeetingCaptureFailed(format!(
                                "the helper acknowledged {ack:?}, not {expected:?}"
                            ))),
                            Err(message) => Err(Error::MeetingCaptureFailed(message)),
                        });
                    }
                }
            }
        }
    }
}

struct NativeStream {
    rx: mpsc::Receiver<Result<CaptureEvent, Error>>,
    shared: Arc<Shared>,
}

impl Stream for NativeStream {
    type Item = Result<CaptureEvent, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

/// The listener let go of the audio: finish the tracks, and kill a helper that will not.
impl Drop for NativeStream {
    fn drop(&mut self) {
        self.shared
            .parked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if self.shared.is_finished() {
            return;
        }
        self.shared.request_stop();
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + shared.timeouts.stop;
            while !shared.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            if !shared.is_finished() {
                tracing::warn!("native_meeting_helper_killed_after_stop_timeout");
                shared.kill();
            }
        });
    }
}

struct NativeControl {
    shared: Arc<Shared>,
}

impl NativeControl {
    fn toggle(&self, ack: Ack) -> BoxFuture<'static, Result<(), Error>> {
        let shared = self.shared.clone();
        async move {
            let (tx, rx) = oneshot::channel();
            {
                let mut pending = shared.pending.lock().unwrap();
                if pending.is_some() {
                    return Err(Error::MeetingCaptureFailed(
                        "another pause or resume is in progress".into(),
                    ));
                }
                *pending = Some((ack, tx));
            }
            let command = match ack {
                Ack::Paused => "pause",
                Ack::Resumed => "resume",
            };
            if let Err(error) = shared.send(command) {
                shared.pending.lock().unwrap().take();
                return Err(error);
            }
            match tokio::time::timeout(shared.timeouts.control, rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(Error::MeetingCaptureFailed(
                    "the native helper ended".into(),
                )),
                Err(_) => {
                    shared.pending.lock().unwrap().take();
                    Err(Error::MeetingCaptureFailed(format!(
                        "the native helper did not acknowledge {command}"
                    )))
                }
            }
        }
        .boxed()
    }
}

impl MeetingCaptureControl for NativeControl {
    fn pause(&self) -> BoxFuture<'static, Result<(), Error>> {
        self.toggle(Ack::Paused)
    }

    fn resume(&self) -> BoxFuture<'static, Result<(), Error>> {
        self.toggle(Ack::Resumed)
    }

    fn stop(&self) -> BoxFuture<'static, Result<(), Error>> {
        let shared = self.shared.clone();
        async move {
            shared.request_stop();
            let timeout = shared.timeouts.stop;
            shared.wait_finished(timeout).await
        }
        .boxed()
    }

    /// The runtime calls this only after Anarlog adopted the audio. A running helper still
    /// owns its files.
    fn release_staging(&self) -> Result<(), Error> {
        if !self.shared.is_finished() {
            return Err(Error::MeetingCaptureFailed(
                "the native capture is still writing its staging files".into(),
            ));
        }
        match std::fs::remove_dir_all(&self.shared.staging_dir) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(Error::MeetingCaptureFailed(error.to_string()))
            }
            _ => Ok(()),
        }
    }
}

// The fake helper is a POSIX shell script.
#[cfg(all(test, unix))]
mod tests;
