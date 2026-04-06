mod common;

use notification_macos::*;

use std::ops::Add;
use std::time::Duration;

fn main() {
    common::run_app(|| {
        std::thread::sleep(Duration::from_millis(200));
        let timeout = Duration::from_secs(5);

        setup_footer_action_handler(|id, _tag| {
            println!("footer_action: {}", id);
        });

        let notification = Notification::builder()
            .key("custom:basic-reminder")
            .title("Quick reminder")
            .message("")
            .timeout(timeout)
            .source(NotificationSource::MicDetected {
                app_names: vec!["Slack".to_string()],
                app_ids: vec!["com.tinyspeck.slackmacgap".to_string()],
                event_ids: vec![],
            })
            .action_label("Open")
            .footer(NotificationFooter {
                text: "Ignore reminders like this?".to_string(),
                action_label: "YES".to_string(),
            })
            .build();

        show(&notification);
        std::thread::sleep(timeout.add(Duration::from_secs(5)));
        std::process::exit(0);
    });
}
