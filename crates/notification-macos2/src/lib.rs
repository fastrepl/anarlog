#![cfg(target_os = "macos")]

mod callbacks;
mod categories;
mod delegate;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, LazyLock, Mutex, Once};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSString, ns_string};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNUserNotificationCenter,
};

pub use callbacks::{
    setup_accept_handler, setup_confirm_handler, setup_dismiss_handler,
    setup_option_selected_handler, setup_timeout_handler,
};

const NEEDS_SIGN: &str = "the application must be code-signed for UNUserNotificationCenter to work";

// UNUserNotificationCenter is thread-safe per Apple docs, but objc2 doesn't impl Send/Sync.
struct SendCenter(Retained<UNUserNotificationCenter>);
unsafe impl Send for SendCenter {}
unsafe impl Sync for SendCenter {}
impl std::ops::Deref for SendCenter {
    type Target = UNUserNotificationCenter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

static CENTER: LazyLock<SendCenter> =
    LazyLock::new(|| SendCenter(UNUserNotificationCenter::currentNotificationCenter()));

const MAX_ACTIVE_TIMEOUTS: usize = 256;

struct TimeoutEntry {
    identifier: String,
    key: String,
    deadline: Instant,
    reservation_id: u64,
    generation: u64,
}

struct TimeoutCandidate {
    identifier: String,
    reservation_id: u64,
    generation: u64,
}

#[derive(Default)]
struct TimeoutState {
    entries: HashMap<String, TimeoutEntry>,
    next_reservation_id: u64,
    generation: u64,
    acknowledged_generation: u64,
    worker_thread_id: Option<std::thread::ThreadId>,
    dispatching: Option<(String, u64)>,
    shutdown: bool,
}

impl TimeoutState {
    fn try_reserve(&mut self, identifier: String, key: String, deadline: Instant) -> bool {
        if !self.entries.contains_key(&identifier) && self.entries.len() >= MAX_ACTIVE_TIMEOUTS {
            return false;
        }

        self.next_reservation_id = self
            .next_reservation_id
            .checked_add(1)
            .expect("notification timeout reservation ID overflowed");

        self.entries.insert(
            identifier.clone(),
            TimeoutEntry {
                identifier,
                key,
                deadline,
                reservation_id: self.next_reservation_id,
                generation: self.generation,
            },
        );
        true
    }

    fn cancel(&mut self, identifier: &str) -> bool {
        self.entries.remove(identifier).is_some()
    }

    fn next_due(&self, now: Instant) -> Option<TimeoutCandidate> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.deadline <= now)
            .min_by_key(|(_, entry)| entry.deadline)
            .map(|(identifier, entry)| TimeoutCandidate {
                identifier: identifier.clone(),
                reservation_id: entry.reservation_id,
                generation: entry.generation,
            })
    }

    fn take_candidate(&mut self, candidate: &TimeoutCandidate) -> Option<TimeoutEntry> {
        let entry = self.entries.get(&candidate.identifier)?;
        if entry.reservation_id != candidate.reservation_id
            || entry.generation != candidate.generation
            || candidate.generation != self.generation
        {
            return None;
        }
        self.entries.remove(&candidate.identifier)
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().map(|entry| entry.deadline).min()
    }

    fn clear(&mut self) -> u64 {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("notification timeout generation overflowed");
        self.entries.clear();
        self.generation
    }
}

struct TimeoutScheduler {
    shared: Arc<(Mutex<TimeoutState>, Condvar)>,
}

type TimeoutExpirer = Arc<dyn Fn(TimeoutEntry) + Send + Sync>;
type BeforeDispatch = Arc<dyn Fn() + Send + Sync>;

impl TimeoutScheduler {
    fn new() -> Self {
        Self::start(Arc::new(expire_timeout), None)
    }

    fn start(expirer: TimeoutExpirer, before_dispatch: Option<BeforeDispatch>) -> Self {
        let shared = Arc::new((Mutex::new(TimeoutState::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("notification-timeouts".to_string())
            .spawn(move || timeout_worker(worker_shared, expirer, before_dispatch))
            .expect("failed to start notification timeout worker");
        Self { shared }
    }

    fn try_reserve(&self, identifier: String, key: String, duration: Duration) -> bool {
        let now = Instant::now();
        let deadline = now.checked_add(duration).unwrap_or(now);
        let (state, wake) = &*self.shared;
        let reserved = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .try_reserve(identifier, key, deadline);
        if reserved {
            wake.notify_one();
        }
        reserved
    }

    fn cancel(&self, identifier: &str) -> bool {
        let (state, wake) = &*self.shared;
        let current_thread_id = std::thread::current().id();
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let canceled = state.cancel(identifier);
        wake.notify_all();

        while state
            .dispatching
            .as_ref()
            .is_some_and(|(dispatching_identifier, _)| dispatching_identifier == identifier)
            && state.worker_thread_id.as_ref() != Some(&current_thread_id)
        {
            state = wake
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }

        canceled
    }

    fn clear(&self) {
        let (state, wake) = &*self.shared;
        let current_thread_id = std::thread::current().id();
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = state.clear();
        wake.notify_all();

        if state.worker_thread_id.as_ref() == Some(&current_thread_id) {
            return;
        }

        while state.acknowledged_generation < generation {
            state = wake
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    #[cfg(test)]
    fn shutdown(&self) {
        let (state, wake) = &*self.shared;
        state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .shutdown = true;
        wake.notify_all();
    }
}

static TIMEOUT_SCHEDULER: LazyLock<TimeoutScheduler> = LazyLock::new(TimeoutScheduler::new);

fn timeout_worker(
    shared: Arc<(Mutex<TimeoutState>, Condvar)>,
    expirer: TimeoutExpirer,
    before_dispatch: Option<BeforeDispatch>,
) {
    let (state, wake) = &*shared;
    {
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.worker_thread_id = Some(std::thread::current().id());
        wake.notify_all();
    }

    loop {
        let candidate = {
            let mut state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if state.shutdown {
                    return;
                }

                state.acknowledged_generation = state.generation;
                wake.notify_all();

                let now = Instant::now();
                if let Some(candidate) = state.next_due(now) {
                    break candidate;
                }

                state = match state.next_deadline() {
                    Some(deadline) => {
                        let wait = deadline.saturating_duration_since(now);
                        wake.wait_timeout(state, wait)
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .0
                    }
                    None => wake
                        .wait(state)
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                };
            }
        };

        if let Some(before_dispatch) = &before_dispatch {
            before_dispatch();
        }

        let entry = {
            let mut state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.shutdown {
                return;
            }

            let entry = state.take_candidate(&candidate);
            if let Some(entry) = &entry {
                state.dispatching = Some((entry.identifier.clone(), entry.reservation_id));
            } else {
                state.acknowledged_generation = state.generation;
                wake.notify_all();
            }
            entry
        };

        let Some(entry) = entry else {
            continue;
        };

        let dispatching = (entry.identifier.clone(), entry.reservation_id);
        expirer(entry);

        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.dispatching.as_ref() == Some(&dispatching) {
            state.dispatching = None;
        }
        state.acknowledged_generation = state.generation;
        wake.notify_all();
    }
}

fn expire_timeout(entry: TimeoutEntry) {
    let ids = NSArray::from_retained_slice(&[NSString::from_str(&entry.identifier)]);
    CENTER.removeDeliveredNotificationsWithIdentifiers(&ids);
    callbacks::fire_timeout(entry.key);
}

fn ns_error_to_string(err: *mut NSError) -> String {
    if err.is_null() {
        "null error".to_string()
    } else {
        unsafe {
            let err: &NSError = &*err;
            format!(
                "{} {:?}",
                err.localizedDescription(),
                err.localizedFailureReason()
            )
        }
    }
}

pub fn initialize() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        CENTER.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert
                | UNAuthorizationOptions::Sound
                | UNAuthorizationOptions::Provisional,
            &RcBlock::new(|ok: Bool, err| {
                if ok.is_false() {
                    log::error!(
                        "requestAuthorization failed: {}. {NEEDS_SIGN}",
                        ns_error_to_string(err)
                    );
                }
            }),
        );

        categories::register_default(&CENTER);
        delegate::set_delegate(&CENTER);
    });
}

pub fn show(notification: &anlg_notification_interface::Notification) {
    initialize();

    let identifier = uuid::Uuid::new_v4().to_string();
    let timeout_reserved = if let Some(duration) = notification.timeout {
        let key = notification.key.clone().unwrap_or_default();
        if !TIMEOUT_SCHEDULER.try_reserve(identifier.clone(), key.clone(), duration) {
            log::error!(
                "notification timeout capacity ({MAX_ACTIVE_TIMEOUTS}) reached; skipping notification"
            );
            callbacks::fire_timeout(key);
            return;
        }
        true
    } else {
        false
    };

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&notification.title));
    content.setBody(&NSString::from_str(&notification.message));

    if let Some(key) = &notification.key {
        let info =
            NSDictionary::from_slices(&[ns_string!("anlg_key")], &[&*NSString::from_str(key)]);
        // Safety: the NSDictionary we built is well-formed.
        unsafe {
            content.setUserInfo(
                info.downcast_ref::<NSDictionary>()
                    .expect("is NSDictionary"),
            );
        }
    }

    if let Some(options) = &notification.options {
        if !options.is_empty() {
            let cat_id = categories::ensure_options_category_with_labels(&CENTER, options);
            content.setCategoryIdentifier(&NSString::from_str(&cat_id));
        }
    } else if notification.action_label.is_some() {
        content.setCategoryIdentifier(ns_string!("ANLG_DEFAULT"));
    }

    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&identifier),
        &content,
        None,
    );

    let identifier_for_completion = identifier.clone();

    CENTER.addNotificationRequest_withCompletionHandler(
        &request,
        Some(&RcBlock::new(move |err: *mut NSError| {
            if !err.is_null() {
                if timeout_reserved {
                    TIMEOUT_SCHEDULER.cancel(&identifier_for_completion);
                }
                log::error!(
                    "addNotificationRequest failed: {}. {NEEDS_SIGN}",
                    ns_error_to_string(err)
                );
            }
        })),
    );
}

pub fn dismiss_all() {
    initialize();
    TIMEOUT_SCHEDULER.clear();
    CENTER.removeAllDeliveredNotifications();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    #[test]
    fn timeout_state_rejects_257th_reservation_at_capacity() {
        let now = Instant::now();
        let mut state = TimeoutState::default();
        for index in 0..MAX_ACTIVE_TIMEOUTS {
            assert!(state.try_reserve(
                format!("notification-{index}"),
                format!("key-{index}"),
                now + Duration::from_secs(index as u64 + 1),
            ));
        }

        assert!(!state.try_reserve(
            "notification-256".to_string(),
            "key-256".to_string(),
            now + Duration::from_secs(1),
        ));

        assert_eq!(state.entries.len(), MAX_ACTIVE_TIMEOUTS);
        assert!(state.entries.contains_key("notification-0"));
        assert!(!state.entries.contains_key("notification-256"));
    }

    #[test]
    fn timeout_state_returns_due_entries_by_deadline() {
        let now = Instant::now();
        let mut state = TimeoutState::default();
        state.try_reserve(
            "later".to_string(),
            "later-key".to_string(),
            now + Duration::from_secs(2),
        );
        state.try_reserve(
            "earlier".to_string(),
            "earlier-key".to_string(),
            now + Duration::from_secs(1),
        );

        assert!(state.next_due(now).is_none());
        assert_eq!(
            state
                .next_due(now + Duration::from_secs(3))
                .expect("entry should be due")
                .identifier,
            "earlier"
        );
    }

    #[test]
    fn timeout_state_replaces_existing_entry_without_eviction() {
        let now = Instant::now();
        let mut state = TimeoutState::default();
        for index in 0..MAX_ACTIVE_TIMEOUTS {
            state.try_reserve(
                format!("notification-{index}"),
                format!("key-{index}"),
                now + Duration::from_secs(index as u64 + 1),
            );
        }

        assert!(state.try_reserve(
            "notification-0".to_string(),
            "replacement-key".to_string(),
            now + Duration::from_secs(500),
        ));

        assert_eq!(state.entries.len(), MAX_ACTIVE_TIMEOUTS);
        assert_eq!(
            state
                .entries
                .get("notification-0")
                .expect("replacement should remain scheduled")
                .key,
            "replacement-key"
        );
        assert!(state.entries.contains_key("notification-1"));
    }

    #[test]
    fn cancel_prevents_a_selected_timeout_from_firing() {
        let (selected_tx, selected_rx) = mpsc::sync_channel(1);
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let release_worker = Arc::clone(&release);
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_worker = Arc::clone(&fired);
        let scheduler = TimeoutScheduler::start(
            Arc::new(move |_entry| {
                fired_worker.fetch_add(1, Ordering::SeqCst);
            }),
            Some(Arc::new(move || {
                selected_tx
                    .send(())
                    .expect("test receiver should remain open");
                let (released, wake) = &*release_worker;
                let mut released = released
                    .lock()
                    .expect("release lock should not be poisoned");
                while !*released {
                    released = wake
                        .wait(released)
                        .expect("release lock should not be poisoned");
                }
            })),
        );

        assert!(scheduler.try_reserve(
            "notification".to_string(),
            "key".to_string(),
            Duration::ZERO,
        ));
        selected_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker should select the timeout");
        assert!(scheduler.cancel("notification"));

        let (released, wake) = &*release;
        *released
            .lock()
            .expect("release lock should not be poisoned") = true;
        wake.notify_all();

        scheduler.clear();
        assert_eq!(fired.load(Ordering::SeqCst), 0);
        scheduler.shutdown();
    }

    #[test]
    fn clear_fences_a_timeout_already_selected_by_the_worker() {
        let (selected_tx, selected_rx) = mpsc::sync_channel(1);
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let release_worker = Arc::clone(&release);
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_worker = Arc::clone(&fired);
        let scheduler = Arc::new(TimeoutScheduler::start(
            Arc::new(move |_entry| {
                fired_worker.fetch_add(1, Ordering::SeqCst);
            }),
            Some(Arc::new(move || {
                selected_tx
                    .send(())
                    .expect("test receiver should remain open");
                let (released, wake) = &*release_worker;
                let mut released = released
                    .lock()
                    .expect("release lock should not be poisoned");
                while !*released {
                    released = wake
                        .wait(released)
                        .expect("release lock should not be poisoned");
                }
            })),
        ));

        assert!(scheduler.try_reserve(
            "notification".to_string(),
            "key".to_string(),
            Duration::ZERO,
        ));
        selected_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker should select the timeout");

        let generation_before_clear = scheduler
            .shared
            .0
            .lock()
            .expect("scheduler state should not be poisoned")
            .generation;
        let (cleared_tx, cleared_rx) = mpsc::sync_channel(1);
        let clearing_scheduler = Arc::clone(&scheduler);
        std::thread::spawn(move || {
            clearing_scheduler.clear();
            cleared_tx
                .send(())
                .expect("test receiver should remain open");
        });
        {
            let (state, wake) = &*scheduler.shared;
            let mut state = state
                .lock()
                .expect("scheduler state should not be poisoned");
            while state.generation == generation_before_clear {
                state = wake
                    .wait(state)
                    .expect("scheduler state should not be poisoned");
            }
        }
        assert!(cleared_rx.recv_timeout(Duration::from_millis(50)).is_err());

        let (released, wake) = &*release;
        *released
            .lock()
            .expect("release lock should not be poisoned") = true;
        wake.notify_all();

        cleared_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("clear should return after the worker acknowledges cancellation");
        assert_eq!(fired.load(Ordering::SeqCst), 0);
        scheduler.shutdown();
    }
}
