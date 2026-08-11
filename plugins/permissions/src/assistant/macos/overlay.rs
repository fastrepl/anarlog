use objc2::rc::Retained;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSFont, NSLineBreakMode, NSPanel, NSTextField, NSView,
};
use objc2_foundation::{MainThreadMarker, NSString};

use anlg_overlay_kit::macos::layout::{LayoutHostView, LayoutSubview};
use anlg_overlay_kit::macos::panel::create_nonactivating_panel_with_content;

use super::host_app::HostApp;
use super::layout;
use super::session::AssistedPane;
use super::views::{
    ContentView, DismissButtonView, DragSourceView, GuideCursorView, SketchArrowView,
};

pub(crate) fn create_overlay_window(
    mtm: MainThreadMarker,
    assisted_pane: AssistedPane,
    host_app: &HostApp,
) -> Retained<NSPanel> {
    create_nonactivating_panel_with_content(
        mtm,
        layout::content_frame(),
        &create_content_view(mtm, assisted_pane, host_app),
    )
}

fn create_content_view(
    mtm: MainThreadMarker,
    assisted_pane: AssistedPane,
    host_app: &HostApp,
) -> Retained<NSView> {
    let content = ContentView::new(mtm, layout::content_frame());
    let drag_view = build_drag_view(mtm, host_app);
    let cursor = GuideCursorView::new(mtm);
    drag_view.set_guide(&cursor);

    let subviews = vec![
        LayoutSubview::new("arrow", unsafe {
            Retained::cast_unchecked(SketchArrowView::new(mtm))
        }),
        LayoutSubview::new("title", unsafe {
            Retained::cast_unchecked(build_title(mtm, assisted_pane))
        }),
        LayoutSubview::new("subtitle", unsafe {
            Retained::cast_unchecked(build_subtitle(mtm))
        }),
        LayoutSubview::new("dismiss", build_dismiss_button(mtm)),
        LayoutSubview::new("drag", unsafe { Retained::cast_unchecked(drag_view) }),
        LayoutSubview::new("cursor", unsafe { Retained::cast_unchecked(cursor) }),
    ];
    let host = LayoutHostView::new(mtm, content.bounds(), layout::content_layout(), subviews)
        .expect("permission assistant layout and bindings are valid");
    host.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    content.addSubview(&host);

    // SAFETY: `ContentView` is an `NSView` subclass.
    unsafe { Retained::cast_unchecked(content) }
}

fn styled_label(
    mtm: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(font));
    label.setTextColor(Some(color));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label
}

fn build_title(mtm: MainThreadMarker, assisted_pane: AssistedPane) -> Retained<NSTextField> {
    styled_label(
        mtm,
        &format!("Allow {}", assisted_pane.title),
        &NSFont::boldSystemFontOfSize(14.0),
        &NSColor::labelColor(),
    )
}

fn build_subtitle(mtm: MainThreadMarker) -> Retained<NSTextField> {
    styled_label(
        mtm,
        "Drag the app below into the list above.",
        &NSFont::systemFontOfSize(12.0),
        &NSColor::secondaryLabelColor(),
    )
}

fn build_dismiss_button(mtm: MainThreadMarker) -> Retained<NSView> {
    DismissButtonView(mtm)
}

fn build_drag_view(mtm: MainThreadMarker, host_app: &HostApp) -> Retained<DragSourceView> {
    DragSourceView::new(
        mtm,
        &host_app.display_name,
        &host_app.bundle_url,
        &host_app.icon,
    )
}
