use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSView};
use objc2_foundation::MainThreadMarker;

use anlg_overlay_kit::macos::button::{
    ButtonRadius, OverlayButtonConfig, OverlayButtonStyle, OwnedFill, overlay_button,
};

use anlg_overlay_kit::macos::support::{rect, symbol_image_view};

use super::super::animation::HOVER_DURATION;
use super::super::dismiss_current;

const IDLE_ALPHA: f64 = 0.06;
const HOVER_ALPHA: f64 = 0.12;

pub(crate) fn new(mtm: MainThreadMarker) -> Retained<NSView> {
    let glyph = symbol_image_view(mtm, "xmark", 10.0, 0.4, &NSColor::secondaryLabelColor());

    overlay_button(
        mtm,
        rect(0.0, 0.0, 0.0, 0.0),
        OverlayButtonConfig {
            tooltip: Some("Dismiss".to_string()),
            accessibility_label: Some("Dismiss".to_string()),
            content: {
                // SAFETY: `NSImageView` is an `NSView` subclass.
                Some(unsafe { Retained::cast_unchecked(glyph) })
            },
            style: OverlayButtonStyle {
                radius: ButtonRadius::Capsule,
                fill: Box::new(|hovered| OwnedFill {
                    background: NSColor::labelColor().colorWithAlphaComponent(if hovered {
                        HOVER_ALPHA
                    } else {
                        IDLE_ALPHA
                    }),
                    border: None,
                }),
                hover_animation: Some(HOVER_DURATION),
            },
            on_click: Box::new(dismiss_current),
        },
    )
}
