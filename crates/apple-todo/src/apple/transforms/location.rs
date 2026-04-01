use objc2::msg_send;
use objc2_event_kit::EKStructuredLocation;

use crate::types::StructuredLocation;

pub fn transform_structured_location(location: &EKStructuredLocation) -> StructuredLocation {
    let title = unsafe { location.title() }
        .map(|s| s.to_string())
        .unwrap_or_default();

    let radius = unsafe {
        let r: f64 = msg_send![location, radius];
        if r == 0.0 { None } else { Some(r) }
    };

    StructuredLocation {
        title,
        geo: None,
        radius,
    }
}
