use crate::error::Error;

#[cfg(target_os = "windows")]
use crate::events::NotificationEvent;

#[cfg(any(target_os = "windows", test))]
use std::collections::HashMap;
#[cfg(any(target_os = "windows", test))]
use std::path::Path;
#[cfg(any(target_os = "windows", test))]
use std::sync::{Mutex, OnceLock};
#[cfg(any(target_os = "windows", test))]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", test))]
static RECENT_WINDOWS_NOTIFICATIONS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[cfg(target_os = "windows")]
static WINDOWS_NOTIFICATION_TIMEOUTS: OnceLock<Mutex<WindowsNotificationTimeouts>> =
    OnceLock::new();

#[cfg(any(target_os = "windows", test))]
const WINDOWS_DEDUPE_WINDOW: Duration = Duration::from_mins(1);

#[cfg(any(target_os = "windows", test))]
#[derive(Default)]
struct WindowsNotificationTimeouts {
    active: HashMap<String, u64>,
    next_generation: u64,
}

#[cfg(any(target_os = "windows", test))]
impl WindowsNotificationTimeouts {
    fn schedule(&mut self, key: &str, timeout: Option<Duration>) -> Option<(u64, Duration)> {
        let Some(timeout) = timeout.filter(|timeout| !timeout.is_zero()) else {
            self.cancel(key);
            return None;
        };

        Some((self.register(key), timeout))
    }

    fn register(&mut self, key: &str) -> u64 {
        self.next_generation = self.next_generation.wrapping_add(1);
        self.active.insert(key.to_string(), self.next_generation);
        self.next_generation
    }

    fn take_if_current(&mut self, key: &str, generation: u64) -> bool {
        if self.active.get(key) != Some(&generation) {
            return false;
        }

        self.active.remove(key);
        true
    }

    fn cancel(&mut self, key: &str) {
        self.active.remove(key);
    }

    fn clear(&mut self) {
        self.active.clear();
    }
}

pub struct Notification<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Notification<'a, R, M> {
    #[tracing::instrument(skip(self))]
    pub fn show(&self, v: anlg_notification::Notification) -> Result<(), Error> {
        #[cfg(target_os = "windows")]
        if should_show_windows_notification(v.key.as_deref()) {
            show_windows_notification(self.manager, &v)?;
            schedule_windows_notification_timeout(self.manager, &v);
        }

        #[cfg(not(target_os = "windows"))]
        anlg_notification::show(&v);

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clear(&self) -> Result<(), Error> {
        let _ = self.manager;

        #[cfg(target_os = "windows")]
        {
            cancel_windows_notification_timeouts();
            clear_windows_notifications(self.manager)?;
        }

        #[cfg(not(target_os = "windows"))]
        anlg_notification::clear();

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
fn windows_notification_timeouts() -> &'static Mutex<WindowsNotificationTimeouts> {
    WINDOWS_NOTIFICATION_TIMEOUTS.get_or_init(|| Mutex::new(WindowsNotificationTimeouts::default()))
}

#[cfg(target_os = "windows")]
fn schedule_windows_notification_timeout<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
    notification: &anlg_notification::Notification,
) {
    use tauri_specta::Event;

    let Some(key) = notification.key.clone() else {
        return;
    };

    let (generation, timeout) = {
        let mut timeouts = windows_notification_timeouts()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(timeout) = timeouts.schedule(&key, notification.timeout) else {
            return;
        };
        timeout
    };
    let app = manager.app_handle().clone();
    let source = notification.source.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(timeout).await;

        let should_emit = windows_notification_timeouts()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take_if_current(&key, generation);
        if !should_emit {
            return;
        }

        if let Err(error) = (NotificationEvent::Timeout { key, source }).emit(&app) {
            tracing::warn!(%error, "failed_to_emit_windows_notification_timeout");
        }
    });
}

#[cfg(target_os = "windows")]
fn cancel_windows_notification_timeouts() {
    windows_notification_timeouts()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
}

#[cfg(any(target_os = "windows", test))]
fn is_cargo_target_profile_dir(path: &Path) -> bool {
    let Some(profile) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if !matches!(profile, "debug" | "release") {
        return false;
    }

    let Some(parent) = path.parent() else {
        return false;
    };

    parent.file_name().is_some_and(|name| name == "target")
        || parent
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "target")
}

#[cfg(any(target_os = "windows", test))]
fn windows_app_id<'a>(
    identifier: &'a str,
    is_dev: bool,
    executable: Option<&Path>,
) -> Option<&'a str> {
    if is_dev {
        return None;
    }

    let executable_dir = executable?.parent()?;

    (!is_cargo_target_profile_dir(executable_dir)).then_some(identifier)
}

#[cfg(target_os = "windows")]
fn show_windows_notification<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
    notification: &anlg_notification::Notification,
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

    let executable = tauri::utils::platform::current_exe().ok();
    if let Some(app_id) = windows_app_id(
        &manager.config().identifier,
        tauri::is_dev(),
        executable.as_deref(),
    ) {
        toast.app_id(app_id);
    }

    toast.show()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn clear_windows_notifications<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
) -> Result<(), Error> {
    let executable = tauri::utils::platform::current_exe().ok();
    let Some(app_id) = windows_app_id(
        &manager.config().identifier,
        tauri::is_dev(),
        executable.as_deref(),
    ) else {
        return Ok(());
    };

    windows::UI::Notifications::ToastNotificationManager::History()?
        .ClearWithId(&windows::core::HSTRING::from(app_id))?;
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

    #[test]
    fn development_windows_notifications_use_the_fallback_app_id() {
        let executable = Path::new("C:/Users/anarlog/Anarlog.exe");

        assert_eq!(
            windows_app_id("com.hyprnote.dev", true, Some(executable)),
            None
        );
    }

    #[test]
    fn direct_cargo_builds_use_the_fallback_app_id() {
        for profile in ["debug", "release"] {
            let executable = Path::new("C:/repo/target")
                .join(profile)
                .join("Anarlog.exe");

            assert_eq!(
                windows_app_id("com.hyprnote.dev", false, Some(&executable)),
                None
            );
        }
    }

    #[test]
    fn target_triple_cargo_builds_use_the_fallback_app_id() {
        for profile in ["debug", "release"] {
            let executable = Path::new("C:/repo/target")
                .join("x86_64-pc-windows-msvc")
                .join(profile)
                .join("Anarlog.exe");

            assert_eq!(
                windows_app_id("com.hyprnote.dev", false, Some(&executable)),
                None
            );
        }
    }

    #[test]
    fn installed_windows_notifications_use_the_configured_app_id() {
        let executable = Path::new("C:/Users/anarlog/AppData/Local/Anarlog/Anarlog.exe");

        assert_eq!(
            windows_app_id("com.hyprnote.stable", false, Some(executable)),
            Some("com.hyprnote.stable")
        );
    }

    #[test]
    fn windows_notification_timeout_only_fires_for_the_current_generation() {
        let mut timeouts = WindowsNotificationTimeouts::default();
        let first = timeouts.register("meeting");
        let second = timeouts.register("meeting");

        assert!(!timeouts.take_if_current("meeting", first));
        assert!(timeouts.take_if_current("meeting", second));
        assert!(!timeouts.take_if_current("meeting", second));
    }

    #[test]
    fn clearing_windows_notification_timeouts_invalidates_pending_generations() {
        let mut timeouts = WindowsNotificationTimeouts::default();
        let meeting_generation = timeouts.register("meeting");
        let mic_generation = timeouts.register("mic");

        timeouts.clear();

        assert!(!timeouts.take_if_current("meeting", meeting_generation));
        assert!(!timeouts.take_if_current("mic", mic_generation));
    }

    #[test]
    fn cancelling_a_windows_notification_timeout_invalidates_its_generation() {
        let mut timeouts = WindowsNotificationTimeouts::default();
        let generation = timeouts.register("meeting");

        timeouts.cancel("meeting");

        assert!(!timeouts.take_if_current("meeting", generation));
    }

    #[test]
    fn zero_windows_notification_timeout_cancels_without_scheduling() {
        let mut timeouts = WindowsNotificationTimeouts::default();
        let generation = timeouts.register("meeting");

        assert_eq!(timeouts.schedule("meeting", Some(Duration::ZERO)), None);
        assert!(!timeouts.take_if_current("meeting", generation));
    }

    #[test]
    fn persistent_windows_notification_cancels_an_older_timeout() {
        let mut timeouts = WindowsNotificationTimeouts::default();
        let (generation, _) = timeouts
            .schedule("meeting", Some(Duration::from_secs(30)))
            .unwrap();

        assert_eq!(timeouts.schedule("meeting", None), None);
        assert!(!timeouts.take_if_current("meeting", generation));
    }
}
