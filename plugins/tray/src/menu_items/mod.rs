mod app_info;
mod app_new;
mod help_report_bug;
mod help_suggest_feature;
mod tray_check_update;
mod tray_open;
mod tray_quit;
mod tray_settings;
mod tray_start;
mod tray_version;

pub use app_info::AppInfo;
pub use app_new::AppNew;
pub use help_report_bug::HelpReportBug;
pub use help_suggest_feature::HelpSuggestFeature;
pub use tray_check_update::{TrayCheckUpdate, UpdateMenuState};
pub use tray_open::TrayOpen;
pub use tray_quit::TrayQuit;
pub use tray_settings::{TraySettings, TraySettingsAI, TraySettingsGeneral};
pub use tray_start::TrayStart;
pub use tray_version::TrayVersion;

use tauri::{AppHandle, Result, menu::MenuItemKind};

pub trait MenuItemHandler {
    const ID: &'static str;

    fn build(app: &AppHandle<tauri::Wry>) -> Result<MenuItemKind<tauri::Wry>>;
    fn handle(app: &AppHandle<tauri::Wry>);
}

macro_rules! menu_items {
    ($($variant:ident => $item:ty),* $(,)?) => {
        #[derive(Debug, Clone, Copy)]
        pub enum HyprMenuItem {
            $($variant),*
        }

        impl From<HyprMenuItem> for tauri::menu::MenuId {
            fn from(value: HyprMenuItem) -> Self {
                match value {
                    $(HyprMenuItem::$variant => <$item as MenuItemHandler>::ID),*
                }.into()
            }
        }

        impl TryFrom<tauri::menu::MenuId> for HyprMenuItem {
            type Error = ();

            fn try_from(id: tauri::menu::MenuId) -> std::result::Result<Self, Self::Error> {
                let id = id.0.as_str();
                match id {
                    $(<$item as MenuItemHandler>::ID => Ok(HyprMenuItem::$variant),)*
                    _ => Err(()),
                }
            }
        }

        impl HyprMenuItem {
            pub fn handle(self, app: &AppHandle<tauri::Wry>) {
                match self {
                    $(HyprMenuItem::$variant => <$item>::handle(app)),*
                }
            }
        }
    };
}

menu_items! {
    TrayOpen => TrayOpen,
    TrayStart => TrayStart,
    TraySettings => TraySettings,
    TraySettingsGeneral => TraySettingsGeneral,
    TraySettingsAI => TraySettingsAI,
    TrayCheckUpdate => TrayCheckUpdate,
    TrayQuit => TrayQuit,
    TrayVersion => TrayVersion,
    AppInfo => AppInfo,
    AppNew => AppNew,
    HelpReportBug => HelpReportBug,
    HelpSuggestFeature => HelpSuggestFeature,
}
