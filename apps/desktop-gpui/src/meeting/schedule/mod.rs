//! Calendar-driven and meeting-app-driven capture automation: scheduled auto-start and its
//! attendance gate, auto-stop with the "Did your meeting end?" prompt, the meeting chat
//! recording disclosure, and meeting chat and shared screen capture into the note.

pub mod apps;
pub mod attendance;
pub mod auto_start;
pub mod auto_stop;
pub mod brief_schedule;
pub mod capture;
pub mod records;

use std::sync::Arc;

use anlg_notification_interface::Notification;
use desktop_runtime::Result;
use gpui::{App, Global};

/// What scheduling needs from meeting detection and OS notifications, which another module
/// owns: the apps carrying the current call and a way to post a notification. Its actions
/// come back through the host's `schedule_notification_action`. Nothing posts until one is
/// installed.
pub trait Notices: Send + Sync {
    /// Apps whose call the current capture belongs to (shipping's `triggerAppIds`).
    fn trigger_apps(&self, cx: &App) -> Vec<String>;
    /// `Ok(false)` when the same key was shown within the last minute.
    fn show(&self, notification: Notification, cx: &mut App) -> Result<bool>;
}

#[derive(Clone)]
pub struct InstalledNotices(pub Arc<dyn Notices>);

impl Global for InstalledNotices {}

pub fn notices(cx: &App) -> Option<Arc<dyn Notices>> {
    cx.try_global::<InstalledNotices>()
        .map(|installed| installed.0.clone())
}
