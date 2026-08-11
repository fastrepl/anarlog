use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSDragOperation, NSDraggingContext,
    NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSFont, NSImage, NSImageScaling,
    NSImageView, NSLineBreakMode, NSPasteboard, NSPasteboardItem, NSPasteboardItemDataProvider,
    NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardWriting, NSTextField, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSObjectProtocol, NSPoint, NSRect, NSString, NSURL,
};
use objc2_quartz_core::CALayer;

use anlg_overlay_kit::layout::{Alignment, Anchor, Dimension, Insets, LayoutNode, OverlayChild};
use anlg_overlay_kit::macos::layout::{LayoutHostView, LayoutSubview};
use anlg_overlay_kit::macos::path::{HatchStyle, stroke_hatch};
use anlg_overlay_kit::macos::support::{apply_continuous_corner, is_dark_appearance, rect};
use anlg_overlay_kit::macos::tracking::visible_hover_tracking_area;

use super::super::animation::{self, HOVER_DURATION};
use super::decorative_switch::DecorativeSwitch;
use super::guide_cursor::GuideCursorView;
use super::interaction::DragInteraction;
use super::theme::permission_drag_row_background;

#[derive(Debug, Default)]
pub(crate) struct DragPlaceholderIvars {}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantDragPlaceholderView"]
    #[ivars = DragPlaceholderIvars]
    pub(crate) struct DragPlaceholderView;

    impl DragPlaceholderView {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw_placeholder();
        }
    }

    unsafe impl NSObjectProtocol for DragPlaceholderView {}
}

impl DragPlaceholderView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DragPlaceholderIvars::default());
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: rect(0.0, 0.0, 0.0, 0.0)] };
        this.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        this.setHidden(true);
        this
    }

    fn draw_placeholder(&self) {
        let bounds = self.bounds();
        let dark = is_dark_appearance(self);
        let inner_rect = inset(bounds, 2.5);
        let radius = 8.0;
        let fill_alpha = if dark { 0.05 } else { 0.10 };
        permission_drag_row_background(dark, false)
            .colorWithAlphaComponent(fill_alpha)
            .setFill();

        let fill_path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inner_rect, radius, radius);
        fill_path.fill();

        let hatch_color =
            NSColor::systemGrayColor().colorWithAlphaComponent(if dark { 0.30 } else { 0.22 });
        stroke_hatch(inner_rect, radius, &hatch_color, HatchStyle::default());

        NSColor::systemGrayColor()
            .colorWithAlphaComponent(if dark { 0.68 } else { 0.52 })
            .setStroke();
        stroke_flat_round_rect(inset(bounds, 2.0), radius, 1.6);
    }
}

pub(crate) struct DragSourceIvars {
    bundle_url: Retained<NSURL>,
    placeholder_view: Retained<DragPlaceholderView>,
    row_view: Retained<NSView>,
    interaction: Cell<DragInteraction>,
    guide: RefCell<Option<Retained<GuideCursorView>>>,
}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantDragSourceView"]
    #[ivars = DragSourceIvars]
    pub(crate) struct DragSourceView;

    impl DragSourceView {
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.apply_row_appearance_instant();
            self.ivars().placeholder_view.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.update_interaction(|interaction| interaction.set_hovered(true));
            self.animate_row_appearance();
            self.update_guide();
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.update_interaction(|interaction| interaction.set_hovered(false));
            self.animate_row_appearance();
            self.update_guide();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let item = NSPasteboardItem::new();
            let provider = ProtocolObject::from_ref(self);
            let pasteboard_types = NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]);
            item.setDataProvider_forTypes(provider, &pasteboard_types);

            let item_ref: &NSPasteboardItem = item.as_ref();
            let writer = ProtocolObject::<dyn NSPasteboardWriting>::from_ref(item_ref);
            let dragging_item =
                NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
            let dragging_frame = self.dragging_frame();
            let dragging_image = self.dragging_image();
            let contents: &AnyObject = dragging_image.as_ref();
            unsafe {
                dragging_item.setDraggingFrame_contents(dragging_frame, Some(contents));
            }
            let items = NSArray::from_slice(&[dragging_item.as_ref()]);
            let source = ProtocolObject::<dyn NSDraggingSource>::from_ref(self);
            let session = self.beginDraggingSessionWithItems_event_source(&items, event, source);
            session.setAnimatesToStartingPositionsOnCancelOrFail(true);
        }
    }

    unsafe impl NSObjectProtocol for DragSourceView {}

    unsafe impl NSPasteboardItemDataProvider for DragSourceView {
        #[unsafe(method(pasteboard:item:provideDataForType:))]
        fn pasteboard_item_provide_data_for_type(
            &self,
            _pasteboard: Option<&NSPasteboard>,
            item: &NSPasteboardItem,
            pasteboard_type: &NSPasteboardType,
        ) {
            if pasteboard_type != unsafe { NSPasteboardTypeFileURL } {
                return;
            }
            item.setData_forType(&self.ivars().bundle_url.dataRepresentation(), pasteboard_type);
        }
    }

    unsafe impl NSDraggingSource for DragSourceView {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn dragging_session_source_operation_mask_for_dragging_context(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }

        #[unsafe(method(draggingSession:willBeginAtPoint:))]
        fn dragging_session_will_begin_at_point(
            &self,
            _session: &NSDraggingSession,
            _screen_point: NSPoint,
        ) {
            self.update_interaction(DragInteraction::begin_drag);
            self.ivars().placeholder_view.setHidden(false);
            self.ivars().row_view.setHidden(true);
            self.update_guide();
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn dragging_session_ended_at_point_operation(
            &self,
            _session: &NSDraggingSession,
            _screen_point: NSPoint,
            operation: NSDragOperation,
        ) {
            let completed = operation.0 & NSDragOperation::Copy.0 != 0;
            self.update_interaction(|interaction| interaction.end_drag(completed));
            self.ivars().placeholder_view.setHidden(true);
            self.ivars().row_view.setHidden(false);
            self.update_guide();
        }
    }
}

impl DragSourceView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        display_name: &str,
        bundle_url: &NSURL,
        icon: &NSImage,
    ) -> Retained<Self> {
        let placeholder_view = DragPlaceholderView::new(mtm);
        let row_view = build_row_view(mtm, display_name, icon);

        let this = Self::alloc(mtm).set_ivars(DragSourceIvars {
            bundle_url: bundle_url.retain(),
            placeholder_view: placeholder_view.retain(),
            row_view: row_view.retain(),
            interaction: Cell::new(DragInteraction::default()),
            guide: RefCell::new(None),
        });
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: rect(0.0, 0.0, 0.0, 0.0)] };
        placeholder_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        row_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        this.addSubview(&placeholder_view);
        this.addSubview(&row_view);
        this.add_tracking_area();
        this.apply_row_appearance_instant();
        this
    }

    fn add_tracking_area(&self) {
        let owner: &AnyObject = self.as_ref();
        self.addTrackingArea(&visible_hover_tracking_area(owner));
    }

    /// Links the drag-guide overlay so its visibility can follow interaction.
    pub(crate) fn set_guide(&self, guide: &GuideCursorView) {
        *self.ivars().guide.borrow_mut() = Some(guide.retain());
        self.update_guide();
    }

    /// Hides the pulse + cursor guide once the user hovers, drags, or drops.
    fn update_guide(&self) {
        let ivars = self.ivars();
        if let Some(guide) = ivars.guide.borrow().as_ref() {
            guide.set_visible(ivars.interaction.get().guide_visible());
        }
    }

    fn row_hovered(&self) -> bool {
        self.ivars().interaction.get().is_hovered()
    }

    fn update_interaction(&self, update: impl FnOnce(&mut DragInteraction)) {
        let interaction = &self.ivars().interaction;
        let mut state = interaction.get();
        update(&mut state);
        interaction.set(state);
    }

    fn apply_row_layer_style(&self, layer: &CALayer) {
        // Tighter radius than the panel so the row nests concentrically inside
        // the panel's rounded corner (inner radius < outer radius) instead of
        // competing with it and pinching the gap at the corner.
        apply_continuous_corner(layer, 8.0);
        layer.setBorderWidth(0.0);
    }

    fn apply_row_appearance_instant(&self) {
        let Some(layer) = self.ivars().row_view.layer() else {
            return;
        };
        self.apply_row_layer_style(&layer);
        let hovered = self.row_hovered();
        layer.setBackgroundColor(Some(
            &permission_drag_row_background(is_dark_appearance(self), hovered).CGColor(),
        ));
    }

    fn animate_row_appearance(&self) {
        let Some(layer) = self.ivars().row_view.layer() else {
            return;
        };
        self.apply_row_layer_style(&layer);
        let hovered = self.row_hovered();

        animation::animate_layer_background_color(
            &layer,
            &permission_drag_row_background(is_dark_appearance(self), hovered),
            HOVER_DURATION,
        );
    }

    fn dragging_frame(&self) -> NSRect {
        let row_view = &*self.ivars().row_view;
        self.convertRect_fromView(row_view.frame(), Some(row_view))
    }

    fn dragging_image(&self) -> Retained<NSImage> {
        let row_view = &*self.ivars().row_view;
        let bounds = row_view.bounds();
        let rep = row_view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .expect("drag source row should produce a bitmap representation");
        row_view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let image = NSImage::initWithSize(NSImage::alloc(), bounds.size);
        image.addRepresentation(rep.as_ref());
        image
    }
}

fn inset(rect: NSRect, amount: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.origin.x + amount, rect.origin.y + amount),
        objc2_foundation::NSSize::new(
            (rect.size.width - amount * 2.0).max(0.0),
            (rect.size.height - amount * 2.0).max(0.0),
        ),
    )
}

fn stroke_flat_round_rect(rect: NSRect, radius: f64, width: f64) {
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    path.setLineWidth(width);
    path.stroke();
}

/// Builds the yellow app row (icon chrome, icon, name label, decorative
/// toggle) that the user drags into the System Settings list.
fn build_row_view(mtm: MainThreadMarker, display_name: &str, icon: &NSImage) -> Retained<NSView> {
    let label = NSTextField::labelWithString(&NSString::from_str(display_name), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize_weight(14.0, 0.23)));
    label.setTextColor(Some(&NSColor::labelColor()));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let icon_chrome = build_icon_chrome(mtm, icon);
    let toggle = DecorativeSwitch::new(mtm);
    let spec = LayoutNode::row(vec![
        LayoutNode::fixed_leaf("icon", 26.0, 26.0),
        LayoutNode::space(6.0),
        LayoutNode::leaf("label", Dimension::fill(1.0), Dimension::fixed(20.0)),
        LayoutNode::space(10.0),
        LayoutNode::fixed_leaf("toggle", 38.0, 22.0),
    ])
    .padding(Insets::xy(12.0, 0.0))
    .cross_alignment(Alignment::Center);
    let row = LayoutHostView::new(
        mtm,
        rect(0.0, 0.0, 0.0, 0.0),
        spec,
        vec![
            LayoutSubview::new("icon", icon_chrome),
            LayoutSubview::new("label", unsafe { Retained::cast_unchecked(label) }),
            LayoutSubview::new("toggle", unsafe { Retained::cast_unchecked(toggle) }),
        ],
    )
    .expect("permission drag row layout and bindings are valid");
    row.setWantsLayer(true);
    unsafe { Retained::cast_unchecked(row) }
}

fn build_icon_chrome(mtm: MainThreadMarker, icon: &NSImage) -> Retained<NSView> {
    let icon_view = NSImageView::imageViewWithImage(icon, mtm);
    icon_view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    let spec = LayoutNode::overlay(vec![OverlayChild::new(
        LayoutNode::fixed_leaf("icon", 22.0, 22.0),
        Anchor::Center(0.0),
        Anchor::Center(0.0),
    )]);
    let chrome = LayoutHostView::new(
        mtm,
        rect(0.0, 0.0, 26.0, 26.0),
        spec,
        vec![LayoutSubview::new("icon", unsafe {
            Retained::cast_unchecked(icon_view)
        })],
    )
    .expect("permission app icon layout and binding are valid");
    chrome.setWantsLayer(true);
    if let Some(layer) = chrome.layer() {
        apply_continuous_corner(&layer, 6.0);
        layer.setMasksToBounds(true);
    }
    unsafe { Retained::cast_unchecked(chrome) }
}
