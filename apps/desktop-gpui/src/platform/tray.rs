//! Shipping's menu-bar tray (`plugins/tray/src/ext.rs`): agenda, menu-bar title and icon
//! states, built as a [`TrayModel`] that the native adapter renders.

mod schedule;

use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use desktop_runtime::{Result, ServiceError};

pub use schedule::{AgendaEvent, AgendaSection, ScheduleEvent, agenda_sections, menu_bar_title};

/// Shipping's tray ID. Linux names the indicator by it, and macOS keeps the menu-bar position
/// under it (`NSStatusItem Preferred Position anlg-tray`), so both carry over from shipping.
const TRAY_ID: &str = "anlg-tray";

/// The `show_tray_icon` setting; the tray owner applies it on its next poll.
static VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn set_visible(visible: bool) {
    VISIBLE.store(visible, Ordering::Relaxed);
}

const AGENDA_PREFIX: &str = "anlg_tray_agenda_";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    StartMeeting,
    Settings,
    ToggleShowEvents,
    CheckUpdate,
    Hide,
    QuitCompletely,
    /// A calendar event from the agenda: start its note and open its meeting link.
    Agenda(Arc<str>),
}

/// The one update state: the tray and app menu render it as shipping's `UpdateMenuState`, the
/// sidebar toast as shipping's `DesktopUpdateControl`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateMenu {
    #[default]
    CheckForUpdate,
    Available(Arc<str>),
    /// The version and the downloaded percentage, once the content length is known.
    Downloading(Arc<str>, Option<u8>),
    RestartToApply(Arc<str>),
    /// The version and the failure message.
    Failed(Arc<str>, Arc<str>),
}

impl UpdateMenu {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CheckForUpdate | Self::Available(_) | Self::Failed(..) => "Check for Updates",
            Self::Downloading(..) => "Downloading...",
            Self::RestartToApply(_) => "Restart to Apply Update",
        }
    }

    pub fn enabled(&self) -> bool {
        !matches!(self, Self::Downloading(..))
    }

    /// Shipping's tray `set_update_available`: on from available until a failure.
    fn available(&self) -> bool {
        matches!(
            self,
            Self::Available(_) | Self::Downloading(..) | Self::RestartToApply(_)
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrayState {
    pub recording: bool,
    /// Capture is running with a degraded transcription source.
    pub degraded: bool,
    pub update: UpdateMenu,
    pub show_events: bool,
    pub schedule: Arc<[ScheduleEvent]>,
    pub recording_title: Option<Arc<str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIcon {
    Default,
    Degraded,
    UpdateAvailable,
    Recording(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrayItem {
    Heading(String),
    Item {
        id: String,
        label: String,
        enabled: bool,
    },
    Check {
        id: String,
        label: String,
        checked: bool,
    },
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayModel {
    pub items: Vec<TrayItem>,
    pub title: Option<String>,
    pub icon: TrayIcon,
}

const RECORDING_FRAME_MS: f64 = 250.0;

pub fn version_label() -> String {
    use crate::platform::package;
    format!(
        "v{} ({})",
        package::VERSION,
        package::tray_channel(package::channel())
    )
}

fn item(id: &str, label: impl Into<String>, enabled: bool) -> TrayItem {
    TrayItem::Item {
        id: id.into(),
        label: label.into(),
        enabled,
    }
}

impl TrayModel {
    pub fn new(state: &TrayState, now_ms: f64) -> Self {
        let mut items = Vec::new();
        for section in agenda_sections(&state.schedule, now_ms, state.show_events) {
            items.push(TrayItem::Heading(section.label.into()));
            items.extend(
                section
                    .events
                    .into_iter()
                    .map(|event| item(&format!("{AGENDA_PREFIX}{}", event.id), event.label, true)),
            );
        }
        items.extend([
            TrayItem::Check {
                id: "anlg_tray_show_events".into(),
                label: "Show events in menu bar".into(),
                checked: state.show_events,
            },
            TrayItem::Separator,
            item("anlg_tray_open", "Open Anarlog", true),
            item("anlg_tray_start", "Start a new meeting", !state.recording),
            item("anlg_tray_settings", "Settings", true),
            TrayItem::Separator,
            item("anlg_tray_version", version_label(), false),
            item(
                "anlg_tray_check_update",
                state.update.label(),
                state.update.enabled(),
            ),
            TrayItem::Separator,
            item("anlg_tray_hide", "Hide", true),
            item("anlg_tray_quit_completely", "Quit Completely…", true),
        ]);
        let icon = if state.recording && !state.degraded {
            TrayIcon::Recording((now_ms / RECORDING_FRAME_MS) as usize % RECORDING_FRAMES.len())
        } else if state.update.available() {
            TrayIcon::UpdateAvailable
        } else if state.degraded {
            TrayIcon::Degraded
        } else {
            TrayIcon::Default
        };
        Self {
            items,
            title: menu_bar_title(
                &state.schedule,
                now_ms,
                state.show_events,
                state.recording,
                state.recording_title.as_deref(),
            ),
            icon,
        }
    }
}

/// Maps a clicked tray item to its action, honoring the model's disabled items.
pub fn action_for(model: &TrayModel, id: &str) -> Option<TrayAction> {
    let enabled = model.items.iter().any(|item| match item {
        TrayItem::Item {
            id: item, enabled, ..
        } => item == id && *enabled,
        TrayItem::Check { id: item, .. } => item == id,
        _ => false,
    });
    if !enabled {
        return None;
    }
    if let Some(event) = id.strip_prefix(AGENDA_PREFIX) {
        return Some(TrayAction::Agenda(event.into()));
    }
    Some(match id {
        "anlg_tray_show_events" => TrayAction::ToggleShowEvents,
        "anlg_tray_open" => TrayAction::Open,
        "anlg_tray_start" => TrayAction::StartMeeting,
        "anlg_tray_settings" => TrayAction::Settings,
        "anlg_tray_check_update" => TrayAction::CheckUpdate,
        "anlg_tray_hide" => TrayAction::Hide,
        "anlg_tray_quit_completely" => TrayAction::QuitCompletely,
        _ => return None,
    })
}

const RECORDING_FRAMES: [&[u8]; 3] = [
    include_bytes!("../../../../plugins/tray/icons/tray_recording_0.png"),
    include_bytes!("../../../../plugins/tray/icons/tray_recording_1.png"),
    include_bytes!("../../../../plugins/tray/icons/tray_recording_2.png"),
];

fn icon_bytes(icon: TrayIcon) -> &'static [u8] {
    match icon {
        TrayIcon::Default => include_bytes!("../../../../plugins/tray/icons/tray_default.png"),
        TrayIcon::Degraded => include_bytes!("../../../../plugins/tray/icons/tray_degraded.png"),
        TrayIcon::UpdateAvailable => {
            include_bytes!("../../../../plugins/tray/icons/tray_update.png")
        }
        TrayIcon::Recording(frame) => RECORDING_FRAMES[frame % RECORDING_FRAMES.len()],
    }
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

fn native_icon(icon: TrayIcon) -> Result<tray_icon::Icon> {
    let image = image::load_from_memory(icon_bytes(icon))
        .map_err(failure)?
        .into_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).map_err(failure)
}

fn native_menu(items: &[TrayItem]) -> Result<tray_icon::menu::Menu> {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    for (index, entry) in items.iter().enumerate() {
        let appended = match entry {
            TrayItem::Heading(label) => menu.append(&MenuItem::with_id(
                format!("{AGENDA_PREFIX}section_{index}"),
                label,
                false,
                None,
            )),
            TrayItem::Item { id, label, enabled } => {
                menu.append(&MenuItem::with_id(id.as_str(), label, *enabled, None))
            }
            TrayItem::Check { id, label, checked } => menu.append(&CheckMenuItem::with_id(
                id.as_str(),
                label,
                true,
                *checked,
                None,
            )),
            TrayItem::Separator => menu.append(&PredefinedMenuItem::separator()),
        };
        appended.map_err(failure)?;
    }
    Ok(menu)
}

pub trait TrayAdapter {
    fn update(&mut self, model: &TrayModel) -> Result<()>;
}

#[cfg(target_os = "macos")]
thread_local! {
    /// muda frees a visible menu's items when it is replaced, crashing the click
    /// (HYPRNOTE2-2MTS), so macOS swaps the menu on the next tray mouse-down instead.
    static PENDING_MENU: std::cell::RefCell<Option<(tray_icon::TrayIcon, tray_icon::menu::Menu)>> =
        const { std::cell::RefCell::new(None) };
}

/// Shipping's `apply_autosave_name`: the status item takes its saved menu-bar position. The
/// saved visibility goes first, since the `show_tray_icon` setting owns visibility.
#[cfg(target_os = "macos")]
fn restore_position(icon: &tray_icon::TrayIcon) {
    use objc2::{
        class, msg_send,
        runtime::{AnyObject, Bool},
    };
    let Some(item) = icon.ns_status_item() else {
        return;
    };
    let item = objc2::rc::Retained::as_ptr(&item) as *mut AnyObject;
    let string = |text: &str| -> *mut AnyObject {
        let text = std::ffi::CString::new(text).expect("no NUL in the tray ID");
        unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
    };
    unsafe {
        let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let visible = string(&format!("NSStatusItem Visible {TRAY_ID}"));
        let _: () = msg_send![defaults, removeObjectForKey: visible];
        let _: () = msg_send![item, setAutosaveName: string(TRAY_ID)];
        let _: () = msg_send![item, setVisible: Bool::YES];
    }
}

pub struct NativeTray {
    icon: tray_icon::TrayIcon,
    model: Option<TrayModel>,
    visible: Cell<bool>,
}

// libappindicator-sys panics at first use when it cannot dlopen any
// appindicator library, so probe the same candidates before tray-icon
// reaches it.
#[cfg(any(target_os = "linux", test))]
const LINUX_APPINDICATOR_LIBRARIES: &[&str] = &[
    "libayatana-appindicator3.so.1",
    "libappindicator3.so.1",
    "libayatana-appindicator3.so",
    "libappindicator3.so",
];

#[cfg(any(target_os = "linux", test))]
fn has_linux_tray_backend<F>(mut probe: F) -> bool
where
    F: FnMut(&'static str) -> bool,
{
    LINUX_APPINDICATOR_LIBRARIES.iter().any(|name| probe(name))
}

impl NativeTray {
    /// Construct and poll on the GPUI main thread. Linux also requires `pump_native_events`.
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Self::new_with_linux_probe(|name| unsafe { libloading::Library::new(name) }.is_ok())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::build_native()
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn new_with_linux_probe<F>(probe: F) -> Result<Self>
    where
        F: FnMut(&'static str) -> bool,
    {
        if !has_linux_tray_backend(probe) {
            return Err(ServiceError::Unsupported(
                "system tray requires libayatana-appindicator3 or libappindicator3".into(),
            ));
        }
        Self::build_native()
    }

    fn build_native() -> Result<Self> {
        #[cfg(target_os = "linux")]
        gtk::init().map_err(failure)?;
        let icon = tray_icon::TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_icon(native_icon(TrayIcon::Default)?)
            .with_icon_as_template(true)
            .with_menu_on_left_click(true)
            .build()
            .map_err(failure)?;
        #[cfg(target_os = "macos")]
        restore_position(&icon);
        #[cfg(target_os = "macos")]
        tray_icon::TrayIconEvent::set_event_handler(Some(|event| {
            if let tray_icon::TrayIconEvent::Click {
                button_state: tray_icon::MouseButtonState::Down,
                ..
            } = event
                && let Some((icon, menu)) = PENDING_MENU.with(|pending| pending.take())
            {
                icon.set_menu(Some(Box::new(menu)));
            }
        }));
        Ok(Self {
            icon,
            model: None,
            visible: Cell::new(true),
        })
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        let visible = VISIBLE.load(Ordering::Relaxed);
        if self.visible.get() != visible && self.icon.set_visible(visible).is_ok() {
            self.visible.set(visible);
        }
        let Some(model) = &self.model else {
            return Vec::new();
        };
        tray_icon::menu::MenuEvent::receiver()
            .try_iter()
            .take(64)
            .filter_map(|event| action_for(model, &event.id.0))
            .collect()
    }
}

impl TrayAdapter for NativeTray {
    fn update(&mut self, model: &TrayModel) -> Result<()> {
        let previous = self.model.as_ref();
        if previous.map(|previous| &previous.items) != Some(&model.items) {
            let menu = native_menu(&model.items)?;
            #[cfg(target_os = "macos")]
            if previous.is_some() {
                PENDING_MENU
                    .with(|pending| *pending.borrow_mut() = Some((self.icon.clone(), menu)));
            } else {
                self.icon.set_menu(Some(Box::new(menu)));
            }
            #[cfg(not(target_os = "macos"))]
            self.icon.set_menu(Some(Box::new(menu)));
        }
        if previous.map(|previous| &previous.title) != Some(&model.title) {
            // tray-icon treats None as a no-op on macOS.
            self.icon
                .set_title(Some(model.title.as_deref().unwrap_or("")));
        }
        if previous.map(|previous| previous.icon) != Some(model.icon) {
            self.icon
                .set_icon(Some(native_icon(model.icon)?))
                .map_err(failure)?;
        }
        self.model = Some(model.clone());
        Ok(())
    }
}

pub fn pump_native_events() {
    #[cfg(target_os = "linux")]
    if gtk::is_initialized_main_thread() {
        for _ in 0..64 {
            if !gtk::events_pending() {
                break;
            }
            gtk::main_iteration_do(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_disables_start_while_recording_and_shows_the_recording_or_update_icon() {
        let idle = TrayState {
            show_events: true,
            ..Default::default()
        };
        let model = TrayModel::new(&idle, 0.);
        assert_eq!(model.icon, TrayIcon::Default);
        assert_eq!(
            action_for(&model, "anlg_tray_start"),
            Some(TrayAction::StartMeeting)
        );
        assert_eq!(action_for(&model, "anlg_tray_version"), None);

        let recording = TrayState {
            recording: true,
            ..idle.clone()
        };
        let model = TrayModel::new(&recording, 600.);
        assert_eq!(model.icon, TrayIcon::Recording(2));
        assert_eq!(action_for(&model, "anlg_tray_start"), None);

        let degraded_update = TrayState {
            degraded: true,
            update: UpdateMenu::RestartToApply("1.2.3".into()),
            ..recording
        };
        assert_eq!(
            TrayModel::new(&degraded_update, 0.).icon,
            TrayIcon::UpdateAvailable
        );
        let downloading = TrayState {
            update: UpdateMenu::Downloading("1.2.3".into(), Some(40)),
            ..idle.clone()
        };
        assert_eq!(
            action_for(&TrayModel::new(&downloading, 0.), "anlg_tray_check_update"),
            None
        );
        let failed = TrayState {
            update: UpdateMenu::Failed("1.2.3".into(), "Failed to download update.".into()),
            ..idle
        };
        let model = TrayModel::new(&failed, 0.);
        assert_eq!(model.icon, TrayIcon::Default);
        assert_eq!(
            action_for(&model, "anlg_tray_check_update"),
            Some(TrayAction::CheckUpdate)
        );
    }

    #[test]
    fn agenda_items_open_their_event() {
        let state = TrayState {
            show_events: true,
            schedule: Arc::from([ScheduleEvent {
                id: "event-1".into(),
                title: "Standup".into(),
                meeting_link: None,
                starts_at_ms: 60_000.,
                ends_at_ms: None,
                day_start_ms: 0.,
                previous_day_start_ms: -86_400_000.,
                time_label: "9:00 AM".into(),
            }]),
            ..Default::default()
        };
        let model = TrayModel::new(&state, 0.);
        assert_eq!(
            action_for(&model, "anlg_tray_agenda_event-1"),
            Some(TrayAction::Agenda("event-1".into()))
        );
        assert_eq!(model.title.as_deref(), Some("Standup • in 1m"));
        let hidden = TrayModel::new(
            &TrayState {
                show_events: false,
                ..state
            },
            0.,
        );
        assert_eq!(action_for(&hidden, "anlg_tray_agenda_event-1"), None);
        assert_eq!(hidden.title, None);
    }

    #[test]
    fn linux_tray_backend_probe_checks_all_appindicator_candidates() {
        let mut probed = Vec::new();
        assert!(matches!(
            NativeTray::new_with_linux_probe(|name| {
                probed.push(name);
                false
            }),
            Err(ServiceError::Unsupported(_))
        ));
        assert_eq!(probed, LINUX_APPINDICATOR_LIBRARIES);
        assert!(has_linux_tray_backend(|name| {
            name == "libappindicator3.so.1"
        }));
    }
}
