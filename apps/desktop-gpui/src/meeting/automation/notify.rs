//! Shipping's notification payloads, the Notifications settings policy
//! (`shared/{notification-policy,completion-sound,app-attention}.ts`), the five-minute event
//! reminders (`services/event-notification`) and where a notification's buttons lead
//! (`services/event-listeners.tsx`).

use std::{collections::HashMap, path::Path, time::Duration};

use anlg_notification_interface::{
    Notification, NotificationAction, NotificationActionMenu, NotificationContext,
    NotificationFooter, NotificationSource,
};
use chrono::{DateTime, Utc};
use desktop_runtime::SessionId;
use serde_json::Value;

use super::{
    apps::{self, MicApp, NearbyEvent},
    detect::Policy,
};
use crate::{
    platform::{notifications::Action, sounds::COMPLETION_SOUNDS},
    product::settings::{self, Snapshot},
};

const REMINDER_WINDOW: chrono::TimeDelta = chrono::TimeDelta::minutes(5);
const REMINDER_TTL: chrono::TimeDelta = chrono::TimeDelta::minutes(10);
const BRIEF: Duration = Duration::from_secs(15);

/// Reads the Notifications and Meetings settings, falling back to the schema defaults until
/// the first snapshot arrives.
#[derive(Clone, Copy)]
pub struct Prefs<'a>(pub Option<&'a Snapshot>);

impl Prefs<'_> {
    fn value(&self, key: &str) -> Option<Value> {
        self.0
            .and_then(|snapshot| snapshot.get(key))
            .map(|setting| setting.value.clone())
            .or_else(|| {
                settings::definition(key)
                    .ok()
                    .map(|def| def.default.clone())
            })
    }

    pub fn flag(&self, key: &str) -> bool {
        self.value(key).and_then(|value| value.as_bool()) == Some(true)
    }

    fn list(&self, key: &str) -> Vec<String> {
        self.value(key)
            .and_then(|value| value.as_str().map(str::to_owned))
            .and_then(|text| serde_json::from_str::<Vec<Value>>(&text).ok())
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `shouldShowNotification`: the kind is on and notifications are not disabled.
    pub fn allows(&self, key: &str) -> bool {
        !self.flag("notification_disabled") && self.flag(key)
    }

    pub fn detection(&self) -> Policy {
        let threshold = self
            .value("mic_active_threshold")
            .and_then(|value| value.as_f64())
            .filter(|seconds| seconds.is_finite() && *seconds > 0.)
            .map_or(Policy::default().threshold, Duration::from_secs_f64);
        Policy {
            enabled: self.flag("notification_detect"),
            respect_dnd: self.flag("respect_dnd"),
            ignored: self.list("ignored_platforms").into_iter().collect(),
            included: self.list("included_platforms").into_iter().collect(),
            threshold,
        }
    }

    pub fn ignored_apps(&self) -> Vec<String> {
        self.list("ignored_platforms")
    }

    /// The completion sound to play, if any (`playCompletionSound`).
    pub fn completion_sound(&self) -> Option<&'static str> {
        if self.flag("notification_disabled") || !self.flag("notification_completion_sound") {
            return None;
        }
        let name = self
            .value("notification_completion_sound_name")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        Some(
            COMPLETION_SOUNDS
                .into_iter()
                .find(|sound| *sound == name)
                .unwrap_or("ready"),
        )
    }

    /// `requestAppAttention`'s settings: bounce on, and the app shown in the Dock.
    pub fn bounce(&self) -> bool {
        if self.flag("notification_disabled") || !self.flag("show_app_in_dock") {
            return false;
        }
        let unset = self
            .0
            .and_then(|snapshot| snapshot.get("notification_bounce"))
            .is_none_or(|setting| setting.source == "Default");
        if unset {
            // The general setting honors existing per-event preferences.
            let legacy: Vec<bool> = [
                "notification_bounce_summary",
                "notification_bounce_transcript",
            ]
            .into_iter()
            .filter_map(|key| self.0?.get(key))
            .filter(|setting| setting.source != "Default")
            .filter_map(|setting| setting.value.as_bool())
            .collect();
            if !legacy.is_empty() {
                return legacy.into_iter().all(|value| value);
            }
        }
        self.flag("notification_bounce")
    }
}

pub fn mic_detected(
    key: String,
    apps: &[MicApp],
    nearby: Option<&NearbyEvent>,
    resources: Option<&Path>,
) -> Notification {
    let ignorable = apps::ignorable(apps);
    let platform = apps::browser_platform(apps, nearby);
    let shown: Vec<MicApp> = ignorable
        .iter()
        .map(|app| apps::display_app(app, platform))
        .collect();
    let footer = (!shown.is_empty()).then(|| NotificationFooter {
        text: apps::ignore_footer_text(&shown),
        action_label: if shown.len() == 1 {
            "Ignore it"
        } else {
            "Ignore them"
        }
        .into(),
        icon: apps::footer_icon(resources, &ignorable, platform),
    });
    let mut notification = Notification::builder()
        .key(key)
        .title(apps::detected_title(nearby))
        .message("")
        .timeout(BRIEF)
        .source(NotificationSource::MicDetected {
            app_names: apps
                .iter()
                .map(|app| apps::display_name(&apps::display_app(app, platform)))
                .collect(),
            app_ids: ignorable.iter().map(|app| app.id.clone()).collect(),
            event_ids: nearby
                .map(|event| vec![event.id.clone()])
                .unwrap_or_default(),
        })
        .action_label("Yes")
        .build();
    notification.footer = footer;
    if let Some(icon) = apps::detected_icon(resources, apps, platform) {
        notification.icon = Some(icon);
    }
    notification
}

pub fn summary_ready(session: &SessionId, title: &str, nonce: &str) -> Notification {
    let title = title.trim();
    Notification::builder()
        .key(format!("summary-ready:{}:{nonce}", session.0))
        .title("Summary ready")
        .message(if title.is_empty() {
            "Your summary is ready.".to_owned()
        } else {
            format!("\"{title}\" is ready to read.")
        })
        .timeout(BRIEF)
        .source(NotificationSource::Session {
            session_id: session.0.to_string(),
        })
        .action_label("Open Anarlog")
        .build()
}

pub fn cloudsync_complete(user: &str) -> Notification {
    Notification::builder()
        .key(format!("cloudsync-initial-sync-complete-{user}"))
        .title("Cloud sync complete")
        .message("Your Anarlog data is ready on this device.")
        .action_label("Open Anarlog")
        .build()
}

pub const TEAM_INVITATION: &str = "team-invitation:";

pub fn team_invitation(id: &str, workspace: &str, invited_by: Option<&str>) -> Notification {
    Notification::builder()
        .key(format!("{TEAM_INVITATION}{id}"))
        .title(format!("You've been invited to join {workspace}"))
        .message(invited_by.map_or_else(String::new, |email| format!("Invited by {email}")))
        .timeout(BRIEF)
        .action_label("View")
        .build()
}

#[derive(Clone, Debug, Default)]
pub struct ReminderEvent {
    pub id: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub meeting_link: String,
}

/// Events already reminded about, so each start is announced once.
#[derive(Default)]
pub struct Reminders {
    notified: HashMap<String, DateTime<Utc>>,
}

impl Reminders {
    pub fn due(&mut self, events: &[ReminderEvent], now: DateTime<Utc>) -> Vec<Notification> {
        self.notified.retain(|_, at| now - *at <= REMINDER_TTL);
        let mut due = Vec::new();
        for event in events {
            let until = event.start - now;
            let key = format!("event-{}-{}", event.id, event.start.timestamp_millis());
            if until <= chrono::TimeDelta::zero() {
                self.notified.remove(&key);
                continue;
            }
            if until > REMINDER_WINDOW || self.notified.contains_key(&key) {
                continue;
            }
            self.notified.insert(key.clone(), now);
            let minutes = (until.num_milliseconds() + 59_999) / 60_000;
            let link = !event.meeting_link.trim().is_empty();
            let mut notification = Notification::builder()
                .key(key)
                .title(if event.title.is_empty() {
                    "Upcoming Event"
                } else {
                    event.title.as_str()
                })
                .message(if minutes == 1 {
                    "Starting in 1 minute".to_owned()
                } else {
                    format!("Starting in {minutes} minutes")
                })
                .source(NotificationSource::CalendarEvent {
                    event_id: event.id.clone(),
                })
                .start_time(event.start.timestamp())
                .action_label(if link {
                    "Join & record"
                } else {
                    "Start recording"
                })
                .action(NotificationAction::JoinAndRecord)
                .build();
            notification.action_menu = link.then(|| NotificationActionMenu {
                label: "Open meeting".into(),
                action: NotificationAction::OpenMeeting,
            });
            due.push(notification);
        }
        due
    }
}

/// What a notification button does in the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Nothing,
    OpenSession(SessionId),
    TeamSettings,
    /// The event's note (or a new note), listening when `listen` holds or, unless set, once
    /// the event has started.
    Listen {
        event: Option<String>,
        triggers: Vec<String>,
        listen: Option<bool>,
    },
    JoinAndRecord(String),
    OpenMeeting(String),
    Ignore(Vec<String>),
}

/// Whether the main window comes forward first, and what follows (shipping's notification
/// plugin handlers plus `useNotificationEvents`). Keys owned elsewhere still arrive here;
/// their sources are `None`, so they only bring the window forward.
pub fn respond(action: Action, context: &NotificationContext) -> (bool, Step) {
    let source = context.source.as_ref();
    let event_of = |index: usize| match source {
        Some(NotificationSource::CalendarEvent { event_id }) if index == 0 => {
            Some(event_id.clone())
        }
        Some(NotificationSource::MicDetected { event_ids, .. }) => event_ids.get(index).cloned(),
        _ => None,
    };
    let triggers = match source {
        Some(NotificationSource::MicDetected { app_ids, .. }) => app_ids.clone(),
        _ => Vec::new(),
    };
    match (action, context.action) {
        (Action::Accept | Action::Option(_), Some(chosen)) => {
            let step = match source {
                Some(NotificationSource::CalendarEvent { event_id }) => match chosen {
                    NotificationAction::JoinAndRecord => Step::JoinAndRecord(event_id.clone()),
                    NotificationAction::OpenMeeting => Step::OpenMeeting(event_id.clone()),
                },
                _ => Step::Nothing,
            };
            (false, step)
        }
        (Action::Confirm, _) | (Action::Accept, None) => {
            let step = if context.key.starts_with(TEAM_INVITATION) {
                Step::TeamSettings
            } else {
                match source {
                    Some(NotificationSource::Session { session_id }) => {
                        Step::OpenSession(SessionId(session_id.as_str().into()))
                    }
                    Some(
                        NotificationSource::CalendarEvent { .. }
                        | NotificationSource::MicDetected { .. },
                    ) => Step::Listen {
                        event: event_of(0),
                        listen: (!triggers.is_empty()).then_some(true),
                        triggers,
                    },
                    None => Step::Nothing,
                }
            };
            (true, step)
        }
        (Action::Option(index), None) => (
            true,
            Step::Listen {
                event: event_of(index),
                triggers,
                listen: Some(true),
            },
        ),
        (Action::Footer, _) => (
            false,
            if triggers.is_empty() {
                Step::Nothing
            } else {
                Step::Ignore(triggers)
            },
        ),
        (Action::Dismiss | Action::Timeout, _) => (false, Step::Nothing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::settings::{Revision, Setting};

    fn snapshot(values: &[(&str, Value, &'static str)]) -> Snapshot {
        values
            .iter()
            .map(|(key, value, source)| {
                (
                    (*key).to_owned(),
                    Setting {
                        value: value.clone(),
                        revision: Revision::default(),
                        source,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn disabling_all_notifications_silences_every_kind_and_bounce_honors_legacy_settings() {
        use serde_json::json;
        let disabled = snapshot(&[("notification_disabled", json!(true), "Device")]);
        let prefs = Prefs(Some(&disabled));
        assert!(!prefs.allows("notification_summary_complete"));
        assert_eq!(prefs.completion_sound(), None);
        assert!(!prefs.bounce());
        let defaults = Prefs(None);
        assert!(defaults.allows("notification_summary_complete"));
        assert_eq!(defaults.completion_sound(), Some("ready"));
        assert!(defaults.bounce());
        let legacy = snapshot(&[
            ("notification_bounce", json!(true), "Default"),
            ("notification_bounce_summary", json!(false), "Legacy"),
            ("show_app_in_dock", json!(true), "Default"),
        ]);
        assert!(!Prefs(Some(&legacy)).bounce());
        let hidden = snapshot(&[("show_app_in_dock", json!(false), "Device")]);
        assert!(!Prefs(Some(&hidden)).bounce());
    }

    #[test]
    fn event_reminders_fire_once_inside_five_minutes_and_rearm_for_a_moved_start() {
        let now = Utc::now();
        let event = ReminderEvent {
            id: "event-1".into(),
            title: String::new(),
            start: now + chrono::TimeDelta::seconds(61),
            meeting_link: "https://meet.google.com/abc".into(),
        };
        let far = ReminderEvent {
            id: "event-2".into(),
            start: now + chrono::TimeDelta::minutes(6),
            ..event.clone()
        };
        let mut reminders = Reminders::default();
        let due = reminders.due(&[event.clone(), far], now);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].message, "Starting in 2 minutes");
        assert_eq!(due[0].action, Some(NotificationAction::JoinAndRecord));
        assert!(due[0].action_menu.is_some());
        assert!(reminders.due(std::slice::from_ref(&event), now).is_empty());
        let moved = ReminderEvent {
            start: event.start + chrono::TimeDelta::minutes(1),
            ..event
        };
        assert_eq!(reminders.due(&[moved], now).len(), 1);
    }

    #[test]
    fn notification_buttons_route_like_shipping() {
        let context = |key: &str, source, action| NotificationContext {
            key: key.into(),
            source,
            action,
        };
        let mic = Some(NotificationSource::MicDetected {
            app_names: vec!["Zoom".into()],
            app_ids: vec!["us.zoom.xos".into()],
            event_ids: vec!["event-1".into()],
        });
        assert_eq!(
            respond(Action::Confirm, &context("k", mic.clone(), None)),
            (
                true,
                Step::Listen {
                    event: Some("event-1".into()),
                    triggers: vec!["us.zoom.xos".into()],
                    listen: Some(true),
                }
            )
        );
        assert_eq!(
            respond(Action::Footer, &context("k", mic, None)),
            (false, Step::Ignore(vec!["us.zoom.xos".into()]))
        );
        let calendar = Some(NotificationSource::CalendarEvent {
            event_id: "event-1".into(),
        });
        assert_eq!(
            respond(Action::Confirm, &context("k", calendar.clone(), None)),
            (
                true,
                Step::Listen {
                    event: Some("event-1".into()),
                    triggers: Vec::new(),
                    listen: None,
                }
            )
        );
        assert_eq!(
            respond(
                Action::Option(0),
                &context("k", calendar, Some(NotificationAction::OpenMeeting))
            ),
            (false, Step::OpenMeeting("event-1".into()))
        );
        assert_eq!(
            respond(Action::Accept, &context("team-invitation:1", None, None)),
            (true, Step::TeamSettings)
        );
        assert_eq!(
            respond(Action::Timeout, &context("auto-stop-ended:s", None, None)),
            (false, Step::Nothing)
        );
    }
}
