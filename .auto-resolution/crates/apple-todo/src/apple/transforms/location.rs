use objc2::msg_send;
use objc2_core_location::CLLocation;
use objc2_event_kit::EKStructuredLocation;

use crate::types::{GeoLocation, StructuredLocation};

pub fn transform_structured_location(location: &EKStructuredLocation) -> StructuredLocation {
    let title = unsafe { location.title() }
        .map(|s| s.to_string())
        .unwrap_or_default();
    let geo = unsafe { location.geoLocation() }.map(extract_geo_location);

    let radius = unsafe {
        let r: f64 = msg_send![location, radius];
        if r == 0.0 { None } else { Some(r) }
    };

    StructuredLocation { title, geo, radius }
}

fn extract_geo_location(location: objc2::rc::Retained<CLLocation>) -> GeoLocation {
    let coordinate = unsafe { location.coordinate() };
    GeoLocation {
        latitude: coordinate.latitude,
        longitude: coordinate.longitude,
    }
}
