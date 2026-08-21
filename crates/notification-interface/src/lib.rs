use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum NotificationEvent {
    Confirm,
    Accept,
    Dismiss,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationKey {
    MicStarted { apps: BTreeSet<String> },
    MicStopped { apps: BTreeSet<String> },
    CalendarEvent { event_id: String },
    Custom(String),
}

impl NotificationKey {
    pub fn mic_started(app_bundle_ids: impl IntoIterator<Item = String>) -> Self {
        Self::MicStarted {
            apps: app_bundle_ids.into_iter().collect(),
        }
    }

    pub fn mic_stopped(app_bundle_ids: impl IntoIterator<Item = String>) -> Self {
        Self::MicStopped {
            apps: app_bundle_ids.into_iter().collect(),
        }
    }

    pub fn calendar_event(event_id: impl Into<String>) -> Self {
        Self::CalendarEvent {
            event_id: event_id.into(),
        }
    }

    pub fn to_dedup_key(&self) -> String {
        match self {
            Self::MicStarted { apps } => {
                let sorted: Vec<_> = apps.iter().cloned().collect();
                format!("mic-started:{}", sorted.join(","))
            }
            Self::MicStopped { apps } => {
                let sorted: Vec<_> = apps.iter().cloned().collect();
                format!("mic-stopped:{}", sorted.join(","))
            }
            Self::CalendarEvent { event_id } => {
                format!("event:{event_id}")
            }
            Self::Custom(s) => s.clone(),
        }
    }
}

impl From<String> for NotificationKey {
    fn from(s: String) -> Self {
        Self::Custom(s)
    }
}

impl From<&str> for NotificationKey {
    fn from(s: &str) -> Self {
        Self::Custom(s.to_string())
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum ParticipantStatus {
    #[default]
    Accepted,
    Maybe,
    Declined,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Participant {
    pub name: Option<String>,
    pub email: String,
    pub status: ParticipantStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct EventDetails {
    pub what: String,
    pub timezone: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotificationFooter {
    pub text: String,
    pub action_label: String,
    pub icon: Option<NotificationIcon>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum NotificationSource {
    #[serde(rename = "calendar_event")]
    CalendarEvent { event_id: String },
    #[serde(rename = "session")]
    Session { session_id: String },
    #[serde(rename = "mic_detected")]
    MicDetected {
        app_names: Vec<String>,
        #[serde(default)]
        app_ids: Vec<String>,
        #[serde(default)]
        event_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum NotificationIconAsset {
    #[serde(rename = "app_icon")]
    AppIcon,
    #[serde(rename = "calendar")]
    Calendar,
    #[serde(rename = "system_symbol")]
    SystemSymbol { name: String },
    #[serde(rename = "bundle_id")]
    BundleId { bundle_id: String },
    #[serde(rename = "path")]
    Path { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum NotificationIcon {
    #[serde(rename = "hidden")]
    Hidden,
    #[serde(rename = "bundle_id")]
    BundleId { bundle_id: String },
    #[serde(rename = "system_symbol")]
    SystemSymbol { name: String },
    #[serde(rename = "path")]
    Path { path: String },
    #[serde(rename = "overlay")]
    Overlay {
        base: NotificationIconAsset,
        badge: NotificationIconAsset,
    },
}

#[derive(Debug, Clone)]
pub struct NotificationContext {
    pub key: String,
    pub source: Option<NotificationSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NotificationActionVariant {
    Default,
    Destructive,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Notification {
    pub key: Option<String>,
    pub title: String,
    pub message: String,
    pub timeout: Option<std::time::Duration>,
    pub source: Option<NotificationSource>,
    pub start_time: Option<i64>,
    pub participants: Option<Vec<Participant>>,
    pub event_details: Option<EventDetails>,
    pub action_label: Option<String>,
    pub action_variant: Option<NotificationActionVariant>,
    pub options: Option<Vec<String>>,
    pub footer: Option<NotificationFooter>,
    pub icon: Option<NotificationIcon>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryAction<'a> {
    Accept { label: &'a str, destructive: bool },
    Options(&'a [String]),
}

#[derive(Debug, Clone)]
pub struct DismissTimer {
    total: Duration,
    remaining: Duration,
    running_since: Option<Instant>,
}

impl DismissTimer {
    pub fn new(total: Duration) -> Self {
        Self::at(total, Instant::now())
    }

    pub fn at(total: Duration, now: Instant) -> Self {
        Self {
            total,
            remaining: total,
            running_since: Some(now),
        }
    }

    pub fn total(&self) -> Duration {
        self.total
    }

    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        match self.running_since {
            Some(started) => self
                .remaining
                .saturating_sub(now.saturating_duration_since(started)),
            None => self.remaining,
        }
    }

    pub fn progress_ratio(&self, now: Instant) -> f64 {
        if self.total.is_zero() {
            return 0.0;
        }

        self.remaining(now).as_secs_f64() / self.total.as_secs_f64()
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.remaining(now).is_zero()
    }

    pub fn pause(&mut self, now: Instant) {
        if let Some(started) = self.running_since.take() {
            self.remaining = self
                .remaining
                .saturating_sub(now.saturating_duration_since(started));
        }
    }

    pub fn resume(&mut self, now: Instant) {
        if self.running_since.is_none() && !self.remaining.is_zero() {
            self.running_since = Some(now);
        }
    }
}

impl Notification {
    pub fn builder() -> NotificationBuilder {
        NotificationBuilder::default()
    }

    pub fn is_persistent(&self) -> bool {
        self.timeout.is_none()
    }

    pub fn is_destructive_action(&self) -> bool {
        matches!(
            self.action_variant,
            Some(NotificationActionVariant::Destructive)
        )
    }

    pub fn shows_stop_countdown(&self) -> bool {
        self.is_destructive_action()
            && self.action_label.as_deref() == Some("Stop")
            && self.timeout.is_some_and(|timeout| !timeout.is_zero())
    }

    pub fn has_options(&self) -> bool {
        self.options
            .as_deref()
            .is_some_and(|options| !options.is_empty())
    }

    pub fn has_expandable_content(&self) -> bool {
        if matches!(self.source, Some(NotificationSource::CalendarEvent { .. })) {
            return false;
        }

        self.participants
            .as_ref()
            .is_some_and(|participants| !participants.is_empty())
            || self.event_details.is_some()
    }

    pub fn default_action_label(&self) -> &str {
        self.action_label.as_deref().unwrap_or("Open Anarlog")
    }

    pub fn expanded_action_label(&self) -> &str {
        self.action_label.as_deref().unwrap_or("Accept")
    }

    pub fn primary_action(&self) -> PrimaryAction<'_> {
        if let Some(options) = self
            .options
            .as_deref()
            .filter(|options| !options.is_empty())
        {
            return PrimaryAction::Options(options);
        }

        PrimaryAction::Accept {
            label: self.default_action_label(),
            destructive: self.is_destructive_action(),
        }
    }

    pub fn compact_title(&self) -> &str {
        self.title.as_str()
    }

    pub fn expanded_title(&self) -> &str {
        self.event_details
            .as_ref()
            .map(|details| details.what.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(self.title.as_str())
    }

    pub fn compact_message(&self, remaining: Option<Duration>) -> String {
        if self.start_time.is_some() {
            return match remaining {
                Some(value) if value.is_zero() => "Started".to_string(),
                Some(value) => compact_schedule_text(value),
                None => "Starting soon".to_string(),
            };
        }

        if self.shows_stop_countdown() {
            return stop_countdown_text(remaining.unwrap_or(Duration::ZERO));
        }

        self.message.clone()
    }
}

pub fn compact_schedule_text(remaining: Duration) -> String {
    let minutes = (remaining.as_secs_f64() / 60.0).ceil().max(1.0) as u64;
    if minutes == 1 {
        "Starting in 1 minute".to_string()
    } else {
        format!("Starting in {minutes} minutes")
    }
}

pub fn expanded_schedule_text(remaining: Duration) -> String {
    if remaining.is_zero() {
        return "Started".to_string();
    }

    let total_seconds = remaining.as_secs();
    format!("Begins in {}:{:02}", total_seconds / 60, total_seconds % 60)
}

pub fn stop_countdown_text(remaining: Duration) -> String {
    let seconds = remaining.as_secs_f64().ceil() as u64;
    format!("Anarlog will stop listening in {seconds} seconds.")
}

impl NotificationSource {
    pub fn default_icon(&self) -> Option<NotificationIcon> {
        match self {
            Self::CalendarEvent { .. } => Some(NotificationIcon::Overlay {
                base: NotificationIconAsset::AppIcon,
                badge: NotificationIconAsset::Calendar,
            }),
            Self::Session { .. } => None,
            Self::MicDetected { app_ids, .. } => app_ids
                .iter()
                .find_map(|app_id| NotificationIcon::from_app_id(app_id)),
        }
    }
}

impl NotificationIcon {
    pub fn from_app_id(app_id: &str) -> Option<Self> {
        if app_id.is_empty() || app_id.starts_with("pid:") {
            return None;
        }

        if app_id.starts_with('/') || app_id.starts_with("~/") {
            return Some(Self::Path {
                path: app_id.to_string(),
            });
        }

        Some(Self::BundleId {
            bundle_id: app_id.to_string(),
        })
    }

    pub fn system_symbol(name: impl Into<String>) -> Self {
        Self::SystemSymbol { name: name.into() }
    }
}

#[derive(Default)]
pub struct NotificationBuilder {
    key: Option<String>,
    title: Option<String>,
    message: Option<String>,
    timeout: Option<std::time::Duration>,
    source: Option<NotificationSource>,
    start_time: Option<i64>,
    participants: Option<Vec<Participant>>,
    event_details: Option<EventDetails>,
    action_label: Option<String>,
    action_variant: Option<NotificationActionVariant>,
    options: Option<Vec<String>>,
    footer: Option<NotificationFooter>,
    icon: Option<NotificationIcon>,
}

impl NotificationBuilder {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn source(mut self, source: NotificationSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn start_time(mut self, start_time: i64) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn participants(mut self, participants: Vec<Participant>) -> Self {
        self.participants = Some(participants);
        self
    }

    pub fn event_details(mut self, event_details: EventDetails) -> Self {
        self.event_details = Some(event_details);
        self
    }

    pub fn action_label(mut self, action_label: impl Into<String>) -> Self {
        self.action_label = Some(action_label.into());
        self
    }

    pub fn action_variant(mut self, action_variant: NotificationActionVariant) -> Self {
        self.action_variant = Some(action_variant);
        self
    }

    pub fn options(mut self, options: Vec<String>) -> Self {
        self.options = Some(options);
        self
    }

    pub fn footer(mut self, footer: NotificationFooter) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn icon(mut self, icon: NotificationIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn build(self) -> Notification {
        let source = self.source;
        let icon = self
            .icon
            .or_else(|| source.as_ref().and_then(NotificationSource::default_icon));

        Notification {
            key: self.key,
            title: self.title.unwrap(),
            message: self.message.unwrap(),
            timeout: self.timeout,
            source,
            start_time: self.start_time,
            participants: self.participants,
            event_details: self.event_details,
            action_label: self.action_label,
            action_variant: self.action_variant,
            options: self.options,
            footer: self.footer,
            icon,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_notifications_default_to_app_plus_calendar_overlay() {
        let source = NotificationSource::CalendarEvent {
            event_id: "evt-1".to_string(),
        };

        assert_eq!(
            source.default_icon(),
            Some(NotificationIcon::Overlay {
                base: NotificationIconAsset::AppIcon,
                badge: NotificationIconAsset::Calendar,
            })
        );
    }

    #[test]
    fn mic_notifications_default_to_first_resolvable_app_icon() {
        let source = NotificationSource::MicDetected {
            app_names: vec!["Zoom".to_string()],
            app_ids: vec!["pid:42".to_string(), "us.zoom.xos".to_string()],
            event_ids: vec![],
        };

        assert_eq!(
            source.default_icon(),
            Some(NotificationIcon::BundleId {
                bundle_id: "us.zoom.xos".to_string(),
            })
        );
    }

    #[test]
    fn system_symbol_icons_are_supported() {
        assert_eq!(
            NotificationIcon::system_symbol("phone.fill"),
            NotificationIcon::SystemSymbol {
                name: "phone.fill".to_string(),
            }
        );
    }

    #[test]
    fn notifications_fill_in_source_default_icon_when_missing() {
        let notification = Notification::builder()
            .title("Title")
            .message("Message")
            .source(NotificationSource::MicDetected {
                app_names: vec!["Zoom".to_string()],
                app_ids: vec!["/Applications/Zoom.app".to_string()],
                event_ids: vec![],
            })
            .build();

        assert_eq!(
            notification.icon,
            Some(NotificationIcon::Path {
                path: "/Applications/Zoom.app".to_string(),
            })
        );
    }

    #[test]
    fn notifications_keep_explicit_icon() {
        let notification = Notification::builder()
            .title("Title")
            .message("Message")
            .source(NotificationSource::CalendarEvent {
                event_id: "evt-1".to_string(),
            })
            .icon(NotificationIcon::Hidden)
            .build();

        assert_eq!(notification.icon, Some(NotificationIcon::Hidden));
    }

    #[test]
    fn notifications_preserve_footer() {
        let notification = Notification::builder()
            .title("Title")
            .message("")
            .footer(NotificationFooter {
                text: "Ignore this app?".to_string(),
                action_label: "YES".to_string(),
                icon: None,
            })
            .build();

        assert_eq!(
            notification
                .footer
                .as_ref()
                .map(|footer| footer.action_label.as_str()),
            Some("YES")
        );
    }

    #[test]
    fn calendar_events_are_not_expandable() {
        let notification = Notification::builder()
            .title("Standup")
            .message("Starting soon")
            .source(NotificationSource::CalendarEvent {
                event_id: "evt-1".to_string(),
            })
            .participants(vec![Participant {
                name: Some("Ada".to_string()),
                email: "ada@example.com".to_string(),
                status: ParticipantStatus::Accepted,
            }])
            .event_details(EventDetails {
                what: "Standup".to_string(),
                timezone: None,
                location: None,
            })
            .build();

        assert!(!notification.has_expandable_content());
    }

    #[test]
    fn session_notifications_expand_when_event_details_are_present() {
        let notification = Notification::builder()
            .title("Design sync")
            .message("")
            .source(NotificationSource::Session {
                session_id: "sess-1".to_string(),
            })
            .event_details(EventDetails {
                what: "Design sync".to_string(),
                timezone: Some("America/Los_Angeles".to_string()),
                location: Some("Zoom".to_string()),
            })
            .build();

        assert!(notification.has_expandable_content());
        assert_eq!(notification.expanded_title(), "Design sync");
    }

    #[test]
    fn options_override_the_accept_action() {
        let notification = Notification::builder()
            .title("Choose a meeting")
            .message("")
            .action_label("Ignored")
            .options(vec!["Design sync".to_string(), "Planning".to_string()])
            .build();

        assert_eq!(
            notification.primary_action(),
            PrimaryAction::Options(&["Design sync".to_string(), "Planning".to_string()])
        );
    }

    #[test]
    fn stop_countdown_copy_matches_macos() {
        let notification = Notification::builder()
            .title("Did your meeting end?")
            .message("Anarlog will stop listening soon.")
            .action_label("Stop")
            .action_variant(NotificationActionVariant::Destructive)
            .timeout(Duration::from_secs(30))
            .build();

        assert!(notification.shows_stop_countdown());
        assert_eq!(
            notification.compact_message(Some(Duration::from_secs_f64(4.2))),
            "Anarlog will stop listening in 5 seconds."
        );
        assert_eq!(
            compact_schedule_text(Duration::from_secs(90)),
            "Starting in 2 minutes"
        );
        assert_eq!(
            expanded_schedule_text(Duration::from_secs(75)),
            "Begins in 1:15"
        );
    }

    #[test]
    fn dismiss_timer_pauses_and_resumes_without_losing_progress() {
        let start = Instant::now();
        let mut timer = DismissTimer::at(Duration::from_secs(10), start);

        assert!((timer.progress_ratio(start) - 1.0).abs() < f64::EPSILON);

        let halfway = start + Duration::from_secs(4);
        timer.pause(halfway);
        assert!(!timer.is_running());
        assert_eq!(
            timer.remaining(halfway + Duration::from_secs(30)),
            Duration::from_secs(6)
        );

        let resumed = halfway + Duration::from_secs(8);
        timer.resume(resumed);
        assert_eq!(
            timer.remaining(resumed + Duration::from_secs(2)),
            Duration::from_secs(4)
        );
        assert!((timer.progress_ratio(resumed + Duration::from_secs(2)) - 0.4).abs() < 1e-9);
        assert!(timer.is_expired(resumed + Duration::from_secs(6)));
    }
}
