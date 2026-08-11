use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAccessibility, NSAccessibilityButtonRole, NSEvent, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSRect, NSString};

use super::animation::animate_layer_background_color;
use super::support::{FillStyle, apply_rounded_fill, rect, with_effective_appearance};
use super::tracking::visible_hover_tracking_area;

#[derive(Clone, Copy, Debug)]
pub enum ButtonRadius {
    /// Pill/circle radius derived from the view height.
    Capsule,
    Fixed(f64),
}

pub struct OwnedFill {
    pub background: Retained<objc2_app_kit::NSColor>,
    pub border: Option<(Retained<objc2_app_kit::NSColor>, f64)>,
}

pub struct OverlayButtonStyle {
    pub radius: ButtonRadius,
    pub fill: Box<dyn Fn(bool) -> OwnedFill>,
    pub hover_animation: Option<f64>,
}

pub struct OverlayButtonConfig {
    pub tooltip: Option<String>,
    pub accessibility_label: Option<String>,
    pub content: Option<Retained<NSView>>,
    pub style: OverlayButtonStyle,
    pub on_click: Box<dyn Fn()>,
}

struct OverlayButtonIvars {
    hovered: Cell<bool>,
    radius: ButtonRadius,
    fill: Box<dyn Fn(bool) -> OwnedFill>,
    hover_animation: Option<f64>,
    content: Option<Retained<NSView>>,
    on_click: Box<dyn Fn()>,
}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "HyprOverlayButton"]
    #[ivars = OverlayButtonIvars]
    struct OverlayButton;

    impl OverlayButton {
        #[unsafe(method(layout))]
        fn layout(&self) {
            let _: () = unsafe { msg_send![super(self), layout] };
            self.apply_fill_instant();
            self.layout_content();
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.apply_fill_instant();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().hovered.set(true);
            self.apply_fill_after_hover();
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().hovered.set(false);
            self.apply_fill_after_hover();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            (self.ivars().on_click)();
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_perform_press(&self) -> bool {
            (self.ivars().on_click)();
            true
        }
    }

    unsafe impl NSObjectProtocol for OverlayButton {}
}

impl OverlayButton {
    fn corner_radius(&self) -> f64 {
        match self.ivars().radius {
            ButtonRadius::Capsule => self.bounds().size.height / 2.0,
            ButtonRadius::Fixed(radius) => radius,
        }
    }

    fn resolve_fill(&self) -> OwnedFill {
        (self.ivars().fill)(self.ivars().hovered.get())
    }

    fn apply_fill_instant(&self) {
        let fill = self.resolve_fill();
        let radius = self.corner_radius();
        let border = fill
            .border
            .as_ref()
            .map(|(color, width)| (color.as_ref(), *width));
        apply_rounded_fill(
            self,
            radius,
            FillStyle {
                background: fill.background.as_ref(),
                border,
            },
        );
    }

    fn apply_fill_after_hover(&self) {
        let Some(duration) = self.ivars().hover_animation else {
            self.apply_fill_instant();
            return;
        };

        let Some(layer) = self.layer() else {
            return;
        };

        let fill = self.resolve_fill();
        with_effective_appearance(self, || {
            animate_layer_background_color(&layer, fill.background.as_ref(), duration);
        });

        if let Some((border, width)) = fill.border.as_ref() {
            layer.setBorderWidth(*width);
            with_effective_appearance(self, || {
                layer.setBorderColor(Some(&border.CGColor()));
            });
        } else {
            layer.setBorderWidth(0.0);
            layer.setBorderColor(None);
        }
    }

    fn layout_content(&self) {
        let Some(content) = self.ivars().content.as_ref() else {
            return;
        };
        let bounds = self.bounds();
        let content_size = content.frame().size;
        content.setFrame(rect(
            bounds.origin.x + (bounds.size.width - content_size.width) / 2.0,
            bounds.origin.y + (bounds.size.height - content_size.height) / 2.0,
            content_size.width,
            content_size.height,
        ));
    }
}

/// Builds a layer-backed overlay button with hover tracking and optional
/// animated fill transitions.
pub fn overlay_button(
    mtm: MainThreadMarker,
    frame: NSRect,
    config: OverlayButtonConfig,
) -> Retained<NSView> {
    let OverlayButtonConfig {
        tooltip,
        accessibility_label,
        content,
        style,
        on_click,
    } = config;

    let this = OverlayButton::alloc(mtm).set_ivars(OverlayButtonIvars {
        hovered: Cell::new(false),
        radius: style.radius,
        fill: style.fill,
        hover_animation: style.hover_animation,
        content: content.clone(),
        on_click,
    });
    let this: Retained<OverlayButton> = unsafe { msg_send![super(this), initWithFrame: frame] };
    this.setWantsLayer(true);

    if let Some(tooltip) = tooltip {
        this.setToolTip(Some(&NSString::from_str(&tooltip)));
    }
    if let Some(accessibility_label) = accessibility_label {
        this.setAccessibilityElement(true);
        this.setAccessibilityRole(Some(unsafe { NSAccessibilityButtonRole }));
        this.setAccessibilityLabel(Some(&NSString::from_str(&accessibility_label)));
    }

    this.add_tracking_area();
    this.apply_fill_instant();

    if let Some(content) = content {
        this.addSubview(&content);
        this.layout_content();
    }

    // SAFETY: `OverlayButton` is an `NSView` subclass.
    unsafe { Retained::cast_unchecked(this) }
}

impl OverlayButton {
    fn add_tracking_area(&self) {
        let owner: &AnyObject = self.as_ref();
        self.addTrackingArea(&visible_hover_tracking_area(owner));
    }
}
