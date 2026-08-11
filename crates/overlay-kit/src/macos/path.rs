use objc2::rc::Retained;
use objc2_app_kit::{
    NSBezierPath, NSColor, NSGraphicsContext, NSLineCapStyle, NSLineJoinStyle, NSWindingRule,
};
use objc2_foundation::{NSPoint, NSRect};

use crate::layout::{LayoutPoint, LayoutRect};
use crate::path::{PathCommand, PathError, PathFit, PathSpec, PathWinding};

pub fn bezier_path(
    spec: &PathSpec,
    bounds: NSRect,
    fit: PathFit,
) -> Result<Retained<NSBezierPath>, PathError> {
    let layout_bounds = LayoutRect::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    );
    let transform = spec.transform(layout_bounds, fit)?;
    let path = NSBezierPath::bezierPath();
    path.setWindingRule(match spec.winding {
        PathWinding::NonZero => NSWindingRule::NonZero,
        PathWinding::EvenOdd => NSWindingRule::EvenOdd,
    });

    for command in &spec.commands {
        match *command {
            PathCommand::MoveTo(point) => {
                path.moveToPoint(appkit_point(transform.point(point), bounds))
            }
            PathCommand::LineTo(point) => {
                path.lineToPoint(appkit_point(transform.point(point), bounds))
            }
            PathCommand::CubicTo {
                control1,
                control2,
                to,
            } => path.curveToPoint_controlPoint1_controlPoint2(
                appkit_point(transform.point(to), bounds),
                appkit_point(transform.point(control1), bounds),
                appkit_point(transform.point(control2), bounds),
            ),
            PathCommand::Close => path.closePath(),
        }
    }
    Ok(path)
}

fn appkit_point(point: LayoutPoint, bounds: NSRect) -> NSPoint {
    NSPoint::new(
        point.x,
        bounds.origin.y + bounds.size.height - (point.y - bounds.origin.y),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HatchStyle {
    pub spacing: f64,
    pub line_width: f64,
    pub angle_degrees: f64,
    pub jitter: f64,
}

impl Default for HatchStyle {
    fn default() -> Self {
        Self {
            spacing: 8.0,
            line_width: 1.2,
            angle_degrees: 45.0,
            jitter: 0.16,
        }
    }
}

/// Draws a clipped diagonal hatch while leaving color ownership with the
/// consumer. Invalid style values fail closed and draw nothing.
pub fn stroke_hatch(rect: NSRect, radius: f64, color: &NSColor, style: HatchStyle) {
    if !valid_hatch(rect, radius, style) {
        return;
    }
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    context.saveGraphicsState();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius).addClip();
    color.setStroke();

    let min_x = rect.origin.x;
    let min_y = rect.origin.y;
    let max_x = rect.origin.x + rect.size.width;
    let max_y = rect.origin.y + rect.size.height;
    let radians = style.angle_degrees.to_radians();
    let dx = rect.size.height / radians.tan();
    let start = min_x - dx.abs();
    let end = max_x + dx.abs();
    let mut index = 0usize;
    let mut x = start;
    while x <= end {
        let path = NSBezierPath::bezierPath();
        path.setLineWidth(style.line_width);
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        path.moveToPoint(NSPoint::new(
            x + jitter(index, 0, style.jitter),
            min_y + jitter(index, 1, style.jitter),
        ));
        path.lineToPoint(NSPoint::new(
            x + dx + jitter(index, 2, style.jitter),
            max_y + jitter(index, 3, style.jitter),
        ));
        path.stroke();
        index += 1;
        x += style.spacing;
    }
    context.restoreGraphicsState();
}

fn valid_hatch(rect: NSRect, radius: f64, style: HatchStyle) -> bool {
    rect.origin.x.is_finite()
        && rect.origin.y.is_finite()
        && rect.size.width.is_finite()
        && rect.size.width >= 0.0
        && rect.size.height.is_finite()
        && rect.size.height >= 0.0
        && radius.is_finite()
        && radius >= 0.0
        && style.spacing.is_finite()
        && style.spacing > 0.0
        && style.line_width.is_finite()
        && style.line_width > 0.0
        && style.angle_degrees.is_finite()
        && style.angle_degrees.abs() > 0.1
        && style.angle_degrees.abs() < 89.9
        && style.jitter.is_finite()
        && style.jitter >= 0.0
}

fn jitter(seed: usize, offset: usize, amount: f64) -> f64 {
    let hash = seed.wrapping_mul(17).wrapping_add(offset).wrapping_mul(13);
    let normalized = (hash % 5) as f64 / 4.0;
    (normalized * 2.0 - 1.0) * amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutSize;
    use crate::macos::support::rect;
    use crate::path::PathWinding;

    #[test]
    fn renderer_preserves_top_left_path_intent() {
        let spec = PathSpec::new(
            LayoutSize::new(10.0, 10.0),
            PathWinding::NonZero,
            vec![
                PathCommand::MoveTo(LayoutPoint::new(0.0, 0.0)),
                PathCommand::LineTo(LayoutPoint::new(10.0, 10.0)),
            ],
        )
        .unwrap();
        let transform = spec
            .transform(LayoutRect::new(0.0, 0.0, 20.0, 20.0), PathFit::Stretch)
            .unwrap();
        assert_eq!(
            transform.point(LayoutPoint::new(0.0, 0.0)),
            LayoutPoint::new(0.0, 0.0)
        );
        assert_eq!(
            appkit_point(
                transform.point(LayoutPoint::new(10.0, 10.0)),
                rect(0.0, 0.0, 20.0, 20.0)
            ),
            NSPoint::new(20.0, 0.0)
        );
    }
}
