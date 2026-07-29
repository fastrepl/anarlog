use std::sync::Mutex;

type NotificationCallback = Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>;
type NotificationOptionCallback = Mutex<Option<Box<dyn Fn(String, i32) + Send + Sync>>>;

static CONFIRM_CB: NotificationCallback = Mutex::new(None);
static ACCEPT_CB: NotificationCallback = Mutex::new(None);
static DISMISS_CB: NotificationCallback = Mutex::new(None);
static TIMEOUT_CB: NotificationCallback = Mutex::new(None);
static OPTION_SELECTED_CB: NotificationOptionCallback = Mutex::new(None);
static FOOTER_ACTION_CB: NotificationCallback = Mutex::new(None);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PrimaryAction<'a> {
    Accept { label: &'a str, destructive: bool },
    Options(&'a [String]),
}

pub(crate) fn primary_action(
    notification: &anlg_notification_interface::Notification,
) -> PrimaryAction<'_> {
    if let Some(options) = notification
        .options
        .as_deref()
        .filter(|options| !options.is_empty())
    {
        return PrimaryAction::Options(options);
    }

    PrimaryAction::Accept {
        label: notification
            .action_label
            .as_deref()
            .unwrap_or("Open Anarlog"),
        destructive: matches!(
            notification.action_variant,
            Some(anlg_notification_interface::NotificationActionVariant::Destructive)
        ),
    }
}

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
    fn resolves_linux_primary_actions_from_the_shared_notification_contract() {
        let default_action = anlg_notification_interface::Notification::builder()
            .title("Upcoming event")
            .message("Starting soon")
            .build();
        assert_eq!(
            primary_action(&default_action),
            PrimaryAction::Accept {
                label: "Open Anarlog",
                destructive: false,
            }
        );

        let destructive_action = anlg_notification_interface::Notification::builder()
            .title("Did your meeting end?")
            .message("Anarlog will stop listening soon.")
            .action_label("Stop")
            .action_variant(anlg_notification_interface::NotificationActionVariant::Destructive)
            .build();
        assert_eq!(
            primary_action(&destructive_action),
            PrimaryAction::Accept {
                label: "Stop",
                destructive: true,
            }
        );

        let options_action = anlg_notification_interface::Notification::builder()
            .title("Choose a meeting")
            .message("")
            .action_label("Ignored when options are present")
            .options(vec!["Design sync".to_string(), "Planning".to_string()])
            .build();
        assert_eq!(
            primary_action(&options_action),
            PrimaryAction::Options(&["Design sync".to_string(), "Planning".to_string()])
        );
    }

    #[test]
    fn routes_each_linux_notification_action_to_its_registered_handler() {
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
