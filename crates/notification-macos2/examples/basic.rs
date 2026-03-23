use std::time::Duration;

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example only works on macOS");
}

#[cfg(target_os = "macos")]
fn main() {
    env_logger::init();

    notification_macos2::setup_confirm_handler(|key| {
        println!("[confirm] key={key}");
    });
    notification_macos2::setup_accept_handler(|key| {
        println!("[accept] key={key}");
    });
    notification_macos2::setup_dismiss_handler(|key| {
        println!("[dismiss] key={key}");
    });
    notification_macos2::setup_timeout_handler(|key| {
        println!("[timeout] key={key}");
    });
    notification_macos2::setup_option_selected_handler(|key, idx| {
        println!("[option_selected] key={key} idx={idx}");
    });

    let notification = hypr_notification_interface::Notification::builder()
        .key("test-1")
        .title("Hello from notification-macos2")
        .message("This is a test notification")
        .timeout(Duration::from_secs(10))
        .action_label("Accept")
        .build();

    notification_macos2::show(&notification);
    println!("Notification sent. Waiting 15s for interactions...");

    std::thread::sleep(Duration::from_secs(15));
    println!("Done.");
}
