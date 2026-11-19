use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFBoolean, CFNumber, CFRetained, CFString, CFType};

use super::AxFailure;

pub(crate) type AxResult<T> = std::result::Result<T, AxFailure>;

pub(crate) fn press(element: &AXUIElement) -> AxResult<()> {
    let action = CFString::from_str("AXPress");
    let result = unsafe { element.perform_action(&action) };
    if result == AXError::Success {
        Ok(())
    } else {
        Err(AxFailure(format!("AXPress returned {result:?}")))
    }
}

pub(crate) fn focus(element: &AXUIElement, pid: i32) -> AxResult<()> {
    let element_focused = unsafe {
        element.set_attribute_value(
            &CFString::from_str("AXFocused"),
            CFBoolean::new(true).as_ref(),
        )
    };
    let application = unsafe { AXUIElement::new_application(pid) };
    let element_value: &CFType = element.as_ref();
    let application_focused = unsafe {
        application.set_attribute_value(&CFString::from_str("AXFocusedUIElement"), element_value)
    };
    if element_focused == AXError::Success || application_focused == AXError::Success {
        Ok(())
    } else {
        Err(AxFailure(format!(
            "could not focus element: element={element_focused:?}, application={application_focused:?}"
        )))
    }
}

pub(crate) fn children(element: &AXUIElement) -> AxResult<Vec<CFRetained<AXUIElement>>> {
    element_array_attribute(element, "AXChildren")
}

pub(crate) fn windows(element: &AXUIElement) -> AxResult<Vec<CFRetained<AXUIElement>>> {
    element_array_attribute(element, "AXWindows")
}

/// The elements an attribute such as `AXRows` or `AXSelectedRows` holds.
#[allow(dead_code, reason = "used by meeting_camera")]
pub(crate) fn element_list(
    element: &AXUIElement,
    attribute: &str,
) -> AxResult<Vec<CFRetained<AXUIElement>>> {
    element_array_attribute(element, attribute)
}

pub(crate) fn element_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> AxResult<Option<CFRetained<AXUIElement>>> {
    Ok(copy_attribute_value(element, attribute)?
        .and_then(|value| value.downcast::<AXUIElement>().ok()))
}

pub(crate) fn string_attribute(element: &AXUIElement, attribute: &str) -> AxResult<Option<String>> {
    Ok(copy_attribute_value(element, attribute)?
        .and_then(|value| value.downcast::<CFString>().ok())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty()))
}

#[allow(dead_code, reason = "used by meeting_camera")]
pub(crate) fn bool_attribute(element: &AXUIElement, attribute: &str) -> AxResult<Option<bool>> {
    Ok(copy_attribute_value(element, attribute)?
        .and_then(|value| value.downcast::<CFBoolean>().ok())
        .map(|value| value.as_bool()))
}

pub(crate) fn role_attribute(element: &AXUIElement) -> AxResult<String> {
    Ok(string_attribute(element, "AXRole")?.unwrap_or_default())
}

pub(crate) fn dom_classes(element: &AXUIElement) -> AxResult<Vec<String>> {
    let Some(value) = copy_attribute_value(element, "AXDOMClassList")? else {
        return Ok(Vec::new());
    };
    let Ok(array) = value.downcast::<CFArray>() else {
        return Ok(Vec::new());
    };
    // SAFETY: AXDOMClassList is a CoreFoundation array whose entries are CF objects. Reinterpret
    // the checked array and downcast each string entry.
    let array = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(array) };
    Ok(array
        .iter()
        .filter_map(|value| value.downcast::<CFString>().ok())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .collect())
}

#[allow(dead_code, reason = "used by meeting_camera")]
pub(crate) fn retain_element(element: &AXUIElement) -> CFRetained<AXUIElement> {
    unsafe { CFRetained::retain(NonNull::from(element)) }
}

pub(crate) fn set_attribute(element: &AXUIElement, attribute: &str, value: &CFType) -> bool {
    let attribute = CFString::from_str(attribute);
    unsafe { element.set_attribute_value(&attribute, value) == AXError::Success }
}

pub(crate) fn perform(element: &AXUIElement, action: &str) -> bool {
    let action = CFString::from_str(action);
    unsafe { element.perform_action(&action) == AXError::Success }
}

/// An app's windows. Zoom leaves `AXWindows` empty while it isn't the front app, yet still names
/// its main and focused windows, so those stand in when the list is empty.
pub(crate) fn app_windows(application: &AXUIElement) -> AxResult<Vec<CFRetained<AXUIElement>>> {
    let listed = windows(application)?;
    if !listed.is_empty() {
        return Ok(listed);
    }
    let mut found: Vec<CFRetained<AXUIElement>> = Vec::new();
    for attribute in ["AXMainWindow", "AXFocusedWindow"] {
        if let Some(window) = element_attribute(application, attribute)?
            && !found.iter().any(|known| **known == *window)
        {
            found.push(window);
        }
    }
    Ok(found)
}

/// Whether a menu item, radio item, check box or row shows as chosen: a checkmark, a checked value
/// (a boolean or 1), or selected.
#[allow(dead_code, reason = "used by meeting_camera")]
pub(crate) fn is_checked(element: &AXUIElement) -> AxResult<bool> {
    if string_attribute(element, "AXMenuItemMarkChar")?.is_some() {
        return Ok(true);
    }
    if bool_attribute(element, "AXSelected")? == Some(true) {
        return Ok(true);
    }
    let Some(value) = copy_attribute_value(element, "AXValue")? else {
        return Ok(false);
    };
    if let Some(flag) = value.downcast_ref::<CFBoolean>() {
        return Ok(flag.as_bool());
    }
    if let Some(number) = value.downcast_ref::<CFNumber>() {
        return Ok(number.as_i64() == Some(1));
    }
    Ok(false)
}

fn element_array_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> AxResult<Vec<CFRetained<AXUIElement>>> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(Vec::new());
    };
    let Ok(array) = value.downcast::<CFArray>() else {
        return Ok(Vec::new());
    };
    // SAFETY: AXChildren and AXWindows are CoreFoundation arrays whose entries are CF objects.
    // Reinterpret the checked array and downcast each entry.
    let array = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(array) };
    Ok(array
        .iter()
        .filter_map(|value| value.downcast::<AXUIElement>().ok())
        .collect())
}

/// A missing or unreadable attribute reads as absent: the scans treat an element they can't read
/// as one that doesn't match.
fn copy_attribute_value(
    element: &AXUIElement,
    attribute: &str,
) -> AxResult<Option<CFRetained<CFType>>> {
    let attribute = CFString::from_str(attribute);
    let mut raw_value = std::ptr::null();
    let result = unsafe { element.copy_attribute_value(&attribute, NonNull::from(&mut raw_value)) };
    match result {
        AXError::Success => {
            let Some(value) = NonNull::new(raw_value.cast_mut()) else {
                return Ok(None);
            };
            Ok(Some(unsafe { CFRetained::from_raw(value) }))
        }
        _ => Ok(None),
    }
}
