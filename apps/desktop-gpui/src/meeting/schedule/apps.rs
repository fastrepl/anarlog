//! Meeting apps as scheduling and capture see them: which apps hold the microphone, what
//! their windows show through accessibility, their chat and shared screen. The real adapter
//! reads `crates/detect` on one dedicated thread, like shipping's `plugins/detect` AX worker;
//! tests and the screenshot harness never install it.

use std::sync::{Arc, Mutex, mpsc};

use futures::{
    FutureExt,
    channel::oneshot,
    future::{BoxFuture, ready},
};
use gpui::{App, Global};

pub use anlg_detect::{
    MeetingAccessibilityInspection as Inspection, MeetingApp, MeetingCapturedChatMessage,
    MeetingChatCaptureResult as ChatCapture, MeetingChatDirection,
    MeetingChatSendResult as ChatSend, MeetingPlatform, MeetingSurface,
};

pub use crate::meeting::automation::apps::MicApp;

/// One `captureMeetingScreenShare` sample: a JPEG only for a new, settled frame.
#[derive(Clone, Debug, Default)]
pub struct ScreenShare {
    pub platform: Option<MeetingPlatform>,
    pub jpeg: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

pub type Reply<T> = BoxFuture<'static, Result<T, String>>;

pub trait MeetingApps: Send + Sync {
    fn mic_apps(&self) -> Reply<Vec<MicApp>>;
    fn inspect(&self) -> Reply<Vec<Inspection>>;
    /// Posts into the meeting chat of an app that still holds the microphone.
    fn send_chat(&self, message: String, mic_app_ids: Vec<String>) -> Reply<ChatSend>;
    fn capture_chat(&self) -> Reply<ChatCapture>;
    fn capture_screen(&self, reset: bool) -> Reply<ScreenShare>;
    fn screen_recording_denied(&self) -> bool;
    fn open_url(&self, url: &str) -> Result<(), String>;
}

#[derive(Clone)]
pub struct InstalledMeetingApps(pub Arc<dyn MeetingApps>);

impl Global for InstalledMeetingApps {}

pub fn installed(cx: &App) -> Option<Arc<dyn MeetingApps>> {
    cx.try_global::<InstalledMeetingApps>()
        .map(|installed| installed.0.clone())
}

/// Browsers, where a stopped microphone may only be a closed tab (`BROWSER_AUTO_STOP_APP_IDS`).
pub const BROWSER_APP_IDS: &[&str] = &[
    "at.studio.AsideBrowser",
    "app.zen-browser.zen",
    "ai.perplexity.comet",
    "com.apple.Safari",
    "com.apple.SafariTechnologyPreview",
    "com.browseros.BrowserOS",
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.brave.Browser.nightly",
    "com.brave.Browser.origin",
    "com.duckduckgo.macos.browser",
    "chrome",
    "comet",
    "chromium",
    "chromium-browser",
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "firefox",
    "google-chrome",
    "google-chrome-beta",
    "google-chrome-stable",
    "microsoft-edge",
    "msedge",
    "helium",
    "com.kagi.kagimacOS",
    "com.kagi.kagimacOS.RC",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.edgemac.Canary",
    "com.microsoft.edgemac.Dev",
    "com.operasoftware.Opera",
    "com.operasoftware.OperaDeveloper",
    "com.operasoftware.OperaGX",
    "com.operasoftware.OperaNext",
    "opera",
    "com.nousresearch.hermes",
    "com.sigmaos.sigmaos.macos",
    "com.vivaldi.Vivaldi",
    "vivaldi",
    "company.thebrowser.Browser",
    "company.thebrowser.dia",
    "net.imput.helium",
    "net.mullvad.mullvadbrowser",
    "net.waterfox.waterfox",
    "org.chromium.Chromium",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "org.mozilla.librewolf",
    "org.mozilla.nightly",
    "org.torproject.torbrowser",
    "zen",
];

/// Calling apps from shipping's `MIC_APP_NOTIFICATION_OVERRIDES`: (bundle ids, names).
const CALL_APPS: &[(&[&str], &[&str])] = &[
    (
        &[
            "/usr/libexec/avconferenced",
            "com.apple.avconferenced",
            "com.apple.TelephonyUtilities",
            "com.apple.TelephonyUtilities.callservicesd",
        ],
        &["av capture", "avcapture", "avconferenced", "iphone call"],
    ),
    (&["com.apple.FaceTime"], &["facetime"]),
    (
        &["us.zoom.xos", "zoom", "Zoom.exe"],
        &["zoom", "zoom helper", "zoom workplace"],
    ),
    (
        &[
            "com.microsoft.teams",
            "com.microsoft.teams2",
            "ms-teams",
            "teams",
            "teams-for-linux",
        ],
        &[
            "microsoft teams",
            "microsoft teams helper",
            "teams",
            "teams helper",
        ],
    ),
    (
        &[
            "Cisco-Systems.Spark",
            "com.cisco.webex",
            "com.cisco.webexmeetingsapp",
        ],
        &["cisco webex", "webex", "webex helper", "webex meetings"],
    ),
    (
        &[
            "com.slack.Slack",
            "com.tinyspeck.slackmacgap",
            "slack",
            "slack-desktop",
            "Slack.exe",
        ],
        &["slack", "slack helper"],
    ),
    (
        &["com.kakao.KakaoTalkMac"],
        &["kakaotalk", "kakaotalk helper"],
    ),
    (&["net.whatsapp.WhatsApp"], &["whatsapp", "whatsapp helper"]),
    (
        &["com.hnc.Discord", "com.discordapp.Discord"],
        &["discord", "discord helper"],
    ),
    (
        &["org.whispersystems.signal-desktop"],
        &["signal", "signal helper"],
    ),
    (
        &["ru.keepcoder.Telegram", "ru.keepcoder.TelegramLite"],
        &["telegram", "telegram helper", "telegram lite"],
    ),
    (&["jp.naver.line.mac"], &["line", "line helper"]),
    (&["com.facebook.archon"], &["messenger", "messenger helper"]),
];

pub fn is_browser(id: &str) -> bool {
    BROWSER_APP_IDS.contains(&id)
}

/// Shipping's `isMeetingCapableMicApp`: a browser or a known calling app.
pub fn is_meeting_capable(app: &MicApp) -> bool {
    let name = app.name.trim().to_lowercase();
    is_browser(&app.id)
        || CALL_APPS
            .iter()
            .any(|(ids, names)| names.contains(&name.as_str()) || ids.contains(&app.id.as_str()))
}

type Job = Box<dyn FnOnce() + Send>;

/// `crates/detect` behind one dedicated thread: accessibility IPC is synchronous and can
/// stall for minutes on large browser trees, so walks never run concurrently or on the UI.
pub struct DetectMeetingApps {
    jobs: Mutex<mpsc::Sender<Job>>,
    sampler: Arc<Mutex<anlg_detect::ScreenShareSampler>>,
}

impl DetectMeetingApps {
    pub fn spawn() -> std::io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("meeting-ax".into())
            .spawn(move || {
                for job in queue {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                }
            })?;
        Ok(Self {
            jobs: Mutex::new(jobs),
            sampler: Arc::default(),
        })
    }

    fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Reply<T> {
        let (send, receive) = oneshot::channel();
        let queued = self
            .jobs
            .lock()
            .map_err(|_| "meeting accessibility worker stopped".to_owned())
            .and_then(|jobs| {
                jobs.send(Box::new(move || {
                    let _ = send.send(job());
                }))
                .map_err(|_| "meeting accessibility worker stopped".to_owned())
            });
        match queued {
            Ok(()) => receive
                .map(|result| {
                    result.unwrap_or_else(|_| Err("meeting accessibility worker stopped".into()))
                })
                .boxed(),
            Err(error) => ready(Err(error)).boxed(),
        }
    }
}

fn mic_apps() -> Result<Vec<MicApp>, String> {
    anlg_detect::list_mic_using_apps()
        .map(|apps| {
            apps.into_iter()
                .map(|app| MicApp::new(app.id, app.name))
                .collect()
        })
        .map_err(|error| error.to_string())
}

impl MeetingApps for DetectMeetingApps {
    fn mic_apps(&self) -> Reply<Vec<MicApp>> {
        ready(mic_apps()).boxed()
    }

    fn inspect(&self) -> Reply<Vec<Inspection>> {
        self.run(|| Ok(anlg_detect::inspect_meeting_accessibility()))
    }

    fn send_chat(&self, message: String, mic_app_ids: Vec<String>) -> Reply<ChatSend> {
        // Only apps still on the microphone may receive the message (`intersect_mic_active_bundle_ids`).
        self.run(move || {
            let current = mic_apps()?;
            let mut verified: Vec<String> = current
                .into_iter()
                .map(|app| app.id.trim().to_owned())
                .filter(|id| mic_app_ids.iter().any(|requested| requested.trim() == id))
                .collect();
            verified.sort();
            verified.dedup();
            Ok(anlg_detect::send_meeting_chat_message(message, verified))
        })
    }

    fn capture_chat(&self) -> Reply<ChatCapture> {
        self.run(|| {
            let ids = mic_apps()?.into_iter().map(|app| app.id).collect();
            Ok(anlg_detect::capture_meeting_chat_messages(ids))
        })
    }

    fn capture_screen(&self, reset: bool) -> Reply<ScreenShare> {
        let sampler = self.sampler.clone();
        self.run(move || {
            let mut sampler = sampler.lock().map_err(|error| error.to_string())?;
            if reset {
                *sampler = anlg_detect::ScreenShareSampler::default();
            }
            let Some(inspection) = sampler.follow(&anlg_detect::inspect_meeting_accessibility())
            else {
                sampler.pause();
                return Ok(ScreenShare::default());
            };
            let image = anlg_detect::capture_meeting_window(
                inspection.pid,
                inspection.window_title.as_deref(),
            )
            .inspect_err(|_| {
                sampler.unfollow();
                sampler.pause();
            })?;
            if !sampler.observe(anlg_detect::FrameSignature::of(&image)) {
                return Ok(ScreenShare {
                    platform: Some(inspection.platform),
                    ..Default::default()
                });
            }
            let (jpeg, width, height) = anlg_detect::encode_capture_jpeg(&image)?;
            Ok(ScreenShare {
                platform: Some(inspection.platform),
                jpeg: Some(jpeg),
                width,
                height,
            })
        })
    }

    fn screen_recording_denied(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            #[link(name = "CoreGraphics", kind = "framework")]
            unsafe extern "C" {
                fn CGPreflightScreenCaptureAccess() -> bool;
            }
            // SAFETY: a side-effect-free TCC preflight with no arguments.
            !unsafe { CGPreflightScreenCaptureAccess() }
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        crate::platform::os::open(url).map_err(|error| error.to_string())
    }
}
