use desktop_runtime::ServiceError;

pub mod accessibility;
pub mod deeplinks;
pub mod dialogs;
pub mod notifications;
pub mod permissions;
pub mod shortcuts;
pub mod tray;
pub mod updater;
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
