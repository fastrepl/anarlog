use crate::error::Error;

#[cfg(any(target_os = "windows", test))]
use std::collections::HashMap;
#[cfg(any(target_os = "windows", test))]
use std::sync::{Mutex, OnceLock};
#[cfg(any(target_os = "windows", test))]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", test))]
static RECENT_WINDOWS_NOTIFICATIONS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[cfg(any(target_os = "windows", test))]
const WINDOWS_DEDUPE_WINDOW: Duration = Duration::from_mins(1);

pub struct Notification<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Notification<'a, R, M> {
    #[tracing::instrument(skip(self))]
    pub fn show(&self, v: hypr_notification::Notification) -> Result<(), Error> {
        #[cfg(target_os = "windows")]
        if should_show_windows_notification(v.key.as_deref()) {
            show_windows_notification(self.manager, &v)?;
        }

        #[cfg(not(target_os = "windows"))]
        hypr_notification::show(&v);

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clear(&self) -> Result<(), Error> {
        let _ = self.manager;
        hypr_notification::clear();
        Ok(())
    }
}

#[cfg(any(target_os = "windows", test))]
fn should_show_windows_notification(key: Option<&str>) -> bool {
    let Some(key) = key else {
        return true;
    };

    let recent = RECENT_WINDOWS_NOTIFICATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut recent = recent.lock().unwrap_or_else(|error| error.into_inner());
    let now = Instant::now();
    recent.retain(|_, timestamp| now.duration_since(*timestamp) < WINDOWS_DEDUPE_WINDOW);

    if recent.contains_key(key) {
        tracing::info!(key, "skipping_notification");
        return false;
    }

    recent.insert(key.to_string(), now);
    true
}

#[cfg(target_os = "windows")]
fn show_windows_notification<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
    notification: &hypr_notification::Notification,
) -> Result<(), Error> {
    let mut toast = notify_rust::Notification::new();
    toast
        .summary(&notification.title)
        .body(&notification.message)
        .timeout(
            notification
                .timeout
                .map(notify_rust::Timeout::from)
                .unwrap_or(notify_rust::Timeout::Never),
        );

    if let Ok(exe) = tauri::utils::platform::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let current_dir = exe_dir.display().to_string();
        let target_debug = format!(
            "{}target{}debug",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        );
        let target_release = format!(
            "{}target{}release",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        );

        if !current_dir.ends_with(&target_debug) && !current_dir.ends_with(&target_release) {
            toast.app_id(&manager.config().identifier);
        }
    }

    toast.show()?;
    Ok(())
}

pub trait NotificationPluginExt<R: tauri::Runtime> {
    fn notification(&self) -> Notification<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> NotificationPluginExt<R> for T {
    fn notification(&self) -> Notification<'_, R, Self>
    where
        Self: Sized,
    {
        Notification {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_notifications_without_keys_are_not_deduplicated() {
        assert!(should_show_windows_notification(None));
        assert!(should_show_windows_notification(None));
    }

    #[test]
    fn windows_notifications_with_duplicate_keys_are_suppressed() {
        let key = "windows-notification-test-duplicate-key";

        assert!(should_show_windows_notification(Some(key)));
        assert!(!should_show_windows_notification(Some(key)));
    }
}
