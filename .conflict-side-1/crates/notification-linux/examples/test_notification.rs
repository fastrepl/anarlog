use gtk::glib;
use gtk::prelude::*;
use notification_linux::*;

fn main() {
    gtk::init().expect("Failed to initialize GTK");

    let notification = Notification::builder()
        .title("Test Notification")
        .message("This is a test notification from hyprnote")
        .timeout(std::time::Duration::from_secs(5))
        .build();

    show(&notification);

    glib::timeout_add_seconds_local_once(10, || {
        gtk::main_quit();
    });

    gtk::main();
}
