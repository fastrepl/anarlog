#[cfg(target_os = "macos")]
fn main() {
    use objc2::{msg_send, runtime::AnyObject};
    use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};
    use objc2_foundation::MainThreadMarker;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tauri::Manager;

    let hidden = Arc::new(AtomicBool::new(false));
    let observed = hidden.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dock::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_windows::init())
        .on_window_event(tauri_plugin_windows::on_window_event)
        .setup(|app| {
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External("about:blank".parse().unwrap()),
            )
            .build()?;
            Ok(())
        })
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let started = std::time::Instant::now();
    let mut requested = false;
    let exit_code = app.run_return(move |app, event| {
        if let tauri::RunEvent::MainEventsCleared = event {
            let window = app.get_webview_window("main").unwrap();
            if !requested {
                assert!(window.is_visible().unwrap());
                let ns_app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
                let reply: NSApplicationTerminateReply = unsafe {
                    let delegate: *mut AnyObject = msg_send![&*ns_app, delegate];
                    msg_send![delegate, applicationShouldTerminate: &*ns_app]
                };
                assert_eq!(reply, NSApplicationTerminateReply::TerminateCancel);
                requested = true;
            } else if !window.is_visible().unwrap() {
                observed.store(true, Ordering::SeqCst);
                anlg_intercept::set_force_quit();
                app.exit(0);
            } else if started.elapsed() > std::time::Duration::from_secs(10) {
                anlg_intercept::set_force_quit();
                app.exit(1);
            }
        }
    });
    assert_eq!(exit_code, 0);
    assert!(
        hidden.load(Ordering::SeqCst),
        "native Quit must hide the main window while the event loop remains running"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}
