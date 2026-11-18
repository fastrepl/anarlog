#![cfg(target_os = "linux")]

use std::time::Duration;

use desktop_gpui::dictation::insertion::{Insertion, NativeInsertion};
use gtk::prelude::*;

#[test]
#[ignore = "requires an isolated X11 display and an AT-SPI session bus"]
fn inserts_into_the_focused_field_and_preserves_unsafe_targets() {
    gtk::init().unwrap();
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    let fields = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let entry = gtk::Entry::new();
    entry.set_text("Hello world");
    let password = gtk::Entry::new();
    password.set_visibility(false);
    fields.add(&entry);
    fields.add(&password);
    window.add(&fields);
    window.show_all();
    window.window().unwrap().focus(0);
    entry.grab_focus();
    entry.set_position(5);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let target = drive(&runtime, || {
        for _ in 0..50 {
            if let Ok(target) = NativeInsertion.capture() {
                return target;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the GTK entry did not acquire AT-SPI focus");
    });

    assert!(
        drive(&runtime, {
            let target = target.clone();
            move || NativeInsertion.insert(&target, ", 世界")
        })
        .is_ok()
    );
    assert_eq!(entry.text().as_str(), "Hello, 世界 world");
    assert_eq!(entry.position(), 9);

    entry.select_region(0, 5);
    let error = drive(&runtime, {
        let target = target.clone();
        move || NativeInsertion.insert(&target, "do not replace")
    })
    .unwrap_err();
    assert!(error.to_string().contains("Clear the text selection"));
    assert_eq!(entry.text().as_str(), "Hello, 世界 world");

    entry.select_region(9, 9);
    entry.set_max_length(17);
    let error = drive(&runtime, {
        let target = target.clone();
        move || NativeInsertion.insert(&target, "XYZ")
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("did not insert the full dictation")
    );
    assert_eq!(entry.text().as_str(), "Hello, 世界XY world");

    password.grab_focus();
    assert!(drive(&runtime, || NativeInsertion.capture()).is_err());
    assert!(drive(&runtime, move || NativeInsertion.insert(&target, "stale")).is_err());
    assert_eq!(entry.text().as_str(), "Hello, 世界XY world");
    assert!(password.text().is_empty());
}

fn drive<T: Send + 'static>(
    runtime: &tokio::runtime::Runtime,
    operation: impl FnOnce() -> T + Send + 'static,
) -> T {
    // GTK must serve accessibility requests while the dictation worker waits for D-Bus.
    let task = runtime.spawn_blocking(operation);
    while !task.is_finished() {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    runtime.block_on(task).unwrap()
}
