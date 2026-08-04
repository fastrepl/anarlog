use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub use anlg_notification_interface::*;

struct TimedEntry<T> {
    value: T,
    timestamp: Instant,
    sequence: u64,
}

struct BoundedTimedMap<T> {
    entries: HashMap<String, TimedEntry<T>>,
    next_sequence: u64,
}

impl<T> Default for BoundedTimedMap<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            next_sequence: 0,
        }
    }
}

impl<T> BoundedTimedMap<T> {
    fn retain_recent(&mut self, now: Instant, ttl: Duration) {
        self.entries
            .retain(|_, entry| now.duration_since(entry.timestamp) < ttl);
    }

    fn insert(&mut self, key: String, value: T, timestamp: Instant, capacity: usize) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.entries.insert(
            key,
            TimedEntry {
                value,
                timestamp,
                sequence,
            },
        );

        while self.entries.len() > capacity {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(key, entry)| (entry.sequence, key.as_str()))
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.entries.remove(&oldest_key);
        }
    }

    fn insert_without_eviction(
        &mut self,
        key: String,
        value: T,
        timestamp: Instant,
        capacity: usize,
    ) -> bool {
        if !self.entries.contains_key(&key) && self.entries.len() >= capacity {
            return false;
        }

        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.entries.insert(
            key,
            TimedEntry {
                value,
                timestamp,
                sequence,
            },
        );
        true
    }

    fn get(&self, key: &str) -> Option<&TimedEntry<T>> {
        self.entries.get(key)
    }

    fn remove(&mut self, key: &str) -> Option<T> {
        self.entries.remove(key).map(|entry| entry.value)
    }
}

type RecentNotificationMap = Mutex<BoundedTimedMap<()>>;
type NotificationContextMap = Mutex<BoundedTimedMap<Option<NotificationSource>>>;

static RECENT_NOTIFICATIONS: OnceLock<RecentNotificationMap> = OnceLock::new();
static NOTIFICATION_CONTEXT: OnceLock<NotificationContextMap> = OnceLock::new();

const DEDUPE_WINDOW: Duration = Duration::from_mins(1);
const CONTEXT_TTL: Duration = Duration::from_mins(10);
const MAX_RECENT_NOTIFICATIONS: usize = 256;
const MAX_NOTIFICATION_CONTEXTS: usize = 256;

fn resolve_default_icon(
    notification: &anlg_notification_interface::Notification,
) -> anlg_notification_interface::Notification {
    let mut resolved = notification.clone();

    if resolved.icon.is_none() {
        resolved.icon = resolved
            .source
            .as_ref()
            .and_then(NotificationSource::default_icon);
    }

    resolved
}

pub enum NotificationMutation {
    Confirm,
    Dismiss,
}

fn store_context(key: &str, source: Option<NotificationSource>) -> bool {
    let ctx_map = NOTIFICATION_CONTEXT.get_or_init(|| Mutex::new(BoundedTimedMap::default()));
    let mut map = ctx_map.lock().unwrap();

    let now = Instant::now();
    map.retain_recent(now, CONTEXT_TTL);

    map.insert_without_eviction(key.to_string(), source, now, MAX_NOTIFICATION_CONTEXTS)
}

fn get_context(key: &str) -> NotificationContext {
    let ctx_map = NOTIFICATION_CONTEXT.get_or_init(|| Mutex::new(BoundedTimedMap::default()));
    let source = ctx_map.lock().unwrap().remove(key).flatten();
    NotificationContext {
        key: key.to_string(),
        source,
    }
}

fn show_inner(notification: &anlg_notification_interface::Notification) {
    #[cfg(all(feature = "legacy", target_os = "macos"))]
    anlg_notification_macos::show(notification);

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    anlg_notification_linux::show(notification);
}

pub fn show(notification: &anlg_notification_interface::Notification) {
    let resolved_notification = resolve_default_icon(notification);

    let Some(key) = &notification.key else {
        show_inner(&resolved_notification);
        return;
    };

    let recent_map = RECENT_NOTIFICATIONS.get_or_init(|| Mutex::new(BoundedTimedMap::default()));

    {
        let mut recent_notifications = recent_map.lock().unwrap();
        let now = Instant::now();

        recent_notifications.retain_recent(now, DEDUPE_WINDOW);

        if let Some(last_shown) = recent_notifications.get(key) {
            let duration = now.duration_since(last_shown.timestamp);

            if duration < DEDUPE_WINDOW {
                tracing::info!(key = key, duration = ?duration, "skipping_notification");
                return;
            }
        }

        if !store_context(key, notification.source.clone()) {
            tracing::warn!(
                key = key,
                capacity = MAX_NOTIFICATION_CONTEXTS,
                "skipping_notification_context_capacity"
            );
            return;
        }

        recent_notifications.insert(key.clone(), (), now, MAX_RECENT_NOTIFICATIONS);
    }

    show_inner(&resolved_notification);
}

pub fn clear() {
    #[cfg(all(feature = "legacy", target_os = "macos"))]
    anlg_notification_macos::dismiss_all();

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    anlg_notification_linux::dismiss_all();
}

pub fn setup_dismiss_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        anlg_notification_macos::setup_dismiss_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_dismiss_handler(move |key| {
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
        anlg_notification_macos::setup_collapsed_confirm_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_confirm_handler(move |key| {
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
        anlg_notification_macos::setup_expanded_accept_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_accept_handler(move |key| {
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
        anlg_notification_macos::setup_collapsed_timeout_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_timeout_handler(move |key| {
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
        anlg_notification_macos::setup_option_selected_handler(move |key, tag| {
            f(get_context(&key), tag);
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_option_selected_handler(move |key, index| {
            f(get_context(&key), index);
        });
    }

    let _ = f;
}

pub fn setup_footer_action_handler<F>(f: F)
where
    F: Fn(NotificationContext) + Send + Sync + 'static,
{
    let f = std::sync::Arc::new(f);

    #[cfg(all(feature = "legacy", target_os = "macos"))]
    {
        let f = f.clone();
        anlg_notification_macos::setup_footer_action_handler(move |key, _tag| {
            f(get_context(&key));
        });
    }

    #[cfg(all(feature = "legacy", target_os = "linux"))]
    {
        let f = f.clone();
        anlg_notification_linux::setup_notification_footer_action_handler(move |key| {
            f(get_context(&key));
        });
    }

    let _ = f;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_map_evicts_the_oldest_entry_at_capacity() {
        let now = Instant::now();
        let mut map = BoundedTimedMap::default();

        map.insert("first".to_string(), 1, now, 2);
        map.insert("second".to_string(), 2, now, 2);
        map.insert("third".to_string(), 3, now, 2);

        assert!(map.get("first").is_none());
        assert_eq!(map.get("second").map(|entry| entry.value), Some(2));
        assert_eq!(map.get("third").map(|entry| entry.value), Some(3));
    }

    #[test]
    fn replacing_an_entry_refreshes_its_eviction_order() {
        let now = Instant::now();
        let mut map = BoundedTimedMap::default();

        map.insert("first".to_string(), 1, now, 2);
        map.insert("second".to_string(), 2, now, 2);
        map.insert("first".to_string(), 3, now, 2);
        map.insert("third".to_string(), 4, now, 2);

        assert_eq!(map.get("first").map(|entry| entry.value), Some(3));
        assert!(map.get("second").is_none());
        assert_eq!(map.get("third").map(|entry| entry.value), Some(4));
    }

    #[test]
    fn timed_map_removes_expired_entries() {
        let now = Instant::now();
        let mut map = BoundedTimedMap::default();

        map.insert("expired".to_string(), 1, now - Duration::from_secs(2), 2);
        map.insert("current".to_string(), 2, now, 2);
        map.retain_recent(now, Duration::from_secs(1));

        assert!(map.get("expired").is_none());
        assert_eq!(map.get("current").map(|entry| entry.value), Some(2));
    }

    #[test]
    fn timed_map_rejects_a_new_entry_at_capacity_without_evicting() {
        let now = Instant::now();
        let mut map = BoundedTimedMap::default();

        assert!(map.insert_without_eviction("first".to_string(), 1, now, 2));
        assert!(map.insert_without_eviction("second".to_string(), 2, now, 2));
        assert!(!map.insert_without_eviction("third".to_string(), 3, now, 2));

        assert_eq!(map.get("first").map(|entry| entry.value), Some(1));
        assert_eq!(map.get("second").map(|entry| entry.value), Some(2));
        assert!(map.get("third").is_none());
    }

    #[test]
    fn timed_map_replaces_an_existing_entry_at_capacity_without_evicting() {
        let now = Instant::now();
        let refreshed_at = now + Duration::from_secs(1);
        let mut map = BoundedTimedMap::default();

        assert!(map.insert_without_eviction("first".to_string(), 1, now, 2));
        assert!(map.insert_without_eviction("second".to_string(), 2, now, 2));
        assert!(map.insert_without_eviction("first".to_string(), 3, refreshed_at, 2));

        let first = map.get("first").unwrap();
        assert_eq!(first.value, 3);
        assert_eq!(first.timestamp, refreshed_at);
        assert_eq!(map.get("second").map(|entry| entry.value), Some(2));
        assert_eq!(map.entries.len(), 2);
    }
}
