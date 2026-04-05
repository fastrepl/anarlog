#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri_plugin_transcription::SessionLifecycleEvent;
use tauri_plugin_windows::{AppWindow, VisibilityEvent};
use tauri_specta::Event;

pub fn setup(app_handle: &tauri::AppHandle<tauri::Wry>) {
    let session_active = Arc::new(AtomicBool::new(false));

    setup_session_lifecycle_listener(app_handle, session_active.clone());
    setup_visibility_listener(app_handle, session_active);
}

fn setup_session_lifecycle_listener(
    app_handle: &tauri::AppHandle<tauri::Wry>,
    session_active: Arc<AtomicBool>,
) {
    let handle = app_handle.clone();

    SessionLifecycleEvent::listen_any(app_handle, move |event| match event.payload {
        SessionLifecycleEvent::Active { .. } => {
            session_active.store(true, Ordering::SeqCst);

            let main_visible = AppWindow::Main
                .get(&handle)
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);

            let _ = AppWindow::Control.show(&handle);
            if main_visible {
                let _ = AppWindow::Control.hide(&handle);
            }
        }
        SessionLifecycleEvent::Inactive { .. } => {
            session_active.store(false, Ordering::SeqCst);
            let _ = AppWindow::Control.destroy(&handle);
        }
        SessionLifecycleEvent::Finalizing { .. } => {}
    });
}

fn setup_visibility_listener(
    app_handle: &tauri::AppHandle<tauri::Wry>,
    session_active: Arc<AtomicBool>,
) {
    let handle = app_handle.clone();

    VisibilityEvent::listen_any(app_handle, move |event| {
        if event.payload.window != AppWindow::Main {
            return;
        }

        if !session_active.load(Ordering::SeqCst) {
            return;
        }

        if event.payload.visible {
            let _ = AppWindow::Control.hide(&handle);
        } else {
            let _ = AppWindow::Control.show(&handle);
        }
    });
}
