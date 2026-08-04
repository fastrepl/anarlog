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
const MAX_RECENT_WINDOWS_NOTIFICATIONS: usize = 256;
#[cfg(any(target_os = "windows", test))]
const MAX_ACTIVE_WINDOWS_NOTIFICATION_TIMEOUTS: usize = 64;

#[cfg(any(target_os = "windows", test))]
#[derive(Default)]
struct WindowsNotificationTimeouts {
    active: HashMap<String, WindowsNotificationTimeout>,
    next_generation: u64,
}

#[cfg(any(target_os = "windows", test))]
struct WindowsNotificationTimeout {
    generation: u64,
    task: Option<tokio::task::AbortHandle>,
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
        self.cancel(key);
        while self.active.len() >= MAX_ACTIVE_WINDOWS_NOTIFICATION_TIMEOUTS {
            let Some(oldest_key) = self
                .active
                .iter()
                .min_by_key(|(_, timeout)| timeout.generation)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.cancel(&oldest_key);
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        self.active.insert(
            key.to_string(),
            WindowsNotificationTimeout {
                generation: self.next_generation,
                task: None,
            },
        );
        self.next_generation
    }

    fn attach_task(&mut self, key: &str, generation: u64, task: tokio::task::AbortHandle) {
        let Some(timeout) = self.active.get_mut(key) else {
            task.abort();
            return;
        };

        if timeout.generation != generation {
            task.abort();
            return;
        }

        timeout.task = Some(task);
    }

    fn take_if_current(&mut self, key: &str, generation: u64) -> bool {
        if self.active.get(key).map(|timeout| timeout.generation) != Some(generation) {
            return false;
        }

        self.active.remove(key);
        true
    }

    fn cancel(&mut self, key: &str) {
        if let Some(timeout) = self.active.remove(key)
            && let Some(task) = timeout.task
        {
            task.abort();
        }
    }

    fn clear(&mut self) {
        for (_, timeout) in self.active.drain() {
            if let Some(task) = timeout.task {
                task.abort();
            }
        }
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
    if !register_recent_windows_notification(&mut recent, key, Instant::now()) {
        tracing::info!(key, "skipping_notification");
        return false;
    }

    true
}

#[cfg(any(target_os = "windows", test))]
fn register_recent_windows_notification(
    recent: &mut HashMap<String, Instant>,
    key: &str,
    now: Instant,
) -> bool {
    recent.retain(|_, timestamp| now.duration_since(*timestamp) < WINDOWS_DEDUPE_WINDOW);
    if recent.contains_key(key) {
        return false;
    }

    while recent.len() >= MAX_RECENT_WINDOWS_NOTIFICATIONS {
        let Some(oldest) = recent
            .iter()
            .min_by_key(|(_, timestamp)| *timestamp)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        recent.remove(&oldest);
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

    let app = manager.app_handle().clone();
    let source = notification.source.clone();
    let mut timeouts = windows_notification_timeouts()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some((generation, timeout)) = timeouts.schedule(&key, notification.timeout) else {
        return;
    };
    let task_key = key.clone();

    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(timeout).await;

        let should_emit = windows_notification_timeouts()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take_if_current(&task_key, generation);
        if !should_emit {
            return;
        }

        if let Err(error) = (NotificationEvent::Timeout {
            key: task_key,
            source,
        })
        .emit(&app)
        {
            tracing::warn!(%error, "failed_to_emit_windows_notification_timeout");
        }
    });
    timeouts.attach_task(&key, generation, task.inner().abort_handle());
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
    fn recent_windows_notifications_stay_bounded() {
        let now = Instant::now();
        let mut recent = HashMap::new();
        for index in 0..MAX_RECENT_WINDOWS_NOTIFICATIONS {
            recent.insert(
                format!("notification-{index}"),
                now - Duration::from_millis(index as u64 + 1),
            );
        }

        assert!(register_recent_windows_notification(
            &mut recent,
            "newest",
            now,
        ));
        assert_eq!(recent.len(), MAX_RECENT_WINDOWS_NOTIFICATIONS);
        assert!(recent.contains_key("newest"));
        assert!(!recent.contains_key(&format!(
            "notification-{}",
            MAX_RECENT_WINDOWS_NOTIFICATIONS - 1
        )));
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

    #[test]
    fn replacing_a_windows_notification_aborts_its_timeout_task() {
        tauri::async_runtime::block_on(async {
            let mut timeouts = WindowsNotificationTimeouts::default();
            let generation = timeouts.register("meeting");
            let task = tauri::async_runtime::spawn(std::future::pending::<()>());
            timeouts.attach_task("meeting", generation, task.inner().abort_handle());

            timeouts.register("meeting");

            assert!(task.await.is_err());
        });
    }

    #[test]
    fn clearing_windows_notifications_aborts_all_timeout_tasks() {
        tauri::async_runtime::block_on(async {
            let mut timeouts = WindowsNotificationTimeouts::default();
            let meeting_generation = timeouts.register("meeting");
            let meeting_task = tauri::async_runtime::spawn(std::future::pending::<()>());
            timeouts.attach_task(
                "meeting",
                meeting_generation,
                meeting_task.inner().abort_handle(),
            );
            let mic_generation = timeouts.register("mic");
            let mic_task = tauri::async_runtime::spawn(std::future::pending::<()>());
            timeouts.attach_task("mic", mic_generation, mic_task.inner().abort_handle());

            timeouts.clear();

            assert!(meeting_task.await.is_err());
            assert!(mic_task.await.is_err());
        });
    }

    #[test]
    fn windows_notification_timeouts_evict_and_abort_the_oldest_task_at_capacity() {
        tauri::async_runtime::block_on(async {
            let mut timeouts = WindowsNotificationTimeouts::default();
            let oldest_generation = timeouts.register("oldest");
            let oldest_task = tauri::async_runtime::spawn(std::future::pending::<()>());
            timeouts.attach_task(
                "oldest",
                oldest_generation,
                oldest_task.inner().abort_handle(),
            );

            for index in 0..MAX_ACTIVE_WINDOWS_NOTIFICATION_TIMEOUTS {
                timeouts.register(&format!("notification-{index}"));
            }

            assert_eq!(
                timeouts.active.len(),
                MAX_ACTIVE_WINDOWS_NOTIFICATION_TIMEOUTS
            );
            assert!(!timeouts.active.contains_key("oldest"));
            assert!(oldest_task.await.is_err());
        });
    }
}
