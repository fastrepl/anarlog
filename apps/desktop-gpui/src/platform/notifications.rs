use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub use anlg_notification_interface::{Notification, NotificationContext, NotificationSource};
use desktop_runtime::{Result, ServiceError};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Confirm,
    Accept,
    Dismiss,
    Timeout,
    Option(usize),
    Footer,
}

pub trait NotificationAdapter {
    fn show(&mut self, notification: &Notification) -> Result<()>;
}

pub struct NativeNotifications;

impl NotificationAdapter for NativeNotifications {
    fn show(&mut self, notification: &Notification) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            gtk::init().map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            anlg_notification_linux::show(notification);
        }
        #[cfg(target_os = "macos")]
        anlg_notification_macos::show(notification);
        #[cfg(target_os = "windows")]
        anlg_notification_windows::show(notification);
        Ok(())
    }
}

impl NativeNotifications {
    /// Install once on the native UI thread; drain actions through `Notifications::action`.
    pub fn install_actions(send: Arc<dyn Fn(String, Action) + Send + Sync>) {
        #[cfg(target_os = "linux")]
        Self::install_portable_actions(send);
        #[cfg(target_os = "windows")]
        Self::install_portable_actions(send);
        #[cfg(target_os = "macos")]
        {
            let cb = send.clone();
            anlg_notification_macos::setup_collapsed_confirm_handler(move |key, _| {
                cb(key, Action::Confirm)
            });
            let cb = send.clone();
            anlg_notification_macos::setup_expanded_accept_handler(move |key, _| {
                cb(key, Action::Accept)
            });
            let cb = send.clone();
            anlg_notification_macos::setup_dismiss_handler(move |key, _| cb(key, Action::Dismiss));
            let cb = send.clone();
            anlg_notification_macos::setup_collapsed_timeout_handler(move |key, _| {
                cb(key, Action::Timeout)
            });
            let cb = send.clone();
            anlg_notification_macos::setup_option_selected_handler(move |key, index| {
                if let Ok(index) = usize::try_from(index) {
                    cb(key, Action::Option(index));
                }
            });
            anlg_notification_macos::setup_footer_action_handler(move |key, _| {
                send(key, Action::Footer)
            });
        }
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn install_portable_actions(send: Arc<dyn Fn(String, Action) + Send + Sync>) {
        let cb = send.clone();
        native_notifications::setup_notification_confirm_handler(move |key| {
            cb(key, Action::Confirm)
        });
        let cb = send.clone();
        native_notifications::setup_notification_accept_handler(move |key| cb(key, Action::Accept));
        let cb = send.clone();
        native_notifications::setup_notification_dismiss_handler(move |key| {
            cb(key, Action::Dismiss)
        });
        let cb = send.clone();
        native_notifications::setup_notification_timeout_handler(move |key| {
            cb(key, Action::Timeout)
        });
        let cb = send.clone();
        native_notifications::setup_notification_option_selected_handler(move |key, index| {
            if let Ok(index) = usize::try_from(index) {
                cb(key, Action::Option(index));
            }
        });
        native_notifications::setup_notification_footer_action_handler(move |key| {
            send(key, Action::Footer)
        });
    }
}

#[cfg(target_os = "linux")]
use anlg_notification_linux as native_notifications;
#[cfg(target_os = "windows")]
use anlg_notification_windows as native_notifications;

pub struct Notifications<A> {
    adapter: A,
    active: HashMap<Arc<str>, (Instant, Notification)>,
    recent: HashMap<Arc<str>, Instant>,
}

impl<A: NotificationAdapter> Notifications<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            active: HashMap::new(),
            recent: HashMap::new(),
        }
    }

    pub fn show(&mut self, notification: Notification, now: Instant) -> Result<bool> {
        self.active
            .retain(|_, (time, _)| now.saturating_duration_since(*time) < Duration::from_secs(600));
        self.recent
            .retain(|_, time| now.saturating_duration_since(*time) < Duration::from_secs(60));
        let Some(key) = notification.key.as_deref() else {
            self.adapter.show(&notification)?;
            return Ok(true);
        };
        if self.recent.contains_key(key) {
            return Ok(false);
        }
        if self.active.len() >= 256 || self.recent.len() >= 256 {
            return Err(ServiceError::Busy);
        }
        self.adapter.show(&notification)?;
        let key: Arc<str> = key.into();
        self.recent.insert(key.clone(), now);
        self.active.insert(key, (now, notification));
        Ok(true)
    }

    pub fn action(
        &mut self,
        key: &str,
        action: Action,
        now: Instant,
    ) -> Result<NotificationContext> {
        let (time, notification) = self.active.get(key).ok_or(ServiceError::Conflict)?;
        if now.saturating_duration_since(*time) >= Duration::from_secs(600) {
            self.active.remove(key);
            return Err(ServiceError::Conflict);
        }
        if let Action::Option(index) = action
            && index >= notification.options.as_ref().map_or(0, Vec::len)
        {
            return Err(ServiceError::Conflict);
        }
        // Mirrors the shipping notification crate: the primary button carries the notification's
        // action and a menu choice carries the menu's action.
        let action = match action {
            Action::Accept => notification.action,
            Action::Option(_) => notification.action_menu.as_ref().map(|menu| menu.action),
            _ => None,
        };
        let context = NotificationContext {
            key: key.into(),
            source: notification.source.clone(),
            action,
        };
        self.active.remove(key);
        Ok(context)
    }
}

pub struct UnavailableNotifications;

impl NotificationAdapter for UnavailableNotifications {
    fn show(&mut self, _: &Notification) -> Result<()> {
        Err(ServiceError::Unsupported(
            "A native notification host with action callbacks is required".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host;
    impl NotificationAdapter for Host {
        fn show(&mut self, _: &Notification) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn context_survives_deduplication_and_invalid_actions_but_is_consumed_once() {
        let mut notifications = Notifications::new(Host);
        let notification = Notification::builder()
            .key("meeting")
            .title("Meeting")
            .message("Soon")
            .source(NotificationSource::Session {
                session_id: "session-1".into(),
            })
            .options(vec!["Join".into()])
            .build();
        let now = Instant::now();
        assert!(notifications.show(notification.clone(), now).unwrap());
        assert!(!notifications.show(notification.clone(), now).unwrap());
        assert!(
            notifications
                .action("meeting", Action::Option(9), now)
                .is_err()
        );
        assert!(
            matches!(notifications.action("meeting", Action::Accept, now).unwrap().source,
            Some(NotificationSource::Session { session_id }) if session_id == "session-1")
        );
        assert!(
            notifications
                .action("meeting", Action::Dismiss, now)
                .is_err()
        );
        assert!(
            notifications
                .show(notification, now + Duration::from_secs(61))
                .unwrap()
        );
        assert!(
            notifications
                .action("meeting", Action::Timeout, now + Duration::from_secs(662))
                .is_err()
        );
    }

    #[test]
    fn unavailable_native_host_never_records_success() {
        let mut notifications = Notifications::new(UnavailableNotifications);
        let notification = Notification::builder()
            .key("x")
            .title("Title")
            .message("Message")
            .build();
        assert!(matches!(
            notifications.show(notification, Instant::now()),
            Err(ServiceError::Unsupported(_))
        ));
        assert!(notifications.active.is_empty());
        assert!(notifications.recent.is_empty());
    }
}
