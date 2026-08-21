#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::sync::Mutex;

type NotificationCallback = Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>;
type NotificationOptionCallback = Mutex<Option<Box<dyn Fn(String, i32) + Send + Sync>>>;

static CONFIRM_CB: NotificationCallback = Mutex::new(None);
static ACCEPT_CB: NotificationCallback = Mutex::new(None);
static DISMISS_CB: NotificationCallback = Mutex::new(None);
static TIMEOUT_CB: NotificationCallback = Mutex::new(None);
static OPTION_SELECTED_CB: NotificationOptionCallback = Mutex::new(None);
static FOOTER_ACTION_CB: NotificationCallback = Mutex::new(None);

pub fn setup_notification_confirm_handler<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    *CONFIRM_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_notification_accept_handler<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    *ACCEPT_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_notification_dismiss_handler<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    *DISMISS_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_notification_timeout_handler<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    *TIMEOUT_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_notification_option_selected_handler<F>(f: F)
where
    F: Fn(String, i32) + Send + Sync + 'static,
{
    *OPTION_SELECTED_CB.lock().unwrap() = Some(Box::new(f));
}

pub fn setup_notification_footer_action_handler<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    *FOOTER_ACTION_CB.lock().unwrap() = Some(Box::new(f));
}

pub(crate) fn confirm(key: String) {
    if let Some(cb) = CONFIRM_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn accept(key: String) {
    if let Some(cb) = ACCEPT_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn dismiss(key: String) {
    if let Some(cb) = DISMISS_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn timeout(key: String) {
    if let Some(cb) = TIMEOUT_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

pub(crate) fn option_selected(key: String, index: i32) {
    if let Some(cb) = OPTION_SELECTED_CB.lock().unwrap().as_ref() {
        cb(key, index);
    }
}

pub(crate) fn footer_action(key: String) {
    if let Some(cb) = FOOTER_ACTION_CB.lock().unwrap().as_ref() {
        cb(key);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn routes_each_windows_notification_action_to_its_registered_handler() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let record = |event: &'static str| {
            let events = events.clone();
            move |key: String| events.lock().unwrap().push(format!("{event}:{key}"))
        };

        setup_notification_confirm_handler(record("confirm"));
        setup_notification_accept_handler(record("accept"));
        setup_notification_dismiss_handler(record("dismiss"));
        setup_notification_timeout_handler(record("timeout"));
        setup_notification_footer_action_handler(record("footer"));
        let option_events = events.clone();
        setup_notification_option_selected_handler(move |key, index| {
            option_events
                .lock()
                .unwrap()
                .push(format!("option:{key}:{index}"));
        });

        confirm("meeting".to_string());
        accept("meeting".to_string());
        dismiss("meeting".to_string());
        timeout("meeting".to_string());
        option_selected("meeting".to_string(), 2);
        footer_action("meeting".to_string());

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "confirm:meeting".to_string(),
                "accept:meeting".to_string(),
                "dismiss:meeting".to_string(),
                "timeout:meeting".to_string(),
                "option:meeting:2".to_string(),
                "footer:meeting".to_string(),
            ]
        );
    }
}
