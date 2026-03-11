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
            .key("test_options")
            .title("Meeting in progress?")
            .message("Noticed microphone usage. Start listening?")
            .timeout(timeout)
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
