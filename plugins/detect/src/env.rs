use tauri::{AppHandle, EventTarget, Runtime};
use tauri_plugin_windows::WindowImpl;
use tauri_specta::Event;

use crate::DetectEvent;

pub trait Env: Clone + Send + Sync + 'static {
    fn emit(&self, event: DetectEvent);
    fn is_do_not_disturb(&self) -> bool;
    fn is_detect_enabled(&self) -> bool;
}

pub(crate) struct TauriEnv<R: Runtime> {
    pub(crate) app_handle: AppHandle<R>,
}

impl<R: Runtime> Clone for TauriEnv<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
        }
    }
}

impl<R: Runtime> Env for TauriEnv<R> {
    fn emit(&self, event: DetectEvent) {
        let _ = event.emit_to(
            &self.app_handle,
            EventTarget::AnyLabel {
                label: tauri_plugin_windows::AppWindow::Main.label(),
            },
        );
    }

    fn is_do_not_disturb(&self) -> bool {
        crate::dnd::is_do_not_disturb()
    }

    fn is_detect_enabled(&self) -> bool {
        read_detect_enabled_settings(&self.app_handle).unwrap_or(true)
    }
}

fn read_detect_enabled_settings<R: Runtime>(app_handle: &AppHandle<R>) -> Option<bool> {
    use tauri_plugin_settings::SettingsPluginExt;

    let path = app_handle.settings().settings_path().ok()?;
    let content = std::fs::read_to_string(path.as_str()).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;

    settings
        .get("notification")
        .and_then(|n| n.get("detect"))
        .and_then(|d| d.as_bool())
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Clone)]
    pub struct TestEnv {
        pub events: Arc<std::sync::Mutex<Vec<DetectEvent>>>,
        dnd: Arc<AtomicBool>,
        detect_enabled: Arc<AtomicBool>,
    }

    impl TestEnv {
        pub fn new() -> Self {
            Self {
                events: Arc::new(std::sync::Mutex::new(Vec::new())),
                dnd: Arc::new(AtomicBool::new(false)),
                detect_enabled: Arc::new(AtomicBool::new(true)),
            }
        }

        pub fn set_dnd(&self, value: bool) {
            self.dnd.store(value, Ordering::Relaxed);
        }

        pub fn set_detect_enabled(&self, value: bool) {
            self.detect_enabled.store(value, Ordering::Relaxed);
        }
    }

    impl Env for TestEnv {
        fn emit(&self, event: DetectEvent) {
            self.events.lock().unwrap().push(event);
        }

        fn is_do_not_disturb(&self) -> bool {
            self.dnd.load(Ordering::Relaxed)
        }

        fn is_detect_enabled(&self) -> bool {
            self.detect_enabled.load(Ordering::Relaxed)
        }
    }
}
