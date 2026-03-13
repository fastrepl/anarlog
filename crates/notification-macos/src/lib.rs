use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Mutex;

use swift_rs::{Bool, SRString, swift};

pub use hypr_notification_interface::*;

swift!(fn _show_notification(json_payload: &SRString) -> Bool);

swift!(fn _dismiss_all_notifications() -> Bool);

macro_rules! define_notification_callback {
    ($static_name:ident, $setup_fn:ident, $extern_fn:ident) => {
        static $static_name: Mutex<Option<Box<dyn Fn(String) + Send + Sync>>> = Mutex::new(None);

        pub fn $setup_fn<F>(f: F)
        where
            F: Fn(String) + Send + Sync + 'static,
        {
            *$static_name.lock().unwrap() = Some(Box::new(f));
        }

        /// # Safety
        /// `key_ptr` must be a valid, non-null pointer to a null-terminated C string.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $extern_fn(key_ptr: *const c_char) {
            if let Some(cb) = $static_name.lock().unwrap().as_ref() {
                let key = unsafe { CStr::from_ptr(key_ptr) }
                    .to_str()
                    .unwrap()
                    .to_string();
                cb(key);
            }
        }
    };
}

define_notification_callback!(
    COLLAPSED_CONFIRM_CB,
    setup_collapsed_confirm_handler,
    rust_on_collapsed_confirm
);
define_notification_callback!(
    EXPANDED_ACCEPT_CB,
    setup_expanded_accept_handler,
    rust_on_expanded_accept
);
define_notification_callback!(DISMISS_CB, setup_dismiss_handler, rust_on_dismiss);
define_notification_callback!(
    COLLAPSED_TIMEOUT_CB,
    setup_collapsed_timeout_handler,
    rust_on_collapsed_timeout
);
define_notification_callback!(
    EXPANDED_START_TIME_REACHED_CB,
    setup_expanded_start_time_reached_handler,
    rust_on_expanded_start_time_reached
);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NotificationPayload<'a> {
    key: &'a str,
    title: &'a str,
    message: &'a str,
    timeout_seconds: f64,
    start_time: Option<i64>,
    participants: Option<&'a [Participant]>,
    event_details: Option<&'a EventDetails>,
    action_label: Option<&'a str>,
}

pub fn show(notification: &hypr_notification_interface::Notification) {
    let key = notification
        .key
        .as_deref()
        .unwrap_or(notification.title.as_str());
    let timeout_seconds = notification.timeout.map(|d| d.as_secs_f64()).unwrap_or(5.0);

    let payload = NotificationPayload {
        key,
        title: &notification.title,
        message: &notification.message,
        timeout_seconds,
        start_time: notification.start_time,
        participants: notification.participants.as_deref(),
        event_details: notification.event_details.as_ref(),
        action_label: notification.action_label.as_deref(),
    };

    let json = serde_json::to_string(&payload).unwrap();
    let json_str = SRString::from(json.as_str());

    unsafe {
        _show_notification(&json_str);
    }
}

pub fn dismiss_all() {
    unsafe {
        _dismiss_all_notifications();
    }
}
