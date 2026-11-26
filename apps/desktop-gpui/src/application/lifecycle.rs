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
    let _ = automatic_updates(this, cx).await?;
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
    let automatic = this
        .read_with(cx, |this, _| {
            this.notifications.automatic_updates() == Some(true)
        })
        .ok()?;
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
    let automatic = this
        .read_with(cx, |this, _| {
            this.notifications.automatic_updates() == Some(true)
        })
        .ok()?;
    if !automatic {
        return Some(false);
    }
    let install = this
        .update(cx, |this, cx| this.install_update(Source::Automatic, cx))
        .ok()?;
    match install {
        // A successful install quits into the new version.
        Some(install) => {
            if install.await {
                Some(false)
            } else {
                this.read_with(cx, |this, _| {
                    this.notifications.automatic_updates() == Some(true)
                })
                .ok()
            }
        }
        None => this
            .read_with(cx, |this, _| {
                this.notifications.automatic_updates() == Some(true)
            })
            .ok(),
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
        if self.lifecycle.installing
            || self.closing
            || self.drained
            || self.shutdown_begun()
            || (automatic && !ready)
        {
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
        let updater_for_check = updater.clone();
        let runtime_for_check = runtime.clone();
        Some(cx.spawn(async move |this, cx| {
            // `Withdrawn` when the feed no longer offers the downloaded release.
            let expected = update.release.version.clone();
            let outcome = install_if_still_offered(
                &expected,
                update,
                cx,
                || {
                    let updater = updater_for_check.clone();
                    let runtime = runtime_for_check.clone();
                    async move {
                        updater
                            .check(&runtime, CancellationToken::new())?
                            .receive()
                            .await
                            .map(|offered| offered.map(|offered| offered.version))
                    }
                },
                |cx| {
                    this.update(cx, |this, cx| {
                        this.can_install_now(automatic, cx)
                            .then(|| this.pause_writers(cx))
                    })
                    .map_err(|_| ServiceError::Closed)
                },
                |cx| {
                    this.update(cx, |this, cx| this.can_install_now(automatic, cx))
                        .map_err(|_| ServiceError::Closed)
                },
                |update| async move {
                    runtime
                        .service(move |_| async move { installer.install(update).await })?
                        .receive()
                        .await
                },
            )
            .await;
            let outcome = this.update(cx, |this, cx| {
                this.lifecycle.installing = false;
                cx.notify();
                match outcome {
                    Ok(InstallOutcome::Installed(restart)) => {
                        crate::platform::windows::request_restart(restart);
                        this.request_quit(cx);
                        (true, None)
                    }
                    Ok(InstallOutcome::Deferred(update)) => {
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                        this.lifecycle.ready = Some(update);
                        (false, None)
                    }
                    Ok(InstallOutcome::Withdrawn) => {
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                        this.lifecycle.update = UpdateMenu::CheckForUpdate;
                        this.lifecycle.release = None;
                        (
                            false,
                            Some(ServiceError::Unsupported(
                                "This update is no longer available.".into(),
                            )),
                        )
                    }
                    Err(error) => {
                        // The verified bytes were consumed; Retry downloads the release again.
                        this.lifecycle.update =
                            UpdateMenu::Failed(version, error.to_string().into());
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                        (false, Some(error))
                    }
                }
            });
            if source == Source::Tray
                && let Ok((_, Some(error))) = &outcome
            {
                alert(
                    "Update Failed",
                    &format!("Failed to install update: {error}"),
                    None,
                )
                .await;
            }
            matches!(outcome, Ok((true, _)))
        }))
    }

    fn can_install_now(&mut self, automatic: bool, cx: &mut Context<Self>) -> bool {
        !(self.closing
            || self.drained
            || self.shutdown_begun()
            || self.meeting_live()
            || (automatic
                && (self.product.is_some()
                    || self.notifications.automatic_updates() != Some(true)))
            || (!automatic
                && self
                    .product
                    .as_ref()
                    .is_some_and(|view| !view.update(cx, |view, cx| view.can_close(cx)))))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum InstallOutcome<U> {
    Installed(Restart),
    Withdrawn,
    Deferred(U),
}

/// Re-checks the feed and install eligibility before pausing writers and again right before
/// installing, dropping downloaded bytes if the release was withdrawn or replaced (a rollback) or
/// restoring them if a UI/meeting guard defers installation.
async fn install_if_still_offered<
    C,
    V,
    U,
    FCheck,
    FCheckFut,
    FPause,
    FPauseFut,
    FCanInstall,
    FInstall,
    FInstallFut,
>(
    expected: &V,
    update: U,
    cx: &mut C,
    mut check: FCheck,
    pause: FPause,
    mut can_install: FCanInstall,
    install: FInstall,
) -> desktop_runtime::Result<InstallOutcome<U>>
where
    V: PartialEq,
    FCheck: FnMut() -> FCheckFut,
    FCheckFut: std::future::Future<Output = desktop_runtime::Result<Option<V>>>,
    FPause: FnOnce(&mut C) -> desktop_runtime::Result<Option<FPauseFut>>,
    FPauseFut: std::future::Future<Output = desktop_runtime::Result<()>>,
    FCanInstall: FnMut(&mut C) -> desktop_runtime::Result<bool>,
    FInstall: FnOnce(U) -> FInstallFut,
    FInstallFut: std::future::Future<Output = desktop_runtime::Result<Restart>>,
{
    let offered = check().await?;
    if offered.is_none_or(|offered| &offered != expected) {
        return Ok(InstallOutcome::Withdrawn);
    }
    let Some(pause) = pause(cx)? else {
        return Ok(InstallOutcome::Deferred(update));
    };
    pause.await?;
    let offered = check().await?;
    if offered.is_none_or(|offered| &offered != expected) {
        return Ok(InstallOutcome::Withdrawn);
    }
    if !can_install(cx)? {
        return Ok(InstallOutcome::Deferred(update));
    }
    install(update).await.map(InstallOutcome::Installed)
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
    use gpui::AppContext as _;

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
    fn install_recheck_skips_withdrawn_or_replaced_release_before_pausing_writers() {
        use std::sync::atomic::{AtomicBool, AtomicUsize};

        use futures::executor::block_on;

        for offered in [None, Some("1.4.9")] {
            let paused = AtomicBool::new(false);
            let installed = AtomicBool::new(false);
            let mut state = ();
            let result = block_on(install_if_still_offered(
                &"1.5.0",
                "bytes",
                &mut state,
                || async move { Ok(offered) },
                |_| {
                    paused.store(true, Ordering::SeqCst);
                    Ok(Some(async { Ok(()) }))
                },
                |_| Ok(true),
                |_| {
                    installed.store(true, Ordering::SeqCst);
                    async { Ok(Restart::App) }
                },
            ))
            .unwrap();
            assert_eq!(result, InstallOutcome::Withdrawn);
            assert!(!paused.load(Ordering::SeqCst));
            assert!(!installed.load(Ordering::SeqCst));
        }

        let checks = AtomicUsize::new(0);
        let installed = AtomicBool::new(false);
        let mut state = ();
        let deferred = block_on(install_if_still_offered(
            &"1.5.0",
            "bytes",
            &mut state,
            || {
                checks.fetch_add(1, Ordering::SeqCst);
                async { Ok(Some("1.5.0")) }
            },
            |_| Ok(None::<std::future::Ready<desktop_runtime::Result<()>>>),
            |_| Ok(true),
            |_| {
                installed.store(true, Ordering::SeqCst);
                async { Ok(Restart::App) }
            },
        ))
        .unwrap();
        assert_eq!(deferred, InstallOutcome::Deferred("bytes"));
        assert_eq!(checks.load(Ordering::SeqCst), 1);
        assert!(!installed.load(Ordering::SeqCst));

        let checks = AtomicUsize::new(0);
        let paused = AtomicBool::new(false);
        let installed = AtomicBool::new(false);
        let mut state = ();
        let withdrawn_after_pause = block_on(install_if_still_offered(
            &"1.5.0",
            "bytes",
            &mut state,
            || {
                let first = checks.fetch_add(1, Ordering::SeqCst) == 0;
                async move { Ok(first.then_some("1.5.0")) }
            },
            |_| {
                paused.store(true, Ordering::SeqCst);
                Ok(Some(async { Ok(()) }))
            },
            |_| Ok(true),
            |_| {
                installed.store(true, Ordering::SeqCst);
                async { Ok(Restart::App) }
            },
        ))
        .unwrap();
        assert_eq!(withdrawn_after_pause, InstallOutcome::Withdrawn);
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        assert!(paused.load(Ordering::SeqCst));
        assert!(!installed.load(Ordering::SeqCst));

        let checks = AtomicUsize::new(0);
        let paused = AtomicBool::new(false);
        let installed = AtomicBool::new(false);
        let mut state = ();
        let deferred_after_recheck = block_on(install_if_still_offered(
            &"1.5.0",
            "bytes",
            &mut state,
            || {
                checks.fetch_add(1, Ordering::SeqCst);
                async { Ok(Some("1.5.0")) }
            },
            |_| {
                paused.store(true, Ordering::SeqCst);
                Ok(Some(async { Ok(()) }))
            },
            |_| Ok(false),
            |_| {
                installed.store(true, Ordering::SeqCst);
                async { Ok(Restart::App) }
            },
        ))
        .unwrap();
        assert_eq!(deferred_after_recheck, InstallOutcome::Deferred("bytes"));
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        assert!(paused.load(Ordering::SeqCst));
        assert!(!installed.load(Ordering::SeqCst));

        let checks = AtomicUsize::new(0);
        let paused = AtomicBool::new(false);
        let mut state = ();
        let result = block_on(install_if_still_offered(
            &"1.5.0",
            "bytes",
            &mut state,
            || {
                checks.fetch_add(1, Ordering::SeqCst);
                async { Ok(Some("1.5.0")) }
            },
            |_| {
                paused.store(true, Ordering::SeqCst);
                Ok(Some(async { Ok(()) }))
            },
            |_| Ok(true),
            |payload| async move {
                assert_eq!(payload, "bytes");
                Ok(Restart::App)
            },
        ))
        .unwrap();
        assert_eq!(result, InstallOutcome::Installed(Restart::App));
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        assert!(paused.load(Ordering::SeqCst));
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

    fn wait_until(
        cx: &mut gpui::TestAppContext,
        mut ready: impl FnMut(&mut gpui::TestAppContext) -> bool,
    ) {
        for _ in 0..2000 {
            cx.run_until_parked();
            if ready(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("condition did not become true before timeout");
    }

    fn try_edit(
        window: gpui::AnyWindowHandle,
        editor: &gpui::Entity<crate::editor::EditorPane>,
        text: &str,
        cx: &mut gpui::TestAppContext,
    ) -> bool {
        cx.update(|cx| {
            cx.update_window(window, |_, window, cx| {
                editor.update(cx, |editor, cx| {
                    gpui::EntityInputHandler::replace_text_in_range(editor, None, text, window, cx);
                    editor.is_dirty()
                })
            })
            .unwrap()
        })
    }

    fn flush_editor(
        editor: &gpui::Entity<crate::editor::EditorPane>,
        cx: &mut gpui::TestAppContext,
    ) {
        let flush = cx.update(|cx| editor.update(cx, |editor, _| editor.flush()));
        let (tx, mut rx) = futures::channel::oneshot::channel();
        cx.spawn(async move |_| {
            let _ = tx.send(flush.await);
        })
        .detach();
        wait_until(cx, |_| {
            rx.try_recv()
                .ok()
                .flatten()
                .transpose()
                .unwrap()
                .transpose()
                .unwrap()
                .is_some()
        });
        wait_until(cx, |cx| !cx.read(|cx| editor.read(cx).is_dirty()));
    }

    static QUIT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct DetachedNoteFixture {
        _quit_guard: std::sync::MutexGuard<'static, ()>,
        _profile: tempfile::TempDir,
        runtime: desktop_runtime::RuntimeHandle,
        main_window: gpui::AnyWindowHandle,
        app_entity: gpui::Entity<ApplicationView>,
        session_id: SessionId,
        main_memo: gpui::Entity<crate::editor::EditorPane>,
        main_summary: gpui::Entity<crate::editor::EditorPane>,
        detached_window: gpui::AnyWindowHandle,
        detached_memo: gpui::Entity<crate::editor::EditorPane>,
        detached_summary: gpui::Entity<crate::editor::EditorPane>,
    }

    fn verify_editable_and_flush(
        app: &DetachedNoteFixture,
        marker: &str,
        cx: &mut gpui::TestAppContext,
    ) {
        let _ = cx.update(|cx| {
            let _ = cx.update_window(app.main_window, |_, window, cx| {
                cx.focus_handle().focus(window, cx)
            });
            cx.update_window(app.detached_window, |_, window, cx| {
                cx.focus_handle().focus(window, cx)
            })
        });
        cx.run_until_parked();
        let memo_tag = format!(" {marker}-memo");
        let detached_tag = format!(" {marker}-detached");
        let summary_tag = format!(" {marker}-summary");
        assert!(try_edit(app.main_window, &app.main_memo, &memo_tag, cx));
        flush_editor(&app.main_memo, cx);
        wait_until(cx, |cx| {
            cx.read(|cx| {
                app.detached_memo
                    .read(cx)
                    .init
                    .document
                    .body
                    .contains(memo_tag.trim())
            })
        });
        assert!(try_edit(
            app.detached_window,
            &app.detached_memo,
            &detached_tag,
            cx
        ));
        flush_editor(&app.detached_memo, cx);
        wait_until(cx, |cx| {
            cx.read(|cx| {
                app.main_memo
                    .read(cx)
                    .init
                    .document
                    .body
                    .contains(detached_tag.trim())
            })
        });
        assert!(try_edit(
            app.main_window,
            &app.main_summary,
            &summary_tag,
            cx
        ));
        flush_editor(&app.main_summary, cx);
        wait_until(cx, |cx| {
            cx.read(|cx| {
                let note = app.app_entity.read(cx).workspace.read(cx).note().read(cx);
                !note.has_unsaved_title(cx) && !note.has_unsaved_summary(cx)
            })
        });
    }

    fn open_app_with_detached_note(cx: &mut gpui::TestAppContext) -> DetachedNoteFixture {
        let _quit_guard = QUIT_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::platform::os::go_headless();
        cx.executor().allow_parking();
        let profile = tempfile::tempdir().unwrap();
        let (runtime, ready) = crate::runtime_bridge::start(desktop_runtime::Profile {
            database: profile.path().join("library.sqlite"),
        })
        .unwrap();
        let mut state = runtime.state();
        while matches!(
            *state.borrow_and_update(),
            desktop_runtime::RuntimeState::Starting
        ) {
            futures::executor::block_on(state.changed()).unwrap();
        }
        futures::executor::block_on(
            runtime
                .submit(|services| async move {
                    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, 'false')")
                        .bind(crate::product::onboarding::NEEDED_KEY)
                        .execute(services.db.pool())
                        .await
                        .map(|_| ())
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))
                })
                .unwrap()
                .receive(),
        )
        .unwrap();
        let created = futures::executor::block_on(
            runtime
                .create_note("Quit drain note".into())
                .unwrap()
                .receive(),
        )
        .unwrap();
        let session_id = created.summary.id;
        let summary_sid = session_id.0.clone();
        let summary_body = json!({
            "type": "doc",
            "content": [
                {"type": "heading", "attrs": {"level": 1}, "content": [{"type": "text", "text": "Quit drain note"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "Summary body"}]}
            ]
        })
        .to_string();
        futures::executor::block_on(
            runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute(
                            "INSERT INTO session_documents (id, session_id, kind, title, body_format, body, sort_order)
                             VALUES ('quit-summary', ?, 'summary', '', 'prosemirror_json', ?, 0)"
                                .into(),
                            vec![json!(summary_sid), json!(summary_body)],
                        )
                        .await
                        .map(|_| ())
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))
                })
                .unwrap()
                .receive(),
        )
        .unwrap();
        cx.update(crate::ui::theme::UiFonts::init);
        let rt = runtime.clone();
        let root_path = profile.path().to_path_buf();
        let window = cx.add_window(|window, cx| {
            ApplicationView::new(
                rt,
                ready,
                crate::launch::Storage {
                    channel: None,
                    root: root_path.clone(),
                    database_dir: root_path.clone(),
                    pointer: Some(root_path.join("storage.json")),
                },
                window,
                cx,
            )
        });
        let main_window: gpui::AnyWindowHandle = window.into();
        let app_entity = window.update(cx, |_, _, cx| cx.entity()).unwrap();
        wait_until(cx, |cx| {
            cx.executor().advance_clock(Duration::from_millis(5));
            cx.read(|cx| app_entity.read(cx).native_ready())
        });
        let sid = session_id.clone();
        window
            .update(cx, |app, _, cx| {
                app.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(
                        crate::workspace::navigation::Route::Session(sid.clone()),
                        false,
                        cx,
                    );
                    cx.emit(crate::workspace::WorkspaceAction::OpenNoteWindow(sid));
                });
            })
            .unwrap();
        let sid = session_id.clone();
        for _ in 0..2000 {
            cx.executor().advance_clock(Duration::from_millis(5));
            cx.run_until_parked();
            let _ = cx.update(|cx| {
                cx.update_window(main_window, |_, window, cx| window.draw(cx).clear(cx))
            });
            let detached_handle = cx.read(|cx| {
                app_entity
                    .read(cx)
                    .note_windows
                    .get(&sid)
                    .copied()
                    .map(gpui::AnyWindowHandle::from)
            });
            if let Some(detached_window) = detached_handle {
                let _ = cx.update(|cx| {
                    cx.update_window(detached_window, |_, window, cx| window.draw(cx).clear(cx))
                });
            }
            let ready = cx.read(|cx| {
                let app = app_entity.read(cx);
                let note = app.workspace.read(cx).note().read(cx);
                app.editor.as_ref().is_some_and(|e| e.read(cx).is_loaded())
                    && note
                        .summary_editors()
                        .first()
                        .is_some_and(|e| e.read(cx).is_loaded())
                    && app
                        .note_windows
                        .get(&sid)
                        .and_then(|w| w.read(cx).ok())
                        .is_some_and(|w| {
                            w.is_editor_loaded(cx)
                                && w.note()
                                    .read(cx)
                                    .summary_editors()
                                    .first()
                                    .is_some_and(|e| e.read(cx).is_loaded())
                        })
            });
            if ready {
                let (main_memo, main_summary, detached_window, detached_memo, detached_summary) =
                    cx.read(|cx| {
                        let app = app_entity.read(cx);
                        let main_memo = app.editor.clone().unwrap();
                        let main_summary =
                            app.workspace.read(cx).note().read(cx).summary_editors()[0].clone();
                        let note_window = app.note_windows[&session_id];
                        let view = note_window.read(cx).unwrap();
                        let detached_memo = view.editor().unwrap();
                        let detached_summary = view.note().read(cx).summary_editors()[0].clone();
                        (
                            main_memo,
                            main_summary,
                            note_window.into(),
                            detached_memo,
                            detached_summary,
                        )
                    });
                return DetachedNoteFixture {
                    _quit_guard,
                    _profile: profile,
                    runtime,
                    main_window,
                    app_entity,
                    session_id,
                    main_memo,
                    main_summary,
                    detached_window,
                    detached_memo,
                    detached_summary,
                };
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let state = cx.read(|cx| {
            let app = app_entity.read(cx);
            let note = app.workspace.read(cx).note().read(cx);
            format!(
                "native={} product={} lock_known={} lock_locked={} main_editor={:?} note_sid={:?} shown_summary={:?} summaries={} detached={}",
                app.native_ready(),
                app.product.is_some(),
                crate::application::lock::note_lock_known(cx),
                crate::application::lock::app_locked(cx),
                app.editor.as_ref().map(|e| e.read(cx).is_loaded()),
                note.session_id(),
                note.shown_summary(),
                note.summary_editors().len(),
                app.note_windows.contains_key(&sid),
            )
        });
        panic!("detached note setup timed out: {state}");
    }

    #[gpui::test]
    fn deferred_update_during_quit_keeps_detached_note_window_locked(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        use futures::channel::oneshot;

        // Case 1: update deferral during an active quit drain keeps both windows locked.
        {
            let fixture = open_app_with_detached_note(cx);
            let (flush_started_tx, mut flush_started_rx) = oneshot::channel::<()>();
            let (release_flush, held_flush) = oneshot::channel::<()>();
            let flush_started_tx = Arc::new(Mutex::new(Some(flush_started_tx)));
            futures::executor::block_on(
                fixture
                    .runtime
                    .register_flush(Box::new(move || {
                        let started = flush_started_tx.clone();
                        let held = held_flush;
                        Box::pin(async move {
                            let tx = started.lock().unwrap().take();
                            if let Some(tx) = tx {
                                let _ = tx.send(());
                            }
                            let _ = held.await;
                            Ok(())
                        })
                    }))
                    .unwrap()
                    .receive(),
            )
            .unwrap();

            assert!(try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " pending-summary",
                cx
            ));
            assert!(cx.read(|cx| {
                fixture
                    .app_entity
                    .read(cx)
                    .workspace
                    .read(cx)
                    .note()
                    .read(cx)
                    .has_unsaved_summary(cx)
            }));

            let (check2_started_tx, mut check2_started_rx) = oneshot::channel::<()>();
            let (check2_release_tx, check2_release_rx) = oneshot::channel::<()>();
            let check2_started_tx = Arc::new(Mutex::new(Some(check2_started_tx)));
            let check2_release_rx = Arc::new(Mutex::new(Some(check2_release_rx)));
            let checks = Arc::new(AtomicUsize::new(0));
            let install_done = Arc::new(AtomicBool::new(false));
            let install_done_flag = install_done.clone();
            let app_for_task = fixture.app_entity.downgrade();

            let _install_task = cx.spawn(async move |mut cx| {
                let outcome = install_if_still_offered(
                    &"1.5.0",
                    "bytes",
                    &mut cx,
                    || {
                        let attempt = checks.fetch_add(1, Ordering::SeqCst);
                        let started = check2_started_tx.clone();
                        let release = check2_release_rx.clone();
                        async move {
                            if attempt == 0 {
                                Ok(Some("1.5.0"))
                            } else {
                                let tx = started.lock().unwrap().take();
                                if let Some(tx) = tx {
                                    let _ = tx.send(());
                                }
                                let rx = release.lock().unwrap().take();
                                if let Some(rx) = rx {
                                    let _ = rx.await;
                                }
                                Ok(Some("1.5.0"))
                            }
                        }
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| {
                                this.can_install_now(false, cx)
                                    .then(|| this.pause_writers(cx))
                            })
                            .map_err(|_| ServiceError::Closed)
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| this.can_install_now(false, cx))
                            .map_err(|_| ServiceError::Closed)
                    },
                    |_| async { panic!("update install must defer while quit drains") },
                )
                .await
                .unwrap();
                assert_eq!(outcome, InstallOutcome::Deferred("bytes"));
                app_for_task
                    .update(&mut cx, |this, cx| {
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                    })
                    .unwrap();
                install_done_flag.store(true, Ordering::SeqCst);
            });

            wait_until(cx, |_| {
                check2_started_rx.try_recv().ok().flatten().is_some()
            });
            cx.read(|cx| {
                let note = fixture
                    .app_entity
                    .read(cx)
                    .workspace
                    .read(cx)
                    .note()
                    .read(cx);
                assert!(!fixture.main_summary.read(cx).is_dirty());
                assert!(!note.has_unsaved_summary(cx));
                assert!(!note.has_unsaved_title(cx));
                assert!(
                    fixture
                        .main_summary
                        .read(cx)
                        .init
                        .document
                        .body
                        .contains("pending-summary")
                );
            });
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_memo,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " dropped",
                cx
            ));

            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.request_quit(cx);
                    assert!(app.closing);
                });
            });
            wait_until(cx, |_| flush_started_rx.try_recv().ok().flatten().is_some());

            check2_release_tx.send(()).unwrap();
            wait_until(cx, |_| install_done.load(Ordering::SeqCst));

            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_memo,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " dropped",
                cx
            ));

            release_flush.send(()).unwrap();
            wait_until(cx, |cx| cx.read(|cx| fixture.app_entity.read(cx).drained));
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }

        // Case 2: pre-shutdown quit failure restores editing in both orderings.
        {
            let fixture = open_app_with_detached_note(cx);

            // Ordering A: updater resumes writers while quit is closing, then quit fails pre-shutdown.
            let pause = cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| app.pause_writers(cx))
            });
            let (pause_tx, mut pause_rx) = oneshot::channel();
            cx.spawn(async move |_| {
                let _ = pause_tx.send(pause.await);
            })
            .detach();
            wait_until(cx, |_| {
                pause_rx
                    .try_recv()
                    .ok()
                    .flatten()
                    .transpose()
                    .unwrap()
                    .is_some()
            });
            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.workspace.update(cx, |ws, cx| {
                        ws.set_recording(fixture.session_id.clone(), true, cx)
                    });
                    app.request_quit(cx);
                    assert!(app.closing);
                    app.resume_writers(cx);
                });
            });
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_memo,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " dropped",
                cx
            ));

            wait_until(cx, |cx| cx.read(|cx| !fixture.app_entity.read(cx).closing));
            cx.read(|cx| assert!(!fixture.app_entity.read(cx).shutdown_begun()));
            verify_editable_and_flush(&fixture, "recovered", cx);

            // Ordering B: quit fails pre-shutdown while updater still holds pause, then updater resumes.
            let pause = cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.workspace.update(cx, |ws, cx| {
                        ws.set_recording(fixture.session_id.clone(), false, cx)
                    });
                    app.pause_writers(cx)
                })
            });
            let (pause_tx, mut pause_rx) = oneshot::channel();
            cx.spawn(async move |_| {
                let _ = pause_tx.send(pause.await);
            })
            .detach();
            wait_until(cx, |_| {
                pause_rx
                    .try_recv()
                    .ok()
                    .flatten()
                    .transpose()
                    .unwrap()
                    .is_some()
            });
            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.workspace.update(cx, |ws, cx| {
                        ws.set_recording(fixture.session_id.clone(), true, cx)
                    });
                    app.request_quit(cx);
                    assert!(app.closing);
                });
            });
            wait_until(cx, |cx| cx.read(|cx| !fixture.app_entity.read(cx).closing));
            cx.read(|cx| assert!(!fixture.app_entity.read(cx).shutdown_begun()));
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_memo,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " dropped",
                cx
            ));
            assert!(!try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " dropped",
                cx
            ));

            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.resume_writers(cx);
                });
            });
            verify_editable_and_flush(&fixture, "resumed", cx);
            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.workspace.update(cx, |ws, cx| {
                        ws.set_recording(fixture.session_id.clone(), false, cx)
                    });
                });
            });
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }
    }

    #[gpui::test]
    fn pause_writers_waits_for_summary_rename_before_installing(cx: &mut gpui::TestAppContext) {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        use futures::channel::oneshot;

        use crate::{contracts::EditorEvent, platform::windows::Restart};

        // Case A: an in-flight summary rename holds installation until it persists, then quits cleanly.
        {
            let fixture = open_app_with_detached_note(cx);
            assert!(try_edit(
                fixture.main_window,
                &fixture.main_summary,
                "Renamed ",
                cx
            ));

            let (rename_gate_started_tx, mut rename_gate_started_rx) = oneshot::channel::<()>();
            let (rename_gate_release_tx, rename_gate_release_rx) = oneshot::channel::<()>();
            let rename_gate_started_tx = Arc::new(Mutex::new(Some(rename_gate_started_tx)));
            let rename_gate_release_rx = Arc::new(Mutex::new(Some(rename_gate_release_rx)));
            let runtime_for_gate = fixture.runtime.clone();
            let _gate_sub = fixture.app_entity.update(cx, |_, cx| {
                cx.subscribe(
                    &fixture.main_summary,
                    move |_, _, event: &EditorEvent, _| {
                        if matches!(event, EditorEvent::Dirty { dirty: false, .. })
                            && let Some(release) = rename_gate_release_rx.lock().unwrap().take()
                        {
                            let started = rename_gate_started_tx.clone();
                            let _ = runtime_for_gate.submit(move |_| async move {
                                if let Some(tx) = started.lock().unwrap().take() {
                                    let _ = tx.send(());
                                }
                                let _ = release.await;
                                Ok(())
                            });
                        }
                    },
                )
            });

            let checks = Arc::new(AtomicUsize::new(0));
            let installed = Arc::new(AtomicBool::new(false));
            let checks_for_task = checks.clone();
            let installed_for_task = installed.clone();
            let app_for_task = fixture.app_entity.downgrade();
            let runtime_for_check = fixture.runtime.clone();
            let session_id = fixture.session_id.clone();

            let _install_task = cx.spawn(async move |mut cx| {
                let outcome = install_if_still_offered(
                    &"1.5.0",
                    "bytes",
                    &mut cx,
                    || {
                        checks_for_task.fetch_add(1, Ordering::SeqCst);
                        async { Ok(Some("1.5.0")) }
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| {
                                this.can_install_now(false, cx)
                                    .then(|| this.pause_writers(cx))
                            })
                            .map_err(|_| ServiceError::Closed)
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| this.can_install_now(false, cx))
                            .map_err(|_| ServiceError::Closed)
                    },
                    |_| {
                        let installed = installed_for_task.clone();
                        async move {
                            installed.store(true, Ordering::SeqCst);
                            Ok(Restart::App)
                        }
                    },
                )
                .await
                .unwrap();
                assert_eq!(outcome, InstallOutcome::Installed(Restart::App));
                let saved = runtime_for_check
                    .open_session(session_id, CancellationToken::new())
                    .unwrap()
                    .receive()
                    .await
                    .unwrap();
                assert!(
                    saved.summary.title.contains("Renamed "),
                    "{}",
                    saved.summary.title
                );
                app_for_task
                    .update(&mut cx, |this, cx| this.request_quit(cx))
                    .unwrap();
            });

            wait_until(cx, |_| {
                rename_gate_started_rx.try_recv().ok().flatten().is_some()
            });
            for _ in 0..20 {
                cx.executor().advance_clock(Duration::from_millis(5));
                cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(5));
            }
            cx.read(|cx| {
                let note = fixture
                    .app_entity
                    .read(cx)
                    .workspace
                    .read(cx)
                    .note()
                    .read(cx);
                assert!(note.is_renaming());
                assert_eq!(checks.load(Ordering::SeqCst), 1);
                assert!(!installed.load(Ordering::SeqCst));
            });

            rename_gate_release_tx.send(()).unwrap();
            wait_until(cx, |cx| cx.read(|cx| fixture.app_entity.read(cx).drained));
            assert_eq!(checks.load(Ordering::SeqCst), 2);
            assert!(installed.load(Ordering::SeqCst));
            drop(_gate_sub);
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }

        // Case B: a failed summary rename aborts installation and resumes editing.
        {
            let fixture = open_app_with_detached_note(cx);
            assert!(try_edit(
                fixture.main_window,
                &fixture.main_summary,
                "Conflict ",
                cx
            ));

            let injected = Arc::new(AtomicBool::new(false));
            let runtime_for_conflict = fixture.runtime.clone();
            let conflict_sid = fixture.session_id.0.clone();
            let _conflict_sub = fixture.app_entity.update(cx, |_, cx| {
                cx.subscribe(
                    &fixture.main_summary,
                    move |_, _, event: &EditorEvent, _| {
                        if matches!(event, EditorEvent::Dirty { dirty: false, .. })
                            && !injected.swap(true, Ordering::SeqCst)
                        {
                            let sid = conflict_sid.clone();
                            let _ = runtime_for_conflict.submit(move |services| async move {
                                services
                                    .executor
                                    .execute(
                                        "UPDATE sessions SET title = 'External title', updated_at = '2099-01-01T00:00:00.000Z' WHERE id = ?".into(),
                                        vec![serde_json::json!(sid)],
                                    )
                                    .await
                                    .map(|_| ())
                                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
                            });
                        }
                    },
                )
            });

            let done = Arc::new(AtomicBool::new(false));
            let done_flag = done.clone();
            let app_for_task = fixture.app_entity.downgrade();
            let _install_task = cx.spawn(async move |mut cx| {
                let outcome = install_if_still_offered(
                    &"1.5.0",
                    "bytes",
                    &mut cx,
                    || async { Ok(Some("1.5.0")) },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| {
                                this.can_install_now(false, cx)
                                    .then(|| this.pause_writers(cx))
                            })
                            .map_err(|_| ServiceError::Closed)
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| this.can_install_now(false, cx))
                            .map_err(|_| ServiceError::Closed)
                    },
                    |_| async { panic!("update install must abort on rename failure") },
                )
                .await;
                assert!(outcome.is_err());
                app_for_task
                    .update(&mut cx, |this, cx| {
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                    })
                    .unwrap();
                done_flag.store(true, Ordering::SeqCst);
            });

            wait_until(cx, |_| done.load(Ordering::SeqCst));
            assert!(try_edit(
                fixture.main_window,
                &fixture.main_memo,
                " resumed-memo",
                cx
            ));
            assert!(try_edit(
                fixture.main_window,
                &fixture.main_summary,
                " resumed-summary",
                cx
            ));
            assert!(try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " resumed-detached",
                cx
            ));
            drop(_conflict_sub);
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }

        // Case C: an in-flight detached summary rename holds installation until it persists, then quits cleanly.
        {
            let fixture = open_app_with_detached_note(cx);
            let (main_note, detached_note) = cx.read(|cx| {
                let app = fixture.app_entity.read(cx);
                (
                    app.workspace.read(cx).note().clone(),
                    app.note_windows[&fixture.session_id]
                        .read(cx)
                        .unwrap()
                        .note(),
                )
            });
            assert!(try_edit(
                fixture.detached_window,
                &fixture.detached_summary,
                "Detached ",
                cx
            ));
            cx.read(|cx| {
                assert!(detached_note.read(cx).has_unsaved_summary(cx));
                assert!(!main_note.read(cx).has_unsaved_summary(cx));
            });

            let (rename_gate_started_tx, mut rename_gate_started_rx) = oneshot::channel::<()>();
            let (rename_gate_release_tx, rename_gate_release_rx) = oneshot::channel::<()>();
            let rename_gate_started_tx = Arc::new(Mutex::new(Some(rename_gate_started_tx)));
            let rename_gate_release_rx = Arc::new(Mutex::new(Some(rename_gate_release_rx)));
            let runtime_for_gate = fixture.runtime.clone();
            let _gate_sub = fixture.app_entity.update(cx, |_, cx| {
                cx.subscribe(
                    &fixture.detached_summary,
                    move |_, _, event: &EditorEvent, _| {
                        if matches!(event, EditorEvent::Dirty { dirty: false, .. })
                            && let Some(release) = rename_gate_release_rx.lock().unwrap().take()
                        {
                            let started = rename_gate_started_tx.clone();
                            let _ = runtime_for_gate.submit(move |_| async move {
                                if let Some(tx) = started.lock().unwrap().take() {
                                    let _ = tx.send(());
                                }
                                let _ = release.await;
                                Ok(())
                            });
                        }
                    },
                )
            });

            let checks = Arc::new(AtomicUsize::new(0));
            let installed = Arc::new(AtomicBool::new(false));
            let checks_for_task = checks.clone();
            let installed_for_task = installed.clone();
            let app_for_task = fixture.app_entity.downgrade();
            let runtime_for_check = fixture.runtime.clone();
            let session_id = fixture.session_id.clone();

            let _install_task = cx.spawn(async move |mut cx| {
                let outcome = install_if_still_offered(
                    &"1.5.0",
                    "bytes",
                    &mut cx,
                    || {
                        checks_for_task.fetch_add(1, Ordering::SeqCst);
                        async { Ok(Some("1.5.0")) }
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| {
                                this.can_install_now(false, cx)
                                    .then(|| this.pause_writers(cx))
                            })
                            .map_err(|_| ServiceError::Closed)
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| this.can_install_now(false, cx))
                            .map_err(|_| ServiceError::Closed)
                    },
                    |_| {
                        let installed = installed_for_task.clone();
                        async move {
                            installed.store(true, Ordering::SeqCst);
                            Ok(Restart::App)
                        }
                    },
                )
                .await
                .unwrap();
                assert_eq!(outcome, InstallOutcome::Installed(Restart::App));
                let saved = runtime_for_check
                    .open_session(session_id, CancellationToken::new())
                    .unwrap()
                    .receive()
                    .await
                    .unwrap();
                assert!(
                    saved.summary.title.contains("Detached "),
                    "{}",
                    saved.summary.title
                );
                app_for_task
                    .update(&mut cx, |this, cx| this.request_quit(cx))
                    .unwrap();
            });

            wait_until(cx, |_| {
                rename_gate_started_rx.try_recv().ok().flatten().is_some()
            });
            for _ in 0..20 {
                cx.executor().advance_clock(Duration::from_millis(5));
                cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(5));
            }
            cx.read(|cx| {
                assert!(detached_note.read(cx).is_renaming());
                assert_eq!(checks.load(Ordering::SeqCst), 1);
                assert!(!installed.load(Ordering::SeqCst));
            });

            rename_gate_release_tx.send(()).unwrap();
            wait_until(cx, |cx| cx.read(|cx| fixture.app_entity.read(cx).drained));
            assert_eq!(checks.load(Ordering::SeqCst), 2);
            assert!(installed.load(Ordering::SeqCst));
            drop(_gate_sub);
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }

        // Case D: unlocked flush_drafts finishes without hanging on a later failing edit, and pause_writers surfaces the error.
        {
            let fixture = open_app_with_detached_note(cx);
            let detached_note = cx.read(|cx| {
                fixture.app_entity.read(cx).note_windows[&fixture.session_id]
                    .read(cx)
                    .unwrap()
                    .note()
            });

            let rev2_injected = Arc::new(AtomicBool::new(false));
            let rev2_injected_cb = rev2_injected.clone();
            let runtime_for_conflict = fixture.runtime.clone();
            let summary_for_sub = fixture.detached_summary.clone();
            let _rev2_sub = cx.update(|cx| {
                cx.update_window(fixture.detached_window, |_, window, cx| {
                    fixture.app_entity.update(cx, |_, cx| {
                        cx.subscribe_in(
                            &summary_for_sub,
                            window,
                            move |_, editor, event: &EditorEvent, window, cx| {
                                if matches!(event, EditorEvent::Saved(_))
                                    && !rev2_injected_cb.swap(true, Ordering::SeqCst)
                                {
                                    runtime_for_conflict
                                        .submit(|services| async move {
                                            services
                                                .executor
                                                .execute(
                                                    "UPDATE session_documents SET updated_at = '2099-01-01T00:00:00.000Z' WHERE id = 'quit-summary'".into(),
                                                    vec![],
                                                )
                                                .await
                                                .map(|_| ())
                                                .map_err(|error| ServiceError::Failed(error.to_string().into()))
                                        })
                                        .unwrap();
                                    let dirty = editor.update(cx, |editor, cx| {
                                        gpui::EntityInputHandler::replace_text_in_range(
                                            editor,
                                            None,
                                            "Rev2 ",
                                            window,
                                            cx,
                                        );
                                        editor.is_dirty()
                                    });
                                    assert!(dirty);
                                }
                            },
                        )
                    })
                })
                .unwrap()
            });

            assert!(try_edit(
                fixture.detached_window,
                &fixture.detached_summary,
                "Rev1 ",
                cx
            ));
            let flush = cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| app.flush_drafts(cx))
            });
            assert!(!rev2_injected.load(Ordering::SeqCst));

            let dirty_on_flush_return = Arc::new(AtomicBool::new(false));
            let dirty_on_flush_return_task = dirty_on_flush_return.clone();
            let summary_for_flush = fixture.detached_summary.clone();
            let (flush_tx, mut flush_rx) = oneshot::channel();
            cx.spawn(async move |cx| {
                let result = flush.await;
                dirty_on_flush_return_task.store(
                    cx.update(|cx| summary_for_flush.read(cx).is_dirty()),
                    Ordering::SeqCst,
                );
                let _ = flush_tx.send(result);
            })
            .detach();

            let mut flushed = None;
            wait_until(cx, |_| {
                flushed = flush_rx.try_recv().ok().flatten();
                flushed.is_some()
            });
            assert!(matches!(flushed, Some(Ok(()))), "{flushed:?}");
            assert!(dirty_on_flush_return.load(Ordering::SeqCst));

            wait_until(cx, |cx| {
                rev2_injected.load(Ordering::SeqCst)
                    && !cx.read(|cx| detached_note.read(cx).has_unsaved_title(cx))
            });
            cx.read(|cx| {
                assert!(
                    fixture
                        .detached_summary
                        .read(cx)
                        .init
                        .document
                        .body
                        .contains("Rev1 ")
                );
                assert!(fixture.detached_summary.read(cx).is_dirty());
            });
            drop(_rev2_sub);

            let pause = cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| app.pause_writers(cx))
            });
            let (pause_tx, mut pause_rx) = oneshot::channel();
            cx.spawn(async move |_| {
                let _ = pause_tx.send(pause.await);
            })
            .detach();
            let mut paused = None;
            wait_until(cx, |_| {
                paused = pause_rx.try_recv().ok().flatten();
                paused.is_some()
            });
            assert!(
                matches!(paused, Some(Err(ServiceError::Conflict))),
                "{paused:?}"
            );
            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    if app.writers_paused {
                        app.resume_writers(cx);
                    }
                });
            });
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }
    }

    struct GatedCapture {
        capture: crate::meeting::capture::CaptureService,
        resolver_calls: Arc<std::sync::atomic::AtomicUsize>,
        resolver_started_rx: futures::channel::oneshot::Receiver<()>,
        resolver_release_tx: Option<futures::channel::oneshot::Sender<()>>,
    }

    fn install_gated_capture(
        fixture: &DetachedNoteFixture,
        cx: &mut gpui::TestAppContext,
    ) -> GatedCapture {
        use std::sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        };

        use futures::channel::oneshot;

        let resolver_calls = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = oneshot::channel::<()>();
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let calls_for_resolver = resolver_calls.clone();
        let started_tx = Arc::new(Mutex::new(Some(started_tx)));
        let release_rx = Arc::new(Mutex::new(Some(release_rx)));
        let resolver: crate::meeting::capture::StartResolver = Arc::new(move |_| {
            let calls = calls_for_resolver.clone();
            let started = started_tx.clone();
            let release = release_rx.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                let tx = started.lock().unwrap().take();
                if let Some(tx) = tx {
                    let _ = tx.send(());
                }
                let rx = release.lock().unwrap().take();
                if let Some(rx) = rx {
                    let _ = rx.await;
                }
                Err(ServiceError::Cancelled)
            })
        });
        let storage = Arc::new(super::super::ProfileStorage {
            root: fixture._profile.path().to_path_buf(),
            vault: fixture._profile.path().to_path_buf(),
        });
        let capture = crate::meeting::capture::CaptureService::spawn(
            fixture.runtime.clone(),
            Arc::new(anlg_audio_mock::MockAudio::new(1)),
            storage,
            resolver,
        )
        .unwrap();
        cx.update(|cx| {
            fixture.app_entity.update(cx, |app, _| {
                app.services.as_mut().unwrap().capture = capture.clone();
            });
            cx.global_mut::<crate::meeting::MeetingServices>().capture = capture.clone();
        });
        let mut ready_rx = capture.devices().unwrap();
        wait_until(cx, |_| ready_rx.try_recv().is_ok());
        GatedCapture {
            capture,
            resolver_calls,
            resolver_started_rx: started_rx,
            resolver_release_tx: Some(release_tx),
        }
    }

    #[gpui::test]
    fn capture_guards_block_start_during_pause_or_quit_and_defer_manual_update(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        use futures::channel::oneshot;

        // Subcase 1: capture requests are ignored/blocked while writers are paused, closing, or drained.
        {
            let fixture = open_app_with_detached_note(cx);
            let mut gated = install_gated_capture(&fixture, cx);

            let pause = cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| app.pause_writers(cx))
            });
            let (pause_tx, mut pause_rx) = oneshot::channel();
            cx.spawn(async move |_| {
                let _ = pause_tx.send(pause.await);
            })
            .detach();
            wait_until(cx, |_| {
                pause_rx
                    .try_recv()
                    .ok()
                    .flatten()
                    .transpose()
                    .unwrap()
                    .is_some()
            });

            cx.update(|cx| {
                cx.update_window(fixture.main_window, |_, window, cx| {
                    fixture.app_entity.update(cx, |app, cx| {
                        app.automation_request(
                            &crate::meeting::automation::Request::OpenSession {
                                session: fixture.session_id.clone(),
                                listen: true,
                            },
                            window,
                            cx,
                        );
                    });
                })
                .unwrap();
            });
            let mut devices_rx = gated.capture.devices().unwrap();
            wait_until(cx, |_| devices_rx.try_recv().is_ok());
            assert_eq!(gated.resolver_calls.load(Ordering::SeqCst), 0);
            cx.read(|cx| assert!(!fixture.app_entity.read(cx).meeting_live()));

            cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| app.resume_writers(cx));
            });

            let (flush_started_tx, mut flush_started_rx) = oneshot::channel::<()>();
            let (release_flush, held_flush) = oneshot::channel::<()>();
            let flush_started_tx = Arc::new(Mutex::new(Some(flush_started_tx)));
            futures::executor::block_on(
                fixture
                    .runtime
                    .register_flush(Box::new(move || {
                        let started = flush_started_tx.clone();
                        let held = held_flush;
                        Box::pin(async move {
                            let tx = started.lock().unwrap().take();
                            if let Some(tx) = tx {
                                let _ = tx.send(());
                            }
                            let _ = held.await;
                            Ok(())
                        })
                    }))
                    .unwrap()
                    .receive(),
            )
            .unwrap();

            let (save_gate_started_tx, mut save_gate_started_rx) = oneshot::channel::<()>();
            let (save_gate_release_tx, save_gate_release_rx) = oneshot::channel::<()>();
            fixture
                .runtime
                .submit(move |_| async move {
                    let _ = save_gate_started_tx.send(());
                    let _ = save_gate_release_rx.await;
                    Ok(())
                })
                .unwrap();
            wait_until(cx, |_| {
                save_gate_started_rx.try_recv().ok().flatten().is_some()
            });

            assert!(try_edit(
                fixture.detached_window,
                &fixture.detached_memo,
                " quit-draft",
                cx
            ));
            cx.update(|cx| {
                fixture.app_entity.update(cx, |app, cx| {
                    app.request_quit(cx);
                    assert!(
                        app.closing && !app.shutdown_begun() && !app.writers_paused && !app.drained
                    );
                });
            });

            cx.update(|cx| {
                cx.update_window(fixture.main_window, |_, window, cx| {
                    fixture.app_entity.update(cx, |app, cx| {
                        app.automation_request(
                            &crate::meeting::automation::Request::OpenSession {
                                session: fixture.session_id.clone(),
                                listen: true,
                            },
                            window,
                            cx,
                        );
                        app.open_meeting(
                            MeetingIntent::Start {
                                session_id: fixture.session_id.clone(),
                            },
                            window,
                            cx,
                        );
                    });
                })
                .unwrap();
            });
            let mut devices_rx = gated.capture.devices().unwrap();
            wait_until(cx, |_| devices_rx.try_recv().is_ok());
            assert_eq!(gated.resolver_calls.load(Ordering::SeqCst), 0);
            cx.read(|cx| {
                let app = fixture.app_entity.read(cx);
                assert!(!app.meeting_live());
                assert!(app.closing && !app.shutdown_begun());
            });

            save_gate_release_tx.send(()).unwrap();
            wait_until(cx, |_| flush_started_rx.try_recv().ok().flatten().is_some());
            cx.read(|cx| {
                let app = fixture.app_entity.read(cx);
                assert!(app.closing && app.shutdown_begun());
            });

            cx.update(|cx| {
                cx.update_window(fixture.main_window, |_, window, cx| {
                    fixture.app_entity.update(cx, |app, cx| {
                        app.automation_request(
                            &crate::meeting::automation::Request::OpenSession {
                                session: fixture.session_id.clone(),
                                listen: true,
                            },
                            window,
                            cx,
                        );
                        app.open_meeting(
                            MeetingIntent::Start {
                                session_id: fixture.session_id.clone(),
                            },
                            window,
                            cx,
                        );
                    });
                })
                .unwrap();
            });
            let mut devices_rx = gated.capture.devices().unwrap();
            wait_until(cx, |_| devices_rx.try_recv().is_ok());
            assert_eq!(gated.resolver_calls.load(Ordering::SeqCst), 0);
            cx.read(|cx| assert!(!fixture.app_entity.read(cx).meeting_live()));

            release_flush.send(()).unwrap();
            wait_until(cx, |cx| cx.read(|cx| fixture.app_entity.read(cx).drained));

            cx.update(|cx| {
                cx.update_window(fixture.main_window, |_, window, cx| {
                    fixture.app_entity.update(cx, |app, cx| {
                        app.open_meeting(
                            MeetingIntent::Start {
                                session_id: fixture.session_id.clone(),
                            },
                            window,
                            cx,
                        );
                    });
                })
                .unwrap();
            });
            let mut devices_rx = gated.capture.devices().unwrap();
            wait_until(cx, |_| devices_rx.try_recv().is_ok());
            assert_eq!(gated.resolver_calls.load(Ordering::SeqCst), 0);
            cx.read(|cx| assert!(!fixture.app_entity.read(cx).meeting_live()));

            drop(gated.resolver_release_tx.take());
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }

        // Subcase 2: manual install defers and preserves update bytes if capture starts during feed recheck.
        {
            let fixture = open_app_with_detached_note(cx);
            let mut gated = install_gated_capture(&fixture, cx);

            let (check2_started_tx, mut check2_started_rx) = oneshot::channel::<()>();
            let (check2_release_tx, check2_release_rx) = oneshot::channel::<()>();
            let check2_started_tx = Arc::new(Mutex::new(Some(check2_started_tx)));
            let check2_release_rx = Arc::new(Mutex::new(Some(check2_release_rx)));
            let checks = Arc::new(AtomicUsize::new(0));
            let done = Arc::new(AtomicBool::new(false));
            let done_flag = done.clone();
            let app_for_task = fixture.app_entity.downgrade();

            let _install_task = cx.spawn(async move |mut cx| {
                let outcome = install_if_still_offered(
                    &"1.5.0",
                    "bytes",
                    &mut cx,
                    || {
                        let attempt = checks.fetch_add(1, Ordering::SeqCst);
                        let started = check2_started_tx.clone();
                        let release = check2_release_rx.clone();
                        async move {
                            if attempt == 0 {
                                Ok(Some("1.5.0"))
                            } else {
                                let tx = started.lock().unwrap().take();
                                if let Some(tx) = tx {
                                    let _ = tx.send(());
                                }
                                let rx = release.lock().unwrap().take();
                                if let Some(rx) = rx {
                                    let _ = rx.await;
                                }
                                Ok(Some("1.5.0"))
                            }
                        }
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| {
                                this.can_install_now(false, cx)
                                    .then(|| this.pause_writers(cx))
                            })
                            .map_err(|_| ServiceError::Closed)
                    },
                    |cx| {
                        app_for_task
                            .update(cx, |this, cx| this.can_install_now(false, cx))
                            .map_err(|_| ServiceError::Closed)
                    },
                    |_| async { panic!("manual install must defer when capture goes live") },
                )
                .await
                .unwrap();
                assert_eq!(outcome, InstallOutcome::Deferred("bytes"));
                app_for_task
                    .update(&mut cx, |this, cx| {
                        if this.writers_paused {
                            this.resume_writers(cx);
                        }
                    })
                    .unwrap();
                done_flag.store(true, Ordering::SeqCst);
            });

            wait_until(cx, |_| {
                check2_started_rx.try_recv().ok().flatten().is_some()
            });
            cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| assert!(app.can_install_now(false, cx)));
            });

            let mut start_rx = gated.capture.start(fixture.session_id.clone()).unwrap();
            wait_until(cx, |cx| {
                gated
                    .resolver_started_rx
                    .try_recv()
                    .ok()
                    .flatten()
                    .is_some()
                    && cx.read(|cx| fixture.app_entity.read(cx).meeting_live())
            });
            cx.update(|cx| {
                fixture
                    .app_entity
                    .update(cx, |app, cx| assert!(!app.can_install_now(false, cx)));
            });

            check2_release_tx.send(()).unwrap();
            wait_until(cx, |_| done.load(Ordering::SeqCst));

            if let Some(release) = gated.resolver_release_tx.take() {
                let _ = release.send(());
            }
            wait_until(cx, |cx| {
                matches!(start_rx.try_recv(), Ok(Err(_)))
                    && !cx.read(|cx| fixture.app_entity.read(cx).meeting_live())
            });
            verify_editable_and_flush(&fixture, "manual-defer", cx);
            let _ = futures::executor::block_on(fixture.runtime.shutdown());
        }
    }

    #[cfg(target_os = "macos")]
    #[gpui::test]
    fn retried_quit_after_failed_drain_runs_pending_macos_restart(cx: &mut gpui::TestAppContext) {
        use std::os::unix::fs::PermissionsExt;

        struct ClearRestart(PathBuf);
        impl Drop for ClearRestart {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = crate::platform::windows::run_requested_restart();
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("relaunch.sh");
        let marker = dir.path().join("restarted");
        std::fs::write(&script, "#!/bin/sh\n: > \"$(dirname \"$0\")/restarted\"\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _clear = ClearRestart(script.clone());

        let fixture = open_app_with_detached_note(cx);
        futures::executor::block_on(
            fixture
                .runtime
                .register_flush(Box::new(|| {
                    Box::pin(async { Err(ServiceError::Failed("simulated drain failure".into())) })
                }))
                .unwrap()
                .receive(),
        )
        .unwrap();

        crate::platform::windows::request_restart(Restart::Installer(script));
        cx.update(|cx| {
            fixture
                .app_entity
                .update(cx, |app, cx| app.request_quit(cx));
        });
        wait_until(cx, |cx| {
            cx.read(|cx| {
                let app = fixture.app_entity.read(cx);
                app.drained && !app.closing
            })
        });
        assert!(!marker.exists());

        cx.update(|cx| {
            fixture
                .app_entity
                .update(cx, |app, cx| app.request_quit(cx));
        });
        wait_until(cx, |_| marker.exists());
        let _ = futures::executor::block_on(fixture.runtime.shutdown());
    }

    #[gpui::test]
    fn settings_requested_before_native_ready_opens_section_unless_dismissed(
        cx: &mut gpui::TestAppContext,
    ) {
        crate::platform::os::go_headless();
        cx.executor().allow_parking();
        cx.update(crate::ui::theme::UiFonts::init);

        let start_app = |cx: &mut gpui::TestAppContext| {
            let profile = tempfile::tempdir().unwrap();
            let (runtime, ready) = crate::runtime_bridge::start(desktop_runtime::Profile {
                database: profile.path().join("library.sqlite"),
            })
            .unwrap();
            let mut state = runtime.state();
            while matches!(
                *state.borrow_and_update(),
                desktop_runtime::RuntimeState::Starting
            ) {
                futures::executor::block_on(state.changed()).unwrap();
            }
            futures::executor::block_on(
                runtime
                    .submit(|services| async move {
                        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, 'false')")
                            .bind(crate::product::onboarding::NEEDED_KEY)
                            .execute(services.db.pool())
                            .await
                            .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                        let pinned = crate::workspace::pins::encode(&[
                            crate::workspace::navigation::Route::settings("general"),
                            crate::workspace::navigation::Route::Calendar,
                        ])
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                        sqlx::query(
                            "INSERT INTO app_settings (id, value_json) VALUES ('gpui_pinned_tabs', ?)",
                        )
                        .bind(pinned)
                        .execute(services.db.pool())
                        .await
                        .map(|_| ())
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))
                    })
                    .unwrap()
                    .receive(),
            )
            .unwrap();
            let rt = runtime.clone();
            let root_path = profile.path().to_path_buf();
            let window = cx.add_window(|window, cx| {
                ApplicationView::new(
                    rt,
                    ready,
                    crate::launch::Storage {
                        channel: None,
                        root: root_path.clone(),
                        database_dir: root_path.clone(),
                        pointer: Some(root_path.join("storage.json")),
                    },
                    window,
                    cx,
                )
            });
            let app = window.update(cx, |_, _, cx| cx.entity()).unwrap();
            (profile, runtime, window, app)
        };

        // Case 1: requested Settings section opens once native startup finishes.
        {
            let _quit_guard = QUIT_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let (_profile, runtime, window, app) = start_app(cx);
            let main_window: gpui::AnyWindowHandle = window.into();
            cx.update(|cx| {
                app.update(cx, |app, cx| {
                    assert!(!app.native_ready());
                    app.workspace.update(cx, |workspace, cx| {
                        workspace.open_route(
                            crate::workspace::navigation::Route::settings("transcription"),
                            false,
                            cx,
                        );
                    });
                });
            });
            cx.read(|cx| {
                let app = app.read(cx);
                assert!(app.product.is_none());
                assert!(app.pending_product.is_some());
            });
            wait_until(cx, |cx| {
                cx.executor().advance_clock(Duration::from_millis(5));
                cx.read(|cx| {
                    let app = app.read(cx);
                    app.pins_revision.is_some() && app.native_ready() && app.product.is_some()
                })
            });
            cx.read(|cx| {
                let app = app.read(cx);
                let product = app.product.as_ref().unwrap().read(cx);
                assert_eq!(product.active_section(), "transcription");
                assert_eq!(
                    app.workspace.read(cx).current_route(),
                    crate::workspace::navigation::Route::settings("transcription")
                );
            });
            let _ = cx
                .update(|cx| cx.update_window(main_window, |_, window, _| window.remove_window()));
            let _ = futures::executor::block_on(runtime.shutdown());
        }

        // Case 2: dismissing loading Settings with Escape cancels pending_product so it stays closed.
        {
            let _quit_guard = QUIT_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let (_profile, runtime, window, app) = start_app(cx);
            let main_window: gpui::AnyWindowHandle = window.into();
            cx.update(|cx| {
                app.update(cx, |app, cx| {
                    assert!(!app.native_ready());
                    app.workspace.update(cx, |workspace, cx| {
                        workspace.open_route(
                            crate::workspace::navigation::Route::settings("transcription"),
                            false,
                            cx,
                        );
                    });
                });
            });
            cx.read(|cx| {
                assert!(app.read(cx).pending_product.is_some());
            });
            let _ = cx.update(|cx| {
                cx.update_window(main_window, |_, window, cx| {
                    let workspace = app.read(cx).workspace.clone();
                    workspace.update(cx, |workspace, cx| {
                        workspace.dismiss_overlay(
                            &gpui::KeyDownEvent {
                                keystroke: gpui::Keystroke::parse("escape").unwrap(),
                                is_held: false,
                                prefer_character_input: false,
                            },
                            window,
                            cx,
                        );
                    });
                })
            });
            cx.read(|cx| {
                let app = app.read(cx);
                assert!(!app.native_ready());
                assert_eq!(
                    app.workspace.read(cx).current_route(),
                    crate::workspace::navigation::Route::Empty
                );
                assert!(app.pending_product.is_none());
            });
            wait_until(cx, |cx| {
                cx.executor().advance_clock(Duration::from_millis(5));
                cx.read(|cx| app.read(cx).native_ready())
            });
            cx.run_until_parked();
            cx.read(|cx| {
                assert!(app.read(cx).product.is_none());
            });
            let _ = cx
                .update(|cx| cx.update_window(main_window, |_, window, _| window.remove_window()));
            let _ = futures::executor::block_on(runtime.shutdown());
        }

        // Case 3: tray-origin Settings requested from Route::Empty cancels if the user navigates away.
        {
            let _quit_guard = QUIT_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let (_profile, runtime, window, app) = start_app(cx);
            let main_window: gpui::AnyWindowHandle = window.into();
            let _ = cx.update(|cx| {
                cx.update_window(main_window, |_, window, cx| {
                    app.update(cx, |app, cx| {
                        assert!(!app.native_ready());
                        app.open_product(crate::contracts::ProductRoute::Settings, window, cx);
                        assert!(app.pending_product.is_some());
                        app.workspace.update(cx, |workspace, cx| {
                            workspace.open_route(
                                crate::workspace::navigation::Route::Calendar,
                                false,
                                cx,
                            );
                        });
                    });
                })
            });
            cx.read(|cx| {
                assert!(app.read(cx).pending_product.is_none());
            });
            wait_until(cx, |cx| {
                cx.executor().advance_clock(Duration::from_millis(5));
                cx.read(|cx| app.read(cx).native_ready())
            });
            cx.run_until_parked();
            cx.read(|cx| {
                assert!(app.read(cx).product.is_none());
                assert_eq!(
                    app.read(cx).workspace.read(cx).current_route(),
                    crate::workspace::navigation::Route::Calendar
                );
            });
            let _ = cx
                .update(|cx| cx.update_window(main_window, |_, window, _| window.remove_window()));
            let _ = futures::executor::block_on(runtime.shutdown());
        }
    }
}
