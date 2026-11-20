//! An extension start goes through the app window's own record path: the window creates the note
//! the way its new-note-and-record action does and starts it with the person's selected providers,
//! carrying this start's request id into the recorder's begin. Only a begin with that request id
//! takes the reservation, so this side follows, and stops when cancelled, exactly the capture that
//! begin started. Any other recording of the note, before or after, is the person's own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anlg_audio::meeting::{
    CaptureAuthority, CaptureIdentity, LifecycleOwner, MeetingLifecycleEvent,
    MeetingLifecycleObserver,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// App → window: create a note to record for this request, or say why not.
pub const START_REQUEST_EVENT: &str = "anlg:browser-meeting-start";
/// App → window: start recording that note now with this request id (`go`), or drop it.
pub const START_GO_EVENT: &str = "anlg:browser-meeting-start-go";
/// Window → app: a [`StartReply`].
pub const START_REPLY_EVENT: &str = "anlg:browser-meeting-start-reply";

/// How long the window may take to create the note.
pub const WINDOW_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the record path may take to open audio and confirm the call, permission prompts
/// included.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);

const NO_WINDOW: &str = "Open Anarlog, then click the extension again.";
const WINDOW_SILENT: &str =
    "Anarlog's window didn't answer. Open Anarlog, then click the extension again.";
const TOO_SLOW: &str = "Anarlog took too long to start recording. Start it in Anarlog instead.";
const DIDNT_START: &str = "Anarlog couldn't start recording this meeting.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartGo {
    pub request_id: String,
    pub session_id: String,
    pub go: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StartReply {
    /// The note to record; it isn't recording yet.
    Session {
        request_id: String,
        session_id: String,
    },
    /// No note was created (already recording, no call recorder, ...).
    Refused { request_id: String, message: String },
    /// The record path finished with this request: it began its capture or never will.
    Done {
        request_id: String,
        session_id: String,
        message: Option<String>,
    },
}

/// The app around the coordinator: its window, the meeting runtime's reservation, and the record
/// path's stop. Calls return quickly; the window answers through [`ExtensionStarts::reply`].
pub trait StartHost: Send + Sync + 'static {
    /// Sends [`StartRequest`] to the main window; `Err` when there is no window to ask.
    fn request(&self, request: StartRequest) -> Result<(), String>;
    fn answer(&self, go: StartGo);
    /// `MeetingRuntime::reserve_extension_start`: the capture id the begin carrying this request
    /// id gets.
    fn reserve(&self, request_id: &str, session_id: &str) -> Result<String, String>;
    /// `MeetingRuntime::cancel_extension_start`: that request's begin is refused from now on;
    /// returns the capture it already began, while that is still active.
    fn cancel(&self, request_id: &str) -> Option<CaptureIdentity>;
    /// Stops exactly this capture through the record path, never a later recording of the same
    /// note; never waits.
    fn stop(&self, capture: &CaptureIdentity);
}

enum Signal {
    Session(String),
    Refused(String),
    NotStarted(Option<String>),
    Started(CaptureIdentity),
    Ended,
}

/// How far the window's record path got with a start's request.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Attempt {
    /// Not told to record (yet).
    NotAsked,
    /// Told to record: the begin carrying this request gets this capture id.
    Asked { capture_id: String },
    /// The capture this request's begin started.
    Began(CaptureIdentity),
    /// That capture ended: nothing more begins for this request.
    Over,
}

struct Pending {
    attempt: Attempt,
    /// `None` once the start was cancelled: the capture it began stops.
    signals: Option<mpsc::UnboundedSender<Signal>>,
}

pub struct ExtensionStarts<H: StartHost> {
    host: H,
    pending: Mutex<HashMap<String, Pending>>,
    window_timeout: Duration,
    start_timeout: Duration,
}

/// Cancels the start however its future ends without a recording: an error, a timeout, or the
/// tab's connection closing. The request's begin is refused from then on, and a capture it already
/// began stops.
struct Cancel<'a, H: StartHost> {
    starts: &'a ExtensionStarts<H>,
    request_id: Option<String>,
}

impl<H: StartHost> Drop for Cancel<'_, H> {
    fn drop(&mut self) {
        if let Some(request_id) = self.request_id.take() {
            self.starts.cancel(&request_id);
        }
    }
}

impl<H: StartHost> ExtensionStarts<H> {
    pub fn new(host: H) -> Arc<Self> {
        Self::with_timeouts(host, WINDOW_TIMEOUT, START_TIMEOUT)
    }

    pub fn with_timeouts(host: H, window_timeout: Duration, start_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            host,
            pending: Mutex::default(),
            window_timeout,
            start_timeout,
        })
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Pending>> {
        self.pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Records a new note as the extension's capture and resolves once the recorder confirmed it.
    /// The caller still checks that it confirmed the tab's own call.
    pub async fn start(&self) -> Result<CaptureIdentity, String> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (signals, mut incoming) = mpsc::unbounded_channel();
        self.lock().insert(
            request_id.clone(),
            Pending {
                attempt: Attempt::NotAsked,
                signals: Some(signals),
            },
        );
        let mut cancel = Cancel {
            starts: self,
            request_id: Some(request_id.clone()),
        };
        self.host
            .request(StartRequest {
                request_id: request_id.clone(),
            })
            .map_err(|_| NO_WINDOW.to_string())?;

        let session_id = match tokio::time::timeout(self.window_timeout, incoming.recv()).await {
            Ok(Some(Signal::Session(session_id))) => session_id,
            Ok(Some(Signal::Refused(message))) => return Err(message),
            Err(_) => return Err(WINDOW_SILENT.into()),
            Ok(_) => return Err(DIDNT_START.into()),
        };
        let capture_id = match self.host.reserve(&request_id, &session_id) {
            Ok(capture_id) => capture_id,
            Err(message) => {
                self.host.answer(StartGo {
                    request_id,
                    session_id,
                    go: false,
                });
                return Err(message);
            }
        };
        if let Some(pending) = self.lock().get_mut(&request_id) {
            pending.attempt = Attempt::Asked { capture_id };
        }
        self.host.answer(StartGo {
            request_id: request_id.clone(),
            session_id,
            go: true,
        });

        let waiting = async {
            loop {
                match incoming.recv().await {
                    Some(Signal::Started(capture)) => return Ok(capture),
                    Some(Signal::NotStarted(message)) => {
                        return Err(message.unwrap_or_else(|| DIDNT_START.into()));
                    }
                    Some(Signal::Session(_)) => continue,
                    Some(Signal::Refused(message)) => return Err(message),
                    Some(Signal::Ended) | None => return Err(DIDNT_START.into()),
                }
            }
        };
        let capture = tokio::time::timeout(self.start_timeout, waiting)
            .await
            .map_err(|_| TOO_SLOW.to_string())??;
        cancel.request_id = None;
        self.lock().remove(&request_id);
        Ok(capture)
    }

    fn cancel(&self, request_id: &str) {
        let stop = {
            let mut pending = self.lock();
            let Some(entry) = pending.get_mut(request_id) else {
                return;
            };
            entry.signals = None;
            let began = match &entry.attempt {
                Attempt::Asked { .. } => self.host.cancel(request_id),
                Attempt::Began(capture) => Some(capture.clone()),
                Attempt::NotAsked | Attempt::Over => None,
            };
            match began {
                // Kept until the capture ends, so it stops again if the recorder ran it only later.
                Some(capture) => {
                    entry.attempt = Attempt::Began(capture.clone());
                    Some(capture)
                }
                // Nothing began, and the runtime refuses this request's begin from now on.
                None => {
                    pending.remove(request_id);
                    None
                }
            }
        };
        if let Some(capture) = stop {
            self.host.stop(&capture);
        }
    }

    /// The window's answer, from [`START_REPLY_EVENT`].
    pub fn reply(&self, reply: StartReply) {
        let mut answer = None;
        let mut stop = None;
        {
            let mut pending = self.lock();
            match reply {
                StartReply::Session {
                    request_id,
                    session_id,
                } => match pending
                    .get(&request_id)
                    .and_then(|entry| entry.signals.clone())
                {
                    Some(signals) => {
                        let _ = signals.send(Signal::Session(session_id));
                    }
                    // Nobody waits for this note anymore: don't record it.
                    None => {
                        answer = Some(StartGo {
                            request_id,
                            session_id,
                            go: false,
                        });
                    }
                },
                StartReply::Refused {
                    request_id,
                    message,
                } => {
                    if let Some(signals) = pending
                        .get(&request_id)
                        .and_then(|entry| entry.signals.clone())
                    {
                        let _ = signals.send(Signal::Refused(message));
                    }
                }
                StartReply::Done {
                    request_id,
                    message,
                    ..
                } => {
                    let Some(entry) = pending.get_mut(&request_id) else {
                        return;
                    };
                    match (&entry.signals, &entry.attempt) {
                        // The record path is finished with this request: a begin that didn't come
                        // yet never will, and what it did begin is followed until confirmed.
                        (Some(signals), Attempt::Asked { .. }) => {
                            match self.host.cancel(&request_id) {
                                Some(capture) => entry.attempt = Attempt::Began(capture),
                                None => {
                                    let _ = signals.send(Signal::NotStarted(message));
                                }
                            }
                        }
                        (Some(_), _) => {}
                        // Its recorder runs by now, so a stop that came too early lands.
                        (None, Attempt::Began(capture)) => stop = Some(capture.clone()),
                        (None, _) => {
                            pending.remove(&request_id);
                        }
                    }
                }
            }
        }
        if let Some(go) = answer {
            self.host.answer(go);
        }
        if let Some(capture) = stop {
            self.host.stop(&capture);
        }
    }

    /// What the recorder did with a start's capture: confirmed it, or ended it.
    pub fn lifecycle(&self, event: &MeetingLifecycleEvent) {
        let (capture, owner) = match event {
            MeetingLifecycleEvent::Started { capture, owner, .. } => (capture, Some(*owner)),
            MeetingLifecycleEvent::Ended { capture, .. } => (capture, None),
            _ => return,
        };
        let stop = {
            let mut pending = self.lock();
            let Some((request_id, entry)) =
                pending.iter_mut().find(|(_, entry)| match &entry.attempt {
                    Attempt::Asked { capture_id } => *capture_id == capture.capture_id,
                    Attempt::Began(began) => began == capture,
                    Attempt::NotAsked | Attempt::Over => false,
                })
            else {
                return;
            };
            let request_id = request_id.clone();
            entry.attempt = match owner {
                Some(_) => Attempt::Began(capture.clone()),
                None => Attempt::Over,
            };
            match (&entry.signals, owner) {
                (Some(signals), Some(LifecycleOwner::Extension)) => {
                    let _ = signals.send(Signal::Started(capture.clone()));
                    None
                }
                // Ended before it was confirmed: the start fails.
                (Some(signals), _) => {
                    let _ = signals.send(Signal::Ended);
                    None
                }
                (None, Some(_)) => Some(capture.clone()),
                (None, None) => {
                    pending.remove(&request_id);
                    None
                }
            }
        };
        if let Some(capture) = stop {
            self.host.stop(&capture);
        }
    }
}

impl<H: StartHost> MeetingLifecycleObserver for ExtensionStarts<H> {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, _authority: Arc<dyn CaptureAuthority>) {
        self.lifecycle(event);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anlg_audio::meeting::{
        CallPlatform, CaptureEndReason, CaptureEngine, CaptureProvenance, CaptureScope,
        ConfirmedCall, MicProcessing,
    };

    use super::*;
    use crate::meeting::{MeetingError, MeetingRuntime};

    #[derive(Default)]
    struct Window {
        requests: Vec<String>,
        answers: Vec<StartGo>,
        stops: Vec<CaptureIdentity>,
        no_window: bool,
        /// The recorder isn't running the capture yet, so its stop is refused.
        recorder_idle: bool,
    }

    /// The real meeting runtime's reservation, with the window and the record path's stop faked.
    #[derive(Clone)]
    struct Host {
        runtime: Arc<MeetingRuntime>,
        window: Arc<Mutex<Window>>,
    }

    impl StartHost for Host {
        fn request(&self, request: StartRequest) -> Result<(), String> {
            let mut window = self.window.lock().unwrap();
            if window.no_window {
                return Err("no main window".into());
            }
            window.requests.push(request.request_id);
            Ok(())
        }

        fn answer(&self, go: StartGo) {
            self.window.lock().unwrap().answers.push(go);
        }

        fn reserve(&self, request_id: &str, session_id: &str) -> Result<String, String> {
            self.runtime
                .reserve_extension_start(request_id, session_id)
                .map_err(|error| error.to_string())
        }

        fn cancel(&self, request_id: &str) -> Option<CaptureIdentity> {
            self.runtime.cancel_extension_start(request_id)
        }

        /// The record path's capture-aware stop: it ends exactly this capture.
        fn stop(&self, capture: &CaptureIdentity) {
            let idle = {
                let mut window = self.window.lock().unwrap();
                window.stops.push(capture.clone());
                window.recorder_idle
            };
            if !idle {
                self.runtime.finish(capture, CaptureEndReason::UserStopped);
            }
        }
    }

    fn setup() -> (Arc<ExtensionStarts<Host>>, Host) {
        let runtime = MeetingRuntime::new(PathBuf::from("/nonexistent"));
        let host = Host {
            runtime: runtime.clone(),
            window: Arc::default(),
        };
        let starts = ExtensionStarts::with_timeouts(
            host.clone(),
            Duration::from_secs(10),
            Duration::from_secs(60),
        );
        runtime.register_observer(starts.clone());
        (starts, host)
    }

    fn meet_call() -> CaptureScope {
        CaptureScope::Call(ConfirmedCall {
            pid: 77,
            bundle_id: "com.google.Chrome".into(),
            platform: CallPlatform::GoogleMeet,
            call_url: Some("https://meet.google.com/abc-defg-hij".into()),
        })
    }

    fn native() -> CaptureProvenance {
        CaptureProvenance {
            engine: CaptureEngine::NativeMeeting,
            source_sample_rate: 48_000,
            mic: MicProcessing::Raw,
        }
    }

    /// The record path as the plugin drives it: `begin_recording` with the request id the
    /// window's start carried (`None` for the person's own start), then the engine's confirmation.
    fn record_with(
        host: &Host,
        session_id: &str,
        extension_start: Option<&str>,
    ) -> Result<CaptureIdentity, MeetingError> {
        let capture = host
            .runtime
            .begin_recording("account-1", session_id, extension_start)?;
        host.runtime
            .confirm(&capture, meet_call(), native())
            .unwrap();
        Ok(capture)
    }

    /// The person's own start of a note: no request id.
    fn record(host: &Host, session_id: &str) -> CaptureIdentity {
        record_with(host, session_id, None).unwrap()
    }

    /// The window's start of the note the app said go for, carrying that answer's request id.
    fn window_records(host: &Host, go: &StartGo) -> Result<CaptureIdentity, MeetingError> {
        assert!(go.go);
        record_with(host, &go.session_id, Some(&go.request_id))
    }

    fn done(starts: &ExtensionStarts<Host>, go: &StartGo) {
        starts.reply(StartReply::Done {
            request_id: go.request_id.clone(),
            session_id: go.session_id.clone(),
            message: None,
        });
    }

    /// The window's oldest request not yet taken by the test.
    async fn next_request(host: &Host) -> String {
        loop {
            {
                let mut window = host.window.lock().unwrap();
                if !window.requests.is_empty() {
                    return window.requests.remove(0);
                }
            }
            tokio::task::yield_now().await;
        }
    }

    async fn next_answer(host: &Host) -> StartGo {
        loop {
            if let Some(answer) = host.window.lock().unwrap().answers.last().cloned() {
                return answer;
            }
            tokio::task::yield_now().await;
        }
    }

    /// The window answers its next request with `session_id` and is told go.
    async fn asked(starts: &Arc<ExtensionStarts<Host>>, host: &Host, session_id: &str) -> StartGo {
        let request_id = next_request(host).await;
        starts.reply(StartReply::Session {
            request_id: request_id.clone(),
            session_id: session_id.into(),
        });
        loop {
            let go = next_answer(host).await;
            if go.request_id == request_id {
                assert!(go.go);
                return go;
            }
            tokio::task::yield_now().await;
        }
    }

    fn spawn_start(
        starts: &Arc<ExtensionStarts<Host>>,
    ) -> tokio::task::JoinHandle<Result<CaptureIdentity, String>> {
        let starts = starts.clone();
        tokio::spawn(async move { starts.start().await })
    }

    async fn cancel(start: tokio::task::JoinHandle<Result<CaptureIdentity, String>>) {
        start.abort();
        assert!(start.await.unwrap_err().is_cancelled());
    }

    fn owner(host: &Host) -> Option<LifecycleOwner> {
        host.runtime.active().map(|active| active.owner)
    }

    #[tokio::test]
    async fn the_window_records_a_new_note_that_the_extension_owns() {
        let (starts, host) = setup();
        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        assert_eq!(go.session_id, "note-1");
        let capture = window_records(&host, &go).unwrap();

        assert_eq!(start.await.unwrap(), Ok(capture.clone()));
        assert_eq!(owner(&host), Some(LifecycleOwner::Extension));
        done(&starts, &go);
        assert!(host.window.lock().unwrap().stops.is_empty());
        // A later recording of another note is the app's again.
        host.runtime.finish(&capture, CaptureEndReason::UserStopped);
        record(&host, "note-2");
        assert_eq!(owner(&host), Some(LifecycleOwner::App));
    }

    #[tokio::test]
    async fn refusals_and_failed_starts_reach_the_tab_without_recording() {
        let (starts, host) = setup();
        host.window.lock().unwrap().no_window = true;
        assert_eq!(starts.start().await, Err(NO_WINDOW.into()));
        host.window.lock().unwrap().no_window = false;

        let start = spawn_start(&starts);
        let request_id = next_request(&host).await;
        starts.reply(StartReply::Refused {
            request_id,
            message: "Anarlog is already recording.".into(),
        });
        assert_eq!(
            start.await.unwrap(),
            Err("Anarlog is already recording.".into())
        );

        // The record path gave up (a permission was denied) without beginning: the start fails
        // with its message, whatever the window believes it recorded.
        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-3").await;
        starts.reply(StartReply::Done {
            request_id: go.request_id.clone(),
            session_id: go.session_id.clone(),
            message: Some("Microphone access is off.".into()),
        });
        assert_eq!(
            start.await.unwrap(),
            Err("Microphone access is off.".into())
        );
        assert_eq!(
            window_records(&host, &go),
            Err(MeetingError::StaleExtensionStart),
            "no reservation outlives its start"
        );
        record(&host, "note-3");
        assert_eq!(owner(&host), Some(LifecycleOwner::App));
    }

    /// The window's JS died after go, before its record path began: no begin and no answer ever
    /// come. The person then records that note themselves, which is theirs and keeps going.
    #[tokio::test]
    async fn a_start_whose_window_died_after_go_never_touches_the_persons_recording_of_its_note() {
        use anlg_audio::meeting::CaptureAuthority;

        let (starts, host) = setup();
        let start = spawn_start(&starts);
        asked(&starts, &host, "note-1").await;
        cancel(start).await;

        let own = record(&host, "note-1");
        assert_eq!(owner(&host), Some(LifecycleOwner::App));
        assert!(host.runtime.is_current(&own));
        assert!(host.window.lock().unwrap().stops.is_empty());
    }

    /// The person clicks record on the opened note before the window's own start runs: theirs is
    /// the app's, and the window's begin finds it recording instead of taking it.
    #[tokio::test]
    async fn the_persons_own_start_of_a_reserved_note_stays_theirs() {
        use anlg_audio::meeting::CaptureAuthority;

        let (starts, host) = setup();
        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        let own = record(&host, "note-1");
        assert_eq!(owner(&host), Some(LifecycleOwner::App));
        assert_eq!(window_records(&host, &go), Err(MeetingError::CaptureActive));
        done(&starts, &go);

        assert_eq!(start.await.unwrap(), Err(DIDNT_START.into()));
        assert!(host.runtime.is_current(&own));
        assert!(host.window.lock().unwrap().stops.is_empty());
    }

    /// A start cancelled (the tab closed, or it timed out) before the window began: the window's
    /// late begin is refused, so nothing records for nobody, and a newer start of the same note
    /// still records as the extension's.
    #[tokio::test]
    async fn a_cancelled_starts_late_begin_is_refused_and_a_newer_start_still_records() {
        let (starts, host) = setup();
        let start = spawn_start(&starts);
        let stale = asked(&starts, &host, "note-1").await;
        cancel(start).await;

        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        assert_eq!(
            window_records(&host, &stale),
            Err(MeetingError::StaleExtensionStart)
        );
        assert!(host.runtime.active().is_none());
        let capture = window_records(&host, &go).unwrap();
        assert_eq!(start.await.unwrap(), Ok(capture));
        assert_eq!(owner(&host), Some(LifecycleOwner::Extension));
        done(&starts, &stale);
        assert!(host.window.lock().unwrap().stops.is_empty());

        // Cancelled before the window answered: its late note is never recorded.
        let start = spawn_start(&starts);
        let request_id = next_request(&host).await;
        cancel(start).await;
        starts.reply(StartReply::Session {
            request_id: request_id.clone(),
            session_id: "note-4".into(),
        });
        let answer = next_answer(&host).await;
        assert_eq!(
            (answer.request_id, answer.session_id.as_str(), answer.go),
            (request_id, "note-4", false)
        );
    }

    /// A start cancelled after its window began stops exactly the capture that begin started,
    /// again once the recorder confirms it if the first stop came too early, and nothing after it:
    /// the person recording the same note again keeps that recording.
    #[tokio::test]
    async fn a_cancelled_start_stops_only_the_capture_its_own_begin_started() {
        use anlg_audio::meeting::CaptureAuthority;

        let (starts, host) = setup();
        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        let began = host
            .runtime
            .begin_recording("account-1", "note-1", Some(&go.request_id))
            .unwrap();
        host.window.lock().unwrap().recorder_idle = true;
        cancel(start).await;
        assert!(host.runtime.is_current(&began));
        host.window.lock().unwrap().recorder_idle = false;
        host.runtime.confirm(&began, meet_call(), native()).unwrap();
        assert_eq!(
            host.window.lock().unwrap().stops,
            vec![began.clone(), began.clone()]
        );
        assert!(!host.runtime.is_current(&began));

        let again = record(&host, "note-1");
        done(&starts, &go);
        assert!(host.runtime.is_current(&again));
        assert_eq!(host.window.lock().unwrap().stops.len(), 2);
        host.runtime.finish(&again, CaptureEndReason::UserStopped);
        assert_eq!(
            window_records(&host, &go),
            Err(MeetingError::StaleExtensionStart)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_window_or_a_stuck_record_path_times_out_and_cleans_up() {
        let (starts, host) = setup();
        assert_eq!(starts.start().await, Err(WINDOW_SILENT.into()));
        next_request(&host).await;

        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        assert_eq!(start.await.unwrap(), Err(TOO_SLOW.into()));
        // The window begins only now: it is refused, and nothing was stopped.
        assert_eq!(
            window_records(&host, &go),
            Err(MeetingError::StaleExtensionStart)
        );
        assert!(host.window.lock().unwrap().stops.is_empty());
        assert!(
            host.runtime
                .reserve_extension_start("next", "note-2")
                .is_ok()
        );
    }

    /// Signing out or switching accounts while the window starts drops the reservation: the
    /// window's begin is refused instead of recording the new account, and the tab hears it
    /// didn't start.
    #[tokio::test]
    async fn an_account_change_before_the_windows_begin_refuses_it() {
        let (starts, host) = setup();
        host.runtime.set_account(Some("account-1".into()));
        let start = spawn_start(&starts);
        let go = asked(&starts, &host, "note-1").await;
        host.runtime.set_account(Some("account-2".into()));
        assert_eq!(
            window_records(&host, &go),
            Err(MeetingError::StaleExtensionStart)
        );
        done(&starts, &go);
        assert_eq!(start.await.unwrap(), Err(DIDNT_START.into()));
        assert!(host.runtime.active().is_none());
    }
}
