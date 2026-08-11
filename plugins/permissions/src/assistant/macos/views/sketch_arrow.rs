use objc2::rc::Retained;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSLineCapStyle, NSLineJoinStyle, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSRect};

use anlg_overlay_kit::macos::path::bezier_path;
use anlg_overlay_kit::macos::support::rect;
use anlg_overlay_kit::{
    layout::{LayoutPoint, LayoutSize},
    path::{PathCommand, PathFit, PathSpec, PathWinding},
};

#[derive(Debug, Default)]
pub(crate) struct SketchArrowIvars {}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AnarlogPermissionAssistantSketchArrowView"]
    #[ivars = SketchArrowIvars]
    pub(crate) struct SketchArrowView;

    impl SketchArrowView {
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw_arrow();
        }
    }

    unsafe impl NSObjectProtocol for SketchArrowView {}
}

impl SketchArrowView {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SketchArrowIvars::default());
        unsafe { msg_send![super(this), initWithFrame: rect(0.0, 0.0, 0.0, 0.0)] }
    }

    /// Strokes a loose, hand-drawn arrow curving upward toward the list above.
    ///
    /// The shaft and arrowhead share a single path stroked once, so
    /// self-overlaps are unioned (painted once) instead of compositing twice
    /// into darker patches. The color is opaque for the same reason.
    fn draw_arrow(&self) {
        NSColor::systemGrayColor().set();
        let point = LayoutPoint::new;
        let spec = PathSpec::new(
            LayoutSize::new(1.0, 1.0),
            PathWinding::NonZero,
            vec![
                PathCommand::MoveTo(point(0.62, 0.94)),
                PathCommand::CubicTo {
                    control1: point(0.66, 0.78),
                    control2: point(0.34, 0.68),
                    to: point(0.42, 0.54),
                },
                PathCommand::CubicTo {
                    control1: point(0.48, 0.38),
                    control2: point(0.50, 0.20),
                    to: point(0.44, 0.10),
                },
                PathCommand::LineTo(point(0.22, 0.22)),
                PathCommand::MoveTo(point(0.44, 0.10)),
                PathCommand::LineTo(point(0.60, 0.26)),
            ],
        )
        .expect("normalized sketch arrow path is valid");
        let path = bezier_path(&spec, self.bounds(), PathFit::Stretch)
            .expect("sketch arrow bounds are valid");
        path.setLineWidth(2.4);
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        path.stroke();
    }
}
