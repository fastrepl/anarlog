//! The browser route's view of the app: the one meeting runtime for identity, links and evidence,
//! the main window's own record path for extension starts, the native recorder's pause, the record
//! path's stop, and the notice and camera plugins' Meet answers. Nothing here records by itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anlg_audio::meeting::{
    CallPlatform, CaptureAuthority, CaptureEngine, CaptureIdentity, LifecycleOwner,
    ObservationSource, ObservedParticipant, SpeakerObservation, SpeakingCue,
};
use anlg_transcription_core::browser_meeting::{
    BrowserMeetingPort, CameraOutcome, ChatOutcome, DetachOutcome, ExtensionStart, ExtensionStarts,
    MeetObservation, Owner, PortError, RecordedCall, RecorderStatus, RecorderView, START_GO_EVENT,
    START_REPLY_EVENT, START_REQUEST_EVENT, StartGo, StartHost, StartReply, StartRequest,
};
use anlg_transcription_core::meeting::{self as runtime, LinkToken, MeetingError};
use tauri::{Emitter, Listener, Manager};

use anlg_audio::meeting::NoticeOutcome;
use anlg_transcription_core::browser_meeting::NoticeKind;
use tauri_plugin_camera_watermark::{CameraWatermarkPluginExt, MeetCameraOutcome};
use tauri_plugin_detect::MeetingNoticeObserver;
use tauri_plugin_transcription::MeetingCapture;

const MAIN_WINDOW: &str = "main";
const NEEDS_CALL_RECORDER: &str = "Only Anarlog's call recorder can confirm a Meet call, and it isn't on. Start the recording in Anarlog instead.";

/// Extension starts through the main window: it creates the note and records it the record
/// button's way, carrying the start's request id; the runtime reserves the capture of exactly that
/// request for the extension.
pub struct TauriStartHost {
    app: tauri::AppHandle,
    meeting: MeetingCapture,
}

impl StartHost for TauriStartHost {
    fn request(&self, request: StartRequest) -> Result<(), String> {
        if self.app.get_webview_window(MAIN_WINDOW).is_none() {
            return Err("no main window".into());
        }
        self.app
            .emit_to(MAIN_WINDOW, START_REQUEST_EVENT, request)
            .map_err(|error| error.to_string())
    }

    fn answer(&self, go: StartGo) {
        if let Err(error) = self.app.emit_to(MAIN_WINDOW, START_GO_EVENT, go) {
            tracing::warn!(%error, "browser_meeting_start_answer_failed");
        }
    }

    fn reserve(&self, request_id: &str, session_id: &str) -> Result<String, String> {
        self.meeting
            .runtime
            .reserve_extension_start(request_id, session_id)
            .map_err(|error| match error {
                MeetingError::CaptureActive => {
                    "Anarlog is already recording a meeting. Stop it first.".to_string()
                }
                other => other.to_string(),
            })
    }

    fn cancel(&self, request_id: &str) -> Option<CaptureIdentity> {
        self.meeting.runtime.cancel_extension_start(request_id)
    }

    fn stop(&self, capture: &CaptureIdentity) {
        let app = self.app.clone();
        let capture = capture.clone();
        tauri::async_runtime::spawn(async move {
            // Decided when it runs: a later recording of the note by then is the person's own.
            tauri_plugin_transcription::stop_capture(&app, &capture).await;
        });
    }
}

pub struct TauriBrowserPort {
    app: tauri::AppHandle,
    meeting: MeetingCapture,
    epoch: Instant,
    /// The same notice dispatcher native captures use, so both share one durable claim.
    notices: Arc<MeetingNoticeObserver<tauri::Wry>>,
    starts: Arc<ExtensionStarts<TauriStartHost>>,
}

impl TauriBrowserPort {
    pub fn new(
        app: tauri::AppHandle,
        meeting: MeetingCapture,
        notices: Arc<MeetingNoticeObserver<tauri::Wry>>,
        starts: Arc<ExtensionStarts<TauriStartHost>>,
    ) -> Self {
        Self {
            app,
            meeting,
            epoch: Instant::now(),
            notices,
            starts,
        }
    }

    async fn set_paused(&self, capture: &CaptureIdentity, paused: bool) -> Result<(), PortError> {
        if !self.meeting.runtime.is_current(capture) {
            return Err(PortError::Stale);
        }
        if !self.meeting.router.can_pause(capture) {
            return Err(PortError::Unsupported);
        }
        let toggled = if paused {
            self.meeting.router.pause(capture)
        } else {
            self.meeting.router.resume(capture)
        };
        toggled.await.map_err(|error| match error {
            anlg_audio::Error::MeetingCaptureUnavailable(_) => PortError::Unsupported,
            other => PortError::Failed(other.to_string()),
        })
    }

    fn view(&self, active: runtime::ActiveCapture) -> RecorderView<CaptureIdentity> {
        RecorderView {
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
            // Only the native recorder pauses its audio and timeline; the legacy one can't.
            pausable: self.meeting.router.can_pause(&active.identity),
            capture: active.identity,
        }
    }
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

impl BrowserMeetingPort for TauriBrowserPort {
    type Capture = CaptureIdentity;

    fn account(&self) -> Option<String> {
        Some(
            self.meeting
                .runtime
                .account()
                .unwrap_or_else(|| "local".to_string()),
        )
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn active(&self) -> Option<RecorderView<CaptureIdentity>> {
        let active = self.meeting.runtime.active()?;
        self.meeting
            .runtime
            .is_current(&active.identity)
            .then(|| self.view(active))
    }

    fn capture(&self, capture: &CaptureIdentity) -> Option<RecorderView<CaptureIdentity>> {
        self.active().filter(|view| view.capture == *capture)
    }

    fn recorded_call(&self, capture: &CaptureIdentity) -> Option<RecordedCall> {
        self.meeting
            .runtime
            .confirmed_call(capture)
            .map(|call| RecordedCall {
                bundle_id: call.bundle_id,
                call_url: call.call_url,
            })
    }

    #[cfg(target_os = "macos")]
    fn bundle_id_of(&self, pid: u32) -> Option<String> {
        anlg_bundle::bundle_id_for_pid(i32::try_from(pid).ok()?)
    }

    #[cfg(not(target_os = "macos"))]
    fn bundle_id_of(&self, _pid: u32) -> Option<String> {
        None
    }

    fn link(&self, capture: &CaptureIdentity, call_url: &str) -> Result<String, PortError> {
        self.meeting
            .runtime
            .link(capture, call_url)
            .map(|token| token.0)
            .map_err(port_error)
    }

    fn resolve_link(&self, token: &str) -> Result<CaptureIdentity, PortError> {
        self.meeting
            .runtime
            .resolve_link(&LinkToken(token.to_string()))
            .map_err(port_error)
    }

    fn detach(&self, token: &str) -> Result<DetachOutcome, PortError> {
        self.meeting
            .runtime
            .detach(&LinkToken(token.to_string()))
            .map(|outcome| match outcome {
                runtime::DetachOutcome::KeepRecording => DetachOutcome::KeepRecording,
                runtime::DetachOutcome::StopRecording => DetachOutcome::StopRecording,
            })
            .map_err(port_error)
    }

    async fn start(&self, _request: ExtensionStart) -> Result<CaptureIdentity, PortError> {
        // The legacy recorder captures the whole system and never confirms a call, so the route
        // would stop its recording again; say so before a note is created. The window checks
        // again with the selected microphone.
        if self.meeting.router.capability(None).engine != CaptureEngine::NativeMeeting {
            return Err(PortError::Failed(NEEDS_CALL_RECORDER.into()));
        }
        self.starts.start().await.map_err(PortError::Failed)
    }

    async fn pause(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        self.set_paused(capture, true).await
    }

    async fn resume(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        self.set_paused(capture, false).await
    }

    async fn stop(&self, capture: &CaptureIdentity) -> Result<(), PortError> {
        if !self.meeting.runtime.is_current(capture) {
            return Err(PortError::Stale);
        }
        if tauri_plugin_transcription::stop_capture(&self.app, capture).await {
            Ok(())
        } else {
            Err(PortError::Stale)
        }
    }

    fn observe(&self, capture: &CaptureIdentity, observation: MeetObservation) {
        let Some(at) = self
            .meeting
            .runtime
            .instant_from_wall_ms(capture, observation.at_wall_ms)
        else {
            return;
        };
        let observation = SpeakerObservation {
            capture: capture.clone(),
            source: ObservationSource::BrowserExtension,
            platform: CallPlatform::GoogleMeet,
            at,
            participants: observation
                .participants
                .into_iter()
                .map(|participant| ObservedParticipant {
                    participant_id: participant.participant_id,
                    display_name: participant.display_name,
                    is_self: participant.is_self,
                })
                .collect(),
            // Meet's grid shows only visible tiles.
            roster_complete: false,
            participant_count: observation.participant_count,
            speaking: observation.speaking.map(|speakers| {
                speakers
                    .into_iter()
                    .map(|speaker| SpeakingCue {
                        participant_id: Some(speaker.participant_id),
                        display_name: speaker.display_name,
                    })
                    .collect()
            }),
        };
        if let Err(error) = self.meeting.runtime.observe(observation) {
            tracing::debug!(%error, "browser_meeting_observation_dropped");
        }
    }

    async fn claim_notice(&self, capture: &CaptureIdentity) -> Option<String> {
        self.notices
            .claim_extension_notice(capture, self.meeting.runtime.as_ref())
            .await
    }

    fn notice_outcome(&self, capture: &CaptureIdentity, outcome: ChatOutcome) {
        let settled = match outcome.kind() {
            NoticeKind::Sent => NoticeOutcome::Sent,
            NoticeKind::NotSent => NoticeOutcome::NotSent,
            NoticeKind::CancelledBeforeDispatch => NoticeOutcome::CancelledBeforeDispatch,
            NoticeKind::Unconfirmed => NoticeOutcome::Unconfirmed,
        };
        let notices = self.notices.clone();
        let capture = capture.clone();
        tauri::async_runtime::spawn(async move {
            notices
                .settle_extension_notice(&capture, settled, "browser_extension")
                .await;
        });
    }

    async fn claim_camera(&self, capture: &CaptureIdentity) -> Option<String> {
        let browser = self.recorded_call(capture)?.bundle_id;
        self.app
            .camera_watermark()
            .camera_for_extension(&browser)
            .map(str::to_string)
    }

    fn camera_outcome(&self, capture: &CaptureIdentity, outcome: CameraOutcome) {
        let outcome = match outcome {
            CameraOutcome::Switched => MeetCameraOutcome::Switched,
            CameraOutcome::AlreadyUsing => MeetCameraOutcome::AlreadyUsing,
            CameraOutcome::AlreadyTried => MeetCameraOutcome::AlreadyTried,
            CameraOutcome::CameraMissing => MeetCameraOutcome::CameraMissing,
            CameraOutcome::PickerUnavailable => MeetCameraOutcome::PickerUnavailable,
            CameraOutcome::Unconfirmed => MeetCameraOutcome::Unconfirmed,
            CameraOutcome::NotInCall => MeetCameraOutcome::NotInCall,
            CameraOutcome::Cancelled => MeetCameraOutcome::Cancelled,
        };
        self.app
            .camera_watermark()
            .meet_camera_outcome(capture, outcome);
    }
}

/// The native-messaging host name for this build's channel, so channels never share an extension.
/// The App Store build (`com.hyprnote.desktop`) and unknown identifiers have none: the sandbox
/// can't serve the bridge. Matches `prepare-meeting-helpers.mjs`'s channels.
pub fn host_name(identifier: &str) -> Option<&'static str> {
    match identifier {
        "com.hyprnote.stable" => Some("so.anarlog.browser_meeting"),
        "com.hyprnote.nightly" => Some("so.anarlog.nightly.browser_meeting"),
        "com.hyprnote.staging" => Some("so.anarlog.staging.browser_meeting"),
        "com.hyprnote.dev" => Some("so.anarlog.dev.browser_meeting"),
        _ => None,
    }
}

/// What packaging writes beside the bundled extension: this channel's pinned extension id, derived
/// from its public key, and native host name.
pub const PACKAGED_CONFIG: &str = "browser-meeting-extension/native-host.json";
const PACKAGED_CONFIG_VERSION: u32 = 1;
/// Qualification in debug builds only; packaged builds read [`PACKAGED_CONFIG`].
const DEV_EXTENSION_ID_ENV: &str = "ANARLOG_BROWSER_MEETING_EXTENSION_ID";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackagedConfig {
    version: u32,
    extension_id: String,
    native_host: String,
}

/// A Chrome extension id: 32 letters `a`–`p` (a hex SHA-256 prefix shifted into letters).
pub fn is_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

/// The extension id this channel's packaged config pins. Fails closed: an absent, unreadable or
/// newer config, another channel's host, or a malformed id turns the bridge off.
pub fn packaged_extension_id(path: &Path, identifier: &str) -> Result<String, String> {
    let host = host_name(identifier).ok_or("this build has no Meet bridge")?;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("no packaged Meet extension config: {error}"))?;
    let config: PackagedConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid packaged Meet extension config: {error}"))?;
    if config.version != PACKAGED_CONFIG_VERSION {
        return Err(format!(
            "unsupported packaged Meet extension config version {}",
            config.version
        ));
    }
    if config.native_host != host {
        return Err(format!(
            "the packaged Meet extension is for {}, not {host}",
            config.native_host
        ));
    }
    if !is_extension_id(&config.extension_id) {
        return Err("the packaged Meet extension id is not a Chrome extension id".into());
    }
    Ok(config.extension_id)
}

#[cfg(target_os = "macos")]
const BROWSER_FOLDERS: &[&str] = &[
    "Google/Chrome",
    "Google/Chrome Beta",
    "Google/Chrome Dev",
    "Google/Chrome Canary",
    "Chromium",
    "BraveSoftware/Brave-Browser",
    "Microsoft Edge",
    "Arc/User Data",
];
#[cfg(target_os = "linux")]
const BROWSER_FOLDERS: &[&str] = &[
    "google-chrome",
    "chromium",
    "BraveSoftware/Brave-Browser",
    "microsoft-edge",
];
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const BROWSER_FOLDERS: &[&str] = &[];

/// Writes the host manifest into each installed browser's folder under `base`; browsers that are
/// not installed get nothing. Returns the manifests written.
pub fn register_host(
    base: &Path,
    host_name: &str,
    host_path: &Path,
    extension_id: &str,
) -> std::io::Result<Vec<PathBuf>> {
    let manifest = serde_json::json!({
        "name": host_name,
        "description": "Anarlog Google Meet bridge",
        "path": host_path,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{extension_id}/")],
    });
    let mut written = Vec::new();
    for folder in BROWSER_FOLDERS {
        let browser = base.join(folder);
        if !browser.is_dir() {
            continue;
        }
        let hosts = browser.join("NativeMessagingHosts");
        std::fs::create_dir_all(&hosts)?;
        let path = hosts.join(format!("{host_name}.json"));
        std::fs::write(&path, serde_json::to_vec_pretty(&manifest)?)?;
        written.push(path);
    }
    Ok(written)
}

pub fn unregister_host(base: &Path, host_name: &str) -> std::io::Result<()> {
    for folder in BROWSER_FOLDERS {
        let path = base
            .join(folder)
            .join("NativeMessagingHosts")
            .join(format!("{host_name}.json"));
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

fn browsers_base() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::data_dir()
    } else {
        dirs::config_dir()
    }
}

/// One enable of the Meet bridge: its endpoint and the route its connections share.
#[cfg(unix)]
struct Serving {
    endpoint: anlg_transcription_core::browser_meeting::Endpoint,
    route: Arc<anlg_transcription_core::browser_meeting::BrowserMeetingRoute<TauriBrowserPort>>,
}

#[cfg(unix)]
impl Serving {
    /// Closes the connections that are open, each letting go of its tab by owner while the route
    /// still answers, then retires the route: it lets go of links nothing let go of in time and
    /// refuses everything after. Only this enable's own links are touched, so a newer enable's tab
    /// keeps its recording.
    fn retire(self) {
        tauri::async_runtime::spawn(async move {
            if !self.endpoint.close().await {
                tracing::warn!("browser_meeting_connections_slow_to_close");
            }
            self.route.retire().await;
        });
    }
}

/// The Meet bridge, on only by explicit opt-in. Turning it off closes connections that are already
/// open before its route stops answering, so each lets go of its tab by owner: an extension-owned
/// recording stops, an app-owned one keeps going (unpaused), and the tab is told.
pub struct MeetingBrowserBridge {
    app: tauri::AppHandle,
    meeting: MeetingCapture,
    notices: Arc<MeetingNoticeObserver<tauri::Wry>>,
    starts: Arc<ExtensionStarts<TauriStartHost>>,
    #[cfg(unix)]
    serving: std::sync::Mutex<Option<Serving>>,
}

impl MeetingBrowserBridge {
    pub fn new(
        app: tauri::AppHandle,
        meeting: MeetingCapture,
        notices: Arc<MeetingNoticeObserver<tauri::Wry>>,
    ) -> Self {
        let starts = ExtensionStarts::new(TauriStartHost {
            app: app.clone(),
            meeting: meeting.clone(),
        });
        meeting.runtime.register_observer(starts.clone());
        let replies = starts.clone();
        app.listen(START_REPLY_EVENT, move |event| {
            match serde_json::from_str::<StartReply>(event.payload()) {
                Ok(reply) => replies.reply(reply),
                Err(error) => tracing::warn!(%error, "browser_meeting_start_reply_invalid"),
            }
        });
        Self {
            app,
            meeting,
            notices,
            starts,
            #[cfg(unix)]
            serving: std::sync::Mutex::new(None),
        }
    }

    /// The extension this build may serve: the packaged config, or an explicit id in a debug build.
    fn extension_id(&self) -> Result<String, String> {
        if cfg!(feature = "app-store") {
            return Err("the App Store build can't serve the Meet bridge".into());
        }
        let identifier = &self.app.config().identifier;
        if cfg!(debug_assertions)
            && let Ok(id) = std::env::var(DEV_EXTENSION_ID_ENV)
        {
            host_name(identifier).ok_or("this build has no Meet bridge")?;
            return is_extension_id(&id)
                .then_some(id)
                .ok_or_else(|| format!("{DEV_EXTENSION_ID_ENV} is not a Chrome extension id"));
        }
        let config = self
            .app
            .path()
            .resource_dir()
            .map_err(|error| error.to_string())?
            .join(PACKAGED_CONFIG);
        packaged_extension_id(&config, identifier)
    }

    #[cfg(unix)]
    fn serving(&self) -> std::sync::MutexGuard<'_, Option<Serving>> {
        self.serving
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Serves the bridge for this channel's packaged extension. `Err` says why it stays off; the
    /// caller then calls [`Self::disable`].
    #[cfg(unix)]
    pub fn enable(&self) -> Result<(), String> {
        use anlg_transcription_core::browser_meeting::{BrowserMeetingRoute, ENDPOINT_DIR_NAME};

        let extension_id = self.extension_id()?;
        let identifier = self.app.config().identifier.clone();
        let host = host_name(&identifier).ok_or("this build has no Meet bridge")?;
        let data_dir = dirs::data_dir().ok_or("no data directory")?;
        let host_path = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .with_file_name("char-chrome-native-host");
        let route = Arc::new(BrowserMeetingRoute::new(TauriBrowserPort::new(
            self.app.clone(),
            self.meeting.clone(),
            self.notices.clone(),
            self.starts.clone(),
        )));
        let dir = data_dir.join(ENDPOINT_DIR_NAME);
        let origins = vec![format!("chrome-extension://{extension_id}/")];
        let endpoint = tauri::async_runtime::block_on({
            let route = route.clone();
            async move {
                anlg_transcription_core::browser_meeting::serve(&dir, &identifier, origins, route)
            }
        })
        .map_err(|error| error.to_string())?;
        if let Some(previous) = self.serving().replace(Serving { endpoint, route }) {
            previous.retire();
        }
        let base = browsers_base().ok_or("no browser folder")?;
        register_host(&base, host, &host_path, &extension_id).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn enable(&self) -> Result<(), String> {
        Err("the Meet bridge isn't available on this system yet".into())
    }

    pub fn disable(&self) {
        // Browsers can't launch this channel's host anymore. A build without a host name (the
        // sandboxed App Store build) never wrote one.
        if let (Some(base), Some(host)) =
            (browsers_base(), host_name(&self.app.config().identifier))
            && let Err(error) = unregister_host(&base, host)
        {
            tracing::warn!(%error, "browser_meeting_host_unregister_failed");
        }
        #[cfg(unix)]
        if let Some(serving) = self.serving().take() {
            serving.retire();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packaged(json: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native-host.json");
        std::fs::write(&path, json).unwrap();
        (dir, path)
    }

    const ID: &str = "abcdefghijklmnopabcdefghijklmnop";

    #[test]
    fn only_this_channels_packaged_extension_turns_the_bridge_on() {
        let config = |host: &str, id: &str| {
            format!(r#"{{"version":1,"extensionId":"{id}","nativeHost":"{host}"}}"#)
        };
        let (_dir, path) = packaged(&config("so.anarlog.nightly.browser_meeting", ID));
        assert_eq!(
            packaged_extension_id(&path, "com.hyprnote.nightly"),
            Ok(ID.to_string())
        );
        // Another channel's config, the App Store build, and unknown builds fail closed.
        for identifier in [
            "com.hyprnote.stable",
            "com.hyprnote.desktop",
            "so.anarlog.Anarlog",
        ] {
            assert!(
                packaged_extension_id(&path, identifier).is_err(),
                "{identifier}"
            );
        }
        assert!(
            packaged_extension_id(&path.with_file_name("absent.json"), "com.hyprnote.nightly")
                .is_err()
        );
        for broken in [
            config("so.anarlog.nightly.browser_meeting", "abcdefghijklmnopabcdefghijklmnoq"),
            config("so.anarlog.nightly.browser_meeting", &ID.to_uppercase()),
            config("so.anarlog.nightly.browser_meeting", &ID[..31]),
            config("so.anarlog.nightly.browser_meeting", &format!("{ID}a")),
            config("so.anarlog.nightly.browser_meeting", ""),
            r#"{"version":2,"extensionId":"abcdefghijklmnopabcdefghijklmnop","nativeHost":"so.anarlog.nightly.browser_meeting"}"#.into(),
            r#"{"version":1,"extensionId":"abcdefghijklmnopabcdefghijklmnop","nativeHost":"so.anarlog.nightly.browser_meeting","key":"x"}"#.into(),
            r#"{"version":1,"nativeHost":"so.anarlog.nightly.browser_meeting"}"#.into(),
            "not json".into(),
        ] {
            let (_dir, path) = packaged(&broken);
            assert!(
                packaged_extension_id(&path, "com.hyprnote.nightly").is_err(),
                "{broken}"
            );
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn host_manifests_go_only_to_installed_browsers_and_come_back_out() {
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(base.path().join(BROWSER_FOLDERS[0])).unwrap();
        let written = register_host(
            base.path(),
            "so.anarlog.dev.browser_meeting",
            Path::new("/Applications/Anarlog.app/Contents/MacOS/char-chrome-native-host"),
            "abcdefghijklmnop",
        )
        .unwrap();
        assert_eq!(written.len(), 1);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&written[0]).unwrap()).unwrap();
        assert_eq!(
            manifest["allowed_origins"],
            serde_json::json!(["chrome-extension://abcdefghijklmnop/"])
        );
        assert!(!base.path().join(BROWSER_FOLDERS[1]).exists());

        unregister_host(base.path(), "so.anarlog.dev.browser_meeting").unwrap();
        assert!(!written[0].exists());
    }
}
