use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_specta::Event;

use crate::{AppWindow, SavedFrame, WindowImpl, WindowReadyState, events};

#[cfg(target_os = "macos")]
pub(crate) fn run_on_main_thread<R: Send + 'static>(
    app: &AppHandle<tauri::Wry>,
    f: impl FnOnce() -> R + Send + 'static,
) -> Result<R, crate::Error> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })?;

    rx.recv().map_err(|_| crate::Error::MainThreadRecvFailed)
}

impl AppWindow {
    #[cfg(target_os = "macos")]
    fn with_ns_window<R: Send + 'static>(
        &self,
        app: &AppHandle<tauri::Wry>,
        f: impl FnOnce(&objc2_app_kit::NSWindow) -> R + Send + 'static,
    ) -> Result<Option<R>, crate::Error> {
        let Some(window) = self.get(app) else {
            return Ok(None);
        };

        run_on_main_thread(app, move || {
            let Ok(ns_win) = window.ns_window() else {
                return None;
            };

            Some(unsafe { f(&*(ns_win as *mut objc2_app_kit::NSWindow)) })
        })
    }

    fn frame(&self, app: &AppHandle<tauri::Wry>) -> Result<Option<SavedFrame>, crate::Error> {
        #[cfg(target_os = "macos")]
        {
            self.with_ns_window(app, |ns_window| {
                let frame = ns_window.frame();
                SavedFrame {
                    x: frame.origin.x,
                    y: frame.origin.y,
                    w: frame.size.width,
                    h: frame.size.height,
                }
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            Ok(None)
        }
    }

    fn visible_frame(
        &self,
        app: &AppHandle<tauri::Wry>,
    ) -> Result<Option<SavedFrame>, crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use objc2::MainThreadMarker;
            use objc2_app_kit::NSScreen;

            self.with_ns_window(app, |ns_window| {
                let mtm =
                    MainThreadMarker::new().expect("run_on_main_thread guarantees main thread");
                let screen = ns_window.screen().or_else(|| NSScreen::mainScreen(mtm))?;
                let frame = screen.visibleFrame();

                Some(SavedFrame {
                    x: frame.origin.x,
                    y: frame.origin.y,
                    w: frame.size.width,
                    h: frame.size.height,
                })
            })
            .map(|frame| frame.flatten())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            Ok(None)
        }
    }

    fn set_frame_animated(
        &self,
        app: &AppHandle<tauri::Wry>,
        frame: SavedFrame,
    ) -> Result<(), crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            self.with_ns_window(app, move |ns_window| {
                let frame = NSRect::new(
                    NSPoint::new(frame.x, frame.y),
                    NSSize::new(frame.w, frame.h),
                );
                ns_window.setFrame_display_animate(frame, true, true);
            })?;

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            let _ = frame;
            Ok(())
        }
    }

    fn emit_navigate(
        &self,
        app: &AppHandle<tauri::Wry>,
        event: events::Navigate,
    ) -> Result<(), crate::Error> {
        if self.get(app).is_some() {
            events::Navigate::emit_to(&event, app, self.label())?;
        }
        Ok(())
    }

    fn navigate(
        &self,
        app: &AppHandle<tauri::Wry>,
        path: impl AsRef<str>,
    ) -> Result<(), crate::Error> {
        if let Some(window) = self.get(app) {
            let mut url = window.url().unwrap();

            let path_str = path.as_ref();
            if let Some(query_index) = path_str.find('?') {
                let (path_part, query_part) = path_str.split_at(query_index);
                url.set_path(path_part);
                url.set_query(Some(&query_part[1..]));
            } else {
                url.set_path(path_str);
                url.set_query(None);
            }

            window.navigate(url)?;
        }

        Ok(())
    }

    pub fn get(&self, app: &AppHandle<tauri::Wry>) -> Option<WebviewWindow> {
        let label = self.label();
        app.get_webview_window(&label)
    }

    pub fn hide(&self, app: &AppHandle<tauri::Wry>) -> Result<(), crate::Error> {
        if let Some(window) = self.get(app) {
            window.hide()?;
            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: false,
            }
            .emit(app);
        }

        Ok(())
    }

    fn close(&self, app: &AppHandle<tauri::Wry>) -> Result<(), crate::Error> {
        if let Some(window) = self.get(app) {
            window.close()?;
        }

        Ok(())
    }

    pub fn destroy(&self, app: &AppHandle<tauri::Wry>) -> Result<(), crate::Error> {
        if let Some(window) = self.get(app) {
            window.destroy()?;
        }

        Ok(())
    }

    fn prepare_show(&self, app: &AppHandle<tauri::Wry>) {
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

        if matches!(self, Self::Main) {
            use tauri_plugin_analytics::{AnalyticsPayload, AnalyticsPluginExt};

            let e = AnalyticsPayload::builder("show_main_window").build();
            app.analytics().event_fire_and_forget(e);
        }
    }

    fn try_show_existing(
        &self,
        app: &AppHandle<tauri::Wry>,
    ) -> Result<Option<WebviewWindow>, crate::Error> {
        if let Some(window) = self.get(app) {
            window.show()?;
            window.set_focus()?;
            return Ok(Some(window));
        }
        Ok(None)
    }

    fn finalize_show(&self, window: &WebviewWindow) -> Result<(), crate::Error> {
        if let Self::Main = self {
            use tauri_plugin_window_state::{StateFlags, WindowExt};
            let _ = window.restore_state(StateFlags::SIZE);
        }

        window.show()?;
        window.set_focus()?;

        Ok(())
    }

    pub fn show(&self, app: &AppHandle<tauri::Wry>) -> Result<WebviewWindow, crate::Error>
    where
        Self: WindowImpl,
    {
        self.prepare_show(app);

        let window = if let Some(window) = self.try_show_existing(app)? {
            window
        } else {
            let window = self.build_window(app)?;
            std::thread::sleep(std::time::Duration::from_millis(100));
            self.finalize_show(&window)?;
            window
        };

        let _ = events::VisibilityEvent {
            window: self.clone(),
            visible: true,
        }
        .emit(app);

        Ok(window)
    }

    pub async fn show_async(
        &self,
        app: &AppHandle<tauri::Wry>,
    ) -> Result<WebviewWindow, crate::Error>
    where
        Self: WindowImpl,
    {
        self.prepare_show(app);

        let window = if let Some(window) = self.try_show_existing(app)? {
            window
        } else {
            let ready_rx = app
                .try_state::<WindowReadyState>()
                .map(|state| state.register(self.label()));

            let window = self.build_window(app)?;

            if let Some(rx) = ready_rx {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rx).await;
            }

            self.finalize_show(&window)?;
            window
        };

        let _ = events::VisibilityEvent {
            window: self.clone(),
            visible: true,
        }
        .emit(app);

        Ok(window)
    }
}

pub struct Windows<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, M: tauri::Manager<tauri::Wry>> Windows<'a, tauri::Wry, M> {
    pub fn show(&self, window: AppWindow) -> Result<WebviewWindow, crate::Error> {
        window.show(self.manager.app_handle())
    }

    pub async fn show_async(&self, window: AppWindow) -> Result<WebviewWindow, crate::Error> {
        window.show_async(self.manager.app_handle()).await
    }

    pub fn hide(&self, window: AppWindow) -> Result<(), crate::Error> {
        window.hide(self.manager.app_handle())
    }

    pub fn close(&self, window: AppWindow) -> Result<(), crate::Error> {
        window.close(self.manager.app_handle())
    }

    pub fn destroy(&self, window: AppWindow) -> Result<(), crate::Error> {
        window.destroy(self.manager.app_handle())
    }

    pub fn is_focused(&self, window: AppWindow) -> Result<bool, crate::Error> {
        #[cfg(target_os = "macos")]
        {
            let app = self.manager.app_handle().clone();
            let lookup_app = app.clone();
            run_on_main_thread(&app, move || {
                window
                    .get(&lookup_app)
                    .and_then(|w| w.is_focused().ok())
                    .unwrap_or(false)
            })
        }

        #[cfg(not(target_os = "macos"))]
        Ok(window
            .get(self.manager.app_handle())
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false))
    }

    pub fn is_exists(&self, window: AppWindow) -> Result<bool, crate::Error> {
        Ok(window.get(self.manager.app_handle()).is_some())
    }

    pub fn emit_navigate(
        &self,
        window: AppWindow,
        event: events::Navigate,
    ) -> Result<(), crate::Error> {
        window.emit_navigate(self.manager.app_handle(), event)
    }

    pub fn navigate(&self, window: AppWindow, path: impl AsRef<str>) -> Result<(), crate::Error> {
        window.navigate(self.manager.app_handle(), path)
    }

    pub fn frame(&self, window: AppWindow) -> Result<Option<SavedFrame>, crate::Error> {
        window.frame(self.manager.app_handle())
    }

    pub fn visible_frame(&self, window: AppWindow) -> Result<Option<SavedFrame>, crate::Error> {
        window.visible_frame(self.manager.app_handle())
    }

    pub fn set_frame_animated(
        &self,
        window: AppWindow,
        frame: SavedFrame,
    ) -> Result<(), crate::Error> {
        window.set_frame_animated(self.manager.app_handle(), frame)
    }

    pub fn close_all(&self) -> Result<(), crate::Error> {
        for (_, window) in self.manager.webview_windows() {
            let _ = window.close();
        }
        Ok(())
    }
}

pub trait WindowsPluginExt<R: tauri::Runtime> {
    fn windows(&self) -> Windows<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<T: tauri::Manager<tauri::Wry>> WindowsPluginExt<tauri::Wry> for T {
    fn windows(&self) -> Windows<'_, tauri::Wry, Self>
    where
        Self: Sized,
    {
        Windows {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
