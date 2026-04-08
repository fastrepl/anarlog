#[cfg(target_os = "macos")]
use objc2::AnyThread;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage,
    NSStringDrawingDeprecated, NSStringDrawingOptions,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSCopying, NSDictionary, NSPoint, NSRect, NSSize, NSString};

pub enum Overlay {
    Recording,
    Degraded,
    Notification(u8),
}

#[cfg(target_os = "macos")]
impl Overlay {
    #[allow(deprecated)]
    pub fn draw(&self, base_image: &NSImage) -> Retained<NSImage> {
        match self {
            Overlay::Recording => draw_recording(base_image),
            Overlay::Degraded => draw_degraded(base_image),
            Overlay::Notification(count) => draw_notification(base_image, *count),
        }
    }
}

#[cfg(target_os = "macos")]
struct BadgeGeometry {
    inner_origin: NSPoint,
    inner_size: f64,
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn draw_badge(
    base_image: &NSImage,
    fill_color: &NSColor,
    draw_symbol: impl FnOnce(&BadgeGeometry),
) -> Retained<NSImage> {
    let size = base_image.size();
    let composite_image = NSImage::initWithSize(NSImage::alloc(), size);

    composite_image.lockFocus();

    base_image.drawAtPoint_fromRect_operation_fraction(
        NSPoint::new(0.0, 0.0),
        NSRect::new(NSPoint::new(0.0, 0.0), size),
        objc2_app_kit::NSCompositingOperation::Copy,
        1.0,
    );

    let dot_size = size.width * 0.33;
    let border_width = dot_size * 0.08;
    let dot_x = size.width - dot_size - (size.width * 0.02);
    let dot_y = size.height * 0.02;

    let white_color = NSColor::whiteColor();
    white_color.setFill();

    let outer_rect = NSRect::new(NSPoint::new(dot_x, dot_y), NSSize::new(dot_size, dot_size));
    let outer_path = NSBezierPath::bezierPathWithOvalInRect(outer_rect);
    outer_path.fill();

    fill_color.setFill();

    let inner_size = dot_size - (border_width * 2.0);
    let inner_x = dot_x + border_width;
    let inner_y = dot_y + border_width;
    let inner_rect = NSRect::new(
        NSPoint::new(inner_x, inner_y),
        NSSize::new(inner_size, inner_size),
    );
    let inner_path = NSBezierPath::bezierPathWithOvalInRect(inner_rect);
    inner_path.fill();

    let geo = BadgeGeometry {
        inner_origin: NSPoint::new(inner_x, inner_y),
        inner_size,
    };

    white_color.setFill();
    draw_symbol(&geo);

    composite_image.unlockFocus();

    composite_image
}

pub(crate) fn notification_badge_label(count: u8) -> Option<String> {
    match count {
        0 => None,
        1..=99 => Some(count.to_string()),
        _ => Some("99+".to_string()),
    }
}

#[cfg(target_os = "macos")]
fn draw_recording(base_image: &NSImage) -> Retained<NSImage> {
    draw_badge(base_image, &NSColor::systemRedColor(), |geo| {
        let center_size = geo.inner_size * 0.45;
        let center_x = geo.inner_origin.x + (geo.inner_size - center_size) / 2.0;
        let center_y = geo.inner_origin.y + (geo.inner_size - center_size) / 2.0;

        let center_rect = NSRect::new(
            NSPoint::new(center_x, center_y),
            NSSize::new(center_size, center_size),
        );
        let center_path = NSBezierPath::bezierPathWithOvalInRect(center_rect);
        center_path.fill();
    })
}

#[cfg(target_os = "macos")]
fn draw_degraded(base_image: &NSImage) -> Retained<NSImage> {
    draw_badge(base_image, &NSColor::systemOrangeColor(), |geo| {
        let stem_width = geo.inner_size * 0.15;
        let stem_height = geo.inner_size * 0.35;
        let stem_x = geo.inner_origin.x + (geo.inner_size - stem_width) / 2.0;
        let stem_y = geo.inner_origin.y + geo.inner_size * 0.42;
        let stem_rect = NSRect::new(
            NSPoint::new(stem_x, stem_y),
            NSSize::new(stem_width, stem_height),
        );
        let stem_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            stem_rect,
            stem_width / 2.0,
            stem_width / 2.0,
        );
        stem_path.fill();

        let dot_diameter = geo.inner_size * 0.15;
        let dot_cx = geo.inner_origin.x + (geo.inner_size - dot_diameter) / 2.0;
        let dot_cy = geo.inner_origin.y + geo.inner_size * 0.18;
        let excl_dot_rect = NSRect::new(
            NSPoint::new(dot_cx, dot_cy),
            NSSize::new(dot_diameter, dot_diameter),
        );
        let excl_dot_path = NSBezierPath::bezierPathWithOvalInRect(excl_dot_rect);
        excl_dot_path.fill();
    })
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn draw_notification(base_image: &NSImage, count: u8) -> Retained<NSImage> {
    let Some(label) = notification_badge_label(count) else {
        return base_image.copy();
    };

    draw_badge(base_image, &NSColor::systemRedColor(), |geo| {
        let text = NSString::from_str(&label);
        let font_size = match label.len() {
            1 => geo.inner_size * 0.6,
            2 => geo.inner_size * 0.48,
            _ => geo.inner_size * 0.38,
        };
        let font = NSFont::boldSystemFontOfSize(font_size);
        let text_color = NSColor::whiteColor();

        let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
        let values = [
            font.into_super().into_super(),
            text_color.into_super().into_super(),
        ];
        let attributes: Retained<NSDictionary<_, AnyObject>> =
            NSDictionary::from_retained_objects(&keys, &values);

        let bounds = unsafe {
            text.boundingRectWithSize_options_attributes(
                NSSize::new(geo.inner_size, geo.inner_size),
                NSStringDrawingOptions::UsesLineFragmentOrigin
                    | NSStringDrawingOptions::UsesFontLeading,
                Some(&attributes),
            )
        };

        let text_rect = NSRect::new(
            NSPoint::new(
                geo.inner_origin.x + (geo.inner_size - bounds.size.width) / 2.0,
                geo.inner_origin.y + (geo.inner_size - bounds.size.height) / 2.0 - 1.0,
            ),
            bounds.size,
        );

        unsafe {
            text.drawWithRect_options_attributes(
                text_rect,
                NSStringDrawingOptions::UsesLineFragmentOrigin
                    | NSStringDrawingOptions::UsesFontLeading,
                Some(&attributes),
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use super::notification_badge_label;

    #[test]
    fn notification_badge_label_clears_zero() {
        assert_eq!(notification_badge_label(0), None);
    }

    #[test]
    fn notification_badge_label_shows_small_count() {
        assert_eq!(notification_badge_label(1), Some("1".to_string()));
    }

    #[test]
    fn notification_badge_label_shows_two_digits() {
        assert_eq!(notification_badge_label(99), Some("99".to_string()));
    }

    #[test]
    fn notification_badge_label_caps_at_ninety_nine_plus() {
        assert_eq!(notification_badge_label(100), Some("99+".to_string()));
    }
}
