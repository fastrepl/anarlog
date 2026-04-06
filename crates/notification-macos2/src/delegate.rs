use block2::Block;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, define_class, msg_send};
use objc2_foundation::{NSString, ns_string};
use objc2_user_notifications::{
    UNNotification, UNNotificationPresentationOptions, UNNotificationResponse,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use crate::callbacks;

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "HyprNotifDelegate"]
    #[derive(Debug)]
    pub(crate) struct NotifDelegate;

    unsafe impl NSObjectProtocol for NotifDelegate {}
    unsafe impl UNUserNotificationCenterDelegate for NotifDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        unsafe fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &Block<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let options = UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound
                | UNNotificationPresentationOptions::List;
            completion_handler.call((options,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        unsafe fn did_receive_notification(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &Block<dyn Fn()>,
        ) {
            let action = response.actionIdentifier();
            let user_info = response.notification().request().content().userInfo();
            let key = user_info
                .valueForKey(ns_string!("hypr_key"))
                .and_then(|v| v.downcast::<NSString>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            log::debug!("did_receive_notification action={action:?} key={key}");

            if action.to_string() == "com.apple.UNNotificationDefaultActionIdentifier" {
                callbacks::fire_confirm(key);
            } else if action.to_string() == "ACCEPT" {
                callbacks::fire_accept(key);
            } else if action.to_string() == "com.apple.UNNotificationDismissActionIdentifier" {
                callbacks::fire_dismiss(key);
            } else {
                let action_str = action.to_string();
                if let Some(suffix) = action_str.strip_prefix("OPTION_")
                    && let Ok(n) = suffix.parse::<i32>()
                {
                    callbacks::fire_option_selected(key, n);
                }
            }

            completion_handler.call(());
        }
    }
);

impl NotifDelegate {
    pub(crate) fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) fn set_delegate(center: &UNUserNotificationCenter) {
    let delegate = NotifDelegate::new();
    let delegate_proto = ProtocolObject::from_retained(delegate.clone());
    center.setDelegate(Some(&delegate_proto));
    // Leak to prevent deallocation — center holds a weak reference.
    Retained::into_raw(delegate);
}
