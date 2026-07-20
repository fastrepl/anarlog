use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use tauri::async_runtime::JoinHandle;
use tauri::{
    AppHandle, Result,
    image::Image,
    menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use crate::{
    schedule::{TrayAgendaSection, TrayScheduleEvent, agenda_sections, menu_bar_title},
    tray_icon::{RECORDING_FRAMES, TrayIconState},
};

use crate::menu_items::{
    AppInfo, AppNew, HelpReportBug, HelpSuggestFeature, MenuItemHandler, TrayCheckUpdate, TrayOpen,
    TrayQuit, TraySettings, TrayShowEvents, TrayStart, TrayVersion, build_agenda_item,
};
use tauri_plugin_store2::Store2PluginExt;

const TRAY_ID: &str = "hypr-tray";

static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static IS_DEGRADED: AtomicBool = AtomicBool::new(false);
static IS_UPDATE_AVAILABLE: AtomicBool = AtomicBool::new(false);
static SHOW_EVENTS: AtomicBool = AtomicBool::new(true);
static START_DISABLED: AtomicBool = AtomicBool::new(false);
static ANIMATION_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static SCHEDULE: Mutex<Vec<TrayScheduleEvent>> = Mutex::new(Vec::new());
static SCHEDULE_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static SCHEDULE_TITLE: Mutex<Option<String>> = Mutex::new(None);
static AGENDA_SECTIONS: Mutex<Vec<TrayAgendaSection>> = Mutex::new(Vec::new());

pub struct Tray<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, M: tauri::Manager<tauri::Wry>> Tray<'a, tauri::Wry, M> {
    pub fn create_app_menu(&self) -> Result<()> {
        let app = self.manager.app_handle();

        let info_item = AppInfo::build(app)?;
        let check_update_item = TrayCheckUpdate::build(app)?;
        let settings_item = TraySettings::build(app)?;
        let new_item = AppNew::build(app)?;
        let report_bug_item = HelpReportBug::build(app)?;
        let suggest_feature_item = HelpSuggestFeature::build(app)?;

        if cfg!(target_os = "macos")
            && let Some(menu) = app.menu()
        {
            let items = menu.items()?;

            if !items.is_empty()
                && let MenuItemKind::Submenu(old_submenu) = &items[0]
            {
                let app_name = old_submenu.text()?;

                let new_app_submenu = Submenu::with_items(
                    app,
                    &app_name,
                    true,
                    &[
                        &info_item,
                        &check_update_item,
                        &settings_item,
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

                menu.remove(old_submenu)?;
                menu.prepend(&new_app_submenu)?;
            }

            if items.len() > 1
                && let MenuItemKind::Submenu(submenu) = &items[1]
            {
                submenu.prepend(&new_item)?;
            }

            for item in &items {
                if let MenuItemKind::Submenu(submenu) = item
                    && submenu.text()? == "Help"
                {
                    menu.remove(submenu)?;
                    break;
                }
            }

            let help_submenu = Submenu::with_items(
                app,
                "Help",
                true,
                &[&report_bug_item, &suggest_feature_item],
            )?;
            menu.append(&help_submenu)?;
        }

        Ok(())
    }

    pub fn create_tray_menu(&self) -> Result<()> {
        let app = self.manager.app_handle();

        if app.tray_by_id(TRAY_ID).is_some() {
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

        Self::refresh_schedule_title(app)?;

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
        Self::refresh_schedule_title(app)?;
        Self::refresh_menu_if_agenda_changed(app)?;

        let mut task = SCHEDULE_TASK.lock().unwrap();
        if task.is_none() {
            let app = app.clone();
            *task = Some(tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    if let Err(error) = Self::refresh_schedule_title(&app) {
                        tracing::warn!(%error, "failed to refresh tray schedule title");
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
        Self::refresh_schedule_title(app)?;
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
            .and_then(|store| {
                store.set("show_events_in_menu_bar".to_string(), show)?;
                store.save()
            });

        if let Err(error) = result {
            tracing::warn!(%error, "failed to persist tray event visibility");
        }
    }

    fn refresh_schedule_title(app: &AppHandle<tauri::Wry>) -> Result<()> {
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return Ok(());
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        let title = menu_bar_title(
            &SCHEDULE.lock().unwrap(),
            now_ms,
            SHOW_EVENTS.load(Ordering::SeqCst),
        );
        let mut current_title = SCHEDULE_TITLE.lock().unwrap();

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
        let mut agenda_index = 0;

        for (section_index, section) in agenda.iter().enumerate() {
            let heading = MenuItem::with_id(
                app,
                format!("hypr_tray_agenda_section_{section_index}"),
                &section.label,
                false,
                None::<&str>,
            )?;
            menu.append(&heading)?;

            for event in &section.events {
                let item = build_agenda_item(app, agenda_index, event)?;
                menu.append(&item)?;
                agenda_index += 1;
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
        menu.append(&TrayQuit::build(app)?)?;

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
        Self::refresh_icon(self.manager.app_handle())
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
