use objc2::{msg_send, rc::Retained};
use objc2_event_kit::{EKAlarm, EKStructuredLocation};
use objc2_foundation::{NSDate, NSInteger, NSString, NSURL};

use crate::types::{Alarm, AlarmProximity, AlarmType};

use super::super::recurrence::offset_date_time_from;
use super::location::transform_structured_location;

pub fn transform_alarm(alarm: &EKAlarm) -> Alarm {
    let absolute_date = unsafe {
        let date: Option<Retained<NSDate>> = msg_send![alarm, absoluteDate];
        date.map(offset_date_time_from)
    };

    let relative_offset: Option<f64> = unsafe {
        let offset: f64 = msg_send![alarm, relativeOffset];
        if offset == 0.0 && absolute_date.is_some() {
            None
        } else {
            Some(offset)
        }
    };

    let proximity = unsafe {
        let p: NSInteger = msg_send![alarm, proximity];
        match p {
            0 => Some(AlarmProximity::None),
            1 => Some(AlarmProximity::Enter),
            2 => Some(AlarmProximity::Leave),
            _ => None,
        }
    };

    let alarm_type = unsafe {
        let t: NSInteger = msg_send![alarm, type];
        match t {
            0 => Some(AlarmType::Display),
            1 => Some(AlarmType::Audio),
            2 => Some(AlarmType::Procedure),
            3 => Some(AlarmType::Email),
            _ => None,
        }
    };

    let email_address = unsafe {
        let email: Option<Retained<NSString>> = msg_send![alarm, emailAddress];
        email.map(|s| s.to_string())
    };

    let sound_name = unsafe {
        let sound: Option<Retained<NSString>> = msg_send![alarm, soundName];
        sound.map(|s| s.to_string())
    };

    let url = unsafe {
        let url_obj: Option<Retained<NSURL>> = msg_send![alarm, url];
        url_obj.and_then(|u| u.absoluteString().map(|s| s.to_string()))
    };

    let structured_location = unsafe {
        let loc: Option<Retained<EKStructuredLocation>> = msg_send![alarm, structuredLocation];
        loc.map(|l| transform_structured_location(&l))
    };

    Alarm {
        absolute_date,
        relative_offset,
        proximity,
        alarm_type,
        email_address,
        sound_name,
        url,
        structured_location,
    }
}
