//! The system tray (`plugins/tray`): the same icon states, agenda sections,
//! and menu as the Tauri app's `TrayPluginExt`, driven by the shared
//! `anlg-tray-core` schedule logic. On Linux the `tray-icon` crate needs a
//! GTK main loop, which runs on its own thread; the shell talks to it through
//! a command channel and receives menu clicks back as [`TrayAction`]s.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use anlg_tray_core::TrayScheduleEvent;
use chrono::{Datelike, Duration, TimeZone, Utc};

use crate::timeline::EventRow;

pub const SCOPE: &str = "anlg-tray";
pub const SHOW_EVENTS_KEY: &str = "show_events_in_menu_bar";

/// A tray menu click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    Start,
    Settings,
    ToggleShowEvents,
    Hide,
    QuitCompletely,
    /// An agenda row: the calendar event id.
    Agenda(String),
}

/// What the shell tells the tray.
#[derive(Debug, Clone, PartialEq)]
pub enum TrayCommand {
    Visible(bool),
    Schedule(Vec<TrayScheduleEvent>),
    ShowEvents(bool),
    Recording(bool),
    Degraded(bool),
    StartDisabled(bool),
}

/// The menu's inputs, kept on the tray thread.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayState {
    pub app_name: String,
    pub version_label: String,
    pub schedule: Vec<TrayScheduleEvent>,
    pub show_events: bool,
    pub start_disabled: bool,
    pub recording: bool,
    pub degraded: bool,
}

/// `TrayVersion::get_channel` over the bundle identifier.
pub fn channel(identifier: &str) -> &'static str {
    match identifier {
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "stable",
        "com.hyprnote.staging" => "staging",
        _ => "dev",
    }
}

/// `productName` of the Tauri build that owns the identifier.
pub fn app_name(identifier: &str) -> &'static str {
    match channel(identifier) {
        "stable" => "Anarlog",
        "staging" => "Anarlog Staging",
        _ => "Anarlog Dev",
    }
}

const ID_OPEN: &str = "anlg_tray_open";
const ID_START: &str = "anlg_tray_start";
const ID_SETTINGS: &str = "anlg_tray_settings";
const ID_SHOW_EVENTS: &str = "anlg_tray_show_events";
const ID_VERSION: &str = "anlg_tray_version";
const ID_HIDE: &str = "anlg_tray_hide";
const ID_QUIT_COMPLETELY: &str = "anlg_tray_quit_completely";
const AGENDA_PREFIX: &str = "anlg_tray_agenda_";
const AGENDA_SECTION_PREFIX: &str = "anlg_tray_agenda_section_";

/// `AnlgMenuItem::try_from` + `handle_agenda_menu_event` over a menu id.
pub fn action_for(id: &str) -> Option<TrayAction> {
    match id {
        ID_OPEN => Some(TrayAction::Open),
        ID_START => Some(TrayAction::Start),
        ID_SETTINGS => Some(TrayAction::Settings),
        ID_SHOW_EVENTS => Some(TrayAction::ToggleShowEvents),
        ID_HIDE => Some(TrayAction::Hide),
        ID_QUIT_COMPLETELY => Some(TrayAction::QuitCompletely),
        _ if id.starts_with(AGENDA_SECTION_PREFIX) => None,
        _ => id
            .strip_prefix(AGENDA_PREFIX)
            .map(|event_id| TrayAction::Agenda(event_id.to_string())),
    }
}

/// `buildTrayScheduleEvents`: the timed, non-ignored events starting within
/// seven days that have not ended, sorted by start then title, with the
/// `Intl.DateTimeFormat({ hour: "numeric", minute: "2-digit" })` labels and
/// the day starts in `tz`.
pub fn schedule_events<Tz: TimeZone>(
    rows: &[EventRow],
    is_ignored: impl Fn(&EventRow) -> bool,
    now: chrono::DateTime<Utc>,
    tz: &Tz,
) -> Vec<TrayScheduleEvent>
where
    Tz::Offset: std::fmt::Display,
{
    const HORIZON: i64 = 7 * 24 * 60 * 60 * 1000;
    let now_ms = now.timestamp_millis();
    let mut events: Vec<TrayScheduleEvent> = rows
        .iter()
        .filter(|row| row.is_all_day == 0 && !is_ignored(row))
        .filter_map(|row| {
            let start = crate::timeline::parse_date(&row.started_at, tz)?;
            let starts_at_ms = start.timestamp_millis();
            if starts_at_ms > now_ms + HORIZON {
                return None;
            }
            let ends_at_ms = crate::timeline::parse_date(&row.ended_at, tz)
                .map(|end| end.timestamp_millis())
                .filter(|end| *end > starts_at_ms);
            match ends_at_ms {
                Some(end) if end <= now_ms => return None,
                None if starts_at_ms <= now_ms => return None,
                _ => {}
            }
            let local_start = start.with_timezone(tz);
            let day_start = tz
                .with_ymd_and_hms(
                    local_start.year(),
                    local_start.month(),
                    local_start.day(),
                    0,
                    0,
                    0,
                )
                .single()?;
            let previous_day_start = day_start.clone() - Duration::days(1);
            let time = |ms: i64| {
                tz.timestamp_millis_opt(ms)
                    .single()
                    .map(|t| t.format("%-I:%M %p").to_string())
                    .unwrap_or_default()
            };
            let time_label = match ends_at_ms {
                Some(end) => format!("{} – {}", time(starts_at_ms), time(end)),
                None => time(starts_at_ms),
            };
            let title = row.title.trim();
            Some(TrayScheduleEvent {
                id: row.id.clone(),
                title: if title.is_empty() {
                    "Untitled event".to_string()
                } else {
                    title.to_string()
                },
                meeting_link: (!row.meeting_link.is_empty()).then(|| row.meeting_link.clone()),
                starts_at_ms: starts_at_ms as f64,
                ends_at_ms: ends_at_ms.map(|end| end as f64),
                day_start_ms: day_start.timestamp_millis() as f64,
                previous_day_start_ms: previous_day_start.timestamp_millis() as f64,
                time_label,
            })
        })
        .collect();
    events.sort_by(|left, right| {
        left.starts_at_ms
            .total_cmp(&right.starts_at_ms)
            .then_with(|| left.title.cmp(&right.title))
    });
    events
}

/// The shell's handle on the tray thread.
pub struct Tray {
    commands: Sender<TrayCommand>,
    actions: Arc<Mutex<Receiver<TrayAction>>>,
}

impl gpui::Global for Tray {}

impl Tray {
    /// Start the tray (hidden until `SetVisible(true)`).
    pub fn start(state: TrayState) -> Self {
        let (commands, command_rx) = std::sync::mpsc::channel::<TrayCommand>();
        let (action_tx, actions) = std::sync::mpsc::channel::<TrayAction>();
        platform::spawn(state, command_rx, action_tx);
        Self {
            commands,
            actions: Arc::new(Mutex::new(actions)),
        }
    }

    pub fn send(&self, command: TrayCommand) {
        if self.commands.send(command).is_err() {
            tracing::debug!("tray thread is gone");
        }
    }

    /// The clicks since the last poll.
    pub fn take_actions(&self) -> Vec<TrayAction> {
        let Ok(receiver) = self.actions.lock() else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        while let Ok(action) = receiver.try_recv() {
            actions.push(action);
        }
        actions
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc::{Receiver, Sender, TryRecvError};
    use std::time::Duration;

    use anlg_tray_core::{agenda_sections, icons, labels};
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    use super::{
        AGENDA_PREFIX, AGENDA_SECTION_PREFIX, ID_HIDE, ID_OPEN, ID_QUIT_COMPLETELY, ID_SETTINGS,
        ID_SHOW_EVENTS, ID_START, ID_VERSION, TrayAction, TrayCommand, TrayState,
    };

    struct Inner {
        state: TrayState,
        icon: Option<TrayIcon>,
        frame: usize,
    }

    /// `linux_tray_backend_available`: skip the tray without an appindicator.
    fn backend_available() -> bool {
        [
            "libayatana-appindicator3.so.1",
            "libappindicator3.so.1",
            "libayatana-appindicator3.so",
            "libappindicator3.so",
        ]
        .iter()
        .any(|name| unsafe { libloading::Library::new(name) }.is_ok())
    }

    fn decode(png: &[u8]) -> Option<Icon> {
        let image = image::load_from_memory(png).ok()?.into_rgba8();
        let (width, height) = image.dimensions();
        Icon::from_rgba(image.into_raw(), width, height).ok()
    }

    /// `build_tray_menu`
    fn build_menu(state: &TrayState) -> Menu {
        let menu = Menu::new();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        for (section_index, section) in agenda_sections(&state.schedule, now_ms, state.show_events)
            .iter()
            .enumerate()
        {
            let _ = menu.append(&MenuItem::with_id(
                format!("{AGENDA_SECTION_PREFIX}{section_index}"),
                &section.label,
                false,
                None,
            ));
            for event in &section.events {
                let _ = menu.append(&MenuItem::with_id(
                    format!("{AGENDA_PREFIX}{}", event.id),
                    &event.label,
                    true,
                    None,
                ));
            }
        }
        let _ = menu.append(&CheckMenuItem::with_id(
            ID_SHOW_EVENTS,
            labels::SHOW_EVENTS,
            true,
            state.show_events,
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            ID_OPEN,
            labels::open(&state.app_name),
            true,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            ID_START,
            labels::START,
            !state.start_disabled,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            ID_SETTINGS,
            labels::SETTINGS,
            true,
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            ID_VERSION,
            &state.version_label,
            false,
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(ID_HIDE, labels::HIDE, true, None));
        let _ = menu.append(&MenuItem::with_id(
            ID_QUIT_COMPLETELY,
            labels::QUIT_COMPLETELY,
            true,
            None,
        ));
        menu
    }

    /// `refresh_icon` for the still states; the recording frames cycle from
    /// the animation timer.
    fn still_icon(state: &TrayState) -> Option<Icon> {
        decode(if state.degraded {
            icons::DEGRADED
        } else {
            icons::DEFAULT
        })
    }

    impl Inner {
        fn apply(&mut self, command: TrayCommand) {
            match command {
                TrayCommand::Visible(visible) => {
                    if visible {
                        match &self.icon {
                            Some(icon) => {
                                let _ = icon.set_visible(true);
                            }
                            None => {
                                let Some(icon) = still_icon(&self.state) else {
                                    return;
                                };
                                match TrayIconBuilder::new()
                                    .with_id("anlg-tray")
                                    .with_icon(icon)
                                    .with_icon_as_template(true)
                                    .with_menu(Box::new(build_menu(&self.state)))
                                    .with_menu_on_left_click(true)
                                    .build()
                                {
                                    Ok(icon) => self.icon = Some(icon),
                                    Err(error) => {
                                        tracing::warn!(%error, "failed to create tray icon")
                                    }
                                }
                            }
                        }
                    } else if let Some(icon) = &self.icon {
                        let _ = icon.set_visible(false);
                    }
                }
                TrayCommand::Schedule(mut events) => {
                    events.retain(|event| event.starts_at_ms.is_finite());
                    events.sort_by(|left, right| left.starts_at_ms.total_cmp(&right.starts_at_ms));
                    self.state.schedule = events;
                    self.rebuild_menu();
                }
                TrayCommand::ShowEvents(show) => {
                    self.state.show_events = show;
                    self.rebuild_menu();
                }
                TrayCommand::Recording(recording) => {
                    self.state.recording = recording;
                    self.frame = 0;
                    self.refresh_icon();
                }
                TrayCommand::Degraded(degraded) => {
                    self.state.degraded = degraded;
                    self.refresh_icon();
                }
                TrayCommand::StartDisabled(disabled) => {
                    self.state.start_disabled = disabled;
                    self.rebuild_menu();
                }
            }
        }

        fn rebuild_menu(&mut self) {
            if let Some(icon) = &self.icon {
                icon.set_menu(Some(Box::new(build_menu(&self.state))));
            }
        }

        fn refresh_icon(&mut self) {
            let Some(icon) = &self.icon else {
                return;
            };
            if self.state.recording && !self.state.degraded {
                let frame = icons::RECORDING_FRAMES[self.frame % icons::RECORDING_FRAMES.len()];
                if let Some(image) = decode(frame) {
                    let _ = icon.set_icon(Some(image));
                }
            } else if let Some(image) = still_icon(&self.state) {
                let _ = icon.set_icon(Some(image));
            }
        }

        /// The 250ms recording animation tick.
        fn animate(&mut self) {
            if self.state.recording && !self.state.degraded && self.icon.is_some() {
                self.frame = (self.frame + 1) % icons::RECORDING_FRAMES.len();
                self.refresh_icon();
            }
        }
    }

    pub(super) fn spawn(
        state: TrayState,
        commands: Receiver<TrayCommand>,
        actions: Sender<TrayAction>,
    ) {
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                if !backend_available() {
                    tracing::warn!("appindicator_library_missing_skipping_tray_icon");
                    return;
                }
                if gtk::init().is_err() {
                    tracing::warn!("gtk init failed; no tray icon");
                    return;
                }
                MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                    if let Some(action) = super::action_for(event.id().as_ref()) {
                        let _ = actions.send(action);
                    }
                }));
                let inner = Rc::new(RefCell::new(Inner {
                    state,
                    icon: None,
                    frame: 0,
                }));
                let poll = inner.clone();
                gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                    loop {
                        match commands.try_recv() {
                            Ok(command) => poll.borrow_mut().apply(command),
                            Err(TryRecvError::Empty) => break gtk::glib::ControlFlow::Continue,
                            Err(TryRecvError::Disconnected) => {
                                gtk::main_quit();
                                break gtk::glib::ControlFlow::Break;
                            }
                        }
                    }
                });
                let animate = inner.clone();
                gtk::glib::timeout_add_local(
                    Duration::from_millis(icons::RECORDING_FRAME_MS),
                    move || {
                        animate.borrow_mut().animate();
                        gtk::glib::ControlFlow::Continue
                    },
                );
                gtk::main();
            })
            .ok();
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::sync::mpsc::{Receiver, Sender};

    use super::{TrayAction, TrayCommand, TrayState};

    /// The macOS and Windows sidecars are not shipped yet; the commands are
    /// drained so senders never block.
    pub(super) fn spawn(
        _state: TrayState,
        commands: Receiver<TrayCommand>,
        _actions: Sender<TrayAction>,
    ) {
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || while commands.recv().is_ok() {})
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, title: &str, started: &str, ended: &str) -> EventRow {
        EventRow {
            id: id.into(),
            title: title.into(),
            started_at: started.into(),
            ended_at: ended.into(),
            tracking_id_event: format!("track-{id}"),
            is_all_day: 0,
            meeting_link: String::new(),
            calendar_color: String::new(),
            calendar_id: String::new(),
            recurrence_series_id: String::new(),
            location: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn maps_menu_ids_to_actions() {
        assert_eq!(action_for("anlg_tray_open"), Some(TrayAction::Open));
        assert_eq!(action_for("anlg_tray_start"), Some(TrayAction::Start));
        assert_eq!(action_for("anlg_tray_settings"), Some(TrayAction::Settings));
        assert_eq!(
            action_for("anlg_tray_show_events"),
            Some(TrayAction::ToggleShowEvents)
        );
        assert_eq!(action_for("anlg_tray_hide"), Some(TrayAction::Hide));
        assert_eq!(
            action_for("anlg_tray_quit_completely"),
            Some(TrayAction::QuitCompletely)
        );
        assert_eq!(action_for("anlg_tray_version"), None);
        assert_eq!(action_for("anlg_tray_agenda_section_0"), None);
        assert_eq!(
            action_for("anlg_tray_agenda_evt-1"),
            Some(TrayAction::Agenda("evt-1".into()))
        );
    }

    #[test]
    fn channel_and_name_follow_the_identifier() {
        assert_eq!(channel("com.hyprnote.dev"), "dev");
        assert_eq!(channel("com.hyprnote.staging"), "staging");
        assert_eq!(channel("com.hyprnote.stable"), "stable");
        assert_eq!(app_name("com.hyprnote.stable"), "Anarlog");
        assert_eq!(app_name("com.hyprnote.dev"), "Anarlog Dev");
    }

    #[test]
    fn schedule_keeps_upcoming_timed_events_within_a_week() {
        let now = Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap();
        let rows = vec![
            row(
                "past",
                "Past",
                "2026-09-07T09:00:00Z",
                "2026-09-07T10:00:00Z",
            ),
            row(
                "active",
                "Active",
                "2026-09-07T11:30:00Z",
                "2026-09-07T12:30:00Z",
            ),
            row(
                "later",
                "b Later",
                "2026-09-07T15:00:00Z",
                "2026-09-07T15:30:00Z",
            ),
            row("same", "a Later", "2026-09-07T15:00:00Z", ""),
            row("far", "Far", "2026-09-20T15:00:00Z", "2026-09-20T15:30:00Z"),
            {
                let mut all_day = row("day", "All day", "2026-09-08T00:00:00Z", "");
                all_day.is_all_day = 1;
                all_day
            },
            row(
                "ignored",
                "Ignored",
                "2026-09-08T09:00:00Z",
                "2026-09-08T09:30:00Z",
            ),
        ];
        let events = schedule_events(&rows, |row| row.id == "ignored", now, &Utc);
        let ids: Vec<&str> = events.iter().map(|event| event.id.as_str()).collect();
        assert_eq!(ids, ["active", "same", "later"]);
        assert_eq!(events[0].time_label, "11:30 AM – 12:30 PM");
        assert_eq!(events[1].time_label, "3:00 PM");
        assert_eq!(events[1].ends_at_ms, None);
        assert_eq!(
            events[0].day_start_ms,
            Utc.with_ymd_and_hms(2026, 9, 7, 0, 0, 0)
                .unwrap()
                .timestamp_millis() as f64
        );
        assert_eq!(
            events[0].previous_day_start_ms,
            Utc.with_ymd_and_hms(2026, 9, 6, 0, 0, 0)
                .unwrap()
                .timestamp_millis() as f64
        );
    }
}
