use objc2::{msg_send, rc::Retained};
use objc2_core_graphics::CGColor;
use objc2_foundation::NSString;

use crate::types::CalendarColor;

pub fn objc_title<T>(obj: &T) -> String
where
    T: objc2::Message + ?Sized,
{
    unsafe {
        let title: Option<Retained<NSString>> = msg_send![obj, title];
        title.map(|s| s.to_string()).unwrap_or_default()
    }
}

pub fn objc_source_identifier<T>(obj: &T) -> String
where
    T: objc2::Message + ?Sized,
{
    unsafe {
        let identifier: Option<Retained<NSString>> = msg_send![obj, sourceIdentifier];
        identifier.map(|s| s.to_string()).unwrap_or_default()
    }
}

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
