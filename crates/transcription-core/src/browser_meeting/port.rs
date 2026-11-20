use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;

use super::cues::MeetObservation;
use super::outcome::{CameraOutcome, ChatOutcome};

/// Who started a recording. An extension recording ends with its tab; an app recording only lets go
/// of the tab, and only the person's own Stop ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    App,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderStatus {
    /// Started but not yet confirmed or capturing.
    Starting,
    Recording,
    Paused,
    /// Stopped and finishing its audio; the tab hears `ended` once it is gone.
    Finalizing,
    Failed {
        message: String,
    },
}

/// The recorder's view of one capture. `capture` is the shared capture identity (account, session
/// and a fresh capture id), never a session id alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderView<C> {
    pub capture: C,
    pub session_id: String,
    pub owner: Owner,
    pub status: RecorderStatus,
    /// Whether this recorder can pause; the extension offers pause only then.
    pub pausable: bool,
}

/// The call the recorder confirmed for a capture, from the same native observation that chose its
/// audio scope (`CaptureAuthority::confirmed_call`): the app holding it, by bundle id, and the
/// address its window showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    pub bundle_id: String,
    pub call_url: Option<String>,
}

/// A recording the extension asks for, on the Meet call in the tab the person clicked. This is a
/// request, not a confirmation: only the recorder's own observation confirms the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionStart {
    /// Canonical, from [`super::meet_call_of`].
    pub call_url: String,
    /// The browser the native host runs under; its audio is process-scoped, not per tab.
    pub browser_pid: Option<u32>,
    pub browser_bundle_id: Option<String>,
    pub account_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachOutcome {
    KeepRecording,
    StopRecording,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    /// The link or capture is no longer current (replaced, ended, or another account).
    Stale,
    /// The confirmed call is missing or isn't this one.
    CallMismatch,
    /// This recorder can't do it (pause on a backend without pause).
    Unsupported,
    Failed(String),
}

/// Everything the browser route needs from the app: the one meeting runtime (capture identity, link
/// tokens, observations, notice claims) and the one recorder path (the record button's start, pause,
/// resume and stop with the person's selected providers). Implemented once by integration; there is
/// no second recorder or link registry here.
pub trait BrowserMeetingPort: Send + Sync + 'static {
    type Capture: Clone + Eq + Hash + Debug + Send + Sync + 'static;

    /// The signed-in account; nothing records without one.
    fn account(&self) -> Option<String>;
    /// Monotonic milliseconds, for link freshness.
    fn now_ms(&self) -> u64;

    /// The capture recording or starting now, if any (one at a time).
    fn active(&self) -> Option<RecorderView<Self::Capture>>;
    /// Exactly this capture while it is current; `None` once ended, stale or another account's.
    fn capture(&self, capture: &Self::Capture) -> Option<RecorderView<Self::Capture>>;
    fn recorded_call(&self, capture: &Self::Capture) -> Option<RecordedCall>;
    /// The macOS bundle id of a running process, to name the browser a tab asks from.
    fn bundle_id_of(&self, pid: u32) -> Option<String>;

    /// A new link token for this capture on this canonical call, replacing any earlier one. Only
    /// the confirmed call's own address links (`MeetingRuntime::link`).
    fn link(&self, capture: &Self::Capture, call_url: &str) -> Result<String, PortError>;
    /// The capture a token links, or `Stale` once replaced, detached or ended.
    fn resolve_link(&self, token: &str) -> Result<Self::Capture, PortError>;
    /// Ends the token. Decides only: the route stops through [`Self::stop`] itself.
    fn detach(&self, token: &str) -> Result<DetachOutcome, PortError>;

    /// Starts a new session the record button's way, with the selected providers, as an
    /// extension-owned capture. Resolves once the recorder confirmed its scope (or failed).
    /// Dropping the future first cancels the start: a capture it already began, or begins later,
    /// stops through the record path rather than recording for a tab that is gone.
    fn start(
        &self,
        request: ExtensionStart,
    ) -> impl Future<Output = Result<Self::Capture, PortError>> + Send;
    /// Pauses the recorder itself (its audio and timeline), resolving once it did; `Unsupported`
    /// where it can't, which [`RecorderView::pausable`] already says.
    fn pause(&self, capture: &Self::Capture) -> impl Future<Output = Result<(), PortError>> + Send;
    fn resume(&self, capture: &Self::Capture)
    -> impl Future<Output = Result<(), PortError>> + Send;
    fn stop(&self, capture: &Self::Capture) -> impl Future<Output = Result<(), PortError>> + Send;

    /// What Meet showed about who is here and who is speaking, for the speakers lane.
    fn observe(&self, capture: &Self::Capture, observation: MeetObservation);

    /// Claims this capture's chat notice for the extension before it is handed out, durably and
    /// account-scoped. `None` when the setting is off, it was claimed already, or storage failed.
    fn claim_notice(&self, capture: &Self::Capture) -> impl Future<Output = Option<String>> + Send;
    /// Settles the claim: only a definitely-unsent outcome releases it.
    fn notice_outcome(&self, capture: &Self::Capture, outcome: ChatOutcome);
    /// The camera name to pick in Meet's settings, when automatic switching is on for this capture.
    fn claim_camera(&self, capture: &Self::Capture) -> impl Future<Output = Option<String>> + Send;
    fn camera_outcome(&self, capture: &Self::Capture, outcome: CameraOutcome);
}
