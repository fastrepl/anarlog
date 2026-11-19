use desktop_runtime::ServiceError;

pub mod deeplinks;
pub mod device_auth;
pub mod dialogs;
pub mod notifications;
pub mod os;
pub mod package;
pub mod permissions;
pub mod shortcuts;
pub mod sounds;
pub mod system_settings;
pub mod tray;
pub mod updater;
pub mod window_level;
pub mod windows;

#[derive(Clone, Debug)]
pub enum PlatformEvent {
    DeepLink(String),
    Shortcut(String),
    OpenMainWindow,
    Failed(ServiceError),
}

#[derive(Clone, Copy, Debug)]
pub enum WindowRole {
    Main,
    Settings,
    MeetingOverlay,
}
