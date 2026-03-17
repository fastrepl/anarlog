use objc2::{msg_send, rc::Retained};
use objc2_core_graphics::CGColor;
use objc2_event_kit::EKCalendar;
use objc2_foundation::{NSInteger, NSURL};

use crate::types::{CalendarColor, CalendarEntityType, EventAvailability};

#[allow(unused_variables)]
pub fn extract_color_components(cg_color: &CGColor) -> CalendarColor {
    let num_components = CGColor::number_of_components(Some(cg_color));
    let components_ptr = CGColor::components(Some(cg_color));
    let alpha = CGColor::alpha(Some(cg_color)) as f32;

    if components_ptr.is_null() || num_components < 1 {
        return CalendarColor {
            red: 0.5,
            green: 0.5,
            blue: 0.5,
            alpha: 1.0,
        };
    }

    let components = unsafe { std::slice::from_raw_parts(components_ptr, num_components) };

    match num_components {
        2 => {
            let gray = components[0] as f32;
            CalendarColor {
                red: gray,
                green: gray,
                blue: gray,
                alpha,
            }
        }
        3 | 4 => CalendarColor {
            red: components[0] as f32,
            green: components[1] as f32,
            blue: components[2] as f32,
            alpha,
        },
        _ => CalendarColor {
            red: 0.5,
            green: 0.5,
            blue: 0.5,
            alpha: 1.0,
        },
    }
}

pub fn extract_supported_availabilities(calendar: &EKCalendar) -> Vec<EventAvailability> {
    let mut availabilities = Vec::new();
    unsafe {
        let mask: NSInteger = msg_send![calendar, supportedEventAvailabilities];
        if mask & 1 != 0 {
            availabilities.push(EventAvailability::Busy);
        }
        if mask & 2 != 0 {
            availabilities.push(EventAvailability::Free);
        }
        if mask & 4 != 0 {
            availabilities.push(EventAvailability::Tentative);
        }
        if mask & 8 != 0 {
            availabilities.push(EventAvailability::Unavailable);
        }
    }
    if availabilities.is_empty() {
        availabilities.push(EventAvailability::NotSupported);
    }
    availabilities
}

pub fn extract_allowed_entity_types(calendar: &EKCalendar) -> Vec<CalendarEntityType> {
    let mut types = Vec::new();
    unsafe {
        let mask: NSInteger = msg_send![calendar, allowedEntityTypes];
        if mask & 1 != 0 {
            types.push(CalendarEntityType::Event);
        }
        if mask & 2 != 0 {
            types.push(CalendarEntityType::Reminder);
        }
    }
    types
}

pub fn get_url_string<T>(obj: &T, _selector: &str) -> Option<String>
where
    T: objc2::Message + ?Sized,
{
    unsafe {
        let url_obj: Option<Retained<NSURL>> = msg_send![obj, URL];
        url_obj.and_then(|u| u.absoluteString().map(|s| s.to_string()))
    }
}
