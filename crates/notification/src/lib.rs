use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub use hypr_notification_interface::*;

type NotificationContextMap = Mutex<HashMap<String, (Option<NotificationSource>, Instant)>>;

static RECENT_NOTIFICATIONS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
static NOTIFICATION_CONTEXT: OnceLock<NotificationContextMap> = OnceLock::new();

const DEDUPE_WINDOW: Duration = Duration::from_mins(1);
const CONTEXT_TTL: Duration = Duration::from_mins(10);

pub enum NotificationMutation {
    Confirm,
    Dismiss,
}

fn store_context(key: &str, source: Option<NotificationSource>) {
    let ctx_map = NOTIFICATION_CONTEXT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = ctx_map.lock().unwrap();

    let now = Instant::now();
    map.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < CONTEXT_TTL);

    map.insert(key.to_string(), (source, now));
}

fn get_context(key: &str) -> NotificationContext {
    let ctx_map = NOTIFICATION_CONTEXT.get_or_init(|| Mutex::new(HashMap::new()));
    let source = ctx_map
        .lock()
        .unwrap()
        .remove(key)
        .and_then(|(source, _)| source);
    NotificationContext {
        key: key.to_string(),
        source,
    }
}

fn show_inner(notification: &hypr_notification_interface::Notification) {
    #[cfg(all(feature = "legacy", target_os = "macos"))]
    hypr_notification_macos::show(notification);

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    hypr_notification_linux::show(notification);
}

pub fn show(notification: &hypr_notification_interface::Notification) {
    let Some(key) = &notification.key else {
        show_inner(notification);
        return;
    };

    let recent_map = RECENT_NOTIFICATIONS.get_or_init(|| Mutex::new(HashMap::new()));

    {
        let mut recent_notifications = recent_map.lock().unwrap();
        let now = Instant::now();

        recent_notifications
            .retain(|_, &mut timestamp| now.duration_since(timestamp) < DEDUPE_WINDOW);

        if let Some(&last_shown) = recent_notifications.get(key) {
            let duration = now.duration_since(last_shown);

            if duration < DEDUPE_WINDOW {
                tracing::info!(key = key, duration = ?duration, "skipping_notification");
                return;
            }
        }

        recent_notifications.insert(key.clone(), now);
    }

    store_context(key, notification.source.clone());
    show_inner(notification);
}

pub fn clear() {
    #[cfg(all(feature = "legacy", target_os = "macos"))]
    hypr_notification_macos::dismiss_all();

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    hypr_notification_linux::dismiss_all();
}

pub fn setup_dismiss_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        hypr_notification_macos::setup_dismiss_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        hypr_notification_linux::setup_notification_dismiss_handler(move |key| {
            f(get_context(&key));
        });
    }

    let _ = f;
}

pub fn setup_collapsed_confirm_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        hypr_notification_macos::setup_collapsed_confirm_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        hypr_notification_linux::setup_notification_confirm_handler(move |key| {
            f(get_context(&key));
        });
    }

    let _ = f;
}

pub fn setup_expanded_accept_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        hypr_notification_macos::setup_expanded_accept_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        hypr_notification_linux::setup_notification_accept_handler(move |key| {
            f(get_context(&key));
        });
    }

    let _ = f;
}

pub fn setup_collapsed_timeout_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        hypr_notification_macos::setup_collapsed_timeout_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        hypr_notification_linux::setup_notification_timeout_handler(move |key| {
            f(get_context(&key));
        });
    }

    let _ = f;
}

pub fn setup_option_selected_handler<F>(f: F)
where
    F: Fn(NotificationContext, i32) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        hypr_notification_macos::setup_option_selected_handler(move |key, tag| {
            f(get_context(&key), tag);
        });
    }

    let _ = f;
}
