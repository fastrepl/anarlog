//! "Switch my camera automatically": once a recording starts in a call the recorder confirmed,
//! switch that meeting app's camera to Anarlog Camera, once per meeting. Off by default. When it
//! can't, one gentle notice asks the person to pick it. A camera that is off stays off: only the
//! picker is used, never the camera button.
//!
//! The app's own background or blur would cut the badge out, so Anarlog Camera hides the room
//! first, Anarlog waits for a receipt that frames sent for exactly that request hide it, rechecks
//! that the person still wants all this, and only then turns the app's effect off. If the person
//! changes anything meanwhile, or the recording stops, Anarlog puts back the Background it saved
//! for them (compare-and-set: never over a choice they made since) and leaves the app alone.
//! Where the app's effect can't be read (Webex, Meet, an ambiguous read) one notice asks the person
//! to check it. Nothing watches afterwards: a person who switches back keeps their choice.
//!
//! Behaviour ported from Char `char/apps/desktop/src/main/camera-watermark/auto-switch.ts`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anlg_audio::meeting::{
    CallPlatform, CaptureAuthority, CaptureIdentity, ConfirmedCall, MeetingLifecycleEvent,
    MeetingLifecycleObserver,
};
use anlg_detect::meeting_camera::{
    BackgroundEffect, BackgroundOutcome, BackgroundReading, NativeMeetingApp, Outcome,
};
use serde::{Deserialize, Serialize};

use crate::prefs::{BackgroundRevision, CameraBackground, PrefsStore};

/// The camera's name in every meeting app's list (`CameraIdentity.deviceName`).
pub const CAMERA_NAME: &str = "Anarlog Camera";
/// Time for the call's window and controls to settle after the recording starts.
pub const SETTLE: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CameraApp {
    Zoom,
    Teams,
    Webex,
    GoogleMeet,
}

/// What the person is asked to do. The copy is the UI's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CameraGuidanceKind {
    /// Anarlog couldn't switch the camera: choose Anarlog Camera in the app's camera menu.
    PickCamera,
    /// The app's blur or background may hide the badge; keep it until Anarlog Camera hides the
    /// room (choose a Background), then check the video before turning the app's off.
    CheckBackgroundFirst,
    /// Anarlog Camera is hiding the room now; turn the app's own blur or background off.
    TurnOffAppBackground,
    /// Anarlog left the app's effect on because it couldn't confirm Anarlog Camera hides the room.
    AppBackgroundLeftOn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CameraGuidance {
    pub session_id: String,
    pub kind: CameraGuidanceKind,
    pub app: CameraApp,
}

/// What the browser extension answers after trying Meet's own camera settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MeetCameraOutcome {
    Switched,
    AlreadyUsing,
    AlreadyTried,
    CameraMissing,
    PickerUnavailable,
    Unconfirmed,
    NotInCall,
    Cancelled,
}

impl MeetCameraOutcome {
    pub fn parse(value: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(value.to_string())).ok()
    }

    fn uses_camera(self) -> bool {
        matches!(self, Self::Switched | Self::AlreadyUsing)
    }

    fn needs_notice(self) -> bool {
        matches!(
            self,
            Self::CameraMissing | Self::PickerUnavailable | Self::Unconfirmed
        )
    }
}

/// Anarlog Camera as the agent last reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraState {
    pub installed: bool,
    /// False while switching would show everyone the idle frame instead of the person.
    pub can_show_camera: bool,
    pub consumers: Vec<String>,
}

/// The switch's outside world: Anarlog Camera's agent, and the meeting apps through
/// Accessibility. Every method may block.
pub trait SwitchHost: Send + Sync {
    /// The agent's state now; None when it isn't running or can't say.
    fn camera_state(&self) -> Option<CameraState>;
    /// Tells the agent to draw `background`; returns the new request id.
    fn apply_background(&self, background: CameraBackground) -> String;
    /// The request the agent was last given.
    fn current_request(&self) -> (CameraBackground, String);
    /// Whether frames Anarlog Camera sent for exactly `request_id` hide the room with `expected`,
    /// waiting a bounded time; never true from a saved or desired Background alone.
    fn output_hides_room(
        &self,
        expected: CameraBackground,
        request_id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> bool;
    fn set_recording(&self, recording: bool);
    fn select(
        &self,
        app: NativeMeetingApp,
        pid: i32,
        cancelled: &dyn Fn() -> bool,
    ) -> anlg_detect::meeting_camera::Result<Outcome>;
    fn read_background(
        &self,
        app: NativeMeetingApp,
        pid: i32,
    ) -> anlg_detect::meeting_camera::Result<BackgroundReading>;
    fn turn_off_background(
        &self,
        app: NativeMeetingApp,
        pid: i32,
        known: BackgroundEffect,
        cancelled: &dyn Fn() -> bool,
    ) -> anlg_detect::meeting_camera::Result<BackgroundOutcome>;
}

enum Route {
    Switch(NativeMeetingApp, CameraApp),
    Notice(CameraApp),
    Skip,
}

/// Zoom, Teams and Webex are switched here when the bundle matches the platform the recorder
/// confirmed; Meet in a browser the extension didn't attach to gets the notice. A platform and
/// bundle that disagree authorize nothing.
fn route(call: &ConfirmedCall) -> Route {
    let native = |app: NativeMeetingApp, camera: CameraApp| {
        if call.bundle_id == app.bundle_id() {
            Route::Switch(app, camera)
        } else {
            Route::Skip
        }
    };
    match call.platform {
        CallPlatform::Zoom => native(NativeMeetingApp::Zoom, CameraApp::Zoom),
        CallPlatform::MicrosoftTeams => native(NativeMeetingApp::Teams, CameraApp::Teams),
        CallPlatform::Webex => native(NativeMeetingApp::Webex, CameraApp::Webex),
        CallPlatform::GoogleMeet => Route::Notice(CameraApp::GoogleMeet),
    }
}

/// Whether the call's app already shows Anarlog Camera. Apps open the camera from helper processes
/// signed with their own family's IDs (Zoom's `us.zoom.caphost`, Chrome's `com.google.Chrome.helper`).
pub fn call_uses_camera(consumers: &[String], bundle_id: &str) -> bool {
    let family = if bundle_id.starts_with("us.zoom.") {
        "us.zoom.".to_string()
    } else if bundle_id.starts_with("com.microsoft.teams") {
        "com.microsoft.teams".to_string()
    } else {
        format!("{bundle_id}.")
    };
    consumers
        .iter()
        .any(|id| id == bundle_id || id.starts_with(&family))
}

#[derive(Default)]
struct State {
    /// Sessions that had their try; a recording stopped before Anarlog acted gets another.
    tried: HashSet<String>,
    /// Sessions that got a notice: one per meeting.
    noticed: HashSet<String>,
    /// Apps whose effects Anarlog can't read get their notice once while it runs, not per call.
    unreadable_noticed: HashSet<CameraApp>,
    /// The pending switch of each recording, by capture, so its end can cancel it.
    pending: HashMap<String, Arc<AtomicBool>>,
    /// Captures recording now (not paused), for the badge.
    recording: HashSet<String>,
}

pub struct Switcher {
    prefs: Arc<PrefsStore>,
    host: Arc<dyn SwitchHost>,
    guide: Arc<dyn Fn(CameraGuidance) + Send + Sync>,
    settle: Duration,
    state: Mutex<State>,
}

impl Switcher {
    pub fn new(
        prefs: Arc<PrefsStore>,
        host: Arc<dyn SwitchHost>,
        guide: Arc<dyn Fn(CameraGuidance) + Send + Sync>,
        settle: Duration,
    ) -> Arc<Self> {
        Arc::new(Self {
            prefs,
            host,
            guide,
            settle,
            state: Mutex::new(State::default()),
        })
    }

    /// The browser extension's answer for a Meet capture it switched (or couldn't).
    pub fn meet_outcome(self: &Arc<Self>, capture: &CaptureIdentity, outcome: MeetCameraOutcome) {
        if outcome.needs_notice() {
            self.notice(
                &capture.session_id,
                CameraGuidanceKind::PickCamera,
                CameraApp::GoogleMeet,
            );
        } else if outcome.uses_camera() && self.prefs.get().switches() {
            // The extension can't read Meet's own effects, so the person is asked to check them.
            let this = self.clone();
            let session = capture.session_id.clone();
            std::thread::spawn(move || {
                this.unreadable_notice(&session, CameraApp::GoogleMeet, &|| false)
            });
        }
    }

    /// The camera to ask the extension for with a Meet start: Anarlog Camera when the camera and
    /// automatic switching are on, the camera exists and can show the person, and the browser
    /// isn't showing it already.
    pub fn camera_for_extension(&self, browser_bundle_id: &str) -> Option<&'static str> {
        if !self.prefs.get().switches() {
            return None;
        }
        let state = self.host.camera_state()?;
        (state.installed
            && state.can_show_camera
            && !call_uses_camera(&state.consumers, browser_bundle_id))
        .then_some(CAMERA_NAME)
    }

    /// The person's own Background. The camera takes it in the same step it is saved, so an
    /// automatic switch or its undo racing it can't leave the camera drawing an older one.
    pub fn set_background(&self, background: CameraBackground) -> crate::Result<()> {
        self.prefs
            .set_background(background, |background| {
                self.host.apply_background(background)
            })
            .map(drop)
    }

    fn update_badge(&self, capture: &CaptureIdentity, recording: bool) {
        let any = {
            let mut state = self.lock();
            if recording {
                state.recording.insert(capture.capture_id.clone());
            } else {
                state.recording.remove(&capture.capture_id);
            }
            !state.recording.is_empty()
        };
        self.host.set_recording(any);
    }

    fn start(self: &Arc<Self>, capture: &CaptureIdentity, authority: Arc<dyn CaptureAuthority>) {
        if !self.prefs.get().switches() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut state = self.lock();
            if state.tried.contains(&capture.session_id)
                || state.pending.contains_key(&capture.capture_id)
            {
                return;
            }
            state.tried.insert(capture.session_id.clone());
            state
                .pending
                .insert(capture.capture_id.clone(), cancel.clone());
        }
        let this = self.clone();
        let capture = capture.clone();
        std::thread::spawn(move || {
            let acted = this.run(&capture, authority.as_ref(), &cancel);
            let mut state = this.lock();
            state.pending.remove(&capture.capture_id);
            // Stopped before Anarlog touched the camera: the meeting hasn't had its try yet.
            if !acted && cancel.load(Ordering::SeqCst) {
                state.tried.remove(&capture.session_id);
            }
        });
    }

    fn cancel(&self, capture: &CaptureIdentity) {
        if let Some(cancel) = self.lock().pending.get(&capture.capture_id) {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Returns whether Anarlog acted on the call (switched, tried to, or asked the person).
    fn run(
        &self,
        capture: &CaptureIdentity,
        authority: &dyn CaptureAuthority,
        cancel: &AtomicBool,
    ) -> bool {
        // Re-checked after every wait: the capture, the account, extension ownership, the person's
        // intent, and cancellation.
        let still_ours = || {
            !cancel.load(Ordering::SeqCst)
                && authority.is_current(capture)
                && !authority.extension_handles_actions(capture)
                && self.prefs.get().switches()
        };
        let interrupted = || !still_ours();
        if !sleep_unless(self.settle, cancel) || !still_ours() {
            return false;
        }
        // Only a call the recorder confirmed for exactly this capture: never guess.
        let Some(call) = authority
            .confirmed_call(capture)
            .filter(|call| call.pid > 0)
        else {
            return false;
        };
        let Some(state) = self.host.camera_state() else {
            return false;
        };
        if !state.installed {
            return false;
        }
        // Without camera access the call would show the idle frame instead of the person.
        if !state.can_show_camera {
            tracing::info!(
                reason = "camera_unavailable",
                "camera_watermark_switch_skipped"
            );
            return false;
        }
        if !still_ours() {
            return false;
        }
        let showing = call_uses_camera(&state.consumers, &call.bundle_id);
        let (app, camera_app) = match route(&call) {
            Route::Skip => return false,
            Route::Notice(camera_app) => {
                if !showing {
                    self.notice(
                        &capture.session_id,
                        CameraGuidanceKind::PickCamera,
                        camera_app,
                    );
                }
                return !showing;
            }
            Route::Switch(app, camera_app) => (app, camera_app),
        };
        if !showing {
            let outcome = self
                .host
                .select(app, call.pid, &interrupted)
                .unwrap_or(Outcome::Unconfirmed);
            tracing::info!(
                app = app.as_str(),
                outcome = outcome.as_str(),
                "camera_watermark_switch"
            );
            match outcome {
                Outcome::Switched => {}
                // Nothing was chosen: the camera is as the person left it.
                Outcome::Cancelled => return false,
                _ => {
                    self.notice(
                        &capture.session_id,
                        CameraGuidanceKind::PickCamera,
                        camera_app,
                    );
                    return true;
                }
            }
        }
        self.app_background(capture, app, camera_app, call.pid, &still_ours);
        true
    }

    /// Anarlog Camera is the call's camera now; the app's own background would hide its badge.
    fn app_background(
        &self,
        capture: &CaptureIdentity,
        app: NativeMeetingApp,
        camera_app: CameraApp,
        pid: i32,
        still_ours: &dyn Fn() -> bool,
    ) {
        if !still_ours() {
            return;
        }
        let reading = self
            .host
            .read_background(app, pid)
            .unwrap_or(BackgroundReading::Unavailable);
        let effect = match reading {
            BackgroundReading::On(effect) => effect,
            BackgroundReading::Off | BackgroundReading::NoCall => return,
            // An effect Anarlog couldn't read, or read ambiguously, may still hide the badge.
            BackgroundReading::Unavailable | BackgroundReading::Unsupported => {
                tracing::info!(
                    app = app.as_str(),
                    reading = reading.as_str(),
                    "camera_watermark_app_background"
                );
                self.unreadable_notice(&capture.session_id, camera_app, &|| !still_ours());
                return;
            }
        };
        let Some(hidden) = self.hide_room(effect) else {
            tracing::info!(
                app = app.as_str(),
                char_background = "failed",
                "camera_watermark_app_background"
            );
            return;
        };
        let confirmed = self
            .host
            .output_hides_room(hidden.background, &hidden.request_id, &|| !still_ours());
        // The person may have turned the camera or switching off, chosen another Background, or
        // the recording stopped: the receipt was for what Anarlog asked, so the app's effect stays.
        let intent = || still_ours() && self.prefs.background() == hidden.saved;
        if !intent() {
            hidden.undo(self);
            return;
        }
        if !confirmed {
            // Never remove the app's effect while Anarlog Camera might still show the room.
            hidden.undo(self);
            self.notice(
                &capture.session_id,
                CameraGuidanceKind::AppBackgroundLeftOn,
                camera_app,
            );
            return;
        }
        let result = self
            .host
            .turn_off_background(app, pid, effect, &|| !intent())
            .unwrap_or(BackgroundOutcome::Unavailable);
        tracing::info!(
            app = app.as_str(),
            outcome = result.as_str(),
            "camera_watermark_app_background"
        );
        match result {
            // Refused before anything in the app changed: the person's Background is theirs again.
            BackgroundOutcome::Cancelled => hidden.undo(self),
            BackgroundOutcome::TurnedOff(_) | BackgroundOutcome::AlreadyOff => {}
            _ => self.notice(
                &capture.session_id,
                CameraGuidanceKind::TurnOffAppBackground,
                camera_app,
            ),
        }
    }

    /// Anarlog Camera hides the room in place of the app's own `effect`, from now on: a saved
    /// Background that already hides the room stays; otherwise it becomes Blur when the app
    /// blurred, else the default scene. The person can change it back in settings.
    fn hide_room(&self, effect: BackgroundEffect) -> Option<HiddenRoom> {
        let saved = self.prefs.background();
        if saved.background.hides_room() {
            let (background, request_id) = self.host.current_request();
            return (background == saved.background).then_some(HiddenRoom {
                background,
                request_id,
                saved,
                undo: None,
            });
        }
        let target = match effect {
            BackgroundEffect::Blur => CameraBackground::Blur,
            BackgroundEffect::Image => CameraBackground::DEFAULT_IMAGE,
        };
        let (written, request_id) = self
            .prefs
            .set_background_if(saved, target, |target| self.host.apply_background(target))
            .ok()??;
        Some(HiddenRoom {
            background: target,
            request_id,
            saved: written,
            undo: Some(saved.background),
        })
    }

    /// One notice per meeting.
    fn notice(&self, session_id: &str, kind: CameraGuidanceKind, app: CameraApp) {
        if !self.lock().noticed.insert(session_id.to_string()) {
            return;
        }
        (self.guide)(CameraGuidance {
            session_id: session_id.to_string(),
            kind,
            app,
        });
    }

    /// An app whose effects Anarlog can't read: ask once, and suggest turning the app's effect off
    /// only when Anarlog Camera's sent frames hide the room, never on a saved Background alone.
    fn unreadable_notice(&self, session_id: &str, app: CameraApp, cancelled: &dyn Fn() -> bool) {
        if !self.lock().unreadable_noticed.insert(app) {
            return;
        }
        let (background, request_id) = self.host.current_request();
        let hidden = background.hides_room()
            && self.prefs.get().background == background
            && self
                .host
                .output_hides_room(background, &request_id, cancelled);
        let kind = if hidden {
            CameraGuidanceKind::TurnOffAppBackground
        } else {
            CameraGuidanceKind::CheckBackgroundFirst
        };
        self.notice(session_id, kind, app);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The Background Anarlog Camera draws in place of the app's effect, and how to undo it.
struct HiddenRoom {
    background: CameraBackground,
    request_id: String,
    /// The saved Background this switch expects to stay: what it wrote, or what it kept.
    saved: BackgroundRevision,
    /// What to put back, when this switch wrote the Background.
    undo: Option<CameraBackground>,
}

impl HiddenRoom {
    /// Puts the person's Background back unless they changed it since.
    fn undo(&self, switcher: &Switcher) {
        let Some(previous) = self.undo else {
            return;
        };
        let _ = switcher
            .prefs
            .set_background_if(self.saved, previous, |previous| {
                switcher.host.apply_background(previous)
            });
    }
}

/// The switch as the meeting runtime's lifecycle observer.
pub struct SwitchObserver(pub Arc<Switcher>);

impl MeetingLifecycleObserver for SwitchObserver {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, authority: Arc<dyn CaptureAuthority>) {
        self.0.on_lifecycle(event, authority);
    }
}

impl Switcher {
    /// Returns at once; every switch runs on its own thread.
    pub fn on_lifecycle(
        self: &Arc<Self>,
        event: &MeetingLifecycleEvent,
        authority: Arc<dyn CaptureAuthority>,
    ) {
        match event {
            MeetingLifecycleEvent::Started { capture, .. } => {
                self.update_badge(capture, true);
                self.start(capture, authority);
            }
            MeetingLifecycleEvent::Resumed { capture } => self.update_badge(capture, true),
            MeetingLifecycleEvent::Paused { capture } => self.update_badge(capture, false),
            MeetingLifecycleEvent::Ended { capture, .. } => {
                self.cancel(capture);
                self.update_badge(capture, false);
            }
            MeetingLifecycleEvent::ExtensionAttached { capture } => self.cancel(capture),
            MeetingLifecycleEvent::ExtensionDetached { .. } => {}
        }
    }
}

/// Sleeps `duration` unless cancelled first; false when cancelled.
fn sleep_unless(duration: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20).min(deadline - Instant::now()));
    }
    !cancel.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use anlg_audio::meeting::{CaptureEndReason, CaptureEngine, CaptureScope, LifecycleOwner};

    use super::*;
    use crate::prefs::CameraScene;

    const DUSK: CameraBackground = CameraBackground::Scene {
        scene: CameraScene::Dusk,
    };

    type Hook = Box<dyn FnOnce() + Send>;

    struct FakeHost {
        prefs: Arc<PrefsStore>,
        camera: Mutex<Option<CameraState>>,
        select: Mutex<Outcome>,
        reading: Mutex<Option<BackgroundReading>>,
        turn_off: Mutex<BackgroundOutcome>,
        /// None: frames hide the room whenever the asked Background is on and still the saved one.
        hides: Mutex<Option<bool>>,
        /// Runs inside the receipt wait, as if the person or the recorder acted meanwhile.
        during_receipt: Mutex<Option<Hook>>,
        /// Runs just before the camera takes request number `.0`.
        before_apply: Mutex<Option<(u32, Hook)>>,
        request: Mutex<(CameraBackground, u32)>,
        steps: Mutex<Vec<String>>,
        badge: Mutex<Vec<bool>>,
    }

    impl FakeHost {
        fn new(prefs: Arc<PrefsStore>) -> Arc<Self> {
            Arc::new(Self {
                prefs,
                camera: Mutex::new(Some(CameraState {
                    installed: true,
                    can_show_camera: true,
                    consumers: vec![],
                })),
                select: Mutex::new(Outcome::Switched),
                reading: Mutex::new(Some(BackgroundReading::Off)),
                turn_off: Mutex::new(BackgroundOutcome::AlreadyOff),
                hides: Mutex::new(None),
                during_receipt: Mutex::new(None),
                before_apply: Mutex::new(None),
                request: Mutex::new((CameraBackground::None, 0)),
                steps: Mutex::new(vec![]),
                badge: Mutex::new(vec![]),
            })
        }

        fn step(&self, step: String) {
            self.steps.lock().unwrap().push(step);
        }

        fn steps(&self) -> Vec<String> {
            self.steps.lock().unwrap().clone()
        }
    }

    fn name(background: CameraBackground) -> &'static str {
        match background {
            CameraBackground::None => "none",
            CameraBackground::Blur => "blur",
            CameraBackground::Scene { .. } => "image",
        }
    }

    impl SwitchHost for FakeHost {
        fn camera_state(&self) -> Option<CameraState> {
            self.camera.lock().unwrap().clone()
        }
        fn apply_background(&self, background: CameraBackground) -> String {
            let next = self.request.lock().unwrap().1 + 1;
            let hook = {
                let mut hook = self.before_apply.lock().unwrap();
                match hook.take() {
                    Some((request, run)) if request == next => Some(run),
                    other => {
                        *hook = other;
                        None
                    }
                }
            };
            if let Some(run) = hook {
                run();
            }
            let mut request = self.request.lock().unwrap();
            *request = (background, request.1 + 1);
            self.step(format!("apply {}", name(background)));
            format!("r-{}", request.1)
        }
        fn current_request(&self) -> (CameraBackground, String) {
            let request = self.request.lock().unwrap();
            (request.0, format!("r-{}", request.1))
        }
        fn output_hides_room(
            &self,
            expected: CameraBackground,
            request_id: &str,
            cancelled: &dyn Fn() -> bool,
        ) -> bool {
            if let Some(hook) = self.during_receipt.lock().unwrap().take() {
                hook();
            }
            let current = self.current_request();
            let hidden =
                !cancelled()
                    && current == (expected, request_id.to_string())
                    && self.hides.lock().unwrap().unwrap_or(
                        expected.hides_room() && self.prefs.get().background == expected,
                    );
            self.step(format!("hides room: {hidden}"));
            hidden
        }
        fn set_recording(&self, recording: bool) {
            self.badge.lock().unwrap().push(recording);
        }
        fn select(
            &self,
            app: NativeMeetingApp,
            pid: i32,
            cancelled: &dyn Fn() -> bool,
        ) -> anlg_detect::meeting_camera::Result<Outcome> {
            if cancelled() {
                return Ok(Outcome::Cancelled);
            }
            self.step(format!("select {} {pid}", app.as_str()));
            Ok(*self.select.lock().unwrap())
        }
        fn read_background(
            &self,
            app: NativeMeetingApp,
            _pid: i32,
        ) -> anlg_detect::meeting_camera::Result<BackgroundReading> {
            self.step(format!("read {}", app.as_str()));
            self.reading
                .lock()
                .unwrap()
                .ok_or(anlg_detect::meeting_camera::Error::PermissionRequired)
        }
        fn turn_off_background(
            &self,
            app: NativeMeetingApp,
            _pid: i32,
            known: BackgroundEffect,
            cancelled: &dyn Fn() -> bool,
        ) -> anlg_detect::meeting_camera::Result<BackgroundOutcome> {
            if cancelled() {
                return Ok(BackgroundOutcome::Cancelled);
            }
            self.step(format!("turn off {} {}", app.as_str(), known.as_str()));
            Ok(*self.turn_off.lock().unwrap())
        }
    }

    struct FakeAuthority {
        current: AtomicBool,
        extension: AtomicBool,
        call: Mutex<Option<ConfirmedCall>>,
    }

    impl CaptureAuthority for FakeAuthority {
        fn is_current(&self, _capture: &CaptureIdentity) -> bool {
            self.current.load(Ordering::SeqCst)
        }
        fn confirmed_call(&self, _capture: &CaptureIdentity) -> Option<ConfirmedCall> {
            self.call.lock().unwrap().clone()
        }
        fn extension_handles_actions(&self, _capture: &CaptureIdentity) -> bool {
            self.extension.load(Ordering::SeqCst)
        }
    }

    fn call(platform: CallPlatform, pid: i32) -> ConfirmedCall {
        let bundle_id = match platform {
            CallPlatform::Zoom => "us.zoom.xos",
            CallPlatform::MicrosoftTeams => "com.microsoft.teams2",
            CallPlatform::Webex => "Cisco-Systems.Spark",
            CallPlatform::GoogleMeet => "com.google.Chrome",
        };
        ConfirmedCall {
            pid,
            bundle_id: bundle_id.into(),
            platform,
            call_url: None,
        }
    }

    struct Setup {
        prefs: Arc<PrefsStore>,
        host: Arc<FakeHost>,
        authority: Arc<FakeAuthority>,
        switcher: Arc<Switcher>,
        guidance: Arc<Mutex<Vec<CameraGuidance>>>,
        captures: Mutex<u32>,
    }

    impl Setup {
        fn new(call: Option<ConfirmedCall>) -> Self {
            Self::with_settle(call, Duration::ZERO)
        }

        fn with_settle(call: Option<ConfirmedCall>, settle: Duration) -> Self {
            let prefs = Arc::new(PrefsStore::load(None));
            prefs.set_enabled(true).unwrap();
            prefs.set_auto_switch(true).unwrap();
            let host = FakeHost::new(prefs.clone());
            let guidance = Arc::new(Mutex::new(vec![]));
            let sink = guidance.clone();
            let switcher = Switcher::new(
                prefs.clone(),
                host.clone(),
                Arc::new(move |g| sink.lock().unwrap().push(g)),
                settle,
            );
            Self {
                prefs,
                host,
                authority: Arc::new(FakeAuthority {
                    current: AtomicBool::new(true),
                    extension: AtomicBool::new(false),
                    call: Mutex::new(call),
                }),
                switcher,
                guidance,
                captures: Mutex::new(0),
            }
        }

        fn capture(&self, session: &str) -> CaptureIdentity {
            let mut count = self.captures.lock().unwrap();
            *count += 1;
            CaptureIdentity {
                account_id: "account".into(),
                session_id: session.into(),
                capture_id: format!("capture-{count}"),
            }
        }

        fn event(&self, event: MeetingLifecycleEvent) {
            self.switcher.on_lifecycle(&event, self.authority.clone());
        }

        fn begin(&self, capture: &CaptureIdentity) {
            self.event(MeetingLifecycleEvent::Started {
                capture: capture.clone(),
                scope: CaptureScope::System,
                owner: LifecycleOwner::App,
                engine: CaptureEngine::NativeMeeting,
            });
        }

        fn end(&self, capture: &CaptureIdentity) {
            self.event(MeetingLifecycleEvent::Ended {
                capture: capture.clone(),
                reason: CaptureEndReason::UserStopped,
            });
        }

        /// Starts a recording of `session` and waits for its switch to finish.
        fn started(&self, session: &str) -> CaptureIdentity {
            let capture = self.capture(session);
            self.begin(&capture);
            self.settled();
            capture
        }

        fn settled(&self) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !self.switcher.lock().pending.is_empty() {
                assert!(Instant::now() < deadline, "the switch never finished");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn guidance(&self) -> Vec<(CameraGuidanceKind, CameraApp)> {
            self.guidance
                .lock()
                .unwrap()
                .iter()
                .map(|g| (g.kind, g.app))
                .collect()
        }

        fn saved(&self) -> CameraBackground {
            self.prefs.get().background
        }

        fn reading(&self, reading: BackgroundReading) {
            *self.host.reading.lock().unwrap() = Some(reading);
        }

        fn during_receipt(&self, hook: impl FnOnce() + Send + 'static) {
            *self.host.during_receipt.lock().unwrap() = Some(Box::new(hook));
        }

        /// The person picks `background` in settings just as the camera is about to take request
        /// `request`. Waits a moment for their change, which may have to wait its turn.
        fn person_picks_during_apply(
            &self,
            request: u32,
            background: CameraBackground,
        ) -> Arc<Mutex<Option<std::thread::JoinHandle<()>>>> {
            let person = Arc::new(Mutex::new(None));
            let slot = person.clone();
            let switcher = self.switcher.clone();
            let pick: Hook = Box::new(move || {
                let (done, finished) = std::sync::mpsc::channel();
                *slot.lock().unwrap() = Some(std::thread::spawn(move || {
                    switcher.set_background(background).unwrap();
                    let _ = done.send(());
                }));
                let _ = finished.recv_timeout(Duration::from_millis(100));
            });
            *self.host.before_apply.lock().unwrap() = Some((request, pick));
            person
        }
    }

    #[test]
    fn does_nothing_with_automatic_switching_or_the_camera_off() {
        for (enabled, auto_switch) in [(true, false), (false, true)] {
            let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
            t.prefs.set_enabled(enabled).unwrap();
            t.prefs.set_auto_switch(auto_switch).unwrap();
            t.started("m1");
            assert_eq!(t.host.steps(), Vec::<String>::new());
            assert_eq!(t.guidance(), vec![]);
        }
    }

    #[test]
    fn switches_the_confirmed_call_once_per_meeting() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.started("m1");
        t.started("m1");
        assert_eq!(t.host.steps(), vec!["select zoom 11", "read zoom"]);
        t.started("m2");
        assert_eq!(
            t.host
                .steps()
                .iter()
                .filter(|s| s.starts_with("select"))
                .count(),
            2
        );
        assert_eq!(t.guidance(), vec![]);
    }

    #[test]
    fn routes_teams_and_webex_to_their_own_apps() {
        for (platform, expected) in [
            (CallPlatform::MicrosoftTeams, "select teams 12"),
            (CallPlatform::Webex, "select webex 15"),
        ] {
            let pid = if platform == CallPlatform::Webex {
                15
            } else {
                12
            };
            let t = Setup::new(Some(call(platform, pid)));
            t.started("m1");
            assert_eq!(t.host.steps()[0], expected);
        }
    }

    #[test]
    fn never_guesses_without_a_confirmed_call_or_with_a_mismatched_one() {
        let mut mismatched = call(CallPlatform::Zoom, 11);
        mismatched.bundle_id = "com.example.zoomish".into();
        for confirmed in [None, Some(call(CallPlatform::Zoom, 0)), Some(mismatched)] {
            let t = Setup::new(confirmed);
            t.started("m1");
            assert_eq!(t.host.steps(), Vec::<String>::new());
            assert_eq!(t.guidance(), vec![]);
        }
    }

    #[test]
    fn keeps_a_call_already_showing_the_camera_but_still_clears_its_background() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.host.camera.lock().unwrap().as_mut().unwrap().consumers = vec!["us.zoom.caphost".into()];
        t.reading(BackgroundReading::On(BackgroundEffect::Blur));
        *t.host.turn_off.lock().unwrap() = BackgroundOutcome::TurnedOff(BackgroundEffect::Blur);
        t.started("m1");
        assert!(!t.host.steps().iter().any(|s| s.starts_with("select")));
        assert!(t.host.steps().contains(&"turn off zoom blur".to_string()));
        assert_eq!(t.guidance(), vec![]);
    }

    #[test]
    fn hides_the_room_before_the_apps_effect_comes_off() {
        let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
        t.reading(BackgroundReading::On(BackgroundEffect::Blur));
        *t.host.turn_off.lock().unwrap() = BackgroundOutcome::TurnedOff(BackgroundEffect::Blur);
        t.started("m1");
        assert_eq!(
            t.host.steps(),
            vec![
                "select teams 12",
                "read teams",
                "apply blur",
                "hides room: true",
                "turn off teams blur"
            ]
        );
        assert_eq!(
            t.saved(),
            CameraBackground::Blur,
            "for every later call too"
        );
        assert_eq!(t.guidance(), vec![]);

        let image = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        image.reading(BackgroundReading::On(BackgroundEffect::Image));
        image.started("m1");
        assert_eq!(image.saved(), CameraBackground::DEFAULT_IMAGE);
    }

    #[test]
    fn keeps_a_background_the_person_chose() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.switcher.set_background(DUSK).unwrap();
        t.reading(BackgroundReading::On(BackgroundEffect::Blur));
        t.started("m1");
        assert_eq!(t.saved(), DUSK);
        assert_eq!(
            t.host
                .steps()
                .iter()
                .filter(|s| s.starts_with("apply"))
                .count(),
            1,
            "only the person's own"
        );
        assert!(t.host.steps().contains(&"turn off zoom blur".to_string()));
    }

    #[test]
    fn leaves_both_backgrounds_alone_when_the_app_showed_none() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.started("m1");
        assert_eq!(t.host.steps(), vec!["select zoom 11", "read zoom"]);
        assert_eq!(t.saved(), CameraBackground::None);
    }

    #[test]
    fn touches_no_background_when_the_camera_did_not_switch() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        *t.host.select.lock().unwrap() = Outcome::CameraMissing;
        t.started("m1");
        t.started("m1");
        assert_eq!(t.host.steps(), vec!["select zoom 11"], "and never again");
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::PickCamera, CameraApp::Zoom)]
        );
    }

    #[test]
    fn skips_when_the_camera_is_missing_unknown_or_cant_show_the_person() {
        for camera in [
            None,
            Some(CameraState {
                installed: false,
                can_show_camera: false,
                consumers: vec![],
            }),
            // Camera access not granted, or revoked: the call would show the idle frame.
            Some(CameraState {
                installed: true,
                can_show_camera: false,
                consumers: vec![],
            }),
        ] {
            let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
            *t.host.camera.lock().unwrap() = camera;
            t.started("m1");
            assert_eq!(t.host.steps(), Vec::<String>::new());
            assert_eq!(t.guidance(), vec![]);
        }
    }

    #[test]
    fn leaves_a_meeting_the_extension_handles_to_the_extension() {
        let t = Setup::new(Some(call(CallPlatform::GoogleMeet, 13)));
        t.authority.extension.store(true, Ordering::SeqCst);
        t.started("m1");
        assert_eq!(t.host.steps(), Vec::<String>::new());
        assert_eq!(t.guidance(), vec![]);
    }

    #[test]
    fn asks_the_person_in_meet_without_the_extension() {
        let t = Setup::new(Some(call(CallPlatform::GoogleMeet, 13)));
        t.started("m1");
        assert_eq!(t.host.steps(), Vec::<String>::new());
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::PickCamera, CameraApp::GoogleMeet)]
        );
    }

    #[test]
    fn a_recording_that_stops_while_the_call_settles_does_nothing_and_keeps_its_try() {
        let t = Setup::with_settle(
            Some(call(CallPlatform::Zoom, 11)),
            Duration::from_millis(300),
        );
        let capture = t.capture("m1");
        t.begin(&capture);
        t.end(&capture);
        t.settled();
        assert_eq!(t.host.steps(), Vec::<String>::new());
        t.started("m1");
        assert_eq!(t.host.steps()[0], "select zoom 11");
    }

    #[test]
    fn a_stale_capture_or_an_extension_that_attached_meanwhile_stops_native_actions() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.authority.current.store(false, Ordering::SeqCst);
        t.started("m1");
        let attached = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        attached.reading(BackgroundReading::On(BackgroundEffect::Blur));
        let authority = attached.authority.clone();
        attached.during_receipt(move || authority.extension.store(true, Ordering::SeqCst));
        attached.started("m1");
        assert_eq!(t.host.steps(), Vec::<String>::new());
        assert!(
            !attached
                .host
                .steps()
                .iter()
                .any(|s| s.starts_with("turn off"))
        );
        assert_eq!(
            attached.saved(),
            CameraBackground::None,
            "its Background put back"
        );
    }

    #[test]
    fn a_cancelled_camera_switch_changes_nothing_and_asks_nothing() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        *t.host.select.lock().unwrap() = Outcome::Cancelled;
        t.started("m1");
        assert_eq!(t.guidance(), vec![]);
        assert!(!t.host.steps().iter().any(|s| s.starts_with("read")));
    }

    #[test]
    fn keeps_the_apps_effect_and_the_persons_background_without_a_receipt() {
        let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
        t.reading(BackgroundReading::On(BackgroundEffect::Image));
        *t.host.hides.lock().unwrap() = Some(false);
        t.started("m1");
        assert!(!t.host.steps().iter().any(|s| s.starts_with("turn off")));
        assert_eq!(t.saved(), CameraBackground::None);
        assert_eq!(
            t.host.current_request().0,
            CameraBackground::None,
            "the agent too"
        );
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::AppBackgroundLeftOn, CameraApp::Teams)]
        );
    }

    #[test]
    fn never_removes_an_effect_it_couldnt_read_and_asks_once() {
        for reading in [
            Some(BackgroundReading::Unavailable),
            None, // permission required
        ] {
            let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
            *t.host.reading.lock().unwrap() = reading;
            t.started("m1");
            t.started("m2");
            assert!(!t.host.steps().iter().any(|s| s.starts_with("turn off")));
            assert_eq!(t.saved(), CameraBackground::None);
            assert_eq!(
                t.guidance(),
                vec![(CameraGuidanceKind::CheckBackgroundFirst, CameraApp::Teams)]
            );
        }
        let none = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
        none.reading(BackgroundReading::NoCall);
        none.started("m1");
        assert_eq!(none.guidance(), vec![]);
    }

    #[test]
    fn webex_effects_stay_manual_and_turning_them_off_is_suggested_only_on_a_receipt() {
        let t = Setup::new(Some(call(CallPlatform::Webex, 15)));
        t.reading(BackgroundReading::Unsupported);
        t.started("m1");
        t.started("m2");
        assert!(!t.host.steps().iter().any(|s| s.starts_with("turn off")));
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::CheckBackgroundFirst, CameraApp::Webex)]
        );

        let hidden = Setup::new(Some(call(CallPlatform::Webex, 15)));
        hidden
            .switcher
            .set_background(CameraBackground::Blur)
            .unwrap();
        hidden.reading(BackgroundReading::Unsupported);
        hidden.started("m1");
        assert_eq!(
            hidden.guidance(),
            vec![(CameraGuidanceKind::TurnOffAppBackground, CameraApp::Webex)]
        );

        // Blur is saved, but no frame was seen hiding the room: an old agent, nobody streaming.
        let saved_only = Setup::new(Some(call(CallPlatform::Webex, 15)));
        saved_only
            .switcher
            .set_background(CameraBackground::Blur)
            .unwrap();
        *saved_only.host.hides.lock().unwrap() = Some(false);
        saved_only.reading(BackgroundReading::Unsupported);
        saved_only.started("m1");
        assert_eq!(
            saved_only.guidance(),
            vec![(CameraGuidanceKind::CheckBackgroundFirst, CameraApp::Webex)]
        );
    }

    #[test]
    fn meet_outcomes_ask_once_and_never_outside_a_call() {
        let t = Setup::new(None);
        let m2 = t.capture("m2");
        t.switcher.meet_outcome(&m2, MeetCameraOutcome::NotInCall);
        t.switcher
            .meet_outcome(&m2, MeetCameraOutcome::AlreadyTried);
        assert_eq!(t.guidance(), vec![]);
        let m3 = t.capture("m3");
        t.switcher
            .meet_outcome(&m3, MeetCameraOutcome::CameraMissing);
        t.switcher.meet_outcome(&m3, MeetCameraOutcome::Unconfirmed);
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::PickCamera, CameraApp::GoogleMeet)]
        );

        let switched = Setup::new(None);
        switched
            .switcher
            .meet_outcome(&switched.capture("m1"), MeetCameraOutcome::Switched);
        let deadline = Instant::now() + Duration::from_secs(5);
        while switched.guidance().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            switched.guidance(),
            vec![(
                CameraGuidanceKind::CheckBackgroundFirst,
                CameraApp::GoogleMeet
            )]
        );
        assert_eq!(
            MeetCameraOutcome::parse("already_using"),
            Some(MeetCameraOutcome::AlreadyUsing)
        );
        assert_eq!(MeetCameraOutcome::parse("exploded"), None);
    }

    #[test]
    fn keeps_anarlogs_background_and_asks_when_the_apps_wont_come_off() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.reading(BackgroundReading::On(BackgroundEffect::Blur));
        *t.host.turn_off.lock().unwrap() = BackgroundOutcome::Unavailable;
        t.started("m1");
        assert_eq!(t.saved(), CameraBackground::Blur);
        assert_eq!(
            t.guidance(),
            vec![(CameraGuidanceKind::TurnOffAppBackground, CameraApp::Zoom)]
        );
    }

    #[test]
    fn puts_the_persons_background_back_when_the_recording_stops_mid_transition() {
        let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
        t.reading(BackgroundReading::On(BackgroundEffect::Blur));
        let capture = t.capture("m1");
        let switcher = t.switcher.clone();
        let authority = t.authority.clone();
        let ended = capture.clone();
        t.during_receipt(move || {
            switcher.on_lifecycle(
                &MeetingLifecycleEvent::Ended {
                    capture: ended,
                    reason: CaptureEndReason::UserStopped,
                },
                authority,
            )
        });
        t.begin(&capture);
        t.settled();
        assert!(!t.host.steps().iter().any(|s| s.starts_with("turn off")));
        assert_eq!(t.saved(), CameraBackground::None);
        assert_eq!(t.host.current_request().0, CameraBackground::None);
    }

    #[test]
    fn keeps_the_apps_effect_when_the_person_changes_their_mind_during_the_wait() {
        type Change = fn(&Switcher);
        let changes: [(&str, Change, CameraBackground); 3] = [
            (
                "switching off",
                |s| drop(s.prefs.set_auto_switch(false)),
                CameraBackground::None,
            ),
            (
                "camera off",
                |s| drop(s.prefs.set_enabled(false)),
                CameraBackground::None,
            ),
            ("another background", |s| drop(s.set_background(DUSK)), DUSK),
        ];
        for (change, apply, saved) in changes {
            let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
            t.reading(BackgroundReading::On(BackgroundEffect::Blur));
            // Anarlog Camera's frames do confirm the blur Anarlog asked for...
            *t.host.hides.lock().unwrap() = Some(true);
            let switcher = t.switcher.clone();
            // ...but the person changed something before the receipt came.
            t.during_receipt(move || apply(&switcher));
            t.started("m1");
            assert!(
                !t.host.steps().iter().any(|s| s.starts_with("turn off")),
                "{change}"
            );
            assert_eq!(t.guidance(), vec![], "{change}");
            assert_eq!(t.saved(), saved, "{change}");
        }
    }

    #[test]
    fn a_background_the_person_picks_while_anarlog_sends_its_own_is_what_the_camera_draws() {
        // Request 1 is the Blur Anarlog saves for the call; request 2, without a receipt, puts the
        // person's None back.
        for (request, receipt) in [(1, None), (2, Some(false))] {
            let t = Setup::new(Some(call(CallPlatform::MicrosoftTeams, 12)));
            t.reading(BackgroundReading::On(BackgroundEffect::Blur));
            *t.host.hides.lock().unwrap() = receipt;
            let person = t.person_picks_during_apply(request, DUSK);
            t.started("m1");
            let picked = person.lock().unwrap().take();
            picked.expect("the person picked").join().unwrap();
            assert_eq!(t.saved(), DUSK, "request {request}");
            assert_eq!(
                t.host.current_request().0,
                DUSK,
                "request {request}: the camera draws it too"
            );
        }
    }

    #[test]
    fn a_turn_off_refused_before_it_changed_anything_puts_the_background_back() {
        let t = Setup::new(Some(call(CallPlatform::Zoom, 11)));
        t.reading(BackgroundReading::On(BackgroundEffect::Image));
        *t.host.turn_off.lock().unwrap() = BackgroundOutcome::Cancelled;
        t.started("m1");
        assert_eq!(t.saved(), CameraBackground::None);
        assert_eq!(t.guidance(), vec![]);
    }

    #[test]
    fn the_badge_follows_every_recording_and_its_pauses() {
        let t = Setup::new(None);
        t.prefs.set_auto_switch(false).unwrap();
        let a = t.capture("m1");
        let b = t.capture("m2");
        t.begin(&a);
        t.begin(&b);
        t.event(MeetingLifecycleEvent::Paused { capture: a.clone() });
        t.end(&b);
        t.event(MeetingLifecycleEvent::Resumed { capture: a.clone() });
        t.end(&a);
        assert_eq!(
            *t.host.badge.lock().unwrap(),
            vec![true, true, true, false, true, false]
        );
    }

    #[test]
    fn asks_for_the_camera_with_a_meet_start_only_when_it_can_show_the_person() {
        let t = Setup::new(None);
        assert_eq!(
            t.switcher.camera_for_extension("com.google.Chrome"),
            Some(CAMERA_NAME)
        );
        t.host.camera.lock().unwrap().as_mut().unwrap().consumers =
            vec!["com.google.Chrome.helper".into()];
        assert_eq!(t.switcher.camera_for_extension("com.google.Chrome"), None);
        t.prefs.set_auto_switch(false).unwrap();
        t.host.camera.lock().unwrap().as_mut().unwrap().consumers = vec![];
        assert_eq!(t.switcher.camera_for_extension("com.google.Chrome"), None);
    }

    #[test]
    fn counts_an_apps_helper_processes_as_the_app() {
        assert!(call_uses_camera(&["us.zoom.caphost".into()], "us.zoom.xos"));
        assert!(call_uses_camera(
            &["com.google.Chrome.helper".into()],
            "com.google.Chrome"
        ));
        assert!(!call_uses_camera(
            &["com.google.Chrome".into()],
            "us.zoom.xos"
        ));
        assert!(call_uses_camera(
            &["Cisco-Systems.Spark".into()],
            "Cisco-Systems.Spark"
        ));
        assert!(!call_uses_camera(
            &["com.google.ChromeCanary".into()],
            "com.google.Chrome"
        ));
    }
}
