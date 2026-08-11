use anlg_overlay_kit::layout::{
    Alignment, Anchor, Dimension, LayoutNode, LayoutSpec, OverlayChild,
};
use objc2_foundation::NSRect;

use anlg_overlay_kit::macos::support::rect;

// Overlay panel geometry. Side margins keep a small native-looking gap between
// the panel border and the content pane edges (matching the gap macOS leaves
// between its sidebar and the list card).
pub(super) const OVERLAY_MAX_WIDTH: f64 = 680.0;
pub(super) const OVERLAY_MIN_WIDTH: f64 = 340.0;
pub(super) const OVERLAY_LEFT_MARGIN: f64 = 10.0;
pub(super) const OVERLAY_RIGHT_MARGIN: f64 = 8.0;
pub(super) const OVERLAY_BOTTOM_MARGIN: f64 = 9.0;
pub(super) const OVERLAY_HEIGHT: f64 = 128.0;

// System Settings window geometry used when anchoring the overlay.
pub(super) const SIDEBAR_WIDTH: f64 = 220.0;
pub(super) const MIN_SETTINGS_WINDOW_WIDTH: f64 = 320.0;
pub(super) const MIN_SETTINGS_WINDOW_HEIGHT: f64 = 240.0;

pub(super) fn content_frame() -> NSRect {
    rect(0.0, 0.0, OVERLAY_MAX_WIDTH, OVERLAY_HEIGHT)
}

pub(super) fn content_layout() -> LayoutSpec {
    let header = LayoutNode::column(vec![
        LayoutNode::leaf("title", Dimension::fill(1.0), Dimension::fixed(20.0)),
        LayoutNode::leaf("subtitle", Dimension::fill(1.0), Dimension::fixed(18.0)),
    ])
    .gap(2.0)
    .cross_alignment(Alignment::Stretch);

    LayoutNode::overlay(vec![
        OverlayChild::new(
            LayoutNode::fixed_leaf("arrow", 44.0, 56.0),
            Anchor::Start(14.0),
            Anchor::Start(14.0),
        ),
        OverlayChild::new(
            header,
            Anchor::Stretch {
                start: 56.0,
                end: 60.0,
            },
            Anchor::Start(18.0),
        ),
        OverlayChild::new(
            LayoutNode::fixed_leaf("dismiss", 24.0, 24.0),
            Anchor::End(20.0),
            Anchor::Start(16.0),
        ),
        OverlayChild::new(
            LayoutNode::leaf("drag", Dimension::fill(1.0), Dimension::fixed(44.0)),
            Anchor::Stretch {
                start: 16.0,
                end: 16.0,
            },
            Anchor::End(18.0),
        ),
        OverlayChild::new(
            LayoutNode::intrinsic_leaf("cursor"),
            Anchor::Center(0.0),
            Anchor::Center(2.0),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_overlay_kit::layout::{LayoutMeasurements, LayoutSize};

    #[test]
    fn assistant_layout_tracks_minimum_and_maximum_widths() {
        let measurements = LayoutMeasurements::new().with("cursor", LayoutSize::new(24.0, 24.0));
        for width in [OVERLAY_MIN_WIDTH, 510.0, OVERLAY_MAX_WIDTH] {
            let result = content_layout()
                .solve(LayoutSize::new(width, OVERLAY_HEIGHT), &measurements)
                .unwrap();
            assert_eq!(result.frame("drag").unwrap().x, 16.0);
            assert_eq!(result.frame("drag").unwrap().width, width - 32.0);
            assert_eq!(result.frame("dismiss").unwrap().x, width - 44.0);
            assert_eq!(result.frame("cursor").unwrap().x, (width - 24.0) / 2.0);
        }
    }
}
