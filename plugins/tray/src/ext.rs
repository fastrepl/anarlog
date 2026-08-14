use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use tauri::async_runtime::JoinHandle;
#[cfg(target_os = "macos")]
use tauri::menu::{HELP_SUBMENU_ID, Submenu, WINDOW_SUBMENU_ID};
use tauri::{
    AppHandle, Result,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use crate::{
    schedule::{TrayAgendaSection, TrayScheduleEvent, agenda_sections, menu_bar_title},
    tray_icon::{RECORDING_FRAMES, TrayIconState},
};

use crate::menu_items::{
    AppInfo, AppNew, HelpReportBug, HelpSuggestFeature, MenuItemHandler, TrayCheckUpdate, TrayHide,
    TrayOpen, TrayQuit, TrayQuitCompletely, TraySettings, TrayShowEvents, TrayStart, TrayVersion,
    build_agenda_item,
};
use tauri_plugin_store2::Store2PluginExt;

const TRAY_ID: &str = "anlg-tray";

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static IS_DEGRADED: AtomicBool = AtomicBool::new(false);
static IS_UPDATE_AVAILABLE: AtomicBool = AtomicBool::new(false);
static SHOW_EVENTS: AtomicBool = AtomicBool::new(true);
static START_DISABLED: AtomicBool = AtomicBool::new(false);
static ANIMATION_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static SCHEDULE: Mutex<Vec<TrayScheduleEvent>> = Mutex::new(Vec::new());
static SCHEDULE_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static MENU_BAR_TITLE: Mutex<Option<String>> = Mutex::new(None);
static RECORDING_TITLE: Mutex<Option<String>> = Mutex::new(None);
static AGENDA_SECTIONS: Mutex<Vec<TrayAgendaSection>> = Mutex::new(Vec::new());

#[cfg(target_os = "macos")]
pub fn build_app_menu(app: &AppHandle<tauri::Wry>) -> Result<Menu<tauri::Wry>> {
    let app_submenu = Submenu::with_items(
        app,
        app.package_info().name.clone(),
        true,
        &[
            &AppInfo::build(app)?,
            &TrayCheckUpdate::build(app)?,
            &TraySettings::build(app)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &TrayQuit::build(app)?,
        ],
    )?;
    let file_submenu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &AppNew::build(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let edit_submenu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let view_submenu = Submenu::with_items(
        app,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app, None)?],
    )?;
    let window_submenu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    let help_submenu = Submenu::with_id_and_items(
        app,
        HELP_SUBMENU_ID,
        "Help",
        true,
        &[
            &HelpReportBug::build(app)?,
            &HelpSuggestFeature::build(app)?,
        ],
    )?;

    Menu::with_items(
        app,
        &[
            &app_submenu,
            &file_submenu,
            &edit_submenu,
            &view_submenu,
            &window_submenu,
            &help_submenu,
        ],
    )
}

pub struct Tray<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

// libappindicator-sys panics at first use when it cannot dlopen any
// appindicator library, so probe the same candidates before tray-icon
// reaches it.
#[cfg(target_os = "linux")]
fn linux_tray_backend_available() -> bool {
    [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ]
    .iter()
    .any(|name| unsafe { libloading::Library::new(name) }.is_ok())
}

impl<'a, M: tauri::Manager<tauri::Wry>> Tray<'a, tauri::Wry, M> {
    pub fn create_tray_menu(&self) -> Result<()> {
        let app = self.manager.app_handle();

        if app.tray_by_id(TRAY_ID).is_some() {
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        if !linux_tray_backend_available() {
            tracing::warn!("appindicator_library_missing_skipping_tray_icon");
            return Ok(());
        }

        SHOW_EVENTS.store(Self::load_show_events(app), Ordering::SeqCst);

        let agenda = Self::current_agenda_sections();
        let menu = Self::build_tray_menu(app, &agenda)?;
        *AGENDA_SECTIONS.lock().unwrap() = agenda;

        TrayIconBuilder::with_id(TRAY_ID)
            .icon(TrayIconState::Default.to_image()?)
            .icon_as_template(true)
            .menu(&menu)
            .show_menu_on_left_click(true)
            .build(app)?;

        Self::refresh_menu_bar_title(app)?;

        Ok(())
    }

    pub fn set_visible(&self, visible: bool) -> Result<()> {
        let app = self.manager.app_handle();

        if visible {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                tray.set_visible(true)?;
            } else {
                self.create_tray_menu()?;
            }
            Self::refresh_icon(app)?;
        } else {
            if let Ok(mut task) = ANIMATION_TASK.lock()
                && let Some(handle) = task.take()
            {
                handle.abort();
            }

            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                tray.set_visible(false)?;
            }
        }

        Ok(())
    }

    pub fn set_title(&self, title: Option<&str>) -> Result<()> {
        let app = self.manager.app_handle();
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            tray.set_title(title)?;
        }
        Ok(())
    }

    pub fn set_schedule(&self, mut events: Vec<TrayScheduleEvent>) -> Result<()> {
        events.retain(|event| event.starts_at_ms.is_finite());
        events.sort_by(|left, right| {
            left.starts_at_ms
                .partial_cmp(&right.starts_at_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        *SCHEDULE.lock().unwrap() = events;

        let app = self.manager.app_handle();
        Self::refresh_menu_bar_title(app)?;
        Self::refresh_menu_if_agenda_changed(app)?;

        let mut task = SCHEDULE_TASK.lock().unwrap();
        if task.is_none() {
            let app = app.clone();
            *task = Some(tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    if let Err(error) = Self::refresh_menu_bar_title(&app) {
                        tracing::warn!(%error, "failed to refresh menu bar title");
                    }
                    if let Err(error) = Self::refresh_menu_if_agenda_changed(&app) {
                        tracing::warn!(%error, "failed to refresh tray agenda");
                    }
                }
            }));
        }

        Ok(())
    }

    pub fn shows_events(&self) -> bool {
        SHOW_EVENTS.load(Ordering::SeqCst)
    }

    pub fn set_show_events(&self, show: bool) -> Result<()> {
        SHOW_EVENTS.store(show, Ordering::SeqCst);

        let app = self.manager.app_handle();
        Self::persist_show_events(app, show);
        Self::refresh_menu_bar_title(app)?;
        Self::rebuild_menu(app)
    }

    fn load_show_events(app: &AppHandle<tauri::Wry>) -> bool {
        let result = app
            .store2()
            .scoped_store::<String>(crate::PLUGIN_NAME)
            .and_then(|store| store.get("show_events_in_menu_bar".to_string()));

        match result {
            Ok(value) => value.unwrap_or(true),
            Err(error) => {
                tracing::warn!(%error, "failed to load tray event visibility");
                true
            }
        }
    }

    fn persist_show_events(app: &AppHandle<tauri::Wry>, show: bool) {
        let result = app
            .store2()
            .scoped_store::<String>(crate::PLUGIN_NAME)
            .and_then(|store| store.set("show_events_in_menu_bar".to_string(), show));

        if let Err(error) = result {
            tracing::warn!(%error, "failed to persist tray event visibility");
        }
    }

    fn refresh_menu_bar_title(app: &AppHandle<tauri::Wry>) -> Result<()> {
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return Ok(());
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        let recording_title = RECORDING_TITLE.lock().unwrap().clone();
        let title = menu_bar_title(
            &SCHEDULE.lock().unwrap(),
            now_ms,
            SHOW_EVENTS.load(Ordering::SeqCst),
            IS_RECORDING.load(Ordering::SeqCst),
            recording_title.as_deref(),
        );
        let mut current_title = MENU_BAR_TITLE.lock().unwrap();

        if *current_title != title {
            // tray-icon currently treats None as a no-op on macOS.
            tray.set_title(Some(title.as_deref().unwrap_or("")))?;
            *current_title = title;
        }

        Ok(())
    }

    fn current_agenda_sections() -> Vec<TrayAgendaSection> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        agenda_sections(
            &SCHEDULE.lock().unwrap(),
            now_ms,
            SHOW_EVENTS.load(Ordering::SeqCst),
        )
    }

    fn build_tray_menu(
        app: &AppHandle<tauri::Wry>,
        agenda: &[TrayAgendaSection],
    ) -> Result<Menu<tauri::Wry>> {
        let menu = Menu::new(app)?;

        for (section_index, section) in agenda.iter().enumerate() {
            let heading = MenuItem::with_id(
                app,
                format!("anlg_tray_agenda_section_{section_index}"),
                &section.label,
                false,
                None::<&str>,
            )?;
            menu.append(&heading)?;

            for event in &section.events {
                let item = build_agenda_item(app, &event.id, &event.label)?;
                menu.append(&item)?;
            }
        }

        menu.append(&TrayShowEvents::build(app)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;

        menu.append(&TrayOpen::build(app)?)?;
        menu.append(&TrayStart::build_with_disabled(
            app,
            START_DISABLED.load(Ordering::SeqCst),
        )?)?;
        menu.append(&TraySettings::build(app)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&TrayVersion::build(app)?)?;
        menu.append(&TrayCheckUpdate::build(app)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&TrayHide::build(app)?)?;
        menu.append(&TrayQuitCompletely::build(app)?)?;

        Ok(menu)
    }

    pub fn refresh_menu(&self) -> Result<()> {
        Self::rebuild_menu(self.manager.app_handle())
    }

    fn rebuild_menu(app: &AppHandle<tauri::Wry>) -> Result<()> {
        let agenda = Self::current_agenda_sections();
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            tray.set_menu(Some(Self::build_tray_menu(app, &agenda)?))?;
        }
        *AGENDA_SECTIONS.lock().unwrap() = agenda;
        Ok(())
    }

    fn refresh_menu_if_agenda_changed(app: &AppHandle<tauri::Wry>) -> Result<()> {
        let agenda = Self::current_agenda_sections();
        if *AGENDA_SECTIONS.lock().unwrap() == agenda {
            return Ok(());
        }

        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            tray.set_menu(Some(Self::build_tray_menu(app, &agenda)?))?;
        }
        *AGENDA_SECTIONS.lock().unwrap() = agenda;
        Ok(())
    }

    pub fn set_recording(&self, recording: bool) -> Result<()> {
        IS_RECORDING.store(recording, Ordering::SeqCst);
        if !recording {
            *RECORDING_TITLE.lock().unwrap() = None;
        }

        let app = self.manager.app_handle();
        Self::refresh_menu_bar_title(app)?;
        Self::refresh_icon(app)
    }

    pub fn set_recording_title(&self, title: Option<String>) -> Result<()> {
        *RECORDING_TITLE.lock().unwrap() = title.and_then(|title| {
            let title = title.trim();
            (!title.is_empty()).then(|| title.to_string())
        });
        Self::refresh_menu_bar_title(self.manager.app_handle())
    }

    pub fn set_degraded(&self, degraded: bool) -> Result<()> {
        IS_DEGRADED.store(degraded, Ordering::SeqCst);
        Self::refresh_icon(self.manager.app_handle())
    }

    pub fn set_update_available(&self, available: bool) -> Result<()> {
        IS_UPDATE_AVAILABLE.store(available, Ordering::SeqCst);
        Self::refresh_icon(self.manager.app_handle())
    }

    fn refresh_icon(app: &AppHandle<tauri::Wry>) -> Result<()> {
        {
            let mut task = ANIMATION_TASK.lock().unwrap();
            if let Some(handle) = task.take() {
                handle.abort();
            }

            if IS_RECORDING.load(Ordering::SeqCst) && !IS_DEGRADED.load(Ordering::SeqCst) {
                let app = app.clone();
                *task = Some(tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
                    let mut frame = 0usize;
                    loop {
                        interval.tick().await;
                        if let Some(tray) = app.tray_by_id(TRAY_ID)
                            && let Ok(image) = Image::from_bytes(RECORDING_FRAMES[frame])
                        {
                            let _ = tray.set_icon(Some(image));
                        }
                        frame = (frame + 1) % RECORDING_FRAMES.len();
                    }
                }));
                return Ok(());
            }
        }

        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return Ok(());
        };

        let state = if IS_UPDATE_AVAILABLE.load(Ordering::SeqCst) {
            TrayIconState::UpdateAvailable
        } else if IS_DEGRADED.load(Ordering::SeqCst) {
            TrayIconState::Degraded
        } else {
            TrayIconState::Default
        };

        tray.set_icon(Some(state.to_image()?))?;

        Ok(())
    }

    pub fn set_start_disabled(&self, disabled: bool) -> Result<()> {
        START_DISABLED.store(disabled, Ordering::SeqCst);
        Self::rebuild_menu(self.manager.app_handle())
    }
}

pub(crate) fn scheduled_event(event_id: &str) -> Option<TrayScheduleEvent> {
    SCHEDULE
        .lock()
        .unwrap()
        .iter()
        .find(|event| event.id == event_id)
        .cloned()
}

pub trait TrayPluginExt<R: tauri::Runtime> {
    fn tray(&self) -> Tray<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> TrayPluginExt<R> for T {
    fn tray(&self) -> Tray<'_, R, Self>
    where
        Self: Sized,
    {
        Tray {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
