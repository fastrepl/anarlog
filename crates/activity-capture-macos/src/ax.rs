#![cfg(target_os = "macos")]

use std::ptr::NonNull;

use hypr_activity_capture_interface::CaptureError;
use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFBoolean, CFRetained, CFString, CFType};
use objc2_foundation::NSString;

const WINDOW_DEPTH_LIMIT: usize = 7;
const WINDOW_NODE_LIMIT: usize = 120;
const FOCUSED_NODE_LIMIT: usize = 40;
const WINDOW_CHARACTER_LIMIT: usize = 2500;
const FOCUSED_CHARACTER_LIMIT: usize = 800;
const ATTRIBUTE_TEXT_LIMIT: usize = 400;

const ATTR_CHILDREN: [&str; 3] = ["AXVisibleChildren", "AXContents", "AXChildren"];
const TEXT_ATTRIBUTES: [&str; 4] = ["AXValue", "AXDescription", "AXTitle", "AXSelectedText"];
const BLOCKED_ROLES: [&str; 16] = [
    "AXButton",
    "AXCheckBox",
    "AXDisclosureTriangle",
    "AXImage",
    "AXIncrementor",
    "AXMenu",
    "AXMenuBar",
    "AXMenuBarItem",
    "AXMenuButton",
    "AXPopUpButton",
    "AXRadioButton",
    "AXScrollBar",
    "AXTab",
    "AXTabGroup",
    "AXToolbar",
    "AXWindow",
];
const PREFERRED_CONTENT_ROLES: [&str; 16] = [
    "AXBrowser",
    "AXCell",
    "AXDocument",
    "AXGroup",
    "AXHeading",
    "AXLayoutArea",
    "AXList",
    "AXListItem",
    "AXOutline",
    "AXRow",
    "AXScrollArea",
    "AXStaticText",
    "AXTable",
    "AXTextArea",
    "AXTextField",
    "AXWebArea",
];

pub(crate) fn child_elements(
    element: &AXUIElement,
) -> Result<Vec<CFRetained<AXUIElement>>, CaptureError> {
    for attribute in ATTR_CHILDREN {
        let Some(value) = copy_attribute_value(element, attribute)? else {
            continue;
        };
        let Ok(array) = value.downcast::<CFArray>() else {
            continue;
        };
        let array = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(array) };

        let mut elements = Vec::new();
        for index in 0..array.len() {
            let Some(item) = array.get(index) else {
                continue;
            };
            let Ok(element) = item.downcast::<AXUIElement>() else {
                continue;
            };
            elements.push(element);
        }

        if !elements.is_empty() {
            return Ok(elements);
        }
    }

    Ok(Vec::new())
}

pub(crate) fn copy_element_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<CFRetained<AXUIElement>>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    match value.downcast::<AXUIElement>() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

pub(crate) fn string_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<String>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    Ok(value
        .downcast_ref::<CFString>()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty()))
}

pub(crate) fn bool_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<bool>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    Ok(value
        .downcast_ref::<CFBoolean>()
        .map(|flag| flag == CFBoolean::new(true)))
}

pub(crate) fn merge_fragments<I>(fragments: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();

    for fragment in fragments {
        for line in fragment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                lines.push(trimmed.to_string());
            }
        }
    }

    lines.join("\n")
}

pub(crate) fn collect_generic_visible_text(
    ax_application: &AXUIElement,
    focused_window: &AXUIElement,
) -> Result<String, CaptureError> {
    let window_text =
        collect_visible_text(focused_window, 0, WINDOW_NODE_LIMIT, WINDOW_CHARACTER_LIMIT)?;
    let focused_text = copy_element_attribute(ax_application, "AXFocusedUIElement")?
        .map(|value| collect_visible_text(&value, 0, FOCUSED_NODE_LIMIT, FOCUSED_CHARACTER_LIMIT))
        .transpose()?
        .unwrap_or_default();

    Ok(merge_fragments([window_text, focused_text]))
}

fn collect_visible_text(
    element: &AXUIElement,
    depth: usize,
    remaining_nodes: usize,
    remaining_characters: usize,
) -> Result<String, CaptureError> {
    if depth >= WINDOW_DEPTH_LIMIT || remaining_nodes == 0 || remaining_characters == 0 {
        return Ok(String::new());
    }

    let role = string_attribute(element, "AXRole")?.unwrap_or_default();
    if role.to_ascii_lowercase().contains("secure") {
        return Ok(String::new());
    }

    let mut pieces = Vec::new();
    if !BLOCKED_ROLES.contains(&role.as_str()) {
        for attribute in TEXT_ATTRIBUTES {
            let Some(text) = string_attribute(element, attribute)? else {
                continue;
            };
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.len() >= ATTRIBUTE_TEXT_LIMIT {
                continue;
            }
            pieces.push(trimmed.to_string());
        }
    }

    let children = prioritized_children(element)?;
    let child_budget = (remaining_nodes / children.len().max(1)).max(1);
    let char_budget = (remaining_characters / (children.len() + 1).max(1)).max(80);

    for child in children.into_iter().take(remaining_nodes.min(20)) {
        let child_text = collect_visible_text(&child, depth + 1, child_budget, char_budget)?;
        if !child_text.is_empty() {
            pieces.push(child_text);
        }
    }

    let merged = merge_fragments(pieces);
    if merged.len() <= remaining_characters {
        Ok(merged)
    } else {
        Ok(merged.chars().take(remaining_characters).collect())
    }
}

fn prioritized_children(
    element: &AXUIElement,
) -> Result<Vec<CFRetained<AXUIElement>>, CaptureError> {
    let mut children = child_elements(element)?;

    children.sort_by_key(|child| std::cmp::Reverse(child_priority(child)));
    Ok(children)
}

fn child_priority(element: &AXUIElement) -> usize {
    let role = string_attribute(element, "AXRole")
        .ok()
        .flatten()
        .unwrap_or_default();
    if PREFERRED_CONTENT_ROLES.contains(&role.as_str()) {
        3
    } else if BLOCKED_ROLES.contains(&role.as_str()) {
        0
    } else {
        1
    }
}

fn copy_attribute_value(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<CFRetained<CFType>>, CaptureError> {
    let attribute = NSString::from_str(attribute);
    let attribute: &CFString = attribute.as_ref();

    let mut raw_value = std::ptr::null();
    let result = unsafe {
        element.copy_attribute_value(
            attribute,
            NonNull::new(&mut raw_value).expect("attribute value pointer is never null"),
        )
    };
    match result {
        value if value == AXError::Success => {
            let Some(value) = NonNull::new(raw_value.cast_mut()) else {
                return Ok(None);
            };
            Ok(Some(unsafe { CFRetained::from_raw(value) }))
        }
        value
            if value == AXError::NoValue
                || value == AXError::AttributeUnsupported
                || value == AXError::Failure
                || value == AXError::CannotComplete =>
        {
            Ok(None)
        }
        error => Err(CaptureError::platform(format!(
            "AX attribute '{attribute}' failed with {:?}",
            error
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::merge_fragments;

    #[test]
    fn merge_fragments_deduplicates_trimmed_lines() {
        assert_eq!(
            merge_fragments([" hello\nworld ".to_string(), "world\nanother".to_string(),]),
            "hello\nworld\nanother"
        );
    }
}
