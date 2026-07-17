mod common;

use notification_macos::*;

use std::ops::Add;
use std::time::Duration;

fn main() {
    common::run_app(|| {
        std::thread::sleep(Duration::from_millis(200));
        let timeout = Duration::from_secs(15);

        setup_option_selected_handler(|key, index| {
            println!("option_selected: key={}, index={}", key, index);
        });
        setup_dismiss_handler(|key, _tag| {
            println!("dismiss: {}", key);
        });
        setup_collapsed_timeout_handler(|key, _tag| {
            println!("collapsed_timeout: {}", key);
        });

        let notification = Notification::builder()
            .key("mic-started:us.zoom.xos")
            .title("Are you in a meeting?")
            .message("")
            .timeout(timeout)
            .source(NotificationSource::MicDetected {
                app_names: vec!["Zoom".to_string()],
                app_ids: vec!["us.zoom.xos".to_string()],
                event_ids: vec![
                    "team-standup".to_string(),
                    "alice-1-1".to_string(),
                    "sprint-planning".to_string(),
                ],
            })
            .options(vec![
                "Team Standup".to_string(),
                "1:1 with Alice".to_string(),
                "Sprint Planning".to_string(),
            ])
            .build();

        show(&notification);
        std::thread::sleep(timeout.add(Duration::from_secs(5)));
        std::process::exit(0);
    });
}
