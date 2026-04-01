mod common;

use notification_macos::*;

use std::time::Duration;

fn main() {
    common::run_app(|| {
        std::thread::sleep(Duration::from_millis(200));

        setup_expanded_accept_handler(|id, _tag| {
            println!("expanded_accept: {}", id);
        });
        setup_collapsed_confirm_handler(|id, _tag| {
            println!("collapsed_confirm: {}", id);
        });
        setup_dismiss_handler(|id, _tag| {
            println!("dismiss: {}", id);
        });
        setup_collapsed_timeout_handler(|id, _tag| {
            println!("collapsed_timeout: {}", id);
        });

        let participants = vec![
            Participant {
                name: None,
                email: "sjobs@apple.com".to_string(),
                status: ParticipantStatus::Accepted,
            },
            Participant {
                name: Some("John Jeong".to_string()),
                email: "john@hyprnote.com".to_string(),
                status: ParticipantStatus::Accepted,
            },
            Participant {
                name: Some("Yujong Lee".to_string()),
                email: "yujonglee@hyprnote.com".to_string(),
                status: ParticipantStatus::Maybe,
            },
            Participant {
                name: Some("Tony Stark".to_string()),
                email: "tony@hyprnote.com".to_string(),
                status: ParticipantStatus::Declined,
            },
        ];

        let event_details = EventDetails {
            what: "Discovery call - Apple <> Hyprnote".to_string(),
            timezone: Some("America/Cupertino".to_string()),
            location: Some("https://zoom.us/j/123456789".to_string()),
        };

        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 120;

        let notification = Notification::builder()
            .key("event:apple-discovery-call")
            .title("Test Notification")
            .message("")
            .source(NotificationSource::CalendarEvent {
                event_id: "apple-discovery-call".to_string(),
            })
            .participants(participants)
            .event_details(event_details)
            .action_label("Take Notes")
            .start_time(start_time)
            .build();

        show(&notification);
        std::thread::sleep(Duration::from_secs(60));
        std::process::exit(0);
    });
}
