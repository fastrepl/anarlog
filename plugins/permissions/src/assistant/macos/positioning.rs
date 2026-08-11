use objc2::Message;
use objc2_app_kit::{NSAnimatablePropertyContainer, NSPanel};
use objc2_foundation::NSRect;

use anlg_overlay_kit::layout::{Alignment, Insets, LayoutRect, LayoutSize};
use anlg_overlay_kit::macos::panel::{OverlayPanelPresentation, reconcile_panel_presentation};
use anlg_overlay_kit::macos::support::rect;
use anlg_overlay_kit::placement::{PlacementExtent, PlacementSpec};

use super::animation::{self, REPOSITION_DURATION, REPOSITION_THRESHOLD};
use super::layout::{
    OVERLAY_BOTTOM_MARGIN, OVERLAY_HEIGHT, OVERLAY_LEFT_MARGIN, OVERLAY_MAX_WIDTH,
    OVERLAY_MIN_WIDTH, OVERLAY_RIGHT_MARGIN, SIDEBAR_WIDTH,
};
use super::settings_window::SettingsWindowSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositionMode {
    Initial,
    Tracking,
}

pub(crate) fn refresh_overlay_position(panel: &NSPanel, mode: PositionMode) {
    let Some(snapshot) = SettingsWindowSnapshot::locate_visible_frontmost_window() else {
        reconcile_panel_presentation(panel, OverlayPanelPresentation::Hidden);
        return;
    };

    position_overlay(panel, snapshot, mode);
}

pub(crate) fn position_overlay(
    panel: &NSPanel,
    snapshot: SettingsWindowSnapshot,
    mode: PositionMode,
) {
    let target = overlay_geometry(snapshot.frame, snapshot.visible_frame);

    match mode {
        PositionMode::Initial => {
            panel.setFrame_display(target, true);
        }
        PositionMode::Tracking => {
            let current = panel.frame();
            let delta = animation::frame_delta(current, target);
            if delta > REPOSITION_THRESHOLD {
                let panel = panel.retain();
                animation::animate(REPOSITION_DURATION, move || {
                    panel.animator().setFrame_display(target, true);
                });
            } else if delta > f64::EPSILON {
                panel.setFrame_display(target, true);
            }
        }
    }

    reconcile_panel_presentation(panel, OverlayPanelPresentation::Visible);
}

/// Anchors the overlay to the System Settings content list card: a small left
/// inset from the sidebar edge and a right inset from the window edge, with the
/// right edge pinned when width clamping applies.
fn overlay_geometry(settings_frame: NSRect, visible_frame: NSRect) -> NSRect {
    let content_min_x = settings_frame.origin.x + SIDEBAR_WIDTH;
    let preferred = LayoutRect::new(
        content_min_x,
        settings_frame.origin.y,
        (settings_frame.size.width - SIDEBAR_WIDTH).max(0.0),
        settings_frame.size.height,
    );
    // Combining the content boundary with the screen boundary before applying
    // the common safe inset preserves the "never cover the sidebar" policy.
    let clamp_min_x = visible_frame.origin.x.max(content_min_x - 8.0);
    let visible = LayoutRect::new(
        clamp_min_x,
        visible_frame.origin.y,
        (visible_frame.origin.x + visible_frame.size.width - clamp_min_x).max(0.0),
        visible_frame.size.height,
    );
    let placement = PlacementSpec::new(
        PlacementExtent::fill_bounded(OVERLAY_MIN_WIDTH, OVERLAY_MAX_WIDTH),
        PlacementExtent::fixed(OVERLAY_HEIGHT),
    )
    .align(Alignment::End, Alignment::Start)
    .margins(Insets::new(
        OVERLAY_BOTTOM_MARGIN,
        OVERLAY_RIGHT_MARGIN,
        0.0,
        OVERLAY_LEFT_MARGIN,
    ))
    .safe_insets(Insets::all(8.0))
    .resolve(
        preferred,
        visible,
        LayoutSize::new(OVERLAY_MAX_WIDTH, OVERLAY_HEIGHT),
    )
    .expect("validated permission assistant placement");
    rect(placement.x, placement.y, placement.width, placement.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_is_right_pinned_and_width_clamped() {
        let visible = rect(0.0, 0.0, 1440.0, 860.0);
        let wide = overlay_geometry(rect(100.0, 80.0, 1000.0, 700.0), visible);
        assert_eq!(wide, rect(412.0, 89.0, 680.0, 128.0));

        let narrow = overlay_geometry(rect(100.0, 80.0, 500.0, 700.0), visible);
        assert_eq!(narrow, rect(320.0, 89.0, 340.0, 128.0));
    }

    #[test]
    fn placement_never_crosses_sidebar_or_visible_edges() {
        let target = overlay_geometry(
            rect(-100.0, -50.0, 420.0, 300.0),
            rect(0.0, 0.0, 800.0, 600.0),
        );
        assert!(target.origin.x >= 120.0);
        assert!(target.origin.y >= 8.0);
        assert!(target.origin.x + target.size.width <= 792.0);
    }
}
