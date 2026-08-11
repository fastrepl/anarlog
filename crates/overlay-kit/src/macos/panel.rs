use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSPanel, NSStatusWindowLevel, NSView, NSWindow,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSRect};

/// Window-level policy applied to an overlay panel.
///
/// Defaults match the small floating chrome (status-window level with a drop
/// shadow); full-screen overlays that don't want a shadow can override it.
#[derive(Debug, Clone, Copy)]
pub struct OverlayPanelConfig {
    pub level: NSWindowLevel,
    pub has_shadow: bool,
}

impl Default for OverlayPanelConfig {
    fn default() -> Self {
        Self {
            level: NSStatusWindowLevel,
            has_shadow: true,
        }
    }
}

pub fn create_nonactivating_panel(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSPanel> {
    create_nonactivating_panel_with(mtm, frame, OverlayPanelConfig::default())
}

pub fn create_nonactivating_panel_with_content(
    mtm: MainThreadMarker,
    frame: NSRect,
    content: &NSView,
) -> Retained<NSPanel> {
    let panel = create_nonactivating_panel(mtm, frame);
    set_panel_content_view(&panel, content);
    panel
}

pub fn create_nonactivating_panel_with(
    mtm: MainThreadMarker,
    frame: NSRect,
    config: OverlayPanelConfig,
) -> Retained<NSPanel> {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        frame,
        style,
        NSBackingStoreType::Buffered,
        false,
    );

    configure_overlay_panel(&panel, config);
    panel
}

pub fn configure_overlay_panel(panel: &NSPanel, config: OverlayPanelConfig) {
    let window: &NSWindow = panel.as_ref();
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setLevel(config.level);
    window.setHasShadow(config.has_shadow);
    window.setHidesOnDeactivate(false);
    window.setAnimationBehavior(NSWindowAnimationBehavior::None);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
}

/// Desired ordering of an overlay panel relative to the window list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayPanelPresentation {
    Visible,
    Hidden,
}

/// Order the panel in or out without activating the host application, making
/// the panel key, or resetting its content. Idempotent for repeated calls and
/// valid for both keyable and nonactivating panels.
pub fn reconcile_panel_presentation(panel: &NSPanel, presentation: OverlayPanelPresentation) {
    let window: &NSWindow = panel.as_ref();
    match presentation {
        OverlayPanelPresentation::Visible => window.orderFrontRegardless(),
        OverlayPanelPresentation::Hidden => window.orderOut(None),
    }
}

pub fn set_panel_content_view(panel: &NSPanel, view: &NSView) {
    let window: &NSWindow = panel.as_ref();
    window.setContentView(Some(view));
}
