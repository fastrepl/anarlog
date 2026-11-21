//! Shipping's app lifecycle (`plugins/windows` close→hide, `plugins/tray`, `crates/intercept`):
//! closing the main window hides it and the app keeps running; only a confirmed
//! "Quit Completely…" or a second ⌘Q quits, always through [`ApplicationView::request_quit`].

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use desktop_runtime::{
    CancellationToken, ServiceError, SessionId,
    updater::{InstallConditions, NativeUpdater, ResolvedRelease, VerifiedUpdate},
};
use gpui::{App, AsyncApp, ClipboardItem, Context, Task, WeakEntity, Window};
use serde_json::{Value, json};

#[cfg(target_os = "macos")]
mod quit;
#[cfg(target_os = "macos")]
pub use quit::{QUIT_OVERLAY_DURATION, QuitKeys, QuitOverlay, QuitSignal};

use super::ApplicationView;
use crate::{
    contracts::{MeetingIntent, ProductRoute},
    meeting::capture::Phase,
    platform::{
        tray::{ScheduleEvent, TrayAction, TrayModel, TrayState, UpdateMenu},
        updater::NativeInstaller,
        windows::Restart,
    },
    product::settings,
};

const SCHEDULE_HORIZON_DAYS: i64 = 7;
/// Shipping's `UPDATE_CHECK_INTERVAL_MS`.
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);
const UNTITLED: [&str; 4] = [
    "untitled",
    "untitled event",
    "untitled meeting",
    "untitled note",
];

/// A native alert like shipping's `tauri-plugin-dialog` message (also rfd's NSAlert).
/// Returns whether the first button was chosen.
async fn alert(title: &str, message: &str, buttons: Option<(&str, &str)>) -> bool {
    if crate::platform::os::headless() {
        return false;
    }
    let buttons = match buttons {
        Some((ok, cancel)) => rfd::MessageButtons::OkCancelCustom(ok.into(), cancel.into()),
        None => rfd::MessageButtons::Ok,
    };
    let ok = match &buttons {
        rfd::MessageButtons::OkCancelCustom(ok, _) => Some(ok.clone()),
        _ => None,
    };
    let result = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(title)
        .set_description(message)
        .set_buttons(buttons)
        .show()
        .await;
    match result {
        rfd::MessageDialogResult::Ok => true,
        rfd::MessageDialogResult::Custom(label) => Some(label) == ok,
        _ => false,
    }
}

fn notify(title: &'static str, message: String, cx: &mut App) {
    cx.foreground_executor()
        .spawn(async move {
            alert(title, &message, None).await;
        })
        .detach();
}

/// Shipping's feed for this build's channel and package (`platform/updater.rs`), in a channel
/// run.
fn updater(cx: &App) -> desktop_runtime::Result<Arc<NativeUpdater>> {
    let channel_run = crate::shipping_store::ShippingStore::get(cx).is_some();
    NativeUpdater::new(crate::platform::updater::config(channel_run)?).map(Arc::new)
}

/// Who asked to install: the tray's dialogs, the update toast, or automatic updates.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Tray,
    Toast,
    Automatic,
}

/// "Automatically install updates" once preferences have loaded, which happens right after
/// startup; preferences that never load leave the feed to manual updates.
async fn automatic_updates(this: &WeakEntity<ApplicationView>, cx: &mut AsyncApp) -> Option<bool> {
    for _ in 0..30 {
        let automatic = this
            .read_with(cx, |this, _| this.notifications.automatic_updates())
            .ok()?;
        if automatic.is_some() {
            return automatic;
        }
        cx.background_executor().timer(Duration::from_secs(1)).await;
    }
    Some(false)
}

/// One pass of shipping's `plugins/updater2` loop, taking and returning its install-at-open
/// intent; `None` once the app is gone. Every pass checks the feed, which also withdraws a
/// downloaded update the feed no longer offers. With automatic updates on, a pass outside a
/// meeting also downloads in the background, and the startup pass installs as soon as its
/// download completes. A meeting, a deferral or a failed install keeps the intent for the next
/// pass; finding nothing or turning automatic updates off drops it.
async fn update_pass(
    this: &WeakEntity<ApplicationView>,
    updater: &Arc<NativeUpdater>,
    install_at_open: bool,
    cx: &mut AsyncApp,
) -> Option<bool> {
    let automatic = automatic_updates(this, cx).await?;
    let check = this
        .update(cx, |this, cx| {
            let busy = matches!(this.lifecycle.update, UpdateMenu::Downloading(..));
            (!busy && !this.lifecycle.checking && !this.lifecycle.installing && !this.closing)
                .then(|| this.check_feed(updater.clone(), cx))
        })
        .ok()?;
    if let Some(check) = check {
        match check.await {
            Ok(Some(_)) => {}
            Ok(None) => return Some(false),
            Err(error) => {
                tracing::warn!(%error, "update check failed");
                return Some(install_at_open);
            }
        }
    }
    if !automatic {
        return Some(false);
    }
    // Never download or install (which restarts the app) while a meeting is captured.
    let download = this
        .update(cx, |this, cx| {
            (!this.meeting_live()).then(|| this.start_download(false, cx))
        })
        .ok()?;
    let Some(download) = download else {
        return Some(install_at_open);
    };
    if let Some(download) = download
        && !download.await
    {
        return Some(install_at_open);
    }
    if !install_at_open {
        return Some(false);
    }
    let install = this
        .update(cx, |this, cx| this.install_update(Source::Automatic, cx))
        .ok()?;
    match install {
        // A successful install quits into the new version.
        Some(install) => Some(!install.await),
        None => Some(true),
    }
}

/// Timed calendar events for the tray, like shipping's `TrayScheduleSync` (all-day and ignored
/// events excluded).
const SCHEDULE_QUERY: &str = "SELECT e.id, substr(e.title,1,4096) AS title, e.meeting_link,
        e.started_at, e.ended_at, e.is_all_day
    FROM events e WHERE e.deleted_at IS NULL
    AND NOT EXISTS(
        SELECT 1 FROM app_settings a,
            json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
        WHERE e.tracking_id_event <> '' AND ignored.type = 'object' AND (
            (a.id = 'ignored_events' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
            OR (a.id = 'ignored_recurring_series' AND e.recurrence_series_id <> ''
                AND json_extract(ignored.value,'$.id') = e.recurrence_series_id)))
    AND julianday(COALESCE(NULLIF(e.ended_at,''), e.started_at)) > julianday('now')
    AND julianday(e.started_at) < julianday('now','+7 days')
    ORDER BY e.started_at, e.id LIMIT 200";

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn local_time(date: DateTime<Utc>, timezone: Option<Tz>) -> DateTime<FixedOffset> {
    match timezone {
        Some(timezone) => date.with_timezone(&timezone).fixed_offset(),
        None => date.with_timezone(&Local).fixed_offset(),
    }
}

fn midnight(date: NaiveDate, timezone: Option<Tz>) -> Option<f64> {
    let naive = date.and_hms_opt(0, 0, 0)?;
    let start = match timezone {
        Some(timezone) => timezone
            .from_local_datetime(&naive)
            .earliest()?
            .with_timezone(&Utc),
        None => Local
            .from_local_datetime(&naive)
            .earliest()?
            .with_timezone(&Utc),
    };
    Some(start.timestamp_millis() as f64)
}

pub fn build_schedule(
    rows: &[Value],
    now: DateTime<Utc>,
    timezone: Option<Tz>,
    use_24_hour_time: bool,
) -> Arc<[ScheduleEvent]> {
    let format = if use_24_hour_time {
        "%H:%M"
    } else {
        "%-I:%M %p"
    };
    let time = |at: DateTime<Utc>| local_time(at, timezone).format(format).to_string();
    let horizon = now + chrono::Duration::days(SCHEDULE_HORIZON_DAYS);
    let mut events = rows
        .iter()
        .filter(|row| row["is_all_day"].as_i64() != Some(1))
        .filter_map(|row| {
            let text = |key: &str| row[key].as_str().unwrap_or_default();
            let start = parse_time(text("started_at"))?;
            let end = parse_time(text("ended_at")).filter(|end| *end > start);
            if end.map_or(start <= now, |end| end <= now) || start > horizon {
                return None;
            }
            let day = local_time(start, timezone).date_naive();
            let title = text("title").trim();
            Some(ScheduleEvent {
                id: text("id").into(),
                title: if title.is_empty() {
                    "Untitled event".into()
                } else {
                    title.into()
                },
                meeting_link: Some(text("meeting_link"))
                    .filter(|link| !link.is_empty())
                    .map(Into::into),
                starts_at_ms: start.timestamp_millis() as f64,
                ends_at_ms: end.map(|end| end.timestamp_millis() as f64),
                day_start_ms: midnight(day, timezone)?,
                previous_day_start_ms: midnight(day.pred_opt()?, timezone)?,
                time_label: match end {
                    Some(end) => format!("{} – {}", time(start), time(end)),
                    None => time(start),
                }
                .into(),
            })
        })
        .collect::<Vec<_>>();
    events.sort_by(|a, b| {
        a.starts_at_ms
            .total_cmp(&b.starts_at_ms)
            .then_with(|| a.title.cmp(&b.title))
    });
    events.into()
}

/// Where "Show events in menu bar" persists: shipping's tray scope of `store.json` in a
/// channel run, this profile's `tray.json` in a sandbox.
#[derive(Clone)]
pub(crate) enum TraySettings {
    File(PathBuf),
    Shipping(Arc<anlg_desktop_identity::store_json::StoreJson>),
}

impl TraySettings {
    /// Blocks on the file system.
    pub(crate) fn show_events(&self) -> bool {
        use crate::shipping_store::{SHOW_EVENTS_IN_MENU_BAR, TRAY};
        let stored = match self {
            Self::File(path) => std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|value| value[SHOW_EVENTS_IN_MENU_BAR].as_bool()),
            Self::Shipping(store) => match store.get(TRAY, SHOW_EVENTS_IN_MENU_BAR) {
                Ok(value) => value.and_then(|value| value.as_bool()),
                Err(error) => {
                    tracing::warn!(%error, "failed to load tray event visibility");
                    None
                }
            },
        };
        stored.unwrap_or(true)
    }

    /// Blocks on the file system.
    pub(crate) fn set_show_events(&self, show: bool) -> std::io::Result<()> {
        use crate::shipping_store::{SHOW_EVENTS_IN_MENU_BAR, TRAY};
        match self {
            Self::File(path) => {
                std::fs::write(path, json!({ SHOW_EVENTS_IN_MENU_BAR: show }).to_string())
            }
            Self::Shipping(store) => store.set(TRAY, SHOW_EVENTS_IN_MENU_BAR, json!(show)),
        }
    }
}

/// Lifecycle state owned by the application view.
pub(super) struct Lifecycle {
    tray_settings: TraySettings,
    show_events: bool,
    schedule: Arc<[ScheduleEvent]>,
    recording_title: Option<(SessionId, Option<Arc<str>>)>,
    title_task: Option<Task<()>>,
    schedule_task: Option<Task<()>>,
    update: UpdateMenu,
    checking: bool,
    installing: bool,
    /// The release the last check found, which Download and Retry fetch.
    release: Option<(Arc<NativeUpdater>, ResolvedRelease)>,
    ready: Option<VerifiedUpdate>,
    confirming: bool,
}

impl Lifecycle {
    pub(super) fn new(tray_settings: TraySettings) -> Self {
        Self {
            tray_settings,
            show_events: true,
            schedule: Arc::from([]),
            recording_title: None,
            title_task: None,
            schedule_task: None,
            update: UpdateMenu::default(),
            checking: false,
            installing: false,
            release: None,
            ready: None,
            confirming: false,
        }
    }
}

impl ApplicationView {
    fn capture_phase(&self) -> Option<(Phase, Option<SessionId>, bool)> {
        let update = self.services.as_ref()?.capture.take_update(u64::MAX);
        Some((update.phase, update.session, update.degraded.is_some()))
    }

    pub fn update_menu(&self) -> UpdateMenu {
        self.lifecycle.update.clone()
    }

    /// Restart was chosen and the install is running.
    pub(super) fn update_installing(&self) -> bool {
        self.lifecycle.installing
    }

    /// Capture is starting, live or finalizing.
    pub(super) fn meeting_live(&self) -> bool {
        self.capture_phase().is_some_and(|(phase, ..)| {
            matches!(phase, Phase::Loading | Phase::Listening | Phase::Finalizing)
        })
    }

    pub fn tray_model(&self) -> TrayModel {
        let (phase, session, degraded) = self.capture_phase().unwrap_or((Phase::Idle, None, false));
        let recording = matches!(phase, Phase::Loading | Phase::Listening);
        let recording_title = self
            .lifecycle
            .recording_title
            .as_ref()
            .filter(|(id, _)| Some(id) == session.as_ref())
            .and_then(|(_, title)| title.clone());
        let now_ms = Utc::now().timestamp_millis() as f64;
        TrayModel::new(
            &TrayState {
                recording,
                degraded: recording && degraded,
                update: self.lifecycle.update.clone(),
                show_events: self.lifecycle.show_events,
                schedule: self.lifecycle.schedule.clone(),
                recording_title,
            },
            now_ms,
        )
    }

    /// Shipping's main-window close: hide and keep running. A platform that cannot hide one
    /// window quits through the drain instead, as does a window whose startup refused the
    /// library; a hide that fails leaves the window open with the error rather than ending
    /// capture.
    pub fn close_main(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if super::startup::refused(cx) {
            self.request_quit(cx);
            return;
        }
        self.main_window_hidden(cx);
        match crate::platform::windows::hide_main(window, cx) {
            Ok(()) => {}
            Err(ServiceError::Unsupported(_)) => self.request_quit(cx),
            Err(error) => self.status(&error.to_string(), cx),
        }
    }

    pub(super) fn show_main(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = crate::platform::windows::show_main(window) {
            tracing::warn!(%error, "Could not show the main window");
        }
        cx.activate(true);
        window.activate_window();
    }

    /// A real quit. When a draft or pending save holds it back, the main window shows why.
    pub(super) fn quit_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_quit(cx);
        if !self.closing && !self.drained {
            self.show_main(window, cx);
        }
    }

    pub(super) fn lifecycle_action(
        &mut self,
        action: TrayAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            TrayAction::Open => self.show_main(window, cx),
            TrayAction::StartMeeting => {
                self.show_main(window, cx);
                if self.writers_paused || self.closing {
                    self.status("Finish pending work before starting capture.", cx);
                } else {
                    self.workspace.update(cx, |view, cx| view.create(true, cx));
                }
            }
            TrayAction::Settings => {
                self.show_main(window, cx);
                self.open_product(ProductRoute::Settings, window, cx);
            }
            TrayAction::ToggleShowEvents => {
                let show = !self.lifecycle.show_events;
                self.lifecycle.show_events = show;
                let settings = self.lifecycle.tray_settings.clone();
                cx.background_executor()
                    .spawn(async move {
                        if let Err(error) = settings.set_show_events(show) {
                            tracing::warn!(%error, "failed to persist tray event visibility");
                        }
                    })
                    .detach();
            }
            TrayAction::CheckUpdate => self.check_for_updates(cx),
            TrayAction::Hide => {
                self.main_window_hidden(cx);
                if let Err(error) = crate::platform::windows::hide_main(window, cx) {
                    self.status(&error.to_string(), cx);
                }
            }
            TrayAction::QuitCompletely => self.confirm_quit_completely(window, cx),
            TrayAction::Agenda(id) => self.open_agenda_event(id, window, cx),
        }
    }

    pub(super) fn show_about(&mut self, cx: &mut Context<Self>) {
        let message = format!(
            "- App Name: Anarlog\n- App Version: {}\n- SHA:\n  {}",
            crate::platform::package::VERSION,
            env!("ANARLOG_GIT_SHA")
        );
        cx.spawn(async move |_, cx| {
            if alert("About Anarlog", &message, Some(("Copy", "Cancel"))).await {
                cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(message)));
            }
        })
        .detach();
    }

    fn confirm_quit_completely(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.lifecycle.confirming {
            return;
        }
        self.lifecycle.confirming = true;
        cx.spawn_in(window, async move |this, cx| {
            let confirmed = alert(
                "Quit Anarlog Completely?",
                "Anarlog will stop running in the background.",
                Some(("Quit Completely", "Cancel")),
            )
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.lifecycle.confirming = false;
                if confirmed {
                    this.quit_now(window, cx);
                }
            });
        })
        .detach();
    }

    /// Shipping's agenda item: a note for the event that starts recording, plus its link.
    fn open_agenda_event(&mut self, id: Arc<str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(link) = self
            .lifecycle
            .schedule
            .iter()
            .find(|event| event.id == id)
            .map(|event| event.meeting_link.clone())
        else {
            return;
        };
        self.show_main(window, cx);
        if let Some(link) = link.filter(|link| !link.trim().is_empty()) {
            cx.open_url(&link);
        }
        if self.writers_paused || self.closing {
            self.status("Finish pending work before starting capture.", cx);
            return;
        }
        let viewer = self
            .native
            .as_ref()
            .and_then(|native| native.cloud.service.as_ref())
            .and_then(|cloud| cloud.scope().account_id.clone());
        let reply = crate::workspace::calendar::open_event(&self.runtime, id, viewer);
        cx.spawn_in(window, async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(session) => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.open_session(session.clone(), cx)
                    });
                    this.open_meeting(
                        MeetingIntent::Start {
                            session_id: session,
                        },
                        window,
                        cx,
                    );
                }
                Err(error) => this.status(&format!("Note not created: {error}"), cx),
            });
        })
        .detach();
    }

    /// Follows the recording note's title for the menu-bar title (`tray-recording.tsx`).
    pub(super) fn sync_recording_title(&mut self, cx: &mut Context<Self>) {
        let session = self
            .capture_phase()
            .filter(|(phase, ..)| matches!(phase, Phase::Loading | Phase::Listening))
            .and_then(|(_, session, _)| session);
        if self.lifecycle.recording_title.as_ref().map(|(id, _)| id) == session.as_ref() {
            return;
        }
        self.lifecycle.title_task = None;
        self.lifecycle.recording_title = session.clone().map(|id| (id, None));
        let Some(session) = session else {
            return;
        };
        let reply = self.runtime.watch_query(
            "SELECT title FROM sessions WHERE id = ?1".into(),
            vec![json!(session.0)],
        );
        self.lifecycle.title_task = Some(cx.spawn(async move |this, cx| {
            let Ok(mut watch) = async { reply?.receive().await }.await else {
                return;
            };
            loop {
                let title = watch
                    .snapshots
                    .borrow_and_update()
                    .rows
                    .first()
                    .and_then(|row| row["title"].as_str())
                    .map(str::trim)
                    .filter(|title| {
                        !title.is_empty() && !UNTITLED.contains(&title.to_lowercase().as_str())
                    })
                    .map(Arc::<str>::from);
                let applied = this.update(cx, |this, _| {
                    if let Some((id, current)) = &mut this.lifecycle.recording_title
                        && *id == session
                    {
                        *current = title;
                    }
                });
                if applied.is_err() || watch.snapshots.changed().await.is_err() {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        }));
    }

    /// Publishes the calendar schedule and "Show events in menu bar" to the tray.
    pub(super) fn start_lifecycle(&mut self, cx: &mut Context<Self>) {
        let settings = self.lifecycle.tray_settings.clone();
        cx.spawn(async move |this, cx| {
            let show = cx
                .background_executor()
                .spawn(async move { settings.show_events() })
                .await;
            let _ = this.update(cx, |this, _| this.lifecycle.show_events = show);
        })
        .detach();
        let runtime = self.runtime.clone();
        let events = runtime.watch_query(SCHEDULE_QUERY.into(), vec![]);
        let preferences = settings::watch(&runtime);
        self.lifecycle.schedule_task = Some(cx.spawn(async move |this, cx| {
            let watches = async {
                Ok::<_, ServiceError>((events?.receive().await?, preferences?.receive().await?))
            }
            .await;
            let (mut events, mut preferences) = match watches {
                Ok(watches) => watches,
                Err(error) => {
                    tracing::warn!(%error, "tray schedule is unavailable");
                    return;
                }
            };
            let mut loaded: Option<Instant> = None;
            loop {
                let (changed, preferences_changed) = (
                    events.snapshots.has_changed(),
                    preferences.snapshots.has_changed(),
                );
                if changed.is_err()
                    || preferences_changed.is_err()
                    || events.terminal_error().is_some()
                {
                    break;
                }
                // The query is bounded by "now", so it is also re-read every few minutes.
                let stale = loaded.is_none_or(|at| at.elapsed() >= Duration::from_secs(300));
                if stale || changed.is_ok_and(|c| c) || preferences_changed.is_ok_and(|c| c) {
                    events.snapshots.mark_unchanged();
                    let rows = preferences.snapshots.borrow_and_update().rows.clone();
                    let snapshot = settings::decode(&rows).unwrap_or_default();
                    let use_24_hour_time = snapshot
                        .get("use_24_hour_time")
                        .and_then(|setting| setting.value.as_bool())
                        .unwrap_or(false);
                    let timezone = snapshot
                        .get("timezone")
                        .and_then(|setting| setting.value.as_str())
                        .and_then(|timezone| timezone.parse::<Tz>().ok());
                    let read = runtime.read(CancellationToken::new(), move |services| async move {
                        services
                            .executor
                            .execute(SCHEDULE_QUERY.into(), vec![])
                            .await
                            .map_err(|error| ServiceError::Failed(error.to_string().into()))
                    });
                    match async { read?.receive().await }.await {
                        Ok(rows) => {
                            let schedule =
                                build_schedule(&rows, Utc::now(), timezone, use_24_hour_time);
                            if this
                                .update(cx, |this, _| this.lifecycle.schedule = schedule)
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => tracing::warn!(%error, "tray schedule read failed"),
                    }
                    loaded = Some(Instant::now());
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
            let _ = events.unsubscribe().await;
            let _ = preferences.unsubscribe().await;
        }));
    }

    /// One feed check, from startup, the 30-minute timer or the tray. A found release becomes
    /// `Available` unless a download, ready update or same-version failure owns the state, like
    /// shipping's update events.
    fn check_feed(
        &mut self,
        updater: Arc<NativeUpdater>,
        cx: &mut Context<Self>,
    ) -> Task<desktop_runtime::Result<Option<Arc<str>>>> {
        self.lifecycle.checking = true;
        let reply = updater.check(&self.runtime, CancellationToken::new());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            this.update(cx, |this, cx| {
                this.lifecycle.checking = false;
                let release = result?;
                let found = release.as_ref().map(release_version);
                let replace = !this.lifecycle.installing
                    && replaces_on_check(&this.lifecycle.update, found.as_ref());
                if replace {
                    this.lifecycle.ready = None;
                    this.lifecycle.update = found
                        .clone()
                        .map_or(UpdateMenu::CheckForUpdate, UpdateMenu::Available);
                    this.lifecycle.release = release.map(|release| (updater, release));
                    cx.notify();
                }
                Ok(found)
            })
            .map_err(|_| ServiceError::Closed)?
        })
    }

    /// Like shipping's updater, a build with a feed runs [`update_pass`] at startup and every
    /// 30 minutes.
    pub(super) fn watch_updates(&mut self, cx: &mut Context<Self>) {
        let updater = match updater(cx) {
            Ok(updater) => updater,
            Err(error) => {
                tracing::info!(%error, "updates are off");
                return;
            }
        };
        cx.spawn(async move |this, cx| {
            let mut install_at_open = true;
            while let Some(next) = update_pass(&this, &updater, install_at_open, cx).await {
                install_at_open = next;
                cx.background_executor().timer(UPDATE_CHECK_INTERVAL).await;
            }
        })
        .detach();
    }

    /// Shipping's "Check for Updates" item (`tray_check_update.rs`) and its dialogs.
    pub(super) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        match self.lifecycle.update {
            UpdateMenu::Downloading(..) => return,
            UpdateMenu::RestartToApply(_) => return self.apply_update(true, cx),
            _ if self.lifecycle.checking || self.closing => return,
            _ => {}
        }
        let updater = match updater(cx) {
            Ok(updater) => updater,
            Err(error) => {
                return notify(
                    "Update Check Failed",
                    format!("Failed to check for updates: {error}"),
                    cx,
                );
            }
        };
        let check = self.check_feed(updater, cx);
        cx.spawn(async move |this, cx| {
            let version = match check.await {
                Ok(Some(version)) => version,
                Ok(None) => {
                    alert(
                        "Check for Updates",
                        "There are currently no updates available.",
                        None,
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    alert(
                        "Update Check Failed",
                        &format!("Failed to check for updates: {error}"),
                        None,
                    )
                    .await;
                    return;
                }
            };
            let accepted = alert(
                "Update Available",
                &format!("Update v{version} is available!"),
                Some(("Download", "Later")),
            )
            .await;
            if accepted {
                let _ = this.update(cx, |this, cx| this.download_update(true, cx));
            }
        })
        .detach();
    }

    /// Downloads the release the last check found: the tray dialog's Download and the toast's
    /// Download and Retry.
    pub(super) fn download_update(&mut self, from_tray: bool, cx: &mut Context<Self>) {
        if let Some(download) = self.start_download(from_tray, cx) {
            download.detach();
        }
    }

    /// The download, which resolves to whether it verified; `None` when nothing starts.
    fn start_download(&mut self, from_tray: bool, cx: &mut Context<Self>) -> Option<Task<bool>> {
        let busy = matches!(
            self.lifecycle.update,
            UpdateMenu::Downloading(..) | UpdateMenu::RestartToApply(_)
        );
        if busy || self.lifecycle.installing || self.closing {
            return None;
        }
        let (updater, release) = self.lifecycle.release.clone()?;
        let version = release_version(&release);
        let progress = Arc::new((AtomicU64::new(0), AtomicU64::new(0)));
        let reply = {
            let progress = progress.clone();
            updater.download_with_progress(
                &self.runtime,
                release,
                CancellationToken::new(),
                move |chunk, total| {
                    progress.0.fetch_add(chunk as u64, Ordering::Relaxed);
                    progress.1.store(total.unwrap_or(0), Ordering::Relaxed);
                },
            )
        };
        self.lifecycle.update = UpdateMenu::Downloading(version.clone(), None);
        cx.notify();
        Some(cx.spawn(async move |this, cx| {
            let download = async { reply?.receive().await };
            futures::pin_mut!(download);
            // Chunks arrive on the runtime; the percentage is published every 250 ms.
            let result = loop {
                let tick = cx.background_executor().timer(Duration::from_millis(250));
                futures::pin_mut!(tick);
                match futures::future::select(download.as_mut(), tick).await {
                    futures::future::Either::Left((result, _)) => break result,
                    futures::future::Either::Right(_) => {
                        let percent = download_percent(
                            progress.0.load(Ordering::Relaxed),
                            progress.1.load(Ordering::Relaxed),
                        );
                        let alive = this.update(cx, |this, cx| {
                            if let UpdateMenu::Downloading(_, shown) = &mut this.lifecycle.update
                                && *shown != percent
                            {
                                *shown = percent;
                                cx.notify();
                            }
                        });
                        if alive.is_err() {
                            return false;
                        }
                    }
                }
            };
            let failed = this.update(cx, |this, cx| {
                cx.notify();
                match result {
                    Ok(update) => {
                        this.lifecycle.ready = Some(update);
                        this.lifecycle.update = UpdateMenu::RestartToApply(version);
                        None
                    }
                    Err(error) => {
                        // Shipping's toast shows the download error; its tray, a dialog.
                        let message = if from_tray {
                            "Failed to download update.".into()
                        } else {
                            error.to_string().into()
                        };
                        this.lifecycle.update = UpdateMenu::Failed(version, message);
                        Some(error)
                    }
                }
            });
            if from_tray && let Ok(Some(error)) = &failed {
                alert(
                    "Update Failed",
                    &format!("Failed to download update: {error}"),
                    None,
                )
                .await;
            }
            matches!(failed, Ok(None))
        }))
    }

    /// Shipping's Restart (toast) and "Restart to Apply Update" (tray).
    pub(super) fn apply_update(&mut self, from_tray: bool, cx: &mut Context<Self>) {
        let source = if from_tray {
            Source::Tray
        } else {
            Source::Toast
        };
        if let Some(install) = self.install_update(source, cx) {
            install.detach();
        }
    }

    /// Installs the downloaded update, then quits through the drain into it: the new package
    /// relaunches, or on Windows the NSIS installer runs and relaunches it. The install resolves
    /// to whether it succeeded; `None` when it doesn't start. An automatic install waits,
    /// silently, for a meeting to end and for onboarding or a settings page to close.
    fn install_update(&mut self, source: Source, cx: &mut Context<Self>) -> Option<Task<bool>> {
        let automatic = source == Source::Automatic;
        let ready = matches!(self.lifecycle.update, UpdateMenu::RestartToApply(_));
        if self.lifecycle.installing || self.closing || (automatic && !ready) {
            return None;
        }
        let onboarding = self
            .product
            .as_ref()
            .is_some_and(|view| matches!(view.read(cx).route, ProductRoute::Onboarding));
        let installer = NativeUpdater::permit_install(InstallConditions {
            meeting_active: self.meeting_live(),
            onboarding,
            automatic,
        })
        .and_then(|()| NativeInstaller::current());
        let installer = match installer {
            Ok(installer) => installer,
            Err(error) => {
                let message = format!("Failed to install update: {error}");
                match source {
                    Source::Tray => notify("Update Failed", message, cx),
                    Source::Toast => super::toasts::error(message, cx),
                    Source::Automatic => tracing::info!(%error, "automatic update deferred"),
                }
                return None;
            }
        };
        if automatic && self.product.is_some() {
            return None;
        }
        if self
            .product
            .as_ref()
            .is_some_and(|view| !view.update(cx, |view, cx| view.can_close(cx)))
        {
            return None;
        }
        let Some(update) = self.lifecycle.ready.take() else {
            self.lifecycle.update = UpdateMenu::CheckForUpdate;
            return None;
        };
        let Some((updater, _)) = self.lifecycle.release.clone() else {
            self.lifecycle.update = UpdateMenu::CheckForUpdate;
            return None;
        };
        let version = release_version(&update.release);
        self.lifecycle.installing = true;
        cx.notify();
        let runtime = self.runtime.clone();
        Some(cx.spawn(async move |this, cx| {
            // `None` when the feed no longer offers the downloaded release.
            let result: desktop_runtime::Result<Option<Restart>> = async {
                // Like shipping, re-check the feed before installing, so a release it withdrew
                // (a rollback) is never installed, even when Restart races a check. Unlike
                // shipping, a different offered version also withdraws the download: during
                // the cutover that may be a rollback build, which the next check downloads.
                let offered = updater
                    .check(&runtime, CancellationToken::new())?
                    .receive()
                    .await?;
                if offered.is_none_or(|offered| offered.version != update.release.version) {
                    return Ok(None);
                }
                let pause = this
                    .update(cx, |this, cx| this.pause_writers(cx))
                    .map_err(|_| ServiceError::Closed)?;
                pause.await?;
                runtime
                    .service(move |_| async move { installer.install(update).await })?
                    .receive()
                    .await
                    .map(Some)
            }
            .await;
            let failed = this.update(cx, |this, cx| {
                this.lifecycle.installing = false;
                cx.notify();
                match result {
                    Ok(Some(restart)) => {
                        crate::platform::windows::request_restart(restart);
                        this.request_quit(cx);
                        None
                    }
                    Ok(None) => {
                        this.lifecycle.update = UpdateMenu::CheckForUpdate;
                        this.lifecycle.release = None;
                        Some(ServiceError::Unsupported(
                            "This update is no longer available.".into(),
                        ))
                    }
                    Err(error) => {
                        // The verified bytes were consumed; Retry downloads the release again.
                        this.lifecycle.update =
                            UpdateMenu::Failed(version, error.to_string().into());
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                        Some(error)
                    }
                }
            });
            if source == Source::Tray
                && let Ok(Some(error)) = &failed
            {
                alert(
                    "Update Failed",
                    &format!("Failed to install update: {error}"),
                    None,
                )
                .await;
            }
            matches!(failed, Ok(None))
        }))
    }
}

/// Whether a check's result replaces the update state, like shipping's update events: a found
/// release becomes `Available` unless a download, a ready update or a same-version failure owns
/// the state. A ready update the feed no longer offers, withdrawn or replaced (a rollback), is
/// dropped rather than installed.
fn replaces_on_check(current: &UpdateMenu, found: Option<&Arc<str>>) -> bool {
    match current {
        UpdateMenu::CheckForUpdate | UpdateMenu::Available(_) => true,
        UpdateMenu::Failed(version, _) => found.is_some_and(|found| found != version),
        UpdateMenu::RestartToApply(version) => found != Some(version),
        UpdateMenu::Downloading(..) => false,
    }
}

fn release_version(release: &ResolvedRelease) -> Arc<str> {
    release.version.to_string().into()
}

/// Shipping's `Math.round(progress * 100)`, or `None` until the content length is known.
fn download_percent(downloaded: u64, total: u64) -> Option<u8> {
    (total > 0).then(|| ((downloaded as f64 / total as f64).clamp(0., 1.) * 100.).round() as u8)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ready_update_the_feed_no_longer_offers_is_dropped() {
        let (ready, newer) = (Arc::<str>::from("1.5.0"), Arc::<str>::from("1.5.1"));
        let restart = UpdateMenu::RestartToApply(ready.clone());
        assert!(replaces_on_check(&restart, None));
        assert!(!replaces_on_check(&restart, Some(&ready)));
        assert!(replaces_on_check(&restart, Some(&newer)));
        let failed = UpdateMenu::Failed(ready.clone(), "offline".into());
        assert!(!replaces_on_check(&failed, Some(&ready)));
        assert!(replaces_on_check(&failed, Some(&newer)));
        assert!(!replaces_on_check(
            &UpdateMenu::Downloading(ready.clone(), None),
            None
        ));
    }

    #[test]
    fn channel_tray_toggle_lives_in_shipping_store_json() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(anlg_desktop_identity::store_json::StoreJson::at(
            root.path().join("store.json"),
        ));
        let settings = TraySettings::Shipping(store.clone());
        assert!(settings.show_events());
        store
            .set("tray", "show_events_in_menu_bar", json!(false))
            .unwrap();
        assert!(!settings.show_events());

        settings.set_show_events(true).unwrap();
        assert_eq!(
            store.get("tray", "show_events_in_menu_bar").unwrap(),
            Some(json!(true))
        );
    }

    #[test]
    fn tray_schedule_skips_all_day_and_finished_events_and_labels_local_days() {
        let now = DateTime::parse_from_rfc3339("2026-11-17T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rows = [
            json!({"id":"later","title":"  ","started_at":"2026-11-18T01:00:00Z","ended_at":"","is_all_day":0,"meeting_link":"https://meet.example/x"}),
            json!({"id":"standup","title":"Standup","started_at":"2026-11-17T09:00:00Z","ended_at":"2026-11-17T09:30:00Z","is_all_day":0,"meeting_link":""}),
            json!({"id":"done","title":"Done","started_at":"2026-11-17T06:00:00Z","ended_at":"2026-11-17T07:00:00Z","is_all_day":0}),
            json!({"id":"holiday","title":"Holiday","started_at":"2026-11-17T00:00:00Z","ended_at":"2026-11-18T00:00:00Z","is_all_day":1}),
        ];
        let seoul: Tz = "Asia/Seoul".parse().unwrap();
        let schedule = build_schedule(&rows, now, Some(seoul), false);
        let ids = schedule.iter().map(|event| &*event.id).collect::<Vec<_>>();
        assert_eq!(ids, ["standup", "later"]);
        assert_eq!(&*schedule[0].time_label, "6:00 PM – 6:30 PM");
        assert_eq!(&*schedule[1].title, "Untitled event");
        assert_eq!(
            schedule[1].meeting_link.as_deref(),
            Some("https://meet.example/x")
        );
        // 2026-11-18T01:00Z is 10:00 on the 18th in Seoul: "Tomorrow" from 17:00 local time.
        let sections =
            crate::platform::tray::agenda_sections(&schedule, now.timestamp_millis() as f64, true);
        assert_eq!(
            sections
                .iter()
                .map(|section| section.label)
                .collect::<Vec<_>>(),
            ["Today", "Tomorrow"]
        );
    }
}
