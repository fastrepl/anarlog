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

    fn is_occluded(&self, app: &AppHandle<tauri::Wry>) -> Result<bool, crate::Error> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSWindowOcclusionState;

            let Some((window_id, appkit_occluded, on_active_space)) =
                self.with_ns_window(app, |ns_window| {
                    let window_number = ns_window.windowNumber();
                    let window_id = u32::try_from(window_number).ok();
                    let appkit_occluded = !ns_window
                        .occlusionState()
                        .contains(NSWindowOcclusionState::Visible);

                    (window_id, appkit_occluded, ns_window.isOnActiveSpace())
                })?
            else {
                return Ok(false);
            };

            if appkit_occluded || !on_active_space {
                return Ok(true);
            }

            Ok(window_id
                .and_then(|id| is_window_mostly_covered(id, i64::from(std::process::id())))
                .unwrap_or(false))
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            Ok(false)
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
        if matches!(self, Self::Composer) {
            crate::window::composer::hide(app)?;
            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: false,
            }
            .emit(app);
            return Ok(());
        }

        if matches!(self, Self::Floating) {
            crate::window::floating::hide(app)?;
            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: false,
            }
            .emit(app);
            return Ok(());
        }

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
        if matches!(self, Self::Main) {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        }

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
        if matches!(self, Self::Composer) {
            return if self.get(app).is_some() {
                crate::window::composer::show(app).map(Some)
            } else {
                Ok(None)
            };
        }

        if matches!(self, Self::Floating) {
            return if self.get(app).is_some() {
                crate::window::floating::show(app).map(Some)
            } else {
                Ok(None)
            };
        }

        if let Some(window) = self.get(app) {
            window.show()?;
            window.set_focus()?;
            return Ok(Some(window));
        }
        Ok(None)
    }

    fn finalize_show(&self, window: &WebviewWindow) -> Result<(), crate::Error> {
        use tauri_plugin_window_state::{StateFlags, WindowExt};

        let _ = self;
        if !matches!(self, Self::Floating) {
            let _ = window.restore_state(StateFlags::SIZE);
        }

        window.show()?;
        if !matches!(self, Self::Floating) {
            window.set_focus()?;
        }

        Ok(())
    }

    pub fn show(&self, app: &AppHandle<tauri::Wry>) -> Result<WebviewWindow, crate::Error>
    where
        Self: WindowImpl,
    {
        self.prepare_show(app);

        if matches!(self, Self::Composer) {
            let window = crate::window::composer::show(app)?;

            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: true,
            }
            .emit(app);

            return Ok(window);
        }

        if matches!(self, Self::Floating) {
            let window = crate::window::floating::show(app)?;

            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: true,
            }
            .emit(app);

            return Ok(window);
        }

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

        if matches!(self, Self::Composer) {
            let window = crate::window::composer::show(app)?;

            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: true,
            }
            .emit(app);

            return Ok(window);
        }

        if matches!(self, Self::Floating) {
            let ready_rx = if self.get(app).is_none() {
                app.try_state::<WindowReadyState>()
                    .map(|state| state.register(self.label()))
            } else {
                None
            };
            let window = crate::window::floating::show(app)?;

            if let Some(rx) = ready_rx {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rx).await;
            }

            let _ = events::VisibilityEvent {
                window: self.clone(),
                visible: true,
            }
            .emit(app);

            return Ok(window);
        }

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

#[cfg(target_os = "macos")]
const MIN_VISIBLE_AREA_RATIO: f64 = 0.08;

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct WindowRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[cfg(target_os = "macos")]
impl WindowRect {
    fn from_cg(rect: objc2_core_foundation::CGRect) -> Option<Self> {
        let rect = Self {
            x: rect.origin.x,
            y: rect.origin.y,
            w: rect.size.width,
            h: rect.size.height,
        };

        if rect.area() > 0.0 { Some(rect) } else { None }
    }

    fn area(self) -> f64 {
        self.w.max(0.0) * self.h.max(0.0)
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);

        if x2 <= x1 || y2 <= y1 {
            return None;
        }

        Some(Self {
            x: x1,
            y: y1,
            w: x2 - x1,
            h: y2 - y1,
        })
    }

    fn subtract(self, cover: Self) -> Vec<Self> {
        let Some(overlap) = self.intersection(cover) else {
            return vec![self];
        };

        let mut remaining = Vec::with_capacity(4);
        let self_right = self.x + self.w;
        let self_bottom = self.y + self.h;
        let overlap_right = overlap.x + overlap.w;
        let overlap_bottom = overlap.y + overlap.h;

        if overlap.y > self.y {
            remaining.push(Self {
                x: self.x,
                y: self.y,
                w: self.w,
                h: overlap.y - self.y,
            });
        }

        if overlap_bottom < self_bottom {
            remaining.push(Self {
                x: self.x,
                y: overlap_bottom,
                w: self.w,
                h: self_bottom - overlap_bottom,
            });
        }

        if overlap.x > self.x {
            remaining.push(Self {
                x: self.x,
                y: overlap.y,
                w: overlap.x - self.x,
                h: overlap.h,
            });
        }

        if overlap_right < self_right {
            remaining.push(Self {
                x: overlap_right,
                y: overlap.y,
                w: self_right - overlap_right,
                h: overlap.h,
            });
        }

        remaining
            .into_iter()
            .filter(|rect| rect.area() > 0.0)
            .collect()
    }
}

#[cfg(target_os = "macos")]
fn is_window_mostly_covered(window_id: u32, app_pid: i64) -> Option<bool> {
    use objc2_core_graphics::{
        CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowAlpha, kCGWindowBounds,
        kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
    };

    let target = cg_window_bounds(window_id)?;
    let target_area = target.area();
    if target_area <= 0.0 {
        return Some(false);
    }

    let window_list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenAboveWindow | CGWindowListOption::ExcludeDesktopElements,
        window_id,
    )?;
    let mut visible_regions = vec![target];

    for index in 0..window_list.count() {
        let Some(info) = cf_array_dictionary_at(&window_list, index) else {
            continue;
        };

        if cf_dictionary_i64(info, unsafe { kCGWindowLayer }).unwrap_or(0) != 0 {
            continue;
        }

        if cf_dictionary_i64(info, unsafe { kCGWindowOwnerPID }).is_some_and(|pid| pid == app_pid) {
            continue;
        }

        if cf_dictionary_i64(info, unsafe { kCGWindowNumber })
            .is_some_and(|id| id == i64::from(window_id))
        {
            continue;
        }

        if cf_dictionary_f64(info, unsafe { kCGWindowAlpha }).unwrap_or(1.0) <= 0.05 {
            continue;
        }

        let Some(bounds) = cf_dictionary_rect(info, unsafe { kCGWindowBounds }) else {
            continue;
        };

        visible_regions = visible_regions
            .into_iter()
            .flat_map(|region| region.subtract(bounds))
            .collect();

        let visible_area: f64 = visible_regions.iter().map(|rect| rect.area()).sum();
        if visible_area / target_area <= MIN_VISIBLE_AREA_RATIO {
            return Some(true);
        }
    }

    Some(false)
}

#[cfg(target_os = "macos")]
fn cg_window_bounds(window_id: u32) -> Option<WindowRect> {
    use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowBounds};

    let window_list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionIncludingWindow | CGWindowListOption::ExcludeDesktopElements,
        window_id,
    )?;
    let info = cf_array_dictionary_at(&window_list, 0)?;

    cf_dictionary_rect(info, unsafe { kCGWindowBounds })
}

#[cfg(target_os = "macos")]
fn cf_array_dictionary_at(
    array: &objc2_core_foundation::CFArray,
    index: objc2_core_foundation::CFIndex,
) -> Option<&objc2_core_foundation::CFDictionary> {
    if index < 0 || index >= array.count() {
        return None;
    }

    let value = unsafe { array.value_at_index(index) };
    if value.is_null() {
        return None;
    }

    Some(unsafe { &*(value.cast::<objc2_core_foundation::CFDictionary>()) })
}

#[cfg(target_os = "macos")]
fn cf_dictionary_value(
    dictionary: &objc2_core_foundation::CFDictionary,
    key: &objc2_core_foundation::CFString,
) -> Option<*const std::ffi::c_void> {
    let value = unsafe { dictionary.value((key as *const objc2_core_foundation::CFString).cast()) };

    if value.is_null() { None } else { Some(value) }
}

#[cfg(target_os = "macos")]
fn cf_dictionary_i64(
    dictionary: &objc2_core_foundation::CFDictionary,
    key: &objc2_core_foundation::CFString,
) -> Option<i64> {
    let value = cf_dictionary_value(dictionary, key)?;
    let number = unsafe { &*(value.cast::<objc2_core_foundation::CFNumber>()) };
    let mut output = 0_i64;

    unsafe {
        number
            .value(
                objc2_core_foundation::CFNumberType::LongLongType,
                (&mut output as *mut i64).cast(),
            )
            .then_some(output)
    }
}

#[cfg(target_os = "macos")]
fn cf_dictionary_f64(
    dictionary: &objc2_core_foundation::CFDictionary,
    key: &objc2_core_foundation::CFString,
) -> Option<f64> {
    let value = cf_dictionary_value(dictionary, key)?;
    let number = unsafe { &*(value.cast::<objc2_core_foundation::CFNumber>()) };
    let mut output = 0_f64;

    unsafe {
        number
            .value(
                objc2_core_foundation::CFNumberType::DoubleType,
                (&mut output as *mut f64).cast(),
            )
            .then_some(output)
    }
}

#[cfg(target_os = "macos")]
fn cf_dictionary_rect(
    dictionary: &objc2_core_foundation::CFDictionary,
    key: &objc2_core_foundation::CFString,
) -> Option<WindowRect> {
    let value = cf_dictionary_value(dictionary, key)?;
    let bounds = unsafe { &*(value.cast::<objc2_core_foundation::CFDictionary>()) };
    let mut rect = objc2_core_foundation::CGRect::ZERO;
    let ok = unsafe {
        objc2_core_graphics::CGRectMakeWithDictionaryRepresentation(Some(bounds), &mut rect)
    };

    ok.then_some(rect).and_then(WindowRect::from_cg)
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

    pub fn is_occluded(&self, window: AppWindow) -> Result<bool, crate::Error> {
        window.is_occluded(self.manager.app_handle())
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
