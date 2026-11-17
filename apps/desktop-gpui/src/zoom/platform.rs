//! The platform adapter behind UI zoom. Each window tells GPUI it is `1 / factor` of its real
//! size at `factor` times the display scale, and input positions are divided by the factor, so
//! GPUI lays out, rasterizes and hit-tests the zoomed UI at full resolution. Screen geometry
//! (window bounds, displays) stays in real points.
//!
//! Every method forwards to the native platform, including ones with default bodies, because a
//! missing forward silently falls back to GPUI's default. Check new trait methods on GPUI bumps.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use anyhow::Result;
use futures::channel::oneshot;
use gpui::{
    A11yCallbacks, Action, ActivationPolicy, ActivityGuard, AnyWindowHandle, AppLifecyclePhase,
    BackgroundExecutor, Bounds, Capslock, ClipboardItem, ClipboardReadError, Corners, CursorStyle,
    Decorations, DispatchEventResult, ExternalDragPayload, FileDropEvent, ForegroundExecutor,
    GpuSpecs, GraphicalEnvironment, Keymap, Menu, MenuItem, Modifiers, OwnedMenu,
    PathPromptOptions, Pixels, Platform, PlatformAtlas, PlatformDisplay, PlatformGestures,
    PlatformInfo, PlatformInput, PlatformInputHandler, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Point, PowerMode, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, Scene, ScreenCaptureSource, ScrollDelta, Size,
    SystemNotification, SystemNotificationResponse, SystemWindowTab, Task, TextInputConfiguration,
    TextInputStateChange, ThermalState, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowButtonLayout, WindowControlArea, WindowControls, WindowDecorations, WindowInsets,
    WindowParams, WindowVisibility, WindowingRequest, accesskit,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use super::{Factor, MinSizes};

pub struct ZoomPlatform {
    native: Rc<dyn Platform>,
    factor: Factor,
    min_sizes: MinSizes,
}

impl ZoomPlatform {
    pub(super) fn new(native: Rc<dyn Platform>, factor: Factor, min_sizes: MinSizes) -> Self {
        Self {
            native,
            factor,
            min_sizes,
        }
    }
}

struct ZoomWindow {
    native: Box<dyn PlatformWindow>,
    factor: Factor,
}

fn down(point: Point<Pixels>, factor: f32) -> Point<Pixels> {
    point.map(|value| value / factor)
}

fn up(bounds: Bounds<Pixels>, factor: f32) -> Bounds<Pixels> {
    bounds.map(|value| value * factor)
}

/// Native input in GPUI's zoomed coordinates. Pixel scroll deltas shrink too, so content
/// follows the pointer at any zoom, like a zoomed webview.
pub(super) fn unzoom(event: PlatformInput, factor: f32) -> PlatformInput {
    match event {
        PlatformInput::MouseDown(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::MouseDown(event)
        }
        PlatformInput::MouseUp(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::MouseUp(event)
        }
        PlatformInput::MousePressure(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::MousePressure(event)
        }
        PlatformInput::MouseMove(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::MouseMove(event)
        }
        PlatformInput::MouseExited(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::MouseExited(event)
        }
        PlatformInput::Scroll(mut event) => {
            event.position = down(event.position, factor);
            if let ScrollDelta::Pixels(delta) = event.delta {
                event.delta = ScrollDelta::Pixels(down(delta, factor));
            }
            PlatformInput::Scroll(event)
        }
        PlatformInput::Pinch(mut event) => {
            event.position = down(event.position, factor);
            PlatformInput::Pinch(event)
        }
        PlatformInput::LongPress(mut event) => {
            event.start_position = down(event.start_position, factor);
            event.position = down(event.position, factor);
            PlatformInput::LongPress(event)
        }
        PlatformInput::TouchDrag(mut event) => {
            event.start_position = down(event.start_position, factor);
            event.position = down(event.position, factor);
            PlatformInput::TouchDrag(event)
        }
        PlatformInput::BackGesture(mut event) => {
            event.start_position = down(event.start_position, factor);
            PlatformInput::BackGesture(event)
        }
        PlatformInput::Touch(mut event) => {
            event.position = down(event.position, factor);
            event.predicted_position = event.predicted_position.map(|at| down(at, factor));
            PlatformInput::Touch(event)
        }
        PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }) => {
            PlatformInput::FileDrop(FileDropEvent::Entered {
                position: down(position, factor),
                paths,
            })
        }
        PlatformInput::FileDrop(FileDropEvent::Pending { position }) => {
            PlatformInput::FileDrop(FileDropEvent::Pending {
                position: down(position, factor),
            })
        }
        PlatformInput::FileDrop(FileDropEvent::Submit { position }) => {
            PlatformInput::FileDrop(FileDropEvent::Submit {
                position: down(position, factor),
            })
        }
        event => event,
    }
}

/// AccessKit scales every node through the root's transform, so assistive technology sees
/// element bounds in real points.
fn zoom_tree(mut update: accesskit::TreeUpdate, factor: f32) -> accesskit::TreeUpdate {
    let root = update.tree.as_ref().map(|tree| tree.root);
    if factor != 1.
        && let Some((_, node)) = update.nodes.iter_mut().find(|(id, _)| Some(*id) == root)
    {
        node.set_transform(accesskit::Affine::scale(factor.into()));
    }
    update
}

impl Platform for ZoomPlatform {
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        mut options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        let factor = self.factor.get();
        if let Some(base) = options.window_min_size {
            let display = options
                .display_id
                .and_then(|id| {
                    self.native
                        .displays()
                        .into_iter()
                        .find(|display| display.id() == id)
                })
                .or_else(|| self.native.primary_display());
            let minimum = super::min_size(
                base,
                factor,
                display.map(|display| display.visible_bounds().size),
            );
            options.window_min_size = Some(minimum);
            options.bounds.size = options.bounds.size.max(&minimum);
            self.min_sizes.borrow_mut().push((handle, base));
        }
        Ok(Box::new(ZoomWindow {
            native: self.native.open_window(handle, options)?,
            factor: self.factor.clone(),
        }))
    }

    fn platform_info(&self) -> PlatformInfo {
        self.native.platform_info()
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.native.background_executor()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.native.foreground_executor()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.native.text_system()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        self.native.run(on_finish_launching)
    }

    fn quit(&self) {
        self.native.quit()
    }

    fn set_initial_windowing(&self, request: WindowingRequest) {
        self.native.set_initial_windowing(request)
    }

    fn request_windowing(&self, request: WindowingRequest) -> Task<Result<()>> {
        self.native.request_windowing(request)
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<OsString>) {
        self.native.restart(binary_path, arguments)
    }

    fn activate(&self, ignoring_other_apps: bool) {
        self.native.activate(ignoring_other_apps)
    }

    fn set_activation_policy(&self, policy: ActivationPolicy) {
        self.native.set_activation_policy(policy)
    }

    fn hide(&self) {
        self.native.hide()
    }

    fn hide_other_apps(&self) {
        self.native.hide_other_apps()
    }

    fn unhide_other_apps(&self) {
        self.native.unhide_other_apps()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.native.displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.native.primary_display()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.native.on_displays_changed(callback)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.native.active_window()
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.native.window_stack()
    }

    fn is_screen_capture_supported(&self) -> bool {
        self.native.is_screen_capture_supported()
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.native.screen_capture_sources()
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.native.window_appearance()
    }

    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        self.native.set_window_appearance(appearance)
    }

    fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.native.button_layout()
    }

    fn open_url(&self, url: &str) {
        self.native.open_url(url)
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.native.on_open_urls(callback)
    }

    fn register_url_scheme(&self, url: &str) -> Task<Result<()>> {
        self.native.register_url_scheme(url)
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.native.prompt_for_paths(options)
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        self.native.prompt_for_new_path(directory, suggested_name)
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.native.can_select_mixed_files_and_dirs()
    }

    fn reveal_path(&self, path: &Path) {
        self.native.reveal_path(path)
    }

    fn open_with_system(&self, path: &Path) {
        self.native.open_with_system(path)
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.native.on_quit(callback)
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.native.on_reopen(callback)
    }

    fn on_system_sleep(&self, callback: Box<dyn FnMut()>) {
        self.native.on_system_sleep(callback)
    }

    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.native.on_system_wake(callback)
    }

    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.native.on_app_lifecycle(callback)
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.native.on_memory_warning(callback)
    }

    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        self.native.gestures()
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.native.set_menus(menus, keymap)
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.native.get_menus()
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.native.set_dock_menu(menu, keymap)
    }

    fn perform_dock_menu_action(&self, action: usize) {
        self.native.perform_dock_menu_action(action)
    }

    fn add_recent_document(&self, path: &Path) {
        self.native.add_recent_document(path)
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.native.on_app_menu_action(callback)
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.native.on_will_open_app_menu(callback)
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.native.on_validate_app_menu_command(callback)
    }

    fn debug_state(&self) -> serde_json::Value {
        self.native.debug_state()
    }

    fn thermal_state(&self) -> ThermalState {
        self.native.thermal_state()
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.native.on_thermal_state_change(callback)
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        self.native.prevent_idle_sleep(reason)
    }

    fn power_mode(&self) -> PowerMode {
        self.native.power_mode()
    }

    fn on_power_mode_change(&self, callback: Box<dyn FnMut()>) {
        self.native.on_power_mode_change(callback)
    }

    fn set_app_identity(&self, identifier: &str, name: &str) {
        self.native.set_app_identity(identifier, name)
    }

    fn show_system_notification(&self, notification: SystemNotification) {
        self.native.show_system_notification(notification)
    }

    fn dismiss_system_notification(&self, tag: &str) -> Task<()> {
        self.native.dismiss_system_notification(tag)
    }

    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        self.native.on_system_notification_response(callback)
    }

    fn compositor_name(&self) -> &'static str {
        self.native.compositor_name()
    }

    fn graphical_environment(&self) -> Option<GraphicalEnvironment> {
        self.native.graphical_environment()
    }

    fn app_path(&self) -> Result<PathBuf> {
        self.native.app_path()
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.native.path_for_auxiliary_executable(name)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.native.set_cursor_style(style)
    }

    fn hide_cursor_until_mouse_moves(&self) {
        self.native.hide_cursor_until_mouse_moves()
    }

    fn is_cursor_visible(&self) -> bool {
        self.native.is_cursor_visible()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        self.native.should_auto_hide_scrollbars()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.native.read_from_clipboard()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.native.write_to_clipboard(item)
    }

    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        self.native.read_from_clipboard_async()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.native.read_from_primary()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        self.native.write_to_primary(item)
    }

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.native.read_from_find_pasteboard()
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.native.write_to_find_pasteboard(item)
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        self.native.write_credentials(url, username, password)
    }

    #[cfg(target_os = "macos")]
    fn write_credentials_if_not_exists(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<Result<bool>> {
        self.native
            .write_credentials_if_not_exists(url, username, password)
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.native.read_credentials(url)
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.native.delete_credentials(url)
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.native.keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.native.keyboard_mapper()
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.native.on_keyboard_layout_change(callback)
    }
}

impl HasWindowHandle for ZoomWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.native.window_handle()
    }
}

impl HasDisplayHandle for ZoomWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.native.display_handle()
    }
}

impl PlatformWindow for ZoomWindow {
    fn content_size(&self) -> Size<Pixels> {
        let factor = self.factor.get();
        self.native.content_size().map(|length| length / factor)
    }

    fn visual_viewport_bounds(&self) -> Bounds<Pixels> {
        let factor = self.factor.get();
        self.native
            .visual_viewport_bounds()
            .map(|value| value / factor)
    }

    fn resize(&mut self, size: Size<Pixels>) {
        let factor = self.factor.get();
        self.native.resize(size.map(|length| length * factor))
    }

    fn scale_factor(&self) -> f32 {
        self.native.scale_factor() * self.factor.get()
    }

    fn mouse_position(&self) -> Point<Pixels> {
        down(self.native.mouse_position(), self.factor.get())
    }

    fn on_input(&self, mut callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        let factor = self.factor.clone();
        self.native
            .on_input(Box::new(move |event| callback(unzoom(event, factor.get()))))
    }

    fn on_resize(&self, mut callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        let factor = self.factor.clone();
        self.native.on_resize(Box::new(move |size, scale| {
            let factor = factor.get();
            callback(size.map(|length| length / factor), scale * factor)
        }))
    }

    fn show_window_menu(&self, position: Point<Pixels>) {
        let factor = self.factor.get();
        self.native
            .show_window_menu(position.map(|value| value * factor))
    }

    fn set_exclusive_zone(&self, zone: Pixels) {
        self.native.set_exclusive_zone(zone * self.factor.get())
    }

    fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        let factor = self.factor.get();
        let region = region.map(|region| region.iter().map(|b| up(*b, factor)).collect::<Vec<_>>());
        self.native.set_input_region(region.as_deref())
    }

    fn set_client_inset(&self, inset: Pixels) {
        self.native.set_client_inset(inset * self.factor.get())
    }

    fn set_client_corner_radii(&self, radii: Corners<Pixels>) {
        let factor = self.factor.get();
        self.native
            .set_client_corner_radii(radii.map(|radius| *radius * factor))
    }

    fn a11y_init(&self, callbacks: A11yCallbacks) {
        let A11yCallbacks {
            activation,
            action,
            deactivation,
        } = callbacks;
        let factor = self.factor.clone();
        self.native.a11y_init(A11yCallbacks {
            activation: Box::new(move || activation().map(|tree| zoom_tree(tree, factor.get()))),
            action,
            deactivation,
        })
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.native
            .a11y_tree_update(zoom_tree(tree_update, self.factor.get()))
    }

    // Input handlers already answer in real points (`ZoomedInput`).
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.native.update_ime_position(bounds)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.native.bounds()
    }

    fn is_maximized(&self) -> bool {
        self.native.is_maximized()
    }

    fn window_bounds(&self) -> WindowBounds {
        self.native.window_bounds()
    }

    fn on_visual_viewport_changed(&self, callback: Box<dyn FnMut()>) {
        self.native.on_visual_viewport_changed(callback)
    }

    fn prepare_frame(&self) -> bool {
        self.native.prepare_frame()
    }

    fn appearance(&self) -> WindowAppearance {
        self.native.appearance()
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.native.display()
    }

    fn modifiers(&self) -> Modifiers {
        self.native.modifiers()
    }

    fn capslock(&self) -> Capslock {
        self.native.capslock()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.native.set_input_handler(input_handler)
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.native.take_input_handler()
    }

    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        self.native.set_text_input_configuration(configuration)
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        self.native.prompt(level, msg, detail, answers)
    }

    fn activate(&self) {
        self.native.activate()
    }

    fn request_attention(&self) {
        self.native.request_attention()
    }

    fn is_active(&self) -> bool {
        self.native.is_active()
    }

    fn visibility(&self) -> WindowVisibility {
        self.native.visibility()
    }

    fn is_hovered(&self) -> bool {
        self.native.is_hovered()
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.native.background_appearance()
    }

    fn set_title(&mut self, title: &str) {
        self.native.set_title(title)
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.native.set_background_appearance(background_appearance)
    }

    fn minimize(&self) {
        self.native.minimize()
    }

    fn zoom(&self) {
        self.native.zoom()
    }

    fn toggle_fullscreen(&self) {
        self.native.toggle_fullscreen()
    }

    fn is_fullscreen(&self) -> bool {
        self.native.is_fullscreen()
    }

    fn debug_state(&self) -> serde_json::Value {
        self.native.debug_state()
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        self.native.frame_waker()
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.native.on_request_frame(callback)
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.native.on_active_status_change(callback)
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.native.on_visibility_change(callback)
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.native.on_hover_status_change(callback)
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.native.on_moved(callback)
    }

    fn on_display_changed(&self, callback: Box<dyn FnMut()>) {
        self.native.on_display_changed(callback)
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.native.on_should_close(callback)
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.native.on_hit_test_window_control(callback)
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.native.on_close(callback)
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.native.on_appearance_changed(callback)
    }

    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.native.on_button_layout_changed(callback)
    }

    fn draw(&self, scene: &Scene) {
        self.native.draw(scene)
    }

    fn schedule_frame(&self) {
        self.native.schedule_frame()
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.native.sprite_atlas()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.native.is_subpixel_rendering_supported()
    }

    fn get_title(&self) -> String {
        self.native.get_title()
    }

    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.native.tabbed_windows()
    }

    fn tab_bar_visible(&self) -> bool {
        self.native.tab_bar_visible()
    }

    fn set_edited(&mut self, edited: bool) {
        self.native.set_edited(edited)
    }

    fn set_document_path(&self, path: Option<&Path>) {
        self.native.set_document_path(path)
    }

    fn toggle_simple_fullscreen(&self) {
        self.native.toggle_simple_fullscreen()
    }

    fn is_simple_fullscreen(&self) -> bool {
        self.native.is_simple_fullscreen()
    }

    // The traffic lights stay in window points.
    #[cfg(target_os = "macos")]
    fn set_traffic_light_position(&self, position: Point<Pixels>) {
        self.native.set_traffic_light_position(position)
    }

    #[cfg(target_os = "macos")]
    fn set_native_titlebar_visibility(&self, visible: bool) {
        self.native.set_native_titlebar_visibility(visible)
    }

    fn show_character_palette(&self) {
        self.native.show_character_palette()
    }

    fn titlebar_double_click(&self, is_resizable: bool, is_minimizable: bool) {
        self.native
            .titlebar_double_click(is_resizable, is_minimizable)
    }

    fn on_move_tab_to_new_window(&self, callback: Box<dyn FnMut()>) {
        self.native.on_move_tab_to_new_window(callback)
    }

    fn on_merge_all_windows(&self, callback: Box<dyn FnMut()>) {
        self.native.on_merge_all_windows(callback)
    }

    fn on_select_previous_tab(&self, callback: Box<dyn FnMut()>) {
        self.native.on_select_previous_tab(callback)
    }

    fn on_select_next_tab(&self, callback: Box<dyn FnMut()>) {
        self.native.on_select_next_tab(callback)
    }

    fn on_toggle_tab_bar(&self, callback: Box<dyn FnMut()>) {
        self.native.on_toggle_tab_bar(callback)
    }

    fn merge_all_windows(&self) {
        self.native.merge_all_windows()
    }

    fn move_tab_to_new_window(&self) {
        self.native.move_tab_to_new_window()
    }

    fn toggle_window_tab_overview(&self) {
        self.native.toggle_window_tab_overview()
    }

    fn set_tabbing_identifier(&self, identifier: Option<String>) {
        self.native.set_tabbing_identifier(identifier)
    }

    fn native_window_state(&self) -> Option<Vec<u8>> {
        self.native.native_window_state()
    }

    fn restore_native_window_state(&self, state: &[u8]) {
        self.native.restore_native_window_state(state)
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        self.native.inner_window_bounds()
    }

    fn request_decorations(&self, decorations: WindowDecorations) {
        self.native.request_decorations(decorations)
    }

    fn start_window_move(&self) {
        self.native.start_window_move()
    }

    fn can_start_external_drag(&self) -> bool {
        self.native.can_start_external_drag()
    }

    fn start_external_drag(&self, payload: &ExternalDragPayload) -> bool {
        self.native.start_external_drag(payload)
    }

    fn start_window_resize(&self, edge: ResizeEdge) {
        self.native.start_window_resize(edge)
    }

    fn window_decorations(&self) -> Decorations {
        self.native.window_decorations()
    }

    fn set_app_id(&mut self, app_id: &str) {
        self.native.set_app_id(app_id)
    }

    fn map_window(&mut self) -> Result<()> {
        self.native.map_window()
    }

    fn window_controls(&self) -> WindowControls {
        self.native.window_controls()
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.native.gpu_specs()
    }

    // Mobile-only geometry; this app runs on desktop.
    fn insets(&self) -> WindowInsets {
        self.native.insets()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.native.on_insets_changed(callback)
    }

    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.native.set_back_handler(callback)
    }

    fn set_back_enabled(&self, enabled: bool) {
        self.native.set_back_enabled(enabled)
    }

    fn show_soft_keyboard(&self) {
        self.native.show_soft_keyboard()
    }

    fn hide_soft_keyboard(&self) {
        self.native.hide_soft_keyboard()
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.native.text_input_state_changed(change)
    }

    fn play_system_bell(&self) {
        self.native.play_system_bell()
    }

    fn a11y_update_window_bounds(&self) {
        self.native.a11y_update_window_bounds()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, MouseButton, MouseDownEvent, ScrollEvent, TouchPhase, point, px};

    use super::*;

    #[test]
    fn pointer_input_lands_on_the_zoomed_layout() {
        let down = unzoom(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(300.), px(150.)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            1.5,
        );
        let PlatformInput::MouseDown(down) = down else {
            panic!("still a mouse down");
        };
        assert_eq!(down.position, point(px(200.), px(100.)));

        let scroll = |delta| {
            let PlatformInput::Scroll(event) = unzoom(
                PlatformInput::Scroll(ScrollEvent {
                    position: point(px(40.), px(40.)),
                    delta,
                    touch_phase: TouchPhase::Moved,
                    ..Default::default()
                }),
                2.,
            ) else {
                panic!("still a scroll");
            };
            assert_eq!(event.position, point(px(20.), px(20.)));
            event.delta
        };
        let ScrollDelta::Pixels(pixels) = scroll(ScrollDelta::Pixels(point(px(0.), px(-30.))))
        else {
            panic!("still in pixels");
        };
        assert_eq!(pixels, point(px(0.), px(-15.)));
        let ScrollDelta::Lines(lines) = scroll(ScrollDelta::Lines(point(0., -3.))) else {
            panic!("still in lines");
        };
        assert_eq!(lines, point(0., -3.));
    }

    #[test]
    fn assistive_technology_sees_real_points_through_the_root() {
        let root = accesskit::NodeId(0);
        let update = |factor| {
            zoom_tree(
                accesskit::TreeUpdate {
                    nodes: vec![
                        (root, accesskit::Node::new(accesskit::Role::Window)),
                        (
                            accesskit::NodeId(1),
                            accesskit::Node::new(accesskit::Role::Button),
                        ),
                    ],
                    tree: Some(accesskit::Tree::new(root)),
                    tree_id: accesskit::TreeId::ROOT,
                    focus: root,
                },
                factor,
            )
        };
        let zoomed = update(2.);
        assert_eq!(
            zoomed.nodes[0].1.transform(),
            Some(&accesskit::Affine::scale(2.))
        );
        assert_eq!(zoomed.nodes[1].1.transform(), None);
        assert_eq!(update(1.).nodes[0].1.transform(), None);
    }
}
