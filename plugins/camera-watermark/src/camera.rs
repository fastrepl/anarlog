use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anlg_audio::meeting::{CaptureIdentity, MeetingLifecycleObserver};
use anlg_detect::meeting_camera::{
    self, BackgroundEffect, BackgroundOutcome, BackgroundReading, NativeMeetingApp, Outcome,
};
use serde::{Deserialize, Serialize};

use crate::helper::{
    AgentRegistration, CameraAccess, ExtensionState, Helper, HelperCommand, HelperEvent,
};
use crate::prefs::{CameraBackground, CameraPrefs, PrefsStore};
use crate::receipt;
use crate::switch::{
    CAMERA_NAME, CameraApp, CameraGuidance, CameraState, MeetCameraOutcome, SETTLE, SwitchHost,
    SwitchObserver, Switcher,
};

/// How long a meeting-app automation step may take in total.
const NATIVE_TIMEOUT: Duration = Duration::from_secs(8);
/// How long "Add extension" waits for macOS's first answer before showing what it knows.
const EXTENSION_ANSWER: Duration = Duration::from_secs(5);
/// How long settings waits for `serve` to look for the camera.
const CHECK_ANSWER: Duration = Duration::from_secs(2);
const LOGIN_ITEMS_AND_EXTENSIONS: &str =
    "x-apple.systempreferences:com.apple.LoginItems-Settings.extension";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// Not macOS.
    UnsupportedPlatform,
    /// This build has no Anarlog Camera (the App Store build, or a build packaged without it).
    NotBundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CameraAvailability {
    Available,
    Unavailable { reason: UnavailableReason },
}

/// Whether macOS has the extension, as far as Anarlog can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSetup {
    NotAdded,
    NeedsApproval,
    Active,
    Failed,
}

/// Which apps Anarlog drives itself, and which stay manual. Static: these are what the code and
/// its fixtures support, not a live check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AutomationSupport {
    /// Apps whose camera Anarlog switches through Accessibility. Meet is the browser extension's.
    pub switches_camera: Vec<CameraApp>,
    /// Apps whose own blur or background Anarlog can read and turn off after the receipt.
    pub removes_app_background: Vec<CameraApp>,
    /// Apps whose own blur or background the person must check and turn off themselves.
    pub manual_app_background: Vec<CameraApp>,
}

impl AutomationSupport {
    fn current() -> Self {
        if !meeting_camera::is_supported() {
            return Self {
                switches_camera: vec![],
                removes_app_background: vec![],
                manual_app_background: vec![],
            };
        }
        Self {
            switches_camera: vec![CameraApp::Zoom, CameraApp::Teams, CameraApp::Webex],
            removes_app_background: vec![CameraApp::Zoom, CameraApp::Teams],
            manual_app_background: vec![CameraApp::Webex, CameraApp::GoogleMeet],
        }
    }
}

/// Everything settings shows about Anarlog Camera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CameraWatermarkState {
    pub prefs: CameraPrefs,
    pub availability: CameraAvailability,
    pub extension: ExtensionSetup,
    /// Why activation failed, as macOS said (`missing_entitlement`, `not_in_applications`, …).
    pub extension_failure: Option<String>,
    pub camera_access: CameraAccess,
    /// The login agent reported lately.
    pub agent_running: bool,
    pub agent_registration: Option<AgentRegistration>,
    /// Signing IDs of the apps showing Anarlog Camera now.
    pub in_use_by: Vec<String>,
    pub automation: AutomationSupport,
}

/// Anarlog Camera from Anarlog's side: the person's choices, the helper, and the automatic switch.
pub struct CameraWatermark {
    prefs: Arc<PrefsStore>,
    helper: Arc<Helper>,
    switcher: Arc<Switcher>,
}

impl CameraWatermark {
    pub fn new(
        prefs_path: Option<PathBuf>,
        helper_executable: Option<PathBuf>,
        agent_directory: PathBuf,
        guide: Arc<dyn Fn(CameraGuidance) + Send + Sync>,
    ) -> Arc<Self> {
        let prefs = Arc::new(PrefsStore::load(prefs_path));
        let helper = Helper::new(helper_executable, agent_directory);
        let host = Arc::new(NativeHost {
            helper: helper.clone(),
            available: meeting_camera::is_supported() && helper.is_bundled(),
        });
        let switcher = Switcher::new(prefs.clone(), host, guide, SETTLE);
        Arc::new(Self {
            prefs,
            helper,
            switcher,
        })
    }

    pub fn availability(&self) -> CameraAvailability {
        if !meeting_camera::is_supported() {
            return CameraAvailability::Unavailable {
                reason: UnavailableReason::UnsupportedPlatform,
            };
        }
        if !self.helper.is_bundled() {
            return CameraAvailability::Unavailable {
                reason: UnavailableReason::NotBundled,
            };
        }
        CameraAvailability::Available
    }

    fn available(&self) -> bool {
        self.availability() == CameraAvailability::Available
    }

    /// At launch: tells the agent the saved choices, and keeps it registered while the camera is
    /// on. Never opens the camera; the agent does that only while a meeting app uses Anarlog Camera.
    pub fn launch(&self) {
        if !self.available() {
            return;
        }
        let prefs = self.prefs.get();
        self.helper.set_background(prefs.background);
        if prefs.enabled {
            self.helper.start();
        }
    }

    /// Anarlog is quitting: the agent keeps showing the camera, without the badge.
    pub fn close(&self) {
        if self.available() {
            self.helper.close();
        }
    }

    pub fn state(&self) -> CameraWatermarkState {
        let snapshot = self.helper.snapshot();
        let request = snapshot.extension.clone();
        let installed = snapshot
            .status
            .as_ref()
            .is_some_and(|status| status.camera_installed)
            || snapshot.camera_found == Some(true)
            || matches!(request, Some((ExtensionState::Active, _)));
        let extension = match &request {
            _ if installed => ExtensionSetup::Active,
            Some((ExtensionState::NeedsApproval, _)) => ExtensionSetup::NeedsApproval,
            Some((ExtensionState::Failed, _)) => ExtensionSetup::Failed,
            _ => ExtensionSetup::NotAdded,
        };
        CameraWatermarkState {
            prefs: self.prefs.get(),
            availability: self.availability(),
            extension,
            extension_failure: match (extension, request) {
                (ExtensionSetup::Failed, Some((_, detail))) => detail,
                _ => None,
            },
            camera_access: snapshot.camera_permission.unwrap_or(CameraAccess::NotAsked),
            agent_running: snapshot.running,
            agent_registration: snapshot.agent.map(|(state, _)| state),
            in_use_by: snapshot
                .status
                .map(|status| status.consumers)
                .unwrap_or_default(),
            automation: AutomationSupport::current(),
        }
    }

    /// Turning the camera on registers the agent, which shows the person's camera in any app that
    /// picks Anarlog Camera; off stops it at once. Refused where this build has no Anarlog Camera.
    pub fn set_enabled(&self, enabled: bool) -> crate::Result<CameraWatermarkState> {
        if enabled && !self.available() {
            return Err(crate::Error::Unavailable);
        }
        self.prefs.set_enabled(enabled)?;
        if enabled {
            self.helper.start();
        } else {
            self.helper.stop();
        }
        Ok(self.state())
    }

    pub fn set_auto_switch(&self, auto_switch: bool) -> crate::Result<CameraWatermarkState> {
        self.prefs.set_auto_switch(auto_switch)?;
        Ok(self.state())
    }

    /// The person's own Background, which always wins over one Anarlog saved for them.
    pub fn set_background(
        &self,
        background: CameraBackground,
    ) -> crate::Result<CameraWatermarkState> {
        self.switcher.set_background(background)?;
        Ok(self.state())
    }

    /// What settings shows. While the agent says nothing (the camera is off), `serve` looks for
    /// the device itself, so an added extension never reads as missing.
    pub async fn refreshed_state(&self) -> CameraWatermarkState {
        if self.available() && !self.helper.snapshot().running {
            self.ask(HelperCommand::Check, CHECK_ANSWER, |event| {
                matches!(event, HelperEvent::Camera { .. })
            })
            .await;
        }
        self.state()
    }

    /// Sends `command` to `serve` and waits up to `wait` for the event `answers` accepts.
    async fn ask(
        &self,
        command: HelperCommand,
        wait: Duration,
        answers: impl Fn(&HelperEvent) -> bool,
    ) {
        let mut events = self.helper.subscribe();
        self.helper.send(command);
        let _ = tokio::time::timeout(wait, async {
            while let Ok(event) = events.recv().await {
                if answers(&event) {
                    break;
                }
            }
        })
        .await;
    }

    /// Submits the extension; macOS asks the person to allow it. Never claims success macOS didn't
    /// report: without the provisioning profile the answer is `failed` / `missing_entitlement`.
    pub async fn add_extension(&self) -> CameraWatermarkState {
        if self.available() {
            self.ask(HelperCommand::Activate, EXTENSION_ANSWER, |event| {
                matches!(event, HelperEvent::Extension { .. })
            })
            .await;
        }
        self.state()
    }

    /// The agent reads the camera under its own permission, so it asks macOS itself.
    pub fn allow_camera(&self) -> CameraWatermarkState {
        if self.available() && self.state().camera_access == CameraAccess::NotAsked {
            self.helper.request_camera();
        }
        self.state()
    }

    pub fn open_extension_settings(&self) -> crate::Result<()> {
        #[cfg(target_os = "macos")]
        std::process::Command::new("/usr/bin/open")
            .arg(LOGIN_ITEMS_AND_EXTENSIONS)
            .spawn()?;
        #[cfg(not(target_os = "macos"))]
        let _ = LOGIN_ITEMS_AND_EXTENSIONS;
        Ok(())
    }

    /// Register this with the meeting runtime's lifecycle fan-out.
    pub fn observer(&self) -> Arc<dyn MeetingLifecycleObserver> {
        Arc::new(SwitchObserver(self.switcher.clone()))
    }

    /// The browser route's input: what the extension answered after trying Meet's camera.
    pub fn meet_camera_outcome(&self, capture: &CaptureIdentity, outcome: MeetCameraOutcome) {
        self.switcher.meet_outcome(capture, outcome);
    }

    /// The camera name the extension should switch Meet to, or None to leave Meet's camera alone.
    pub fn camera_for_extension(&self, browser_bundle_id: &str) -> Option<&'static str> {
        self.switcher.camera_for_extension(browser_bundle_id)
    }
}

/// The real switch host: Anarlog Camera's agent files, and `anlg_detect::meeting_camera`.
struct NativeHost {
    helper: Arc<Helper>,
    /// Without Anarlog Camera in this build, nothing is written for an agent that can't exist.
    available: bool,
}

impl SwitchHost for NativeHost {
    fn camera_state(&self) -> Option<CameraState> {
        let snapshot = self.helper.snapshot();
        let status = snapshot.status.filter(|_| snapshot.running)?;
        Some(CameraState {
            installed: status.camera_installed,
            can_show_camera: status.camera_permission == CameraAccess::Allowed
                && status.problem.is_none(),
            consumers: status.consumers,
        })
    }

    fn apply_background(&self, background: CameraBackground) -> String {
        self.helper.set_background(background)
    }

    fn current_request(&self) -> (CameraBackground, String) {
        let snapshot = self.helper.snapshot();
        (snapshot.background, snapshot.background_id)
    }

    fn output_hides_room(
        &self,
        expected: CameraBackground,
        request_id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> bool {
        receipt::wait_for(
            || self.helper.snapshot(),
            expected,
            request_id,
            receipt::RECEIPT_WAIT,
            cancelled,
        )
    }

    fn set_recording(&self, recording: bool) {
        if self.available {
            self.helper.set_recording(recording);
        }
    }

    fn select(
        &self,
        app: NativeMeetingApp,
        pid: i32,
        cancelled: &dyn Fn() -> bool,
    ) -> meeting_camera::Result<Outcome> {
        meeting_camera::select_camera(app, pid, CAMERA_NAME, NATIVE_TIMEOUT, cancelled)
    }

    fn read_background(
        &self,
        app: NativeMeetingApp,
        pid: i32,
    ) -> meeting_camera::Result<BackgroundReading> {
        meeting_camera::read_background(app, pid, NATIVE_TIMEOUT)
    }

    fn turn_off_background(
        &self,
        app: NativeMeetingApp,
        pid: i32,
        known: BackgroundEffect,
        cancelled: &dyn Fn() -> bool,
    ) -> meeting_camera::Result<BackgroundOutcome> {
        meeting_camera::turn_off_background(app, pid, Some(known), NATIVE_TIMEOUT, cancelled)
    }
}
