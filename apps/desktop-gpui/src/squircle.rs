//! Figma-style corner smoothing, as `@lisse/core` applies it to the app's
//! buttons, select triggers, badges (`controlSquircle`, radius 8) and app
//! floating panels (`panelSquircle`, radius 20) with `APPLE_SMOOTHING`.
//!
//! Port of figma-squircle's `getPathParamsForCorner` / `getSvgPath` as Lisse
//! drives it: when the corner does not fit, the smoothing is reduced rather
//! than preserved (checked against `generatePath` output).

use gpui::{Bounds, Path, PathBuilder, Pixels, Point, Rgba, canvas, point, prelude::*, px};

pub const APPLE_SMOOTHING: f32 = 0.65;

/// `DesignRadius.lg`: buttons, select triggers, badges.
pub const CONTROL_RADIUS: f32 = 8.0;
/// `DesignRadius.panel`: `AppFloatingPanel`.
pub const PANEL_RADIUS: f32 = 20.0;

/// One corner's segment lengths (figma-squircle's `CornerPathParams`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    /// How far along each edge the corner extends: `(1 + smoothing) * radius`,
    /// capped by the budget.
    pub p: f32,
    pub arc: f32,
    pub radius: f32,
}

fn radians(degrees: f32) -> f32 {
    degrees * std::f32::consts::PI / 180.0
}

/// `getPathParamsForCorner`; `budget` is half the box's shorter side.
pub fn corner(radius: f32, mut smoothing: f32, budget: f32) -> Corner {
    if radius <= 0.0 {
        return Corner {
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            p: 0.0,
            arc: 0.0,
            radius: 0.0,
        };
    }
    let mut p = (1.0 + smoothing) * radius;
    // Without `preserveSmoothing`, the smoothing shrinks to what fits.
    let max_smoothing = budget / radius - 1.0;
    smoothing = smoothing.min(max_smoothing);
    p = p.min(budget);
    let arc_angle = 90.0 * (1.0 - smoothing);
    let arc = (radians(arc_angle / 2.0)).sin() * radius * 2f32.sqrt();
    let angle_alpha = (90.0 - arc_angle) / 2.0;
    let p3_to_p4 = radius * (radians(angle_alpha / 2.0)).tan();
    let angle_beta = 45.0 * smoothing;
    let c = p3_to_p4 * (radians(angle_beta)).cos();
    let d = c * (radians(angle_beta)).tan();
    let b = (p - arc - c - d) / 3.0;
    let a = 2.0 * b;
    Corner {
        a,
        b,
        c,
        d,
        p,
        arc,
        radius,
    }
}

/// Maps a vector in the top-right corner's frame into another corner's.
type Rotate = fn(f32, f32) -> (f32, f32);

/// The closed outline of `bounds` with every corner smoothed.
pub fn path(bounds: Bounds<Pixels>, radius: f32, smoothing: f32) -> Option<Path<Pixels>> {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let corner = corner(radius, smoothing, width.min(height) / 2.0);
    let origin = bounds.origin;
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    let mut builder = PathBuilder::fill();
    // Each corner is the top-right one rotated a quarter turn further:
    // `rotate` maps a vector in the top-right frame into the corner's frame.
    let corners: [(f32, f32, Rotate); 4] = [
        (width - corner.p, 0.0, |x, y| (x, y)),
        (width, height - corner.p, |x, y| (-y, x)),
        (corner.p, height, |x, y| (-x, -y)),
        (0.0, corner.p, |x, y| (y, -x)),
    ];
    builder.move_to(at(corners[0].0, corners[0].1));
    for (index, (start_x, start_y, rotate)) in corners.iter().enumerate() {
        if index > 0 {
            builder.line_to(at(*start_x, *start_y));
        }
        let mut cursor = (*start_x, *start_y);
        // Relative vectors in the top-right frame -> absolute points.
        let rel = |cursor: (f32, f32), dx: f32, dy: f32| -> ((f32, f32), Point<Pixels>) {
            let (rx, ry) = rotate(dx, dy);
            let next = (cursor.0 + rx, cursor.1 + ry);
            (next, at(next.0, next.1))
        };
        // `c a 0 ab 0 abc d`
        let (_, control_a) = rel(cursor, corner.a, 0.0);
        let (_, control_b) = rel(cursor, corner.a + corner.b, 0.0);
        let (next, end) = rel(cursor, corner.a + corner.b + corner.c, corner.d);
        cursor = next;
        builder.cubic_bezier_to(end, control_a, control_b);
        // `a r r 0 0 1 arc arc`
        let (next, end) = rel(cursor, corner.arc, corner.arc);
        cursor = next;
        builder.arc_to(
            point(px(corner.radius), px(corner.radius)),
            px(0.0),
            false,
            true,
            end,
        );
        // `c d c d bc d abc`
        let (_, control_a) = rel(cursor, corner.d, corner.c);
        let (_, control_b) = rel(cursor, corner.d, corner.b + corner.c);
        let (_, end) = rel(cursor, corner.d, corner.a + corner.b + corner.c);
        builder.cubic_bezier_to(end, control_a, control_b);
    }
    builder.close();
    builder.build().ok()
}

/// A smoothed-corner box painted behind its siblings: place it as the first
/// child of a `relative()` container. `border` is `(width, colour)`.
pub fn squircle(radius: f32, fill: Option<Rgba>, border: Option<(f32, Rgba)>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if let Some((width, color)) = border {
                if let Some(outer) = path(bounds, radius, APPLE_SMOOTHING) {
                    window.paint_path(outer, color);
                }
                let inner = Bounds {
                    origin: point(bounds.origin.x + px(width), bounds.origin.y + px(width)),
                    size: gpui::size(
                        bounds.size.width - px(width * 2.0),
                        bounds.size.height - px(width * 2.0),
                    ),
                };
                if let Some(fill) = fill
                    && let Some(path) = path(inner, (radius - width).max(0.0), APPLE_SMOOTHING)
                {
                    window.paint_path(path, fill);
                }
            } else if let Some(fill) = fill
                && let Some(path) = path(bounds, radius, APPLE_SMOOTHING)
            {
                window.paint_path(path, fill);
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn control_corner_matches_lisse() {
        // `generatePath(32, 32, { radius: 8, smoothing: APPLE_SMOOTHING })`
        let corner = corner(CONTROL_RADIUS, APPLE_SMOOTHING, 16.0);
        assert!(close(corner.p, 13.2));
        assert!(close(corner.a, 4.8584));
        assert!(close(corner.a + corner.b, 7.2876));
        assert!(close(corner.a + corner.b + corner.c, 9.1090));
        assert!(close(corner.d, 1.0200));
        assert!(close(corner.arc, 3.0710));
    }

    #[test]
    fn panel_corner_matches_lisse() {
        // `generatePath(224, 120, { radius: 20, smoothing: APPLE_SMOOTHING })`
        let corner = corner(PANEL_RADIUS, APPLE_SMOOTHING, 60.0);
        assert!(close(corner.p, 33.0));
        assert!(close(corner.a, 12.1460));
        assert!(close(corner.a + corner.b, 18.2189));
        assert!(close(corner.a + corner.b + corner.c, 22.7724));
        assert!(close(corner.d, 2.5501));
        assert!(close(corner.arc, 7.6775));
    }

    #[test]
    fn small_box_reduces_smoothing_to_the_budget() {
        // `generatePath(20, 20, { radius: 8, smoothing: APPLE_SMOOTHING })`
        let corner = corner(CONTROL_RADIUS, APPLE_SMOOTHING, 10.0);
        assert!(close(corner.p, 10.0));
        assert!(close(corner.a, 1.8586));
        assert!(close(corner.a + corner.b, 2.7879));
        assert!(close(corner.a + corner.b + corner.c, 3.5607));
        assert!(close(corner.d, 0.1537));
        assert!(close(corner.arc, 6.2856));
    }

    #[test]
    fn mid_box_matches_lisse() {
        // `generatePath(24, 24, { radius: 8, smoothing: APPLE_SMOOTHING })`
        let corner = corner(CONTROL_RADIUS, APPLE_SMOOTHING, 12.0);
        assert!(close(corner.p, 12.0));
        assert!(close(corner.a, 3.7275));
        assert!(close(corner.a + corner.b, 5.5913));
        assert!(close(corner.a + corner.b + corner.c, 7.0615));
        assert!(close(corner.d, 0.6090));
        assert!(close(corner.arc, 4.3296));
    }

    #[test]
    fn builds_a_closed_path_for_a_button() {
        let bounds = Bounds::new(point(px(10.0), px(10.0)), gpui::size(px(32.0), px(32.0)));
        assert!(path(bounds, CONTROL_RADIUS, APPLE_SMOOTHING).is_some());
        assert!(path(Bounds::default(), CONTROL_RADIUS, APPLE_SMOOTHING).is_none());
    }
}
