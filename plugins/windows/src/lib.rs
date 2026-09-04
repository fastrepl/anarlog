mod commands;
mod errors;
mod events;
mod ext;
mod tab;
mod window;

pub use errors::*;
pub use events::*;
pub use ext::{Windows, WindowsPluginExt};
pub use tab::*;
pub use window::*;

use std::collections::{HashMap, HashSet};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const PLUGIN_NAME: &str = "windows";

pub fn persisted_window_state_flags() -> tauri_plugin_window_state::StateFlags {
    use tauri_plugin_window_state::StateFlags;
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

#[derive(Clone, Copy)]
pub struct SavedFrame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Default)]
pub struct SavedFrames(pub Mutex<HashMap<String, SavedFrame>>);

impl SavedFrames {
    fn take(&self, label: &str) -> Option<SavedFrame> {
        self.0.lock().unwrap().remove(label)
    }

    fn remove(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }
}

#[derive(Default)]
pub struct WindowExpansions(pub Mutex<HashMap<String, Vec<(f64, f64, bool)>>>);

impl WindowExpansions {
    fn pop(&self, label: &str) -> Option<(f64, f64, bool)> {
        let mut expansions = self.0.lock().unwrap();
        let entry = expansions.get_mut(label).and_then(Vec::pop);

        if expansions.get(label).is_some_and(Vec::is_empty) {
            expansions.remove(label);
        }

        entry
    }

    fn remove(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }
}

pub struct DockVisibilityState(AtomicBool);

impl Default for DockVisibilityState {
    fn default() -> Self {
        Self(AtomicBool::new(true))
    }
}

impl DockVisibilityState {
    pub fn set_show_app_in_dock(&self, value: bool) {
        self.0.store(value, Ordering::SeqCst);
    }

    pub fn show_app_in_dock(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

use tauri::Manager;
use tokio::sync::oneshot;

#[derive(Default)]
pub struct WindowReadyState {
    // Using tokio::sync::oneshot instead of std::sync::mpsc because:
    // std::mpsc::Receiver::recv_timeout blocks the thread, which can deadlock
    // when on_webview_ready callback needs the blocked thread to signal.
    // tokio::oneshot allows async await with timeout that yields instead of blocking.
    next_registration_id: AtomicU64,
    pending: Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>,
}

#[derive(Default)]
struct WebviewHealthState {
    next_registration_id: AtomicU64,
    pending: Mutex<HashMap<String, (u64, String, oneshot::Sender<()>)>>,
    recovering: Mutex<HashSet<String>>,
}

impl WebviewHealthState {
    fn register(&self, label: String) -> Option<(u64, String, oneshot::Receiver<()>)> {
        let recovering = self.recovering.lock().unwrap();
        if recovering.contains(&label) {
            return None;
        }

        let mut pending = self.pending.lock().unwrap();
        if pending.contains_key(&label) {
            return None;
        }

        let (tx, rx) = oneshot::channel();
        let registration_id = self.next_registration_id.fetch_add(1, Ordering::Relaxed);
        let request_id = uuid::Uuid::new_v4().to_string();
        pending.insert(label, (registration_id, request_id.clone(), tx));
        drop(recovering);

        Some((registration_id, request_id, rx))
    }

    fn acknowledge(&self, label: &str, request_id: &str) -> bool {
        let mut pending = self.pending.lock().unwrap();
        let is_match = pending
            .get(label)
            .is_some_and(|(_, current_request_id, _)| current_request_id == request_id);

        if !is_match {
            return false;
        }

        if let Some((_, _, tx)) = pending.remove(label) {
            let _ = tx.send(());
        }
        true
    }

    fn unregister(&self, label: &str, registration_id: u64) -> bool {
        let mut pending = self.pending.lock().unwrap();
        let is_match = pending
            .get(label)
            .is_some_and(|(current_id, _, _)| *current_id == registration_id);

        if is_match {
            pending.remove(label);
        }
        is_match
    }

    fn begin_recovery(&self, label: &str) -> bool {
        let mut recovering = self.recovering.lock().unwrap();
        if !recovering.insert(label.to_string()) {
            return false;
        }
        self.pending.lock().unwrap().remove(label);
        true
    }

    fn ready(&self, label: &str) {
        self.recovering.lock().unwrap().remove(label);
    }

    fn remove(&self, label: &str) {
        self.recovering.lock().unwrap().remove(label);
        self.pending.lock().unwrap().remove(label);
    }
}

impl WindowReadyState {
    pub fn register(&self, label: String) -> (u64, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        let registration_id = self.next_registration_id.fetch_add(1, Ordering::Relaxed);
        self.pending
            .lock()
            .unwrap()
            .insert(label, (registration_id, tx));
        (registration_id, rx)
    }

    pub fn unregister(&self, label: &str, registration_id: u64) {
        let mut pending = self.pending.lock().unwrap();
        if pending
            .get(label)
            .is_some_and(|(current_id, _)| *current_id == registration_id)
        {
            pending.remove(label);
        }
    }

    fn remove(&self, label: &str) {
        self.pending.lock().unwrap().remove(label);
    }

    pub fn signal(&self, label: &str) {
        if let Some((_, tx)) = self.pending.lock().unwrap().remove(label) {
            let _ = tx.send(());
        }
    }
}

pub(crate) fn clear_window_state(app: &tauri::AppHandle<tauri::Wry>, label: &str) {
    if let Some(state) = app.try_state::<WindowReadyState>() {
        state.remove(label);
    }
    if let Some(state) = app.try_state::<WebviewHealthState>() {
        state.remove(label);
    }
    if let Some(state) = app.try_state::<SavedFrames>() {
        state.remove(label);
    }
    if let Some(state) = app.try_state::<WindowExpansions>() {
        state.remove(label);
    }
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![
            events::Navigate,
            events::WindowDestroyed,
            events::OpenTab,
            events::VisibilityEvent,
            events::WebviewHealthCheck,
            events::FloatingBarStop,
            events::FloatingBarOpenMain,
            events::FloatingBarOverlayState,
            events::FloatingBarOverlayAmplitude,
            events::LiveCaptionOverlayState,
            events::FloatingBarSettingsChange,
        ])
        .commands(tauri_specta::collect_commands![
            commands::window_show,
            commands::window_hide,
            commands::window_destroy,
            commands::window_navigate,
            commands::window_emit_navigate,
            commands::window_is_exists,
            commands::window_is_occluded,
            commands::webview_health_ack,
            commands::webview_health_ready,
            commands::window_set_frame_animated,
            commands::window_save_frame,
            commands::window_restore_frame_animated,
            commands::window_expand_width,
            commands::window_restore_width,
            commands::set_show_app_in_dock,
            commands::floating_bar_show,
            commands::floating_bar_hide,
            commands::floating_bar_update,
            commands::floating_bar_update_amplitude,
            commands::floating_bar_current_state,
            commands::live_caption_show,
            commands::live_caption_hide,
            commands::live_caption_update,
            commands::live_caption_current_state,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            crate::window::floating_bar::set_app_handle(app.clone());
            crate::window::live_caption::set_app_handle(app.clone());

            {
                let ready_state = WindowReadyState::default();
                app.manage(ready_state);
            }

            {
                let health_state = WebviewHealthState::default();
                app.manage(health_state);
            }

            {
                let dock_visibility_state = DockVisibilityState::default();
                app.manage(dock_visibility_state);
            }

            {
                let saved_frames = SavedFrames::default();
                app.manage(saved_frames);
            }

            {
                let window_expansions = WindowExpansions::default();
                app.manage(window_expansions);
            }

            Ok(())
        })
        .on_webview_ready(|webview| {
            if let Some(state) = webview.app_handle().try_state::<WindowReadyState>() {
                state.signal(webview.label());
            }
        })
        .on_event(move |app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                use tauri_plugin_window_state::AppHandleExt;
                let _ = app.save_window_state(persisted_window_state_flags());
            }
        })
        .build()
}

pub fn extend_builder(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(AppWindow::recover_terminated_webview);

    #[cfg(all(target_os = "macos", feature = "macos-private-api"))]
    {
        builder.plugin(tauri_nspanel::init())
    }

    #[cfg(any(
        not(target_os = "macos"),
        all(target_os = "macos", not(feature = "macos-private-api"))
    ))]
    {
        builder
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn unregister_only_removes_matching_window_ready_registration() {
        let state = WindowReadyState::default();
        let (first_id, first_rx) = state.register("note-1".into());
        let (second_id, second_rx) = state.register("note-1".into());

        assert!(first_rx.await.is_err());
        state.unregister("note-1", first_id);
        state.signal("note-1");

        assert!(second_rx.await.is_ok());
        state.unregister("note-1", second_id);
        assert!(state.pending.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn webview_health_acknowledges_only_the_current_request() {
        let state = WebviewHealthState::default();
        let (first_id, first_request_id, first_rx) = state.register("main".into()).unwrap();

        assert!(!state.acknowledge("main", "stale-request"));
        assert!(!state.unregister("main", first_id + 1));
        assert!(state.acknowledge("main", &first_request_id));
        assert!(first_rx.await.is_ok());
        assert!(!state.unregister("main", first_id));
        assert!(state.pending.lock().unwrap().is_empty());
    }

    #[test]
    fn webview_health_allows_only_one_probe_per_window() {
        let state = WebviewHealthState::default();
        let (first_id, _, _) = state.register("main".into()).unwrap();

        assert!(state.register("main".into()).is_none());
        assert!(state.unregister("main", first_id));
        assert!(state.register("main".into()).is_some());
    }

    #[test]
    fn webview_health_recovery_starts_once_and_blocks_probes() {
        let state = WebviewHealthState::default();
        assert!(state.begin_recovery("main"));

        assert!(state.register("main".into()).is_none());
        assert!(!state.begin_recovery("main"));
        state.ready("main");
        assert!(state.register("main".into()).is_some());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn terminated_webview_restarts_only_for_visible_main_window() {
        assert!(crate::ext::should_restart_terminated_webview("main", true));
        assert!(!crate::ext::should_restart_terminated_webview(
            "main", false
        ));
        assert!(!crate::ext::should_restart_terminated_webview(
            "composer", true
        ));
        assert!(!crate::ext::should_restart_terminated_webview(
            "note-1", true
        ));
    }

    #[test]
    fn expansion_pop_removes_empty_window_entry() {
        let expansions = WindowExpansions::default();
        expansions
            .0
            .lock()
            .unwrap()
            .insert("note-1".into(), vec![(100.0, 120.0, false)]);

        assert_eq!(expansions.pop("note-1"), Some((100.0, 120.0, false)));
        assert!(expansions.0.lock().unwrap().is_empty());
    }

    #[test]
    fn persisted_window_state_includes_size_and_position() {
        use tauri_plugin_window_state::StateFlags;

        let flags = persisted_window_state_flags();
        assert!(flags.contains(StateFlags::SIZE));
        assert!(flags.contains(StateFlags::POSITION));
        assert!(flags.contains(StateFlags::MAXIMIZED));
        assert!(!flags.contains(StateFlags::VISIBLE));
        assert!(!flags.contains(StateFlags::DECORATIONS));
        assert!(!flags.contains(StateFlags::FULLSCREEN));
    }

    #[test]
    fn saved_frame_take_consumes_window_entry() {
        let frames = SavedFrames::default();
        frames.0.lock().unwrap().insert(
            "note-1".into(),
            SavedFrame {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            },
        );

        let frame = frames.take("note-1").unwrap();
        assert_eq!((frame.x, frame.y, frame.w, frame.h), (1.0, 2.0, 3.0, 4.0));
        assert!(frames.0.lock().unwrap().is_empty());
    }

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    #[test]
    fn test_version() {
        let version = tauri_plugin_os::version()
            .to_string()
            .split('.')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        println!("version: {}", version);
    }
}
