//! The live meeting as the note, its transcript and the floating bar see it: shipping's
//! listener store (`store/zustand/listener/general-shared.ts`) reduced to what they render.

use std::collections::BTreeMap;
use std::sync::Arc;

use anlg_listener_core::{DegradedError, LiveTranscriptSegment};
use desktop_runtime::SessionId;
use gpui::{App, AppContext, Context, Entity, Global, SharedString};

use super::capture::{CaptureService, CaptureUpdate, Health, Phase};

/// Shipping's `SessionMode` for one session. Starting a capture stays `Inactive` until the
/// recorder reports it active, like shipping's `loading` flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionMode {
    #[default]
    Inactive,
    Active,
    Finalizing,
}

/// Shipping's `LiveCaptureUiMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureMode {
    #[default]
    Live,
    /// Batch transcription by choice: audio is recorded and transcribed after stopping.
    RecordOnly,
    /// Live transcription was requested but fell back to recording only.
    FallbackRecordOnly,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveMeeting {
    pub session: SessionId,
    pub mode: SessionMode,
    /// `hypot(mic, speaker)` of the 0–1 channel levels, capped at 1.
    pub amplitude: f32,
    pub capture: CaptureMode,
    /// A capture-health warning such as "Microphone unavailable".
    pub warning: Option<SharedString>,
    /// Why the live transcript is paused, while audio keeps being saved.
    pub paused: Option<SharedString>,
    /// Live segments in time order, partial words included.
    pub segments: Arc<[Arc<LiveTranscriptSegment>]>,
}

impl LiveMeeting {
    pub fn new(session: SessionId, mode: SessionMode) -> Self {
        Self {
            session,
            mode,
            amplitude: 0.,
            capture: CaptureMode::Live,
            warning: None,
            paused: None,
            segments: Arc::from([]),
        }
    }

    pub fn live(&self) -> bool {
        matches!(self.mode, SessionMode::Active | SessionMode::Finalizing)
    }
}

#[derive(Default)]
pub struct LiveState {
    current: Option<LiveMeeting>,
    segments: BTreeMap<String, Arc<LiveTranscriptSegment>>,
    revision: u64,
}

struct LiveGlobal(Entity<LiveState>);
impl Global for LiveGlobal {}

impl LiveState {
    pub fn install(cx: &mut App) -> Entity<Self> {
        let state = cx.new(|_| Self::default());
        cx.set_global(LiveGlobal(state.clone()));
        state
    }

    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<LiveGlobal>().map(|global| global.0.clone())
    }

    pub fn current(&self) -> Option<&LiveMeeting> {
        self.current.as_ref()
    }

    pub fn session(&self, session: &SessionId) -> Option<&LiveMeeting> {
        self.current
            .as_ref()
            .filter(|live| &live.session == session)
    }

    /// The session's mode, `Inactive` when nothing is live or the state is not installed.
    pub fn mode(cx: &App, session: &SessionId) -> SessionMode {
        Self::global(cx)
            .and_then(|state| state.read(cx).session(session).map(|live| live.mode))
            .unwrap_or_default()
    }

    /// Replaces the live meeting outright; the capture poll uses `sync` instead.
    pub fn set(&mut self, live: Option<LiveMeeting>, cx: &mut Context<Self>) {
        self.segments.clear();
        self.current = live;
        cx.notify();
    }

    /// Folds the capture's latest state in, notifying observers only when something changed.
    pub fn sync(&mut self, capture: &CaptureService, cx: &mut Context<Self>) {
        let update = capture.take_update(self.revision);
        self.revision = update.revision;
        let next = self.apply(update);
        if next != self.current {
            self.current = next;
            cx.notify();
        }
    }

    fn apply(&mut self, update: CaptureUpdate) -> Option<LiveMeeting> {
        let mode = match update.phase {
            Phase::Listening => SessionMode::Active,
            Phase::Finalizing => SessionMode::Finalizing,
            Phase::Idle | Phase::Loading | Phase::Failed => SessionMode::Inactive,
        };
        let session = update.session.filter(|_| mode != SessionMode::Inactive);
        let Some(session) = session else {
            self.segments.clear();
            return None;
        };
        if update.resync || self.current.as_ref().map(|live| &live.session) != Some(&session) {
            self.segments.clear();
        }
        let changed = !update.changes.is_empty();
        for (id, segment) in update.changes {
            match segment {
                Some(segment) => self.segments.insert(id, segment),
                None => self.segments.remove(&id),
            };
        }
        let segments = match &self.current {
            Some(current) if !changed && current.session == session => current.segments.clone(),
            _ => {
                let mut segments: Vec<_> = self.segments.values().cloned().collect();
                segments.sort_by(|a, b| {
                    (a.start_ms, a.end_ms, &a.id).cmp(&(b.start_ms, b.end_ms, &b.id))
                });
                segments.into()
            }
        };
        let (mic, speaker) = update.amplitude;
        let level = |value: u16| (f32::from(value) / 1000.).clamp(0., 1.);
        let active = mode == SessionMode::Active;
        let interrupted = active
            && update.requested_live == Some(true)
            && (update.live_active == Some(false) || update.degraded.is_some() || update.stalled);
        Some(LiveMeeting {
            session,
            mode,
            amplitude: level(mic).hypot(level(speaker)).min(1.),
            capture: match (update.live_active, update.requested_live) {
                (Some(false), Some(true)) => CaptureMode::FallbackRecordOnly,
                (Some(false), _) => CaptureMode::RecordOnly,
                _ => CaptureMode::Live,
            },
            warning: capture_warning(&update.health, update.audio_interrupted).map(Into::into),
            paused: interrupted.then(|| {
                paused_message(update.degraded.as_ref(), &update.provider, online()).into()
            }),
            segments,
        })
    }
}

/// Shipping's `getCaptureWarning`.
pub fn capture_warning(health: &Health, interrupted: bool) -> Option<&'static str> {
    let warning =
        if health.unavailable || (health.mic == Some(false) && health.speaker == Some(false)) {
            "No audio is being captured"
        } else if health.mic == Some(false) {
            if health.speaker == Some(true) {
                "Microphone unavailable. System audio continues"
            } else {
                "Microphone unavailable"
            }
        } else if health.speaker == Some(false) {
            if health.mic == Some(true) {
                "System audio unavailable. Microphone capture continues"
            } else {
                "System audio unavailable"
            }
        } else if interrupted {
            "Audio capture was interrupted"
        } else {
            return None;
        };
    Some(warning)
}

const LOCAL_PROVIDERS: [&str; 3] = ["soniqo", "apple_speech", "local_file"];

/// Whether the system has a network route, like the webview's `navigator.onLine`. A UDP
/// `connect` only consults the routing table; nothing is sent.
pub fn online() -> bool {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| socket.connect("1.1.1.1:53"))
        .is_ok()
}

/// Shipping's `getLiveTranscriptPausedMessage`.
pub fn paused_message(degraded: Option<&DegradedError>, provider: &str, online: bool) -> String {
    let cloud = provider == "anarlog";
    let local = LOCAL_PROVIDERS.contains(&provider);
    let name = crate::product::provider_catalog::STT
        .iter()
        .find(|info| info.id == provider)
        .map(|info| info.name)
        .unwrap_or("the transcription service");
    let provider_side = |message: &str| {
        let lower = message.to_lowercase();
        lower.contains("provider error")
            || message.contains("code=Some(4")
            || message.split_once(": ").is_some_and(|(prefix, _)| {
                !prefix.is_empty()
                    && prefix
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            })
    };
    let cause = match degraded {
        _ if !online && !local => "you're offline".to_owned(),
        Some(DegradedError::AuthenticationFailed { .. }) if cloud => {
            "Anarlog couldn't verify your account".to_owned()
        }
        Some(DegradedError::AuthenticationFailed { .. }) => format!("{name} rejected your API key"),
        Some(DegradedError::ProviderConfiguration { .. }) if cloud => {
            "our speech-to-text provider rejected the request".to_owned()
        }
        Some(DegradedError::ProviderConfiguration { .. }) => {
            format!("{name} rejected the transcription settings")
        }
        Some(DegradedError::StreamError { message }) if provider_side(message) => {
            if cloud {
                "our speech-to-text provider is having an outage".to_owned()
            } else {
                format!("{name} is having an outage")
            }
        }
        Some(_) if cloud => "Anarlog's transcription server is having issues".to_owned(),
        Some(_) if local => "the local transcription model stopped responding".to_owned(),
        Some(_) => format!("Anarlog can't reach {name}"),
        None if cloud => "Anarlog's transcription server stopped sending words".to_owned(),
        None => format!("{name} stopped sending words"),
    };
    format!(
        "Live transcript paused because {cause}. Your audio is still being saved, and the missing part will be filled in from the recording."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_transcript::{ChannelProfile, SegmentKey};

    fn update(phase: Phase) -> CaptureUpdate {
        CaptureUpdate {
            session: Some(SessionId("s".into())),
            phase,
            status: "".into(),
            error: None,
            amplitude: (0, 0),
            changes: Default::default(),
            revision: 1,
            resync: false,
            health: Health::default(),
            audio_interrupted: false,
            requested_live: Some(true),
            live_active: Some(true),
            degraded: None,
            stalled: false,
            provider: "deepgram".into(),
        }
    }

    fn segment(id: &str, start_ms: i64) -> Option<Arc<LiveTranscriptSegment>> {
        Some(Arc::new(LiveTranscriptSegment {
            id: id.into(),
            key: SegmentKey {
                channel: ChannelProfile::DirectMic,
                speaker_index: None,
                speaker_human_id: None,
            },
            start_ms,
            end_ms: start_ms + 100,
            text: String::new(),
            words: Vec::new(),
        }))
    }

    #[test]
    fn capture_updates_follow_shipping_session_modes_and_warnings() {
        let mut state = LiveState::default();
        assert_eq!(state.apply(update(Phase::Loading)), None);

        let mut listening = update(Phase::Listening);
        listening.amplitude = (600, 800);
        listening.changes = [
            ("b".into(), segment("b", 2000)),
            ("a".into(), segment("a", 0)),
        ]
        .into();
        listening.health = Health {
            mic: Some(false),
            speaker: Some(true),
            unavailable: false,
        };
        let live = state.apply(listening).unwrap();
        assert_eq!(live.mode, SessionMode::Active);
        assert!((live.amplitude - 1.).abs() < 1e-6);
        let order: Vec<_> = live
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect();
        assert_eq!(order, ["a", "b"]);
        assert_eq!(
            live.warning.as_deref(),
            Some("Microphone unavailable. System audio continues")
        );
        assert_eq!(
            (live.capture, live.paused.clone()),
            (CaptureMode::Live, None)
        );
        state.current = Some(live);

        let mut stalled = update(Phase::Listening);
        stalled.live_active = Some(false);
        stalled.changes = [("a".into(), None)].into();
        let live = state.apply(stalled).unwrap();
        assert_eq!(live.capture, CaptureMode::FallbackRecordOnly);
        assert_eq!(live.segments.len(), 1);
        assert!(
            live.paused
                .as_deref()
                .unwrap_or_default()
                .starts_with("Live transcript paused because Deepgram stopped sending words.")
        );
        state.current = Some(live.clone());

        let finalizing = state.apply(update(Phase::Finalizing)).unwrap();
        assert_eq!(finalizing.mode, SessionMode::Finalizing);
        assert_eq!(finalizing.segments, live.segments);
        assert_eq!(state.apply(update(Phase::Idle)), None);
        assert!(state.segments.is_empty());
    }

    #[test]
    fn paused_message_names_what_failed() {
        let outage = DegradedError::StreamError {
            message: "deepgram: overloaded".into(),
        };
        assert!(
            paused_message(Some(&outage), "deepgram", true)
                .contains("because Deepgram is having an outage.")
        );
        let auth = DegradedError::AuthenticationFailed {
            provider: "anarlog".into(),
        };
        assert!(
            paused_message(Some(&auth), "anarlog", true)
                .contains("because Anarlog couldn't verify your account.")
        );
        assert!(
            paused_message(Some(&DegradedError::ConnectionTimeout), "soniqo", true)
                .contains("the local transcription model stopped responding")
        );
        // Offline names the cause for cloud providers; a local model needs no network.
        assert!(
            paused_message(Some(&outage), "deepgram", false)
                .starts_with("Live transcript paused because you're offline.")
        );
        assert!(paused_message(None, "soniqo", false).ends_with(
            "stopped sending words. Your audio is still being saved, and the missing part will be filled in from the recording."
        ));
    }
}
