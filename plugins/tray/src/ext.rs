use tauri::{
    Result,
    menu::{Menu, MenuItemKind, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use crate::tray_icon::TrayIconState;

use crate::menu_items::{
    AppInfo, AppNew, HelpReportBug, HelpSuggestFeature, MenuItemHandler, TrayCheckUpdate, TrayOpen,
    TrayQuit, TraySettings, TrayStart, TrayVersion,
};

const TRAY_ID: &str = "hypr-tray";

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

        let menu = Menu::with_items(
            app,
            &[
                &TrayVersion::build(app)?,
                &PredefinedMenuItem::separator(app)?,
                &TrayOpen::build(app)?,
                &TrayStart::build_with_disabled(app, false)?,
                &PredefinedMenuItem::separator(app)?,
                &TrayCheckUpdate::build(app)?,
                &PredefinedMenuItem::separator(app)?,
                &TrayQuit::build(app)?,
            ],
        )?;

        TrayIconBuilder::with_id(TRAY_ID)
            .icon(TrayIconState::Default.to_image()?)
            .icon_as_template(true)
            .menu(&menu)
            .show_menu_on_left_click(true)
            .build(app)?;

        Ok(())
    }

    pub fn set_start_disabled(&self, disabled: bool) -> Result<()> {
        let app = self.manager.app_handle();

        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let menu = Menu::with_items(
                app,
                &[
                    &TrayVersion::build(app)?,
                    &PredefinedMenuItem::separator(app)?,
                    &TrayOpen::build(app)?,
                    &TrayStart::build_with_disabled(app, disabled)?,
                    &PredefinedMenuItem::separator(app)?,
                    &TrayCheckUpdate::build(app)?,
                    &PredefinedMenuItem::separator(app)?,
                    &TrayQuit::build(app)?,
                ],
            )?;

            tray.set_menu(Some(menu))?;
        }

        Ok(())
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
