//! Meeting automation shared by the host: microphone meeting detection (`stt/detect-events.ts`),
//! the five-minute event reminders, the summary-ready and cloud-sync notifications, the
//! completion sound and the Dock bounce, and where each notification button leads.
//!
//! OS integrations sit behind [`Platform`]; nothing here runs until `main` installs
//! [`NativePlatform`], so tests and screenshots never post notifications or watch the mic.

pub mod apps;
pub mod detect;
pub mod notify;

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anlg_notification_interface::{
    Notification, NotificationAction, NotificationContext, NotificationSource,
};
use chrono::Utc;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use futures::{StreamExt, channel::mpsc};
use gpui::{
    App, AppContext, Context, EventEmitter, Global, Subscription, Task, WeakEntity, Window,
};
use serde_json::{Value, json};

use self::{
    apps::{MicApp, NearbyEvent},
    detect::Detection,
    notify::{Prefs, ReminderEvent, Reminders, Step},
};
use crate::{
    meeting::{MeetingServices, capture::Phase, live::LiveState},
    platform::notifications::{Action, NotificationAdapter, Notifications},
    product::settings::{self, Snapshot},
};

const REMINDER_INTERVAL: Duration = Duration::from_secs(30);
const SYNC_POLL: Duration = Duration::from_secs(2);

/// What the OS reports to the app.
#[derive(Clone, Debug)]
pub enum Signal {
    MicStarted(Vec<MicApp>),
    MicStopped(Vec<MicApp>),
    Sleep(bool),
    Notification(String, Action),
}

pub type Sink = Arc<dyn Fn(Signal) + Send + Sync>;

pub trait Platform: Send + Sync {
    /// Starts microphone, sleep and notification-button callbacks; called once.
    fn start(&self, sink: Sink);
    fn show(&self, notification: &Notification) -> Result<()>;
    fn clear(&self);
    fn do_not_disturb(&self) -> bool;
    fn play(&self, sound: &'static str);
    /// Shipping's informational `requestUserAttention` (a single Dock bounce).
    fn request_attention(&self);
    fn open_url(&self, url: &str) -> Result<()>;
    /// Applications the user can exclude from detection.
    fn installed_apps(&self) -> Vec<MicApp>;
}

pub struct Installed(pub Arc<dyn Platform>);
impl Global for Installed {}

struct Handle(WeakEntity<MeetingAutomation>);
impl Global for Handle {}

struct Shown(Arc<dyn Platform>);

impl NotificationAdapter for Shown {
    fn show(&mut self, notification: &Notification) -> Result<()> {
        self.0.show(notification)
    }
}

/// Raised for automation that reacts to calls and notifications (auto-stop, scheduling).
#[derive(Clone, Debug)]
pub enum Observed {
    /// An app used the microphone for the detection delay (shipping's `micDetected`).
    MicDetected {
        key: String,
        apps: Vec<MicApp>,
    },
    MicStopped(Vec<MicApp>),
    Sleep(bool),
    /// Every notification button, with the context of notifications posted through `show`.
    Notification {
        key: String,
        action: Action,
        context: Option<NotificationContext>,
    },
}

/// What the host does for automation.
#[derive(Clone, Debug)]
pub enum Request {
    ShowMain,
    /// Opens the note, starting to listen when `listen` holds.
    OpenSession {
        session: SessionId,
        listen: bool,
    },
    /// A new note that starts listening.
    NewMeeting,
    OpenSettings(&'static str),
    Stop,
}

pub struct MeetingAutomation {
    runtime: RuntimeHandle,
    platform: Option<Arc<dyn Platform>>,
    notifications: Option<Notifications<Shown>>,
    preferences: Option<Arc<Snapshot>>,
    detection: Detection,
    detection_timer: Option<Task<()>>,
    reminders: Reminders,
    prompting: bool,
    triggers: Vec<String>,
    live: bool,
    window_active: bool,
    viewer: Option<Arc<str>>,
    cloudsync: Option<(Arc<str>, Task<()>)>,
    profile: Option<PathBuf>,
    installed: Option<Arc<[MicApp]>>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Observed> for MeetingAutomation {}
impl EventEmitter<Request> for MeetingAutomation {}

impl MeetingAutomation {
    pub fn new(
        runtime: RuntimeHandle,
        profile: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let platform = cx
            .try_global::<Installed>()
            .map(|installed| installed.0.clone());
        let handle = Handle(cx.entity().downgrade());
        cx.set_global(handle);
        let mut subscriptions = vec![cx.observe_window_activation(window, |this, window, _| {
            this.window_active = window.is_window_active();
        })];
        if let Some(live) = LiveState::global(cx) {
            subscriptions.push(cx.observe(&live, |this, live, cx| {
                let live = live.read(cx).current().is_some_and(|live| live.live());
                if live && !this.live {
                    // A capture that starts answers every pending prompt.
                    if let Some(platform) = &this.platform {
                        platform.clear();
                    }
                } else if !live && this.live {
                    this.triggers.clear();
                }
                this.live = live;
            }));
        }
        let mut tasks = Vec::new();
        if let Some(platform) = &platform {
            let (send, mut signals) = mpsc::unbounded();
            platform.start(Arc::new(move |signal| {
                let _ = send.unbounded_send(signal);
            }));
            tasks.push(cx.spawn(async move |this, cx| {
                while let Some(signal) = signals.next().await {
                    if this.update(cx, |this, cx| this.signal(signal, cx)).is_err() {
                        break;
                    }
                }
            }));
            tasks.push(cx.spawn(async move |this, cx| {
                loop {
                    let reminders = this.update(cx, |this, _| this.reminder_query());
                    match reminders {
                        Ok(Some(reply)) => {
                            let events = async { reply?.receive().await }.await;
                            let _ = this.update(cx, |this, cx| match events {
                                Ok(events) => this.remind(&events, cx),
                                Err(error) => tracing::warn!(%error, "event reminders failed"),
                            });
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                    cx.background_executor().timer(REMINDER_INTERVAL).await;
                }
            }));
        }
        Self {
            runtime,
            notifications: platform
                .clone()
                .map(|platform| Notifications::new(Shown(platform))),
            platform,
            preferences: None,
            detection: Detection::default(),
            detection_timer: None,
            reminders: Reminders::default(),
            prompting: false,
            triggers: Vec::new(),
            live: false,
            window_active: window.is_window_active(),
            viewer: None,
            cloudsync: None,
            profile,
            installed: None,
            _tasks: tasks,
            _subscriptions: subscriptions,
        }
    }

    pub fn global(cx: &App) -> Option<gpui::Entity<Self>> {
        cx.try_global::<Handle>()?.0.upgrade()
    }

    pub fn set_preferences(&mut self, snapshot: Arc<Snapshot>) {
        self.preferences = Some(snapshot);
    }

    pub fn prefs(&self) -> Prefs<'_> {
        Prefs(self.preferences.as_deref())
    }

    /// Apps whose call the current capture belongs to (shipping's `triggerAppIds`).
    pub fn trigger_apps(&self) -> &[String] {
        &self.triggers
    }

    fn notification_source_kind(source: Option<&NotificationSource>) -> &'static str {
        match source {
            Some(NotificationSource::CalendarEvent { .. }) => "calendar_event",
            Some(NotificationSource::Session { .. }) => "session",
            Some(NotificationSource::MicDetected { .. }) => "mic_detected",
            None => "unknown",
        }
    }

    /// Posts a notification; its buttons come back as [`Observed::Notification`].
    /// `Ok(false)` means the same key was shown within the last minute.
    pub fn show(&mut self, notification: Notification) -> Result<bool> {
        let source_type = Self::notification_source_kind(notification.source.as_ref());
        let is_persistent = notification.is_persistent();
        let has_options = notification
            .options
            .as_ref()
            .is_some_and(|options| !options.is_empty());
        let notifications = self
            .notifications
            .as_mut()
            .ok_or_else(|| ServiceError::Unsupported("Notifications are not installed".into()))?;
        let shown = notifications.show(notification, Instant::now())?;
        if shown {
            crate::telemetry::track(
                "notification_shown",
                &[
                    ("source_type", json!(source_type)),
                    ("is_persistent", json!(is_persistent)),
                    ("has_options", json!(has_options)),
                ],
            );
        }
        Ok(shown)
    }

    pub fn clear_notifications(&self) {
        if let Some(platform) = &self.platform {
            platform.clear();
        }
    }

    pub fn play_completion_sound(&self) {
        if let (Some(platform), Some(sound)) = (&self.platform, self.prefs().completion_sound()) {
            platform.play(sound);
        }
    }

    fn post(&mut self, notification: Notification) {
        if let Err(error) = self.show(notification) {
            tracing::warn!(%error, "notification not shown");
        }
    }

    fn signal(&mut self, signal: Signal, cx: &mut Context<Self>) {
        match signal {
            Signal::MicStarted(apps) => {
                let policy = self.prefs().detection();
                self.detection.mic_started(&apps, &policy, Instant::now());
                self.schedule_detection(cx);
            }
            Signal::MicStopped(apps) => {
                self.detection.mic_stopped(&apps);
                self.schedule_detection(cx);
                cx.emit(Observed::MicStopped(apps));
            }
            Signal::Sleep(asleep) => {
                cx.emit(Observed::Sleep(asleep));
                if asleep && self.capturing(cx) {
                    cx.emit(Request::Stop);
                }
            }
            Signal::Notification(key, action) => self.notification_action(key, action, cx),
        }
    }

    fn schedule_detection(&mut self, cx: &mut Context<Self>) {
        let Some(deadline) = self.detection.next_deadline() else {
            self.detection_timer = None;
            return;
        };
        let platform = self.platform.clone();
        self.detection_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(deadline.saturating_duration_since(Instant::now()))
                .await;
            let respect = this
                .update(cx, |this, _| this.prefs().detection().respect_dnd)
                .unwrap_or(false);
            let dnd = match platform.filter(|_| respect) {
                Some(platform) => {
                    cx.background_spawn(async move { platform.do_not_disturb() })
                        .await
                }
                None => false,
            };
            let _ = this.update(cx, |this, cx| {
                let policy = this.prefs().detection();
                for app in this.detection.due(Instant::now(), &policy, || dnd) {
                    this.mic_detected(uuid::Uuid::new_v4().to_string(), vec![app], cx);
                }
                this.schedule_detection(cx);
            });
        }));
    }

    fn capture(cx: &App) -> Option<(Phase, Option<SessionId>)> {
        let update = cx
            .try_global::<MeetingServices>()?
            .capture
            .take_update(u64::MAX);
        Some((update.phase, update.session))
    }

    /// Listening, or starting for a note (shipping's active or loading-with-session state).
    fn capturing(&self, cx: &App) -> bool {
        matches!(
            Self::capture(cx),
            Some((Phase::Listening, _)) | Some((Phase::Loading, Some(_)))
        )
    }

    fn capture_triggers(&mut self, apps: &[MicApp], cx: &App) {
        if apps.is_empty() {
            return;
        }
        for app in apps {
            if !self.triggers.contains(&app.id) {
                self.triggers.push(app.id.clone());
            }
        }
        if let Some((_, Some(session))) = Self::capture(cx)
            && let Err(error) = apps::record_source_apps(&self.runtime, session, apps.to_vec())
        {
            tracing::warn!(%error, "detected meeting apps not recorded");
        }
    }

    fn mic_detected(&mut self, key: String, detected: Vec<MicApp>, cx: &mut Context<Self>) {
        cx.emit(Observed::MicDetected {
            key: key.clone(),
            apps: detected.clone(),
        });
        let ignorable = apps::ignorable(&detected);
        if self.capturing(cx) {
            self.capture_triggers(&ignorable, cx);
            return;
        }
        if !self.prefs().allows("notification_detect") || self.prompting {
            return;
        }
        self.prompting = true;
        let nearby = nearby_event(&self.runtime);
        cx.spawn(async move |this, cx| {
            let nearby = match async { nearby?.receive().await }.await {
                Ok(nearby) => nearby,
                Err(error) => {
                    tracing::warn!(%error, "nearby events unavailable");
                    None
                }
            };
            let notification = cx
                .background_spawn(async move {
                    notify::mic_detected(
                        key,
                        &detected,
                        nearby.as_ref(),
                        apps::notification_icons().as_deref(),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.prompting = false;
                if this.capturing(cx) {
                    this.capture_triggers(&ignorable, cx);
                } else {
                    this.post(notification);
                }
            });
        })
        .detach();
    }

    fn reminder_query(&self) -> Option<Result<desktop_runtime::Reply<Vec<ReminderEvent>>>> {
        self.prefs()
            .allows("notification_event")
            .then(|| reminder_events(&self.runtime))
    }

    fn remind(&mut self, events: &[ReminderEvent], _: &mut Context<Self>) {
        if !self.prefs().allows("notification_event") {
            return;
        }
        for notification in self.reminders.due(events, Utc::now()) {
            self.post(notification);
        }
    }

    fn notification_action(&mut self, key: String, action: Action, cx: &mut Context<Self>) {
        let context = self
            .notifications
            .as_mut()
            .and_then(|notifications| notifications.action(&key, action, Instant::now()).ok());
        if let Some(ctx) = &context {
            let source_type = Self::notification_source_kind(ctx.source.as_ref());
            let action_name = |chosen: NotificationAction| match chosen {
                NotificationAction::JoinAndRecord => "join_and_record",
                NotificationAction::OpenMeeting => "open_meeting",
            };
            match (action, ctx.action) {
                (Action::Confirm, _) => {
                    crate::telemetry::track("collapsed_confirm", &[]);
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!("confirm")),
                        ],
                    );
                }
                (Action::Accept, Some(chosen)) => {
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!(action_name(chosen))),
                        ],
                    );
                }
                (Action::Accept, None) => {
                    crate::telemetry::track("expanded_accept", &[]);
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!("accept")),
                        ],
                    );
                }
                (Action::Option(_), Some(chosen)) => {
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!(action_name(chosen))),
                        ],
                    );
                }
                (Action::Option(option_index), None) => {
                    crate::telemetry::track("option_selected", &[]);
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!("option_selected")),
                            ("option_index", json!(option_index)),
                        ],
                    );
                }
                (Action::Dismiss, _) => {
                    crate::telemetry::track("dismiss", &[]);
                    crate::telemetry::track(
                        "notification_dismissed",
                        &[("source_type", json!(source_type))],
                    );
                }
                (Action::Timeout, _) => {
                    crate::telemetry::track("collapsed_timeout", &[]);
                    crate::telemetry::track(
                        "notification_timed_out",
                        &[("source_type", json!(source_type))],
                    );
                }
                (Action::Footer, _) => {
                    crate::telemetry::track("footer_action", &[]);
                    crate::telemetry::track(
                        "notification_actioned",
                        &[
                            ("source_type", json!(source_type)),
                            ("action", json!("footer_action")),
                        ],
                    );
                }
            }
        }
        cx.emit(Observed::Notification {
            key: key.clone(),
            action,
            context: context.clone(),
        });
        let context = context.unwrap_or(NotificationContext {
            key,
            source: None,
            action: None,
        });
        let (show_main, step) = notify::respond(action, &context);
        if show_main {
            cx.emit(Request::ShowMain);
        }
        match step {
            Step::Nothing => {}
            Step::OpenSession(session) => cx.emit(Request::OpenSession {
                session,
                listen: false,
            }),
            Step::TeamSettings => cx.emit(Request::OpenSettings("team")),
            Step::Ignore(ids) => self.ignore_apps(ids, cx),
            Step::OpenMeeting(event) => self.open_meeting(event, cx),
            // The meeting opens last so focus returns to the meeting client.
            Step::JoinAndRecord(event) => {
                self.listen(Some(event), Vec::new(), Some(true), true, cx)
            }
            Step::Listen {
                event,
                triggers,
                listen,
            } => self.listen(event, triggers, listen, false, cx),
        }
    }

    fn listen(
        &mut self,
        event: Option<String>,
        triggers: Vec<String>,
        listen: Option<bool>,
        join: bool,
        cx: &mut Context<Self>,
    ) {
        if !triggers.is_empty() {
            self.triggers = triggers;
        }
        let Some(event) = event else {
            cx.emit(Request::NewMeeting);
            return;
        };
        let runtime = self.runtime.clone();
        let viewer = self.viewer.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                let session = crate::workspace::calendar::open_event(
                    &runtime,
                    event.as_str().into(),
                    viewer,
                )?
                .receive()
                .await?;
                let listen = match listen {
                    Some(listen) => listen,
                    None => event_started(&runtime, event.clone())?.receive().await?,
                };
                Ok::<_, ServiceError>((session, listen))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((session, listen)) => cx.emit(Request::OpenSession { session, listen }),
                    Err(error) => tracing::warn!(%error, "notification note not opened"),
                }
                if join {
                    this.open_meeting(event, cx);
                }
            });
        })
        .detach();
    }

    fn open_meeting(&self, event: String, cx: &mut Context<Self>) {
        let Some(platform) = self.platform.clone() else {
            return;
        };
        let reply = meeting_link(&self.runtime, event);
        cx.background_spawn(async move {
            let result = async {
                if let Some(link) = reply?.receive().await? {
                    platform.open_url(&link)?;
                }
                Ok::<_, ServiceError>(())
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, "meeting link not opened");
            }
        })
        .detach();
    }

    fn ignore_apps(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let Some(base) = self
            .preferences
            .as_ref()
            .and_then(|snapshot| snapshot.get("ignored_platforms").cloned())
        else {
            return;
        };
        let mut ignored = self.prefs().ignored_apps();
        let before = ignored.len();
        for id in ids {
            if !ignored.contains(&id) {
                ignored.push(id);
            }
        }
        if ignored.len() == before {
            return;
        }
        let reply = settings::save(
            &self.runtime,
            "ignored_platforms".into(),
            base,
            json!(json!(ignored).to_string()),
        );
        cx.background_spawn(async move {
            if let Err(error) = async { reply?.receive().await }.await {
                tracing::warn!(%error, "ignored apps not saved");
            }
        })
        .detach();
    }

    /// Shipping's `enhanceSuccess` tail: the summary-ready notification while the app is in
    /// the background, the completion sound and the Dock bounce.
    pub fn summary_ready(&mut self, session: &SessionId, title: &str) {
        let prefs = self.prefs();
        let background = !self.window_active;
        let notification = (background && prefs.allows("notification_summary_complete"))
            .then(|| notify::summary_ready(session, title, &uuid::Uuid::new_v4().to_string()));
        let bounce = background && prefs.bounce();
        if let Some(notification) = notification {
            self.post(notification);
        }
        self.play_completion_sound();
        if let Some(platform) = self.platform.as_ref().filter(|_| bounce) {
            platform.request_attention();
        }
    }

    /// The team invitation's background notification (`settings/team/invitation-toast.tsx`).
    pub fn team_invitation(&mut self, id: &str, workspace: &str, invited_by: Option<&str>) {
        if self.window_active || self.prefs().flag("notification_disabled") {
            return;
        }
        self.post(notify::team_invitation(id, workspace, invited_by));
    }

    /// Follows the signed-in account: event notes open in its workspace, and the first
    /// completed cloud sync is announced once per account (`auth/cloudsync-progress.ts`).
    pub fn set_account(
        &mut self,
        account: Option<Arc<str>>,
        status: Option<Arc<dyn Fn() -> Option<u64> + Send + Sync>>,
        cx: &mut Context<Self>,
    ) {
        self.viewer = account.clone();
        let (Some(account), Some(status), Some(profile), Some(_)) =
            (account, status, self.profile.clone(), &self.platform)
        else {
            self.cloudsync = None;
            return;
        };
        if self
            .cloudsync
            .as_ref()
            .is_some_and(|(user, _)| *user == account)
            || cloudsync_announced(&profile, &account)
        {
            return;
        }
        let user = account.clone();
        let task = cx.spawn(async move |this, cx| {
            loop {
                if status().is_some() {
                    let _ = this.update(cx, |this, _| this.cloudsync_complete(&profile, &user));
                    break;
                }
                cx.background_executor().timer(SYNC_POLL).await;
            }
        });
        self.cloudsync = Some((account, task));
    }

    fn cloudsync_complete(&mut self, profile: &std::path::Path, user: &str) {
        if let Err(error) = mark_cloudsync_announced(profile, user) {
            // The notification may repeat after restart.
            tracing::warn!(%error, "initial cloud sync completion not recorded");
        }
        self.play_completion_sound();
        if self.prefs().allows("notification_cloudsync_complete") {
            self.post(notify::cloudsync_complete(user));
        }
    }
}

/// Called when a summary finishes saving, from wherever it was generated.
pub fn summary_ready(session: &SessionId, title: &str, cx: &mut App) {
    if let Some(automation) = MeetingAutomation::global(cx) {
        automation.update(cx, |automation, _| automation.summary_ready(session, title));
    }
}

/// Installed applications for the excluded-apps picker, listed once in the background; windows
/// refresh when the list arrives.
pub fn installed_apps(cx: &mut App) -> Arc<[MicApp]> {
    let Some(automation) = MeetingAutomation::global(cx) else {
        return Arc::from([]);
    };
    automation.update(cx, |automation, cx| {
        if let Some(installed) = &automation.installed {
            return installed.clone();
        }
        let empty: Arc<[MicApp]> = Arc::from([]);
        automation.installed = Some(empty.clone());
        if let Some(platform) = automation.platform.clone() {
            cx.spawn(async move |this, cx| {
                let apps = cx
                    .background_spawn(async move { platform.installed_apps() })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.installed = Some(apps.into());
                    cx.refresh_windows();
                });
            })
            .detach();
        }
        empty
    })
}

/// A capture finished saving its transcript.
pub fn capture_completed(cx: &mut App) {
    if let Some(automation) = MeetingAutomation::global(cx) {
        automation.read(cx).play_completion_sound();
    }
}

fn cloudsync_marker(profile: &std::path::Path) -> PathBuf {
    profile.join("cloudsync-initial-sync-completed.json")
}

pub(crate) fn cloudsync_announced(profile: &std::path::Path, user: &str) -> bool {
    std::fs::read(cloudsync_marker(profile))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .is_some_and(|users| users.iter().any(|known| known == user))
}

pub(crate) fn mark_cloudsync_announced(
    profile: &std::path::Path,
    user: &str,
) -> std::io::Result<()> {
    let path = cloudsync_marker(profile);
    let mut users: Vec<String> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if !users.iter().any(|known| known == user) {
        users.push(user.to_owned());
    }
    std::fs::write(path, serde_json::to_vec(&users)?)
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Calendar events hidden with "Ignore this event" or "Ignore series" (shipping skips them
/// only when the event carries a tracking id).
const NOT_IGNORED: &str = "NOT (e.tracking_id_event <> '' AND EXISTS(
        SELECT 1 FROM app_settings a,
            json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
        WHERE (a.id = 'ignored_events' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
            OR (a.id = 'ignored_recurring_series' AND e.recurrence_series_id <> '' AND json_extract(ignored.value,'$.id') = e.recurrence_series_id)))";

fn reminder_events(runtime: &RuntimeHandle) -> Result<desktop_runtime::Reply<Vec<ReminderEvent>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                format!(
                    "SELECT e.id, substr(e.title,1,4096) AS title, e.started_at, substr(e.meeting_link,1,4096) AS meeting_link
                     FROM events e
                     WHERE e.deleted_at IS NULL AND e.started_at <> '' AND e.is_all_day = 0
                        AND julianday(e.started_at) BETWEEN julianday('now','-1 day') AND julianday('now','+1 day')
                        AND {NOT_IGNORED}
                     ORDER BY e.started_at, e.id LIMIT 512"
                ),
                vec![],
            )
            .await
            .map_err(failure)?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                Some(ReminderEvent {
                    id: text(row, "id"),
                    title: text(row, "title"),
                    start: crate::workspace::calendar::parse_instant(&text(row, "started_at"))?,
                    meeting_link: text(row, "meeting_link"),
                })
            })
            .collect())
    })
}

/// Shipping's `getNearbyCalendarEvents(now, 15 minutes)`, closest first.
fn nearby_event(runtime: &RuntimeHandle) -> Result<desktop_runtime::Reply<Option<NearbyEvent>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let now = Utc::now().timestamp_millis();
        let rows = services
            .executor
            .execute(
                "SELECT id, substr(title,1,4096) AS title, substr(meeting_link,1,4096) AS meeting_link,
                    substr(location,1,4096) AS location, substr(description,1,16384) AS description,
                    substr(participants_json,1,65536) AS participants_json
                 FROM events
                 WHERE deleted_at IS NULL AND is_all_day = 0 AND started_at <> ''
                    AND abs(((julianday(started_at) - 2440587.5) * 86400000) - ?1) <= 900000
                 ORDER BY abs(((julianday(started_at) - 2440587.5) * 86400000) - ?1), julianday(started_at), id
                 LIMIT 1"
                    .into(),
                vec![json!(now)],
            )
            .await
            .map_err(failure)?;
        Ok(rows.first().map(|row| {
            let participants: Vec<Value> =
                serde_json::from_str(&text(row, "participants_json")).unwrap_or_default();
            let mut names = Vec::new();
            for participant in participants {
                if participant["is_current_user"] == true {
                    continue;
                }
                let name = participant["name"].as_str().unwrap_or_default().trim();
                if !name.is_empty() && !names.iter().any(|known| known == name) {
                    names.push(name.to_owned());
                }
            }
            let title = text(row, "title");
            NearbyEvent {
                id: text(row, "id"),
                title: if title.is_empty() {
                    "Untitled Event".into()
                } else {
                    title
                },
                meeting_link: text(row, "meeting_link"),
                location: text(row, "location"),
                description: text(row, "description"),
                participant_names: names,
            }
        }))
    })
}

fn event_started(runtime: &RuntimeHandle, event: String) -> Result<desktop_runtime::Reply<bool>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT started_at FROM events WHERE id = ? AND deleted_at IS NULL".into(),
                vec![json!(event)],
            )
            .await
            .map_err(failure)?;
        Ok(rows
            .first()
            .and_then(|row| crate::workspace::calendar::parse_instant(&text(row, "started_at")))
            .is_some_and(|start| start <= Utc::now()))
    })
}

fn meeting_link(
    runtime: &RuntimeHandle,
    event: String,
) -> Result<desktop_runtime::Reply<Option<String>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT meeting_link FROM events WHERE id = ? AND deleted_at IS NULL".into(),
                vec![json!(event)],
            )
            .await
            .map_err(failure)?;
        Ok(rows
            .first()
            .map(|row| text(row, "meeting_link").trim().to_owned())
            .filter(|link| !link.is_empty()))
    })
}

/// The installed OS integrations; scheduling posts its notices through them too.
pub struct NativePlatform;

impl NativePlatform {
    pub fn install(cx: &mut App) {
        cx.set_global(Installed(Arc::new(Self)));
        cx.set_global(crate::meeting::schedule::InstalledNotices(Arc::new(
            AutomationNotices,
        )));
    }
}

/// Scheduling's `Notices` over this module: its trigger apps and notifications, whose buttons
/// come back as [`Observed::Notification`].
struct AutomationNotices;

impl crate::meeting::schedule::Notices for AutomationNotices {
    fn trigger_apps(&self, cx: &App) -> Vec<String> {
        MeetingAutomation::global(cx)
            .map(|automation| automation.read(cx).trigger_apps().to_vec())
            .unwrap_or_default()
    }

    fn show(&self, notification: Notification, cx: &mut App) -> Result<bool> {
        MeetingAutomation::global(cx)
            .ok_or_else(|| ServiceError::Unsupported("Notifications are not installed".into()))?
            .update(cx, |automation, _| automation.show(notification))
    }
}

impl Platform for NativePlatform {
    fn start(&self, sink: Sink) {
        let actions = sink.clone();
        crate::platform::notifications::NativeNotifications::install_actions(Arc::new(
            move |key, action| actions(Signal::Notification(key, action)),
        ));
        let own_pid = format!("pid:{}", std::process::id());
        let own_exe = std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let convert = move |apps: Vec<anlg_detect::InstalledApp>| -> Vec<MicApp> {
            apps.into_iter()
                .filter(|app| app.id != own_pid && Some(&app.id) != own_exe.as_ref())
                .map(|app| MicApp::new(app.id, app.name))
                .collect()
        };
        let mut detector = anlg_detect::Detector::default();
        detector.start(anlg_detect::new_callback(move |event| {
            let signal = match event {
                anlg_detect::DetectEvent::MicStarted(apps) => Signal::MicStarted(convert(apps)),
                anlg_detect::DetectEvent::MicStopped(apps) => Signal::MicStopped(convert(apps)),
                #[cfg(target_os = "macos")]
                anlg_detect::DetectEvent::SleepStateChanged { value } => Signal::Sleep(value),
            };
            sink(signal);
        }));
        // Runs for the app's lifetime.
        std::mem::forget(detector);
    }

    fn show(&self, notification: &Notification) -> Result<()> {
        crate::platform::notifications::NativeNotifications.show(notification)
    }

    fn clear(&self) {
        crate::platform::notifications::dismiss_all();
    }

    fn do_not_disturb(&self) -> bool {
        crate::platform::notifications::do_not_disturb()
    }

    fn play(&self, sound: &'static str) {
        crate::platform::sounds::play_completion(sound);
    }

    fn request_attention(&self) {
        crate::platform::notifications::request_attention();
    }

    fn open_url(&self, url: &str) -> Result<()> {
        crate::platform::os::open(url).map_err(failure)
    }

    fn installed_apps(&self) -> Vec<MicApp> {
        anlg_detect::list_installed_apps()
            .into_iter()
            .map(|app| MicApp::new(app.id, app.name))
            .collect()
    }
}
