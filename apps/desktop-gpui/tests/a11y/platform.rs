//! A windowless GPUI platform whose windows switch AccessKit on as if a screen reader had
//! connected, and keep the tree updates GPUI sends to the OS adapter. GPUI's own test
//! platform drops the accessibility callbacks, so the tree can't be built there. Nothing here
//! reaches the OS: no window, dialog, URL, clipboard, keychain or notification.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use futures::channel::oneshot;
use gpui::{
    A11yCallbacks, ActivityGuard, AnyWindowHandle, BackgroundExecutor, Bounds, Capslock,
    ClipboardItem, CursorStyle, DispatchEventResult, DisplayId, DummyKeyboardMapper,
    ForegroundExecutor, GpuSpecs, Keymap, Menu, MenuItem, Modifiers, NoopTextSystem,
    PathPromptOptions, Pixels, Platform, PlatformAtlas, PlatformDisplay, PlatformInput,
    PlatformInputHandler, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformWindow, Point, PromptButton, PromptLevel, RequestFrameOptions, Scene, Size, Task,
    TestDispatcher, ThermalState, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowParams, WindowVisibility, accesskit, point, px,
};

#[derive(Default)]
pub struct WindowState {
    pub tree: Option<accesskit::TreeUpdate>,
    pub callbacks: Option<A11yCallbacks>,
}

pub struct A11yPlatform {
    pub dispatcher: TestDispatcher,
    background: BackgroundExecutor,
    foreground: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    pub windows: RefCell<Vec<(AnyWindowHandle, Rc<RefCell<WindowState>>)>>,
    clipboard: RefCell<Option<ClipboardItem>>,
    pub opened_urls: RefCell<Vec<String>>,
    /// The app asked to quit.
    pub quit: std::cell::Cell<bool>,
}

impl A11yPlatform {
    pub fn new() -> Rc<Self> {
        let dispatcher = TestDispatcher::new(0);
        let shared = Arc::new(dispatcher.clone());
        Rc::new(Self {
            background: BackgroundExecutor::new(shared.clone()),
            foreground: ForegroundExecutor::new(shared),
            dispatcher,
            text_system: Arc::new(NoopTextSystem::new()),
            windows: RefCell::default(),
            clipboard: RefCell::default(),
            opened_urls: RefCell::default(),
            quit: Default::default(),
        })
    }

    pub fn window(&self, handle: AnyWindowHandle) -> Rc<RefCell<WindowState>> {
        self.windows
            .borrow()
            .iter()
            .find(|(window, _)| *window == handle)
            .map(|(_, state)| state.clone())
            .expect("window opened on this platform")
    }
}

#[derive(Debug)]
struct Display;

impl PlatformDisplay for Display {
    fn id(&self) -> DisplayId {
        DisplayId::new(1)
    }

    fn uuid(&self) -> anyhow::Result<uuid::Uuid> {
        Ok(uuid::Uuid::nil())
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(point(px(0.), px(0.)), gpui::size(px(1920.), px(1080.)))
    }

    fn refresh_interval(&self) -> Option<Duration> {
        Some(Duration::from_secs(1) / 60)
    }
}

struct Layout;

impl PlatformKeyboardLayout for Layout {
    fn id(&self) -> &str {
        "us"
    }

    fn name(&self) -> &str {
        "US"
    }
}

impl Platform for A11yPlatform {
    fn platform_info(&self) -> gpui::PlatformInfo {
        gpui::PlatformInfo {
            os: if cfg!(target_os = "macos") {
                gpui::OperatingSystem::Mac
            } else if cfg!(target_os = "windows") {
                gpui::OperatingSystem::Windows
            } else {
                gpui::OperatingSystem::Linux
            },
            target: gpui::PlatformTarget::Native,
        }
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.background.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
    }

    fn quit(&self) {
        self.quit.set(true);
    }

    fn restart(&self, _: Option<PathBuf>, _: Vec<std::ffi::OsString>) {}

    fn activate(&self, _: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![Rc::new(Display)]
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(Display))
    }

    fn on_displays_changed(&self, _: Box<dyn FnMut()>) {}

    fn active_window(&self) -> Option<AnyWindowHandle> {
        None
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let state = Rc::new(RefCell::new(WindowState::default()));
        self.windows.borrow_mut().push((handle, state.clone()));
        Ok(Box::new(Window {
            bounds: options.bounds,
            state,
            input_handler: None,
            atlas: Arc::new(gpui::HeadlessAtlas::default()),
        }))
    }

    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn open_url(&self, url: &str) {
        self.opened_urls.borrow_mut().push(url.to_string());
    }

    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}

    fn register_url_scheme(&self, _: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _: PathPromptOptions,
    ) -> oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }

    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, _: &Path) {}

    fn open_with_system(&self, _: &Path) {}

    fn on_quit(&self, _: Box<dyn FnMut() -> bool>) {}

    fn on_reopen(&self, _: Box<dyn FnMut()>) {}

    fn on_system_sleep(&self, _: Box<dyn FnMut()>) {}

    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}

    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}

    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}

    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn gpui::Action)>) {}

    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}

    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn gpui::Action) -> bool>) {}

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}

    fn prevent_idle_sleep(&self, _: &str) -> Task<anyhow::Result<ActivityGuard>> {
        Task::ready(Ok(ActivityGuard::new(|| {})))
    }

    fn app_path(&self) -> anyhow::Result<PathBuf> {
        anyhow::bail!("no app path in tests")
    }

    fn path_for_auxiliary_executable(&self, _: &str) -> anyhow::Result<PathBuf> {
        anyhow::bail!("no auxiliary executables in tests")
    }

    fn set_cursor_style(&self, _: CursorStyle) {}

    fn hide_cursor_until_mouse_moves(&self) {}

    fn is_cursor_visible(&self) -> bool {
        true
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        false
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.clipboard.borrow().clone()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        *self.clipboard.borrow_mut() = Some(item);
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {}

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        None
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _: ClipboardItem) {}

    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow::anyhow!("no credentials in tests")))
    }

    fn read_credentials(&self, _: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(Layout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}

struct Window {
    bounds: Bounds<Pixels>,
    state: Rc<RefCell<WindowState>>,
    input_handler: Option<PlatformInputHandler>,
    atlas: Arc<dyn PlatformAtlas>,
}

impl raw_window_handle::HasWindowHandle for Window {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl raw_window_handle::HasDisplayHandle for Window {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl PlatformWindow for Window {
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds)
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.size
    }

    fn resize(&mut self, size: Size<Pixels>) {
        self.bounds.size = size;
    }

    fn scale_factor(&self) -> f32 {
        1.
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(Display))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.take()
    }

    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }

    fn activate(&self) {}

    fn is_active(&self) -> bool {
        true
    }

    fn visibility(&self) -> WindowVisibility {
        WindowVisibility::Visible
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, _: &str) {}

    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}

    fn minimize(&self) {}

    fn zoom(&self) {}

    fn toggle_fullscreen(&self) {}

    fn is_fullscreen(&self) -> bool {
        false
    }

    fn on_request_frame(&self, _: Box<dyn FnMut(RequestFrameOptions)>) {}

    fn on_input(&self, _: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {}

    fn on_active_status_change(&self, _: Box<dyn FnMut(bool)>) {}

    fn on_visibility_change(&self, _: Box<dyn FnMut(WindowVisibility)>) {}

    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}

    fn on_resize(&self, _: Box<dyn FnMut(Size<Pixels>, f32)>) {}

    fn on_moved(&self, _: Box<dyn FnMut()>) {}

    fn on_display_changed(&self, _: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}

    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}

    fn on_close(&self, _: Box<dyn FnOnce()>) {}

    fn on_appearance_changed(&self, _: Box<dyn FnMut()>) {}

    fn draw(&self, _: &Scene) {}

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        unimplemented!()
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn update_ime_position(&self, _: Bounds<Pixels>) {}

    /// Connects a screen reader at once: GPUI builds the tree from the next frame on.
    fn a11y_init(&self, callbacks: A11yCallbacks) {
        (callbacks.activation)();
        self.state.borrow_mut().callbacks = Some(callbacks);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.state.borrow_mut().tree = Some(tree_update);
    }
}
