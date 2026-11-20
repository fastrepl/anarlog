//! Shared contract between the native meeting capture engine, the single meeting runtime that owns
//! a recording's lifecycle, and the features that act on a confirmed call (speaker labels, chat
//! notices, camera). Audio itself keeps flowing through `CaptureStream`; everything here is the
//! context that must never be attributed to the wrong account, session, or recording.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::Stream;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::{CaptureConfig, CaptureStream, Error};

pub const NATIVE_SAMPLE_RATE: u32 = 48_000;

const FRAMES_PER_MS: f64 = NATIVE_SAMPLE_RATE as f64 / 1_000.0;

/// A session can be recorded many times, so the session id alone cannot scope evidence or side
/// effects. `capture_id` is fresh for every start and is never reused or restored.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CaptureIdentity {
    pub account_id: String,
    pub session_id: String,
    pub capture_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallPlatform {
    GoogleMeet,
    Zoom,
    MicrosoftTeams,
    Webex,
}

impl CallPlatform {
    pub fn from_helper(name: &str) -> Option<Self> {
        match name {
            "google-meet" => Some(Self::GoogleMeet),
            "zoom" => Some(Self::Zoom),
            "teams" => Some(Self::MicrosoftTeams),
            "webex" => Some(Self::Webex),
            _ => None,
        }
    }

    pub fn helper_name(self) -> &'static str {
        match self {
            Self::GoogleMeet => "google-meet",
            Self::Zoom => "zoom",
            Self::MicrosoftTeams => "teams",
            Self::Webex => "webex",
        }
    }
}

/// One call, confirmed by the same native observation that chose what the recording taps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmedCall {
    pub pid: i32,
    pub bundle_id: String,
    pub platform: CallPlatform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureScope {
    Call(ConfirmedCall),
    /// No single call was confirmed: the microphone plus the whole system output.
    System,
}

impl CaptureScope {
    pub fn call(&self) -> Option<&ConfirmedCall> {
        match self {
            Self::Call(call) => Some(call),
            Self::System => None,
        }
    }
}

/// `Discover` is only valid for the first open of a capture. Re-opening the same capture must ask
/// for exactly what it had, so a failure never broadens an app-scoped recording to the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeRequest {
    Discover,
    Exactly(CaptureScope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEngine {
    Legacy,
    NativeMeeting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicProcessing {
    Raw,
    /// Already echo-cancelled by the OS: never presented as raw input or cancelled again.
    VoiceProcessed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureProvenance {
    pub engine: CaptureEngine,
    pub source_sample_rate: u32,
    pub mic: MicProcessing,
}

impl CaptureProvenance {
    pub fn legacy(sample_rate: u32) -> Self {
        Self {
            engine: CaptureEngine::Legacy,
            source_sample_rate: sample_rate,
            mic: MicProcessing::Raw,
        }
    }

    /// Whether Anarlog's own echo cancellation may run on this microphone.
    pub fn allows_aec(&self) -> bool {
        self.mic == MicProcessing::Raw
    }
}

/// Capture-relative monotonic time in `NATIVE_SAMPLE_RATE` frames since the capture's first frame.
/// Pauses and gaps advance it. A transcript offset is the session's audio offset plus `as_millis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CaptureInstant(pub u64);

impl CaptureInstant {
    pub fn from_millis(ms: f64) -> Option<Self> {
        (ms.is_finite() && ms >= 0.0).then(|| Self((ms * FRAMES_PER_MS).round() as u64))
    }

    pub fn as_millis(self) -> f64 {
        self.0 as f64 / FRAMES_PER_MS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimelineError {
    #[error("empty or invalid audio span")]
    InvalidSpan,
    #[error("capture audio went backwards")]
    Backwards,
    #[error("time is outside the delivered audio")]
    OutOfRange,
}

#[derive(Debug, Clone, Copy)]
struct Span {
    delivered: u64,
    capture: u64,
    frames: u64,
}

/// Maps the contiguous audio delivered to a consumer back onto capture time. The holes between
/// spans are pauses or gaps: evidence on either side of one must never be joined.
#[derive(Debug, Clone, Default)]
pub struct CaptureTimeline {
    spans: Vec<Span>,
    delivered: u64,
}

impl CaptureTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, capture_frame: u64, frames: u64) -> Result<(), TimelineError> {
        if frames == 0 {
            return Err(TimelineError::InvalidSpan);
        }
        match self.spans.last_mut() {
            Some(last) if capture_frame < last.capture + last.frames => {
                return Err(TimelineError::Backwards);
            }
            Some(last) if capture_frame == last.capture + last.frames => last.frames += frames,
            _ => self.spans.push(Span {
                delivered: self.delivered,
                capture: capture_frame,
                frames,
            }),
        }
        self.delivered += frames;
        Ok(())
    }

    pub fn delivered_frames(&self) -> u64 {
        self.delivered
    }

    /// At a hole, an end time belongs before it and a start time after it.
    pub fn capture_ms(&self, stream_ms: f64, end: bool) -> Result<f64, TimelineError> {
        let frame = stream_ms * FRAMES_PER_MS;
        if !frame.is_finite() || frame < 0.0 || frame > (self.delivered as f64 + FRAMES_PER_MS) {
            return Err(TimelineError::OutOfRange);
        }
        let span = self
            .spans
            .iter()
            .find(|span| {
                let span_end = (span.delivered + span.frames) as f64;
                if end {
                    frame <= span_end
                } else {
                    frame < span_end
                }
            })
            .or(self.spans.last())
            .ok_or(TimelineError::OutOfRange)?;
        let within = (frame - span.delivered as f64).clamp(0.0, span.frames as f64);
        Ok((span.capture as f64 + within) / FRAMES_PER_MS)
    }

    pub fn span_of(&self, at: CaptureInstant) -> Option<usize> {
        self.spans
            .iter()
            .position(|span| span.capture <= at.0 && at.0 < span.capture + span.frames)
    }

    pub fn holes(&self) -> Vec<(CaptureInstant, CaptureInstant)> {
        let mut holes = Vec::new();
        let mut cursor = 0;
        for span in &self.spans {
            if span.capture > cursor {
                holes.push((CaptureInstant(cursor), CaptureInstant(span.capture)));
            }
            cursor = span.capture + span.frames;
        }
        holes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    Accessibility,
    BrowserExtension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedParticipant {
    pub participant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// `None` when the meeting UI does not say whether this row is the local user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_self: Option<bool>,
}

/// An explicit "is speaking" signal. A name that merely appears in a roster is not one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeakingCue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeakerObservation {
    pub capture: CaptureIdentity,
    pub source: ObservationSource,
    pub platform: CallPlatform,
    pub at: CaptureInstant,
    /// Visible rows only; never assumed to be the whole membership.
    pub participants: Vec<ObservedParticipant>,
    /// True only for a roster shown to be complete, which alone may support 1:1 labeling.
    pub roster_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant_count: Option<u32>,
    /// `None`: this source gives no speaking signal. `Some(vec![])`: it says nobody is speaking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking: Option<Vec<SpeakingCue>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEndReason {
    UserStopped,
    AppClosed,
    CaptureFailed,
    AccountChanged,
    /// The helper reached its maximum recording length.
    DurationLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Microphone,
    Remote,
}

/// Device errors, missing buffers, and real silence are different failures and stay distinct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TrackStatus {
    Ready,
    NoBuffers,
    Silent,
    Interrupted,
    Recovered,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeetingCaptureEvent {
    Observation(SpeakerObservation),
    ObservationUnavailable {
        at: CaptureInstant,
        reason: String,
    },
    Paused {
        at: CaptureInstant,
    },
    Resumed {
        at: CaptureInstant,
    },
    TrackStatus {
        track: TrackKind,
        status: TrackStatus,
    },
    /// The OS switched the microphone between voice processing and raw input (headphones on or
    /// off). Frames carry the current mode through `aec_mic`; this records when it changed.
    MicProcessingChanged {
        at: CaptureInstant,
        mic: MicProcessing,
    },
    Ended {
        at: CaptureInstant,
        reason: CaptureEndReason,
    },
}

pub type MeetingEventStream = Pin<Box<dyn Stream<Item = MeetingCaptureEvent> + Send>>;

pub trait MeetingCaptureControl: Send + Sync {
    fn pause(&self) -> BoxFuture<'static, Result<(), Error>>;
    fn resume(&self) -> BoxFuture<'static, Result<(), Error>>;
    /// Resolves once both tracks are finished.
    fn stop(&self) -> BoxFuture<'static, Result<(), Error>>;
    /// Only called after Anarlog durably adopted the audio; removes the raw staging files.
    fn release_staging(&self) -> Result<(), Error>;
}

pub struct MeetingCaptureRequest {
    pub capture: CaptureIdentity,
    pub scope: ScopeRequest,
    /// New, empty, and owned by the runtime's cleanup. Engines never choose their own paths.
    pub staging_dir: PathBuf,
    /// `sample_rate` and `chunk_size` describe Anarlog's live-consumer boundary.
    pub config: CaptureConfig,
    pub locale: Option<String>,
}

pub struct MeetingCapture {
    /// Frame/Gap at `config.sample_rate` (Gap pads pauses so offsets stay capture-relative) and
    /// per-track ChannelReady/ChannelFailed, exactly like the legacy engine.
    pub stream: CaptureStream,
    pub scope: CaptureScope,
    pub provenance: CaptureProvenance,
    pub events: MeetingEventStream,
    pub control: Arc<dyn MeetingCaptureControl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EngineAvailability {
    /// `selects_microphone: false` means only the system default input can be recorded.
    Available {
        selects_microphone: bool,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallStatus {
    InCall,
    NotInCall,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallApp {
    pub bundle_id: String,
    pub platform: CallPlatform,
    pub status: CallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_url: Option<String>,
}

pub trait MeetingCaptureEngine: Send + Sync {
    fn availability(&self) -> EngineAvailability;
    /// A read of the call windows that can be shown to the user. It never authorizes anything.
    fn calls_in_progress(&self) -> Result<Vec<CallApp>, Error>;
    fn open(&self, request: MeetingCaptureRequest) -> Result<MeetingCapture, Error>;
}

/// The single lifecycle owner's answer to "may I still act for this capture?".
pub trait CaptureAuthority: Send + Sync {
    /// Same capture, same signed-in account, and still recording or finalizing.
    fn is_current(&self, capture: &CaptureIdentity) -> bool;
    /// The confirmed call of exactly this capture; `None` for system scope, stale, or unknown.
    fn confirmed_call(&self, capture: &CaptureIdentity) -> Option<ConfirmedCall>;
    /// Once a browser extension link is attached it owns that call's Meet actions, so native
    /// dispatch must not also act for the same capture.
    fn extension_handles_actions(&self, capture: &CaptureIdentity) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleOwner {
    App,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeetingLifecycleEvent {
    Started {
        capture: CaptureIdentity,
        scope: CaptureScope,
        owner: LifecycleOwner,
        engine: CaptureEngine,
    },
    Paused {
        capture: CaptureIdentity,
    },
    Resumed {
        capture: CaptureIdentity,
    },
    ExtensionAttached {
        capture: CaptureIdentity,
    },
    ExtensionDetached {
        capture: CaptureIdentity,
    },
    Ended {
        capture: CaptureIdentity,
        reason: CaptureEndReason,
    },
}

impl MeetingLifecycleEvent {
    pub fn capture(&self) -> &CaptureIdentity {
        match self {
            Self::Started { capture, .. }
            | Self::Paused { capture }
            | Self::Resumed { capture }
            | Self::ExtensionAttached { capture }
            | Self::ExtensionDetached { capture }
            | Self::Ended { capture, .. } => capture,
        }
    }
}

/// Observers must return quickly and re-check `authority.is_current` after every await.
pub trait MeetingLifecycleObserver: Send + Sync {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, authority: Arc<dyn CaptureAuthority>);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeOutcome {
    Sent,
    NotSent,
    CancelledBeforeDispatch,
    Unconfirmed,
}

impl NoticeOutcome {
    /// Once a send may have happened, retrying could post twice.
    pub fn may_have_sent(self) -> bool {
        matches!(self, Self::Sent | Self::Unconfirmed)
    }

    pub fn releases_claim(self) -> bool {
        !self.may_have_sent()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraBackgroundRequest {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraOutputReceipt {
    pub request_id: String,
    pub emitted_at_ms: u64,
}

impl CameraOutputReceipt {
    /// A fresh output for exactly the current request, which an app effect may only be disabled
    /// after.
    pub fn confirms(&self, request: &CameraBackgroundRequest, not_before_ms: u64) -> bool {
        self.request_id == request.request_id && self.emitted_at_ms >= not_before_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 48;

    #[test]
    fn timeline_maps_delivered_audio_across_a_pause() {
        let mut timeline = CaptureTimeline::new();
        timeline.append(0, 1_000 * MS).unwrap();
        // Paused from 1 s to 3 s, then contiguous again.
        timeline.append(3_000 * MS, 500 * MS).unwrap();
        timeline.append(3_500 * MS, 500 * MS).unwrap();

        assert_eq!(timeline.delivered_frames(), 2_000 * MS);
        assert_eq!(timeline.capture_ms(500.0, false).unwrap(), 500.0);
        assert_eq!(timeline.capture_ms(1_500.0, false).unwrap(), 3_500.0);
        // The boundary: an end stays before the pause, a start goes after it.
        assert_eq!(timeline.capture_ms(1_000.0, true).unwrap(), 1_000.0);
        assert_eq!(timeline.capture_ms(1_000.0, false).unwrap(), 3_000.0);
        assert_eq!(
            timeline.holes(),
            vec![(CaptureInstant(1_000 * MS), CaptureInstant(3_000 * MS))]
        );
        assert_eq!(timeline.span_of(CaptureInstant(999 * MS)), Some(0));
        assert_eq!(timeline.span_of(CaptureInstant(2_000 * MS)), None);
        assert_eq!(timeline.span_of(CaptureInstant(3_000 * MS)), Some(1));
    }

    #[test]
    fn timeline_rejects_backwards_and_out_of_range_audio() {
        let mut timeline = CaptureTimeline::new();
        assert_eq!(
            timeline.capture_ms(0.0, false),
            Err(TimelineError::OutOfRange)
        );
        timeline.append(100, 100).unwrap();
        assert_eq!(timeline.append(150, 10), Err(TimelineError::Backwards));
        assert_eq!(timeline.append(300, 0), Err(TimelineError::InvalidSpan));
        assert_eq!(
            timeline.capture_ms(-1.0, false),
            Err(TimelineError::OutOfRange)
        );
        assert_eq!(
            timeline.capture_ms(f64::NAN, false),
            Err(TimelineError::OutOfRange)
        );
        assert_eq!(
            timeline.capture_ms(60.0, false),
            Err(TimelineError::OutOfRange)
        );
        assert_eq!(
            timeline.holes(),
            vec![(CaptureInstant(0), CaptureInstant(100))]
        );
    }

    #[test]
    fn only_definitely_unsent_notices_release_their_claim() {
        assert!(NoticeOutcome::NotSent.releases_claim());
        assert!(NoticeOutcome::CancelledBeforeDispatch.releases_claim());
        assert!(!NoticeOutcome::Sent.releases_claim());
        assert!(!NoticeOutcome::Unconfirmed.releases_claim());
    }

    #[test]
    fn camera_receipt_must_match_the_current_request_and_be_fresh() {
        let request = CameraBackgroundRequest {
            request_id: "r2".into(),
            background: None,
        };
        let receipt = |request_id: &str, emitted_at_ms| CameraOutputReceipt {
            request_id: request_id.into(),
            emitted_at_ms,
        };
        assert!(receipt("r2", 20).confirms(&request, 10));
        assert!(!receipt("r1", 20).confirms(&request, 10));
        assert!(!receipt("r2", 9).confirms(&request, 10));
    }
}
