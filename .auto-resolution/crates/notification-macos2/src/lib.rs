#![cfg(target_os = "macos")]

mod callbacks;
mod categories;
mod delegate;

use std::sync::{LazyLock, Once};

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

pub fn show(notification: &hypr_notification_interface::Notification) {
    initialize();

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&notification.title));
    content.setBody(&NSString::from_str(&notification.message));

    if let Some(key) = &notification.key {
        let info =
            NSDictionary::from_slices(&[ns_string!("hypr_key")], &[&*NSString::from_str(key)]);
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
        content.setCategoryIdentifier(ns_string!("HYPR_DEFAULT"));
    }

    let identifier = uuid::Uuid::new_v4().to_string();
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&identifier),
        &content,
        None,
    );

    let timeout = notification.timeout;
    let key_for_timeout = notification.key.clone().unwrap_or_default();

    CENTER.addNotificationRequest_withCompletionHandler(
        &request,
        Some(&RcBlock::new(move |err: *mut NSError| {
            if err.is_null() {
                if let Some(duration) = timeout {
                    let identifier = identifier.clone();
                    let key = key_for_timeout.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(duration);
                        let ids = NSArray::from_retained_slice(&[NSString::from_str(&identifier)]);
                        CENTER.removeDeliveredNotificationsWithIdentifiers(&ids);
                        callbacks::fire_timeout(key);
                    });
                }
            } else {
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
    CENTER.removeAllDeliveredNotifications();
}
