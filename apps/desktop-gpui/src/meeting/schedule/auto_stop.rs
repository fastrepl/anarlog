//! Shipping's "Stop when meeting ends" (`stt/auto-stop.ts`, `stt/auto-stop-notification.ts`
//! and the micStopped branch of `stt/detect-events.ts`). When an app that triggered the
//! recording releases the microphone, the stop is confirmed five seconds later against the
//! current microphone users and the meeting window; ambiguous cases ask "Did your meeting
//! end?" and stop after a 30-second countdown.

use std::{collections::HashMap, time::Duration};

use anlg_notification_interface::{Notification, NotificationActionVariant, NotificationIcon};
use desktop_runtime::SessionId;

use super::apps::{Inspection, MeetingPlatform, MicApp, is_browser, is_meeting_capable};

pub const CONFIRM_DELAY: Duration = Duration::from_secs(5);
pub const ACCESSIBILITY_TIMEOUT: Duration = Duration::from_secs(10);
pub const PROMPT_SECONDS: u64 = 30;
pub const PROMPT_PREFIX: &str = "auto-stop-ended:";
const PROMPT_SEPARATOR: &str = ":prompt:";

/// KakaoTalk keeps reporting the microphone after a call, so it never confirms a stop alone.
const UNRELIABLE_APP_IDS: &[&str] = &["com.kakao.KakaoTalkMac"];

/// Shipping's `getAutoStopCandidateAppIds`: the triggers that went quiet, else every trigger.
pub fn candidates(triggers: &[String], stopped: &[MicApp]) -> Vec<String> {
    let quiet: Vec<&String> = triggers
        .iter()
        .filter(|id| stopped.iter().any(|app| &app.id == *id))
        .collect();
    let chosen: Vec<&String> = if quiet.is_empty() {
        triggers.iter().collect()
    } else {
        quiet
    };
    chosen
        .into_iter()
        .filter(|id| !UNRELIABLE_APP_IDS.contains(&id.as_str()))
        .cloned()
        .collect()
}

/// Shipping's `getAutoStopActiveCheckAppIds`: candidates plus unreliable triggers.
fn active_check(triggers: &[String], candidates: &[String]) -> Vec<String> {
    let mut ids = candidates.to_vec();
    for id in triggers {
        if UNRELIABLE_APP_IDS.contains(&id.as_str()) && !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids
}

/// Shipping's `getMeetingAppsStillOnCall`: the app that went quiet is not always the one
/// carrying the call. When a meeting app went quiet, only another trigger counts, so an
/// unrelated browser tab cannot keep a finished call recording.
fn still_on_call(triggers: &[String], quiet: &[MicApp], mic: &[MicApp]) -> Vec<MicApp> {
    let meeting_app_went_quiet = quiet.iter().any(is_meeting_capable);
    mic.iter()
        .filter(|app| {
            !quiet.iter().any(|quiet| quiet.id == app.id)
                && is_meeting_capable(app)
                && (!meeting_app_went_quiet || triggers.contains(&app.id))
        })
        .cloned()
        .collect()
}

/// Shipping's `inspectionShowsActiveMeeting` for one of `app_ids`.
pub fn shows_active_meeting(inspections: &[Inspection], app_ids: &[String]) -> bool {
    inspections.iter().any(|inspection| {
        app_ids.contains(&inspection.app.id)
            && inspection.active_call
            && inspection.accessibility_trusted
            && inspection.platform != MeetingPlatform::Unknown
            && inspection
                .window_title
                .as_deref()
                .is_some_and(|title| !title.trim().is_empty())
            && !inspection.warnings.iter().any(|warning| {
                let warning = warning.to_lowercase();
                warning.contains("ambiguous")
                    || warning.contains("incomplete")
                    || warning.contains("no uniquely validated")
            })
    })
}

/// A scheduled confirmation of an auto-stop.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub session: SessionId,
    pub candidates: Vec<String>,
    pub stopped: Vec<MicApp>,
    /// No trigger app itself went quiet, so only a microphone snapshot can confirm.
    pub require_mic_snapshot: bool,
}

/// After the microphone snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum MicCheck {
    /// A candidate still uses the microphone: keep recording.
    StillActive,
    /// Other meeting apps carry the call: track them as triggers and keep recording.
    OnCall(Vec<MicApp>),
    /// Without a snapshot the stop cannot be confirmed.
    Unconfirmed,
    /// Ask the meeting window.
    Inspect,
}

impl Pending {
    /// Shipping's micStopped handling: returns the pending confirmation, if any.
    pub fn after_stop(
        session: SessionId,
        triggers: &[String],
        stopped: &[MicApp],
        current: Option<&Pending>,
    ) -> Option<Self> {
        let candidates = candidates(triggers, stopped);
        if candidates.is_empty() {
            return None;
        }
        let require_mic_snapshot = !triggers
            .iter()
            .any(|id| stopped.iter().any(|app| &app.id == id));
        // A weaker stop must not replace one that a trigger itself reported.
        if current.is_some_and(|pending| !pending.require_mic_snapshot) && require_mic_snapshot {
            return None;
        }
        Some(Self {
            session,
            candidates,
            stopped: stopped.to_vec(),
            require_mic_snapshot,
        })
    }

    fn active_check(&self, triggers: &[String]) -> Vec<String> {
        active_check(triggers, &self.candidates)
    }

    pub fn check_mic(&self, triggers: &[String], mic: Result<&[MicApp], ()>) -> MicCheck {
        let active_check = self.active_check(triggers);
        match mic {
            Ok(mic) => {
                if mic.iter().any(|app| active_check.contains(&app.id)) {
                    return MicCheck::StillActive;
                }
                // Every app in the stop event went quiet, including meeting apps that never
                // became triggers.
                let mut quiet = self.stopped.clone();
                for id in &self.candidates {
                    if !quiet.iter().any(|app| &app.id == id) {
                        quiet.push(MicApp::new(id.clone(), ""));
                    }
                }
                let on_call = still_on_call(triggers, &quiet, mic);
                if on_call.is_empty() {
                    MicCheck::Inspect
                } else {
                    MicCheck::OnCall(on_call)
                }
            }
            Err(()) => {
                let unreliable = active_check.len() > self.candidates.len();
                if self.require_mic_snapshot || unreliable {
                    MicCheck::Unconfirmed
                } else {
                    MicCheck::Inspect
                }
            }
        }
    }

    /// After the accessibility inspection (`None` when it failed or timed out).
    pub fn decide(&self, triggers: &[String], inspections: Option<&[Inspection]>) -> Verdict {
        match inspections {
            Some(inspections)
                if shows_active_meeting(inspections, &self.active_check(triggers)) =>
            {
                Verdict::Recheck
            }
            // A browser tab closing does not prove the call ended, so ask first.
            Some(_) if !self.candidates.iter().any(|id| is_browser(id)) => Verdict::Stop,
            _ => Verdict::Prompt,
        }
    }

    /// The app the prompt shows (shipping's `getPrimaryStoppedApp`).
    fn primary_app(&self) -> Option<&MicApp> {
        let stopped_candidate = |app: &&MicApp| self.candidates.contains(&app.id);
        self.stopped
            .iter()
            .filter(stopped_candidate)
            .find(|app| is_browser(&app.id))
            .or_else(|| self.stopped.iter().find(stopped_candidate))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The meeting window still shows a call: confirm again after the delay.
    Recheck,
    Prompt,
    Stop,
}

/// One live "Did your meeting end?" key per session; a new trigger or a new recording
/// invalidates it, so a late countdown cannot stop the wrong capture.
#[derive(Default)]
pub struct Prompts {
    active: HashMap<SessionId, String>,
}

impl Prompts {
    pub fn key(&mut self, session: &SessionId) -> String {
        if let Some(key) = self.active.get(session) {
            return key.clone();
        }
        let key = format!(
            "{PROMPT_PREFIX}{}{PROMPT_SEPARATOR}{}",
            session.0,
            uuid::Uuid::new_v4()
        );
        self.active.insert(session.clone(), key.clone());
        key
    }

    pub fn cancel(&mut self, session: &SessionId) -> bool {
        self.active.remove(session).is_some()
    }

    pub fn is_active(&self, key: &str) -> bool {
        parse(key).is_some_and(|session| self.active.get(&session).map(String::as_str) == Some(key))
    }

    /// The session to stop, consuming the key, when an active prompt is answered.
    pub fn consume(&mut self, key: &str) -> Option<SessionId> {
        let session = parse(key)?;
        if self.active.get(&session).map(String::as_str) != Some(key) {
            return None;
        }
        self.active.remove(&session);
        Some(session)
    }
}

/// The session in a prompt key, if the key is one.
pub fn parse(key: &str) -> Option<SessionId> {
    let value = key.strip_prefix(PROMPT_PREFIX)?;
    let session = value
        .rfind(PROMPT_SEPARATOR)
        .map_or(value, |index| &value[..index]);
    (!session.is_empty()).then(|| SessionId(session.into()))
}

pub fn prompt(key: String, pending: &Pending) -> Notification {
    let mut notification = Notification::builder()
        .key(key)
        .title("Did your meeting end?")
        .message(format!(
            "Anarlog will stop listening in {PROMPT_SECONDS} seconds."
        ))
        .timeout(Duration::from_secs(PROMPT_SECONDS))
        .action_label("Stop")
        .action_variant(NotificationActionVariant::Destructive);
    if let Some(app) = pending
        .primary_app()
        .filter(|app| !app.id.is_empty() && !app.id.starts_with("pid:"))
    {
        notification = notification.icon(if app.id.starts_with('/') || app.id.starts_with("~/") {
            NotificationIcon::Path {
                path: app.id.clone(),
            }
        } else {
            NotificationIcon::BundleId {
                bundle_id: app.id.clone(),
            }
        });
    }
    notification.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::schedule::apps::{MeetingApp, MeetingSurface};

    fn session() -> SessionId {
        SessionId("s-1".into())
    }

    fn zoom() -> MicApp {
        MicApp::new("us.zoom.xos", "zoom.us")
    }

    fn chrome() -> MicApp {
        MicApp::new("com.google.Chrome", "Google Chrome")
    }

    fn inspection(app: &MicApp, active: bool) -> Inspection {
        Inspection {
            active_call: active,
            app: MeetingApp {
                id: app.id.clone(),
                name: app.name.clone(),
            },
            pid: 1,
            platform: MeetingPlatform::Zoom,
            surface: MeetingSurface::Native,
            accessibility_trusted: true,
            window_title: Some("Zoom Meeting".into()),
            remote_screen_share: false,
            warnings: vec![],
        }
    }

    #[test]
    fn a_quiet_meeting_app_stops_when_its_window_shows_no_call() {
        let triggers = vec![zoom().id];
        let pending = Pending::after_stop(session(), &triggers, &[zoom()], None).unwrap();
        assert!(!pending.require_mic_snapshot);
        assert_eq!(pending.check_mic(&triggers, Ok(&[])), MicCheck::Inspect);
        assert_eq!(
            pending.decide(&triggers, Some(&[inspection(&zoom(), false)])),
            Verdict::Stop
        );
        assert_eq!(
            pending.decide(&triggers, Some(&[inspection(&zoom(), true)])),
            Verdict::Recheck
        );
        assert_eq!(pending.decide(&triggers, None), Verdict::Prompt);
    }

    #[test]
    fn browsers_and_ongoing_calls_never_stop_without_asking() {
        let triggers = vec![chrome().id, zoom().id];
        let pending = Pending::after_stop(session(), &triggers, &[chrome()], None).unwrap();
        assert_eq!(
            pending.check_mic(&triggers, Ok(&[zoom()])),
            MicCheck::OnCall(vec![zoom()])
        );
        assert_eq!(
            pending.check_mic(&triggers, Ok(&[chrome()])),
            MicCheck::StillActive
        );
        assert_eq!(pending.decide(&triggers, Some(&[])), Verdict::Prompt);
        let weaker = Pending::after_stop(
            session(),
            &triggers,
            &[MicApp::new("x", "X")],
            Some(&pending),
        );
        assert_eq!(weaker, None);
    }

    #[test]
    fn a_prompt_stops_only_its_own_session_once() {
        let mut prompts = Prompts::default();
        let key = prompts.key(&session());
        assert_eq!(prompts.key(&session()), key);
        assert_eq!(parse(&key), Some(session()));
        assert_eq!(prompts.consume(&key), Some(session()));
        assert_eq!(prompts.consume(&key), None);
        let stale = prompts.key(&session());
        prompts.cancel(&session());
        assert!(!prompts.is_active(&stale));
        assert_eq!(parse("meeting-detected:1"), None);
    }
}
