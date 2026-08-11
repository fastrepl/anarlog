use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds};
use objc2_foundation::{MainThreadMarker, NSNumber, NSRect, NSSize, NSString};

use crate::geometry::{CheckedRect, InvalidRect};

use super::support::rect;

/// `NSRect` is a type alias for `CGRect` on macOS, so these conversions cover
/// both AppKit and Core Graphics rectangles. They validate numeric soundness
/// only; coordinate-space flips remain explicit via helpers such as
/// [`top_left_rect_to_appkit`] and [`cg_frame_to_appkit`].
impl TryFrom<CGRect> for CheckedRect {
    type Error = InvalidRect;

    fn try_from(value: CGRect) -> Result<Self, Self::Error> {
        Self::new(
            value.origin.x,
            value.origin.y,
            value.size.width,
            value.size.height,
        )
        .ok_or(InvalidRect)
    }
}

impl From<CheckedRect> for CGRect {
    fn from(value: CheckedRect) -> Self {
        CGRect::new(
            CGPoint::new(value.x(), value.y()),
            CGSize::new(value.width(), value.height()),
        )
    }
}

pub fn main_screen_frame(mtm: MainThreadMarker, fallback_size: NSSize) -> NSRect {
    NSScreen::mainScreen(mtm)
        .map(|screen| screen.frame())
        .filter(|frame| CheckedRect::try_from(*frame).is_ok())
        .unwrap_or_else(|| rect(0.0, 0.0, fallback_size.width, fallback_size.height))
}

pub fn top_left_rect_to_appkit(target: CheckedRect, screen_height: f64) -> Option<NSRect> {
    if !screen_height.is_finite() || screen_height <= 0.0 {
        return None;
    }
    let converted = CheckedRect::new(
        target.x(),
        screen_height - target.y() - target.height(),
        target.width(),
        target.height(),
    )?;
    Some(converted.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ScreenGeometry {
    pub frame: NSRect,
    pub visible_frame: NSRect,
    pub cg_bounds: CGRect,
}

pub fn screen_geometries(mtm: MainThreadMarker) -> Vec<ScreenGeometry> {
    NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let device = screen.deviceDescription();
            let number = device.objectForKey(&NSString::from_str("NSScreenNumber"))?;
            let ns_number = number.downcast_ref::<NSNumber>()?;
            let display_id = ns_number.unsignedIntValue() as CGDirectDisplayID;
            let geometry = ScreenGeometry {
                frame: screen.frame(),
                visible_frame: screen.visibleFrame(),
                cg_bounds: CGDisplayBounds(display_id),
            };
            valid_screen_geometry(geometry)
        })
        .collect()
}

/// Map a CG (top-left) window rect to AppKit (bottom-left) coords on the
/// screen it overlaps most, returning `(appkit_frame, matched_visible_frame)`.
pub fn cg_frame_to_appkit(
    cg_frame: CGRect,
    screens: &[ScreenGeometry],
) -> Option<(NSRect, NSRect)> {
    let cg_frame = CheckedRect::try_from(cg_frame).ok()?;
    let matched = screens
        .iter()
        .filter_map(|screen| valid_screen_geometry(*screen))
        .filter(|screen| {
            CheckedRect::try_from(screen.cg_bounds)
                .is_ok_and(|bounds| cg_rects_intersect(bounds, cg_frame))
        })
        .max_by(|left, right| {
            let left_area = CheckedRect::try_from(left.cg_bounds)
                .map(|bounds| cg_intersection_area(bounds, cg_frame))
                .unwrap_or(0.0);
            let right_area = CheckedRect::try_from(right.cg_bounds)
                .map(|bounds| cg_intersection_area(bounds, cg_frame))
                .unwrap_or(0.0);
            left_area.total_cmp(&right_area)
        })?;

    Some((
        cg_to_appkit_frame(cg_frame, &matched)?,
        matched.visible_frame,
    ))
}

fn valid_screen_geometry(value: ScreenGeometry) -> Option<ScreenGeometry> {
    CheckedRect::try_from(value.frame).ok()?;
    CheckedRect::try_from(value.visible_frame).ok()?;
    CheckedRect::try_from(value.cg_bounds).ok()?;
    Some(value)
}

fn cg_to_appkit_frame(cg_frame: CheckedRect, matched: &ScreenGeometry) -> Option<NSRect> {
    let cg_bounds = CheckedRect::try_from(matched.cg_bounds).ok()?;
    let frame = CheckedRect::try_from(matched.frame).ok()?;
    let local_x = cg_frame.x() - cg_bounds.x();
    let local_y = cg_frame.y() - cg_bounds.y();
    let converted = CheckedRect::new(
        frame.x() + local_x,
        frame.y() + frame.height() - local_y - cg_frame.height(),
        cg_frame.width(),
        cg_frame.height(),
    )?;
    Some(converted.into())
}

fn cg_rects_intersect(left: CheckedRect, right: CheckedRect) -> bool {
    cg_intersection_area(left, right) > 0.0
}

fn cg_intersection_area(left: CheckedRect, right: CheckedRect) -> f64 {
    let x1 = left.x().max(right.x());
    let y1 = left.y().max(right.y());
    let x2 = left.right().min(right.right());
    let y2 = left.bottom().min(right.bottom());
    let width = (x2 - x1).max(0.0);
    let height = (y2 - y1).max(0.0);
    width * height
}
