#[macro_export]
macro_rules! common_event_derives {
    ($item:item) => {
        #[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
        $item
    };
}

common_event_derives! {
    #[serde(tag = "type")]
    pub enum NotificationEvent {
        #[serde(rename = "notification_confirm")]
        Confirm { key: String, source: Option<hypr_notification::NotificationSource> },
        #[serde(rename = "notification_accept")]
        Accept { key: String, source: Option<hypr_notification::NotificationSource> },
        #[serde(rename = "notification_dismiss")]
        Dismiss { key: String, source: Option<hypr_notification::NotificationSource> },
        #[serde(rename = "notification_timeout")]
        Timeout { key: String, source: Option<hypr_notification::NotificationSource> },
        #[serde(rename = "notification_option_selected")]
        OptionSelected { key: String, source: Option<hypr_notification::NotificationSource>, selected_index: i32 },
    }
}
