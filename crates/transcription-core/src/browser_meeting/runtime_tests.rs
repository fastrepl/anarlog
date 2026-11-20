//! The bridge over the real meeting runtime and audio router, through real Unix sockets. Only the
//! native helper is faked: it pauses when the test lets its acknowledgement through and reports
//! that on its event stream, also when nobody waits for the answer anymore, as the real one does.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_audio::meeting::{
    CallApp, CallPlatform, CaptureAuthority, CaptureEndReason, CaptureEngine, CaptureIdentity,
    CaptureInstant, CaptureProvenance, CaptureScope, ConfirmedCall, EngineAvailability,
    LifecycleOwner, MeetingCapture, MeetingCaptureControl, MeetingCaptureEngine,
    MeetingCaptureEvent, MeetingCaptureRequest, MicProcessing, ScopeRequest,
};
use anlg_audio::{AudioProvider, CaptureConfig, CaptureStream, Error};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, stream};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, mpsc};

use super::*;
use crate::meeting::{self as runtime, MeetingAudioRouter, MeetingError, MeetingRuntime};

const MEET: &str = "https://meet.google.com/abc-defg-hij";
const CHROME_PID: u32 = 77;
const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
const APP: &str = "com.hyprnote.dev";

#[derive(Default)]
struct Helper {
    events: Mutex<Option<mpsc::UnboundedSender<MeetingCaptureEvent>>>,
    /// Holds each pause until notified.
    pause_gate: Option<Arc<Notify>>,
    commands: Mutex<Vec<&'static str>>,
}

impl Helper {
    fn toggle(self: &Arc<Self>, paused: bool) -> BoxFuture<'static, Result<(), Error>> {
        let helper = self.clone();
        async move {
            helper
                .commands
                .lock()
                .unwrap()
                .push(if paused { "pause" } else { "resume" });
            // The command is out: the helper applies it whether or not anyone waits.
            let gate = paused.then(|| helper.pause_gate.clone()).flatten();
            let (acked, ack) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                if let Some(gate) = gate {
                    gate.notified().await;
                }
                let at = CaptureInstant(0);
                if let Some(events) = helper.events.lock().unwrap().as_ref() {
                    let _ = events.send(if paused {
                        MeetingCaptureEvent::Paused { at }
                    } else {
                        MeetingCaptureEvent::Resumed { at }
                    });
                }
                let _ = acked.send(());
            });
            ack.await
                .map_err(|_| Error::MeetingCaptureFailed("the helper ended".into()))
        }
        .boxed()
    }
}

struct Control(Arc<Helper>);

impl MeetingCaptureControl for Control {
    fn pause(&self) -> BoxFuture<'static, Result<(), Error>> {
        self.0.toggle(true)
    }
    fn resume(&self) -> BoxFuture<'static, Result<(), Error>> {
        self.0.toggle(false)
    }
    fn stop(&self) -> BoxFuture<'static, Result<(), Error>> {
        async { Ok(()) }.boxed()
    }
    fn release_staging(&self) -> Result<(), Error> {
        Ok(())
    }
}

struct Engine(Arc<Helper>);

impl MeetingCaptureEngine for Engine {
    fn availability(&self) -> EngineAvailability {
        EngineAvailability::Available {
            selects_microphone: true,
        }
    }

    fn calls_in_progress(&self) -> Result<Vec<CallApp>, Error> {
        Ok(vec![])
    }

    fn open(&self, request: MeetingCaptureRequest) -> Result<MeetingCapture, Error> {
        let (events, received) = mpsc::unbounded_channel();
        *self.0.events.lock().unwrap() = Some(events);
        Ok(MeetingCapture {
            stream: CaptureStream::with_events(stream::empty()),
            scope: match request.scope {
                ScopeRequest::Exactly(scope) => scope,
                ScopeRequest::Discover => CaptureScope::Call(ConfirmedCall {
                    pid: CHROME_PID as i32,
                    bundle_id: "com.google.Chrome".into(),
                    platform: CallPlatform::GoogleMeet,
                    call_url: Some(MEET.into()),
                }),
            },
            provenance: CaptureProvenance {
                engine: CaptureEngine::NativeMeeting,
                source_sample_rate: 48_000,
                mic: MicProcessing::Raw,
            },
            events: Box::pin(stream::unfold(received, |mut received| async move {
                received.recv().await.map(|event| (event, received))
            })),
            control: Arc::new(Control(self.0.clone())),
        })
    }
}

struct Audio(Arc<Engine>);

impl AudioProvider for Audio {
    fn open_capture(&self, _: CaptureConfig) -> Result<CaptureStream, Error> {
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn open_speaker_capture(&self, _: u32, _: usize) -> Result<CaptureStream, Error> {
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn open_mic_capture(
        &self,
        _: Option<String>,
        _: u32,
        _: usize,
    ) -> Result<CaptureStream, Error> {
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn default_device_name(&self) -> String {
        "Built-in".into()
    }
    fn list_mic_devices(&self) -> Vec<String> {
        vec!["Built-in".into()]
    }
    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }
    fn play_bytes(&self, _: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }
    fn probe_mic(&self, _: Option<String>) -> Result<(), Error> {
        Ok(())
    }
    fn probe_speaker(&self) -> Result<(), Error> {
        Ok(())
    }
    fn meeting_engine(&self) -> Option<Arc<dyn MeetingCaptureEngine>> {
        Some(self.0.clone())
    }
}

/// The app's port as `meeting_browser.rs` wires it, over the real runtime and router. Stop is the
/// record path's capture-aware stop, which ends exactly that capture.
#[derive(Clone)]
struct RuntimePort {
    router: Arc<MeetingAudioRouter>,
    clock: Arc<AtomicU64>,
    stops: Arc<Mutex<Vec<CaptureIdentity>>>,
}

fn port_error(error: MeetingError) -> PortError {
    match error {
        MeetingError::NoConfirmedCall | MeetingError::CallMismatch => PortError::CallMismatch,
        MeetingError::Stale | MeetingError::StaleLink | MeetingError::AccountMismatch => {
            PortError::Stale
        }
        other => PortError::Failed(other.to_string()),
    }
}

impl RuntimePort {
    fn runtime(&self) -> &Arc<MeetingRuntime> {
        self.router.runtime()
    }

    async fn set_paused(&self, capture: &CaptureIdentity, paused: bool) -> Result<(), PortError> {
        if !self.runtime().is_current(capture) {
            return Err(PortError::Stale);
        }
        if !self.router.can_pause(capture) {
            return Err(PortError::Unsupported);
        }
        let toggled = if paused {
            self.router.pause(capture)
        } else {
            self.router.resume(capture)
        };
        toggled
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

impl BrowserMeetingPort for RuntimePort {
    type Capture = CaptureIdentity;

    fn account(&self) -> Option<String> {
        Some(self.runtime().account().unwrap_or_else(|| "local".into()))
    }

    fn now_ms(&self) -> u64 {
        self.clock.load(Ordering::Relaxed)
    }

    fn active(&self) -> Option<RecorderView<CaptureIdentity>> {
        let active = self.runtime().active()?;
        Some(RecorderView {
            session_id: active.identity.session_id.clone(),
            owner: match active.owner {
                LifecycleOwner::App => Owner::App,
                LifecycleOwner::Extension => Owner::Extension,
            },
            status: match (&active.scope, active.paused) {
                (None, _) => RecorderStatus::Starting,
                (Some(_), true) => RecorderStatus::Paused,
                (Some(_), false) => RecorderStatus::Recording,
            },
            pausable: self.router.can_pause(&active.identity),
            capture: active.identity,
        })
    }

    fn capture(&self, capture: &CaptureIdentity) -> Option<RecorderView<CaptureIdentity>> {
        self.active().filter(|view| view.capture == *capture)
    }

    fn recorded_call(&self, capture: &CaptureIdentity) -> Option<RecordedCall> {
        self.runtime()
            .confirmed_call(capture)
            .map(|call| RecordedCall {
                bundle_id: call.bundle_id,
                call_url: call.call_url,
            })
    }

    fn bundle_id_of(&self, pid: u32) -> Option<String> {
        (pid == CHROME_PID).then(|| "com.google.Chrome".into())
    }

    fn link(&self, capture: &CaptureIdentity, call_url: &str) -> Result<String, PortError> {
        self.runtime()
            .link(capture, call_url)
            .map(|token| token.0)
            .map_err(port_error)
    }

    fn resolve_link(&self, token: &str) -> Result<CaptureIdentity, PortError> {
        self.runtime()
            .resolve_link(&runtime::LinkToken(token.into()))
            .map_err(port_error)
    }

    fn detach(&self, token: &str) -> Result<DetachOutcome, PortError> {
        self.runtime()
            .detach(&runtime::LinkToken(token.into()))
            .map(|outcome| match outcome {
                runtime::DetachOutcome::KeepRecording => DetachOutcome::KeepRecording,
                runtime::DetachOutcome::StopRecording => DetachOutcome::StopRecording,
            })
            .map_err(port_error)
    }

    async fn start(&self, _: ExtensionStart) -> Result<CaptureIdentity, PortError> {
        Err(PortError::Failed("not under test".into()))
    }

    async fn pause(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        self.set_paused(capture, true).await
    }

    async fn resume(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        self.set_paused(capture, false).await
    }

    async fn stop(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        if !self.runtime().is_current(capture) {
            return Err(PortError::Stale);
        }
        self.stops.lock().unwrap().push(capture.clone());
        self.router
            .finish_capture(capture, CaptureEndReason::UserStopped);
        Ok(())
    }

    fn observe(&self, _: &CaptureIdentity, _: MeetObservation) {}

    async fn claim_notice(&self, _: &CaptureIdentity) -> Option<String> {
        None
    }

    fn notice_outcome(&self, _: &CaptureIdentity, _: ChatOutcome) {}

    async fn claim_camera(&self, _: &CaptureIdentity) -> Option<String> {
        None
    }

    fn camera_outcome(&self, _: &CaptureIdentity, _: CameraOutcome) {}
}

struct Setup {
    _dir: tempfile::TempDir,
    helper: Arc<Helper>,
    port: RuntimePort,
}

fn setup(pause_gate: Option<Arc<Notify>>) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let helper = Arc::new(Helper {
        pause_gate,
        ..Helper::default()
    });
    let router = MeetingAudioRouter::new(
        Arc::new(Audio(Arc::new(Engine(helper.clone())))),
        MeetingRuntime::new(dir.path().join("staging")),
    );
    Setup {
        _dir: dir,
        helper,
        port: RuntimePort {
            router,
            clock: Arc::default(),
            stops: Arc::default(),
        },
    }
}

impl Setup {
    fn runtime(&self) -> &Arc<MeetingRuntime> {
        self.port.runtime()
    }

    /// A native recording of the Meet call, the app's own or the extension's.
    fn record(&self, owner: LifecycleOwner) -> CaptureIdentity {
        let capture = self.runtime().begin("local", "note", owner).unwrap();
        self.port
            .router
            .open_capture(CaptureConfig {
                sample_rate: 16_000,
                chunk_size: 320,
                mic_device: None,
                enable_aec: true,
            })
            .unwrap();
        assert_eq!(self.runtime().active().unwrap().owner, owner);
        capture
    }

    fn paused(&self) -> bool {
        self.runtime().active().is_some_and(|active| active.paused)
    }

    fn stops(&self) -> Vec<CaptureIdentity> {
        self.port.stops.lock().unwrap().clone()
    }

    async fn helper_got(&self, command: &str) {
        while !self.helper.commands.lock().unwrap().contains(&command) {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}

fn peer() -> Peer {
    Peer {
        origin: ORIGIN.into(),
        browser_pid: Some(CHROME_PID),
    }
}

fn endpoints() -> (tempfile::TempDir, std::path::PathBuf) {
    // Short, so the socket path stays inside `sun_path`.
    let dir = tempfile::Builder::new()
        .prefix("bm")
        .tempdir_in("/tmp")
        .unwrap();
    let endpoints = dir.path().join(ENDPOINT_DIR_NAME);
    (dir, endpoints)
}

fn record_of(dir: &std::path::Path) -> Option<EndpointRecord> {
    serde_json::from_slice(&std::fs::read(dir.join(format!("{APP}.json"))).ok()?).ok()
}

async fn read_event(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> Option<serde_json::Value> {
    let read = async {
        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.ok()?;
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        stream.read_exact(&mut body).await.ok()?;
        serde_json::from_slice(&body).ok()
    };
    tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .expect("the app neither answered nor hung up")
}

async fn hello(record: &EndpointRecord) -> tokio::net::UnixStream {
    let mut tab = tokio::net::UnixStream::connect(&record.socket)
        .await
        .unwrap();
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        secret: record.secret.clone(),
        origin: ORIGIN.into(),
        browser_pid: Some(CHROME_PID),
    };
    tab.write_all(&encode_frame(&hello).unwrap()).await.unwrap();
    assert_eq!(read_event(&mut tab).await.unwrap()["type"], "welcome");
    tab
}

async fn send(tab: &mut (impl tokio::io::AsyncWrite + Unpin), message: serde_json::Value) {
    tab.write_all(&encode_frame(&message).unwrap())
        .await
        .unwrap();
}

/// A tab on the recorded call that follows the active recording.
async fn follow(record: &EndpointRecord) -> tokio::net::UnixStream {
    let mut tab = hello(record).await;
    send(&mut tab, json!({"type": "meeting_start", "url": MEET})).await;
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "started");
    tab
}

/// Turning the bridge off and on again quickly leaves the old bridge's cleanup to finish after the
/// new one served a tab. The old cleanup lets go of its own links only: the new tab keeps its
/// link and the recording it holds keeps going.
#[tokio::test]
async fn an_old_bridge_letting_go_never_touches_a_newer_bridges_tab() {
    let setup = setup(None);
    let capture = setup.record(LifecycleOwner::Extension);

    // The old bridge holds a link no connection knew about (its start was cut short), and must
    // let go of it by owner when it retires.
    let old = BrowserMeetingRoute::new(setup.port.clone());
    old.start(MEET, &peer()).await.unwrap();

    let (_dir, dir) = endpoints();
    let new = Arc::new(BrowserMeetingRoute::new(setup.port.clone()));
    let endpoint = serve(&dir, APP, vec![ORIGIN.into()], new).unwrap();
    let mut tab = follow(&record_of(&dir).unwrap()).await;

    old.retire().await;
    assert!(
        setup.runtime().is_current(&capture),
        "the new tab's recording"
    );
    assert!(setup.runtime().active().unwrap().linked);
    assert!(setup.stops().is_empty());
    send(&mut tab, json!({"type": "meeting_pause"})).await;
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "paused");

    // Retired, the old bridge acts on nothing again.
    assert_eq!(
        old.start(MEET, &peer()).await.unwrap_err().code,
        RefusalCode::AppNotReady
    );
    assert!(endpoint.close().await);
    assert_eq!(setup.stops(), vec![capture]);
}

/// A bridge turned off with its old link still current lets go of it the way the tab leaving does.
#[tokio::test]
async fn a_retiring_bridge_lets_go_of_a_link_no_connection_knew_by_owner() {
    let setup = setup(None);
    let capture = setup.record(LifecycleOwner::Extension);
    let route = BrowserMeetingRoute::new(setup.port.clone());
    route.start(MEET, &peer()).await.unwrap();
    route.retire().await;
    assert!(!setup.runtime().is_current(&capture));
    assert_eq!(setup.stops(), vec![capture]);

    let setup = self::setup(None);
    let capture = setup.record(LifecycleOwner::App);
    let route = BrowserMeetingRoute::new(setup.port.clone());
    let link = route.start(MEET, &peer()).await.unwrap().link;
    route.command(&link, Command::Pause).await.unwrap();
    route.retire().await;
    // Whatever the helper still reports has landed.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(setup.runtime().is_current(&capture));
    assert!(!setup.paused(), "nothing in Anarlog's window resumes it");
    assert!(!setup.runtime().active().unwrap().linked);
}

/// A newer endpoint for the same app (the bridge turned on again, or another launch) replaced the
/// record; the older one closing or dropping leaves that record for the native host to find.
#[tokio::test]
async fn an_older_endpoint_never_removes_a_newer_endpoints_record() {
    let setup = setup(None);
    let (_dir, dir) = endpoints();
    let route = || Arc::new(BrowserMeetingRoute::new(setup.port.clone()));
    let older = serve(&dir, APP, vec![ORIGIN.into()], route()).unwrap();
    let older_socket = record_of(&dir).unwrap().socket;
    let newer = serve(&dir, APP, vec![ORIGIN.into()], route()).unwrap();
    let record = record_of(&dir).unwrap();
    assert_ne!(record.socket, older_socket);

    assert!(older.close().await);
    assert_eq!(record_of(&dir), Some(record.clone()));
    assert!(!std::path::Path::new(&older_socket).exists());
    drop(hello(&record).await);

    drop(newer);
    assert_eq!(record_of(&dir), None);
}

/// Closing never waits on a host that stopped reading, or one that never finished its hello or
/// its frame: each connection still lets go of its tab by owner and ends within the close bound.
#[tokio::test]
async fn closing_ends_connections_whose_host_stopped_reading_or_stalled_mid_frame() {
    let setup = setup(None);
    let capture = setup.record(LifecycleOwner::Extension);
    let (_dir, dir) = endpoints();
    let endpoint = serve(
        &dir,
        APP,
        vec![ORIGIN.into()],
        Arc::new(BrowserMeetingRoute::new(setup.port.clone())),
    )
    .unwrap();
    let record = record_of(&dir).unwrap();

    let mut stalled_hello = tokio::net::UnixStream::connect(&record.socket)
        .await
        .unwrap();
    stalled_hello.write_all(&64u32.to_le_bytes()).await.unwrap();
    let mut stalled_frame = hello(&record).await;
    stalled_frame.write_all(&64u32.to_le_bytes()).await.unwrap();

    // The tab keeps asking and never reads the answers, until the app can't write to it anymore.
    let (mut reading, mut writing) = follow(&record).await.into_split();
    let sent = Arc::new(AtomicU64::new(0));
    let flooding = tokio::spawn({
        let sent = sent.clone();
        async move {
            for paused in [true, false].into_iter().cycle() {
                let message =
                    json!({"type": if paused { "meeting_pause" } else { "meeting_resume" }});
                if writing
                    .write_all(&encode_frame(&message).unwrap())
                    .await
                    .is_err()
                {
                    return;
                }
                sent.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    let mut last = u64::MAX;
    while sent.load(Ordering::Relaxed) != last {
        last = sent.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let closing = std::time::Instant::now();
    assert!(endpoint.close().await, "every connection let go in time");
    assert!(closing.elapsed() < Duration::from_secs(3));
    assert!(!setup.runtime().is_current(&capture));
    assert_eq!(setup.stops(), vec![capture]);
    // The app hung up on each of them.
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(2), reading.read_to_end(&mut rest)).await;
    assert!(read.is_ok());
    flooding.await.unwrap();
    for stalled in [&mut stalled_hello, &mut stalled_frame] {
        assert_eq!(read_event(stalled).await, None);
    }
}

/// Turning the bridge off while the recorder still acknowledges a pause the tab asked for: the
/// pause lands, and the app recording the tab lets go of is resumed rather than left paused with
/// nothing in Anarlog's window to resume it.
#[tokio::test]
async fn closing_while_a_pause_is_acknowledged_leaves_the_app_recording_running() {
    let gate = Arc::new(Notify::new());
    let setup = setup(Some(gate.clone()));
    let capture = setup.record(LifecycleOwner::App);
    let (_dir, dir) = endpoints();
    let endpoint = serve(
        &dir,
        APP,
        vec![ORIGIN.into()],
        Arc::new(BrowserMeetingRoute::new(setup.port.clone())),
    )
    .unwrap();
    let mut tab = follow(&record_of(&dir).unwrap()).await;
    send(&mut tab, json!({"type": "meeting_pause"})).await;
    setup.helper_got("pause").await;

    let closing = tokio::spawn(endpoint.close());
    tokio::time::sleep(Duration::from_millis(50)).await;
    gate.notify_one();
    assert!(closing.await.unwrap());
    assert_eq!(read_event(&mut tab).await.unwrap()["event"], "detached");
    // Whatever the helper still reports has landed.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(setup.runtime().is_current(&capture));
    assert!(!setup.paused());
    assert!(setup.stops().is_empty());
}

/// The bridge's own cleanup after a slow close: it waits for a pause still being acknowledged
/// before letting go, and resumes the app recording it kept.
#[tokio::test]
async fn retiring_waits_for_a_pause_still_being_acknowledged() {
    let gate = Arc::new(Notify::new());
    let setup = setup(Some(gate.clone()));
    let capture = setup.record(LifecycleOwner::App);
    let route = Arc::new(BrowserMeetingRoute::new(setup.port.clone()));
    let link = route.start(MEET, &peer()).await.unwrap().link;
    let pausing = tokio::spawn({
        let route = route.clone();
        async move { route.command(&link, Command::Pause).await }
    });
    setup.helper_got("pause").await;
    let retiring = tokio::spawn({
        let route = route.clone();
        async move { route.retire().await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    gate.notify_one();
    let _ = pausing.await.unwrap();
    retiring.await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(setup.runtime().is_current(&capture));
    assert!(!setup.paused());
    assert!(!setup.runtime().active().unwrap().linked);
}
