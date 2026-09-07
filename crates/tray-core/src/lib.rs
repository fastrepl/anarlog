//! The tray's platform-independent pieces, shared by the Tauri tray plugin
//! and the GPUI shell: the agenda / menu-bar schedule logic, the icon
//! bitmaps, and the menu labels, so both shells show the same tray.

pub mod schedule;

pub use schedule::{
    TrayAgendaEvent, TrayAgendaSection, TrayScheduleEvent, agenda_sections, menu_bar_title,
    next_schedule_refresh_ms,
};

/// The tray bitmaps (`icons/`), as PNG bytes.
pub mod icons {
    pub const DEFAULT: &[u8] = include_bytes!("../icons/tray_default.png");
    pub const DEGRADED: &[u8] = include_bytes!("../icons/tray_degraded.png");
    pub const UPDATE_AVAILABLE: &[u8] = include_bytes!("../icons/tray_update.png");
    /// Cycled every 250ms while recording.
    pub const RECORDING_FRAMES: &[&[u8]] = &[
        include_bytes!("../icons/tray_recording_0.png"),
        include_bytes!("../icons/tray_recording_1.png"),
        include_bytes!("../icons/tray_recording_2.png"),
    ];
    pub const RECORDING_FRAME_MS: u64 = 250;
}

/// The menu item labels, in the menu's order.
pub mod labels {
    pub fn open(app_name: &str) -> String {
        format!("Open {app_name}")
    }
    pub const START: &str = "Start a new meeting";
    pub const SETTINGS: &str = "Settings";
    pub const SHOW_EVENTS: &str = "Show events in menu bar";
    pub fn version(version: &str, channel: &str) -> String {
        format!("v{version} ({channel})")
    }
    pub const CHECK_FOR_UPDATE: &str = "Check for Updates";
    pub const HIDE: &str = "Hide";
    pub const QUIT_COMPLETELY: &str = "Quit Completely…";
    pub fn quit_completely_title(app_name: &str) -> String {
        format!("Quit {app_name} Completely?")
    }
    pub fn quit_completely_message(app_name: &str) -> String {
        format!("{app_name} will stop running in the background.")
    }
    pub const QUIT_COMPLETELY_CONFIRM: &str = "Quit Completely";
    pub const CANCEL: &str = "Cancel";
}
