#![cfg(target_os = "macos")]

use std::{
    collections::BTreeSet,
    hash::{DefaultHasher, Hash, Hasher},
    ptr::NonNull,
};

use hypr_activity_capture_interface::{CaptureError, TextAnchorConfidence, TextAnchorKind};
use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFBoolean, CFRetained, CFString, CFType};
use objc2_foundation::NSString;

const WINDOW_DEPTH_LIMIT: usize = 7;
const WINDOW_NODE_LIMIT: usize = 120;
const FOCUSED_NODE_LIMIT: usize = 40;
const WINDOW_CHARACTER_LIMIT: usize = 2500;
const FOCUSED_CHARACTER_LIMIT: usize = 800;
const ATTRIBUTE_TEXT_LIMIT: usize = 400;
const ANCHOR_TEXT_LIMIT: usize = 160;
const ANCHOR_CONTEXT_LIMIT: usize = 48;
const ANCHOR_ANCESTOR_LIMIT: usize = 6;

const ATTR_CHILDREN: [&str; 3] = ["AXVisibleChildren", "AXContents", "AXChildren"];
const TEXT_ATTRIBUTES: [&str; 4] = ["AXValue", "AXDescription", "AXTitle", "AXSelectedText"];
const ANCHOR_DIRECT_TEXT_ATTRIBUTES: [&str; 3] = ["AXValue", "AXDescription", "AXTitle"];
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
const TEXT_ENTRY_ROLES: [&str; 5] = [
    "AXTextArea",
    "AXTextField",
    "AXSearchField",
    "AXComboBox",
    "AXWebArea",
];
const DOCUMENT_ROLES: [&str; 3] = ["AXDocument", "AXTextArea", "AXWebArea"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextAnchorCapture {
    pub kind: TextAnchorKind,
    pub identity: String,
    pub text: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub selected_text: Option<String>,
    pub confidence: TextAnchorConfidence,
}

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

pub(crate) fn best_effort_element_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Option<CFRetained<AXUIElement>> {
    copy_element_attribute(element, attribute).ok().flatten()
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

pub(crate) fn best_effort_string_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Option<String> {
    string_attribute(element, attribute).ok().flatten()
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

pub(crate) fn best_effort_bool_attribute(element: &AXUIElement, attribute: &str) -> Option<bool> {
    bool_attribute(element, attribute).ok().flatten()
}

pub(crate) fn enable_manual_accessibility(element: &AXUIElement) {
    let attribute = CFString::from_str("AXManualAccessibility");
    let value = CFBoolean::new(true);
    let _ = unsafe { element.set_attribute_value(&attribute, value.as_ref()) };
}

pub(crate) fn merge_fragments<I>(fragments: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut seen = BTreeSet::new();
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

pub(crate) fn collect_text_anchor(
    ax_application: &AXUIElement,
    focused_window: &AXUIElement,
    app_name: &str,
    window_title: &str,
) -> Result<Option<TextAnchorCapture>, CaptureError> {
    let Some(focused_element) = copy_element_attribute(ax_application, "AXFocusedUIElement")?
    else {
        return Ok(None);
    };

    if let Some(anchor) = focused_anchor_candidate(&focused_element, app_name, window_title) {
        return Ok(Some(anchor));
    }

    let mut current = best_effort_element_attribute(&focused_element, "AXParent");
    let mut depth = 0;
    while let Some(element) = current {
        if depth >= ANCHOR_ANCESTOR_LIMIT {
            break;
        }
        if let Some(anchor) = document_anchor_candidate(&element, app_name, window_title) {
            return Ok(Some(anchor));
        }
        current = best_effort_element_attribute(&element, "AXParent");
        depth += 1;
    }

    Ok(document_anchor_candidate(
        focused_window,
        app_name,
        window_title,
    ))
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

fn focused_anchor_candidate(
    element: &AXUIElement,
    app_name: &str,
    window_title: &str,
) -> Option<TextAnchorCapture> {
    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    if is_secure_role(&role) {
        return None;
    }

    let selected_text = best_effort_string_attribute(element, "AXSelectedText")
        .map(|value| normalize_text(&value))
        .filter(|value| !value.is_empty());
    let direct_text = collect_direct_text(element);
    let focused = best_effort_bool_attribute(element, "AXFocused") == Some(true);
    let editable = best_effort_bool_attribute(element, "AXEditable") == Some(true);
    let entry_role = TEXT_ENTRY_ROLES.contains(&role.as_str());

    if let Some(selected_text) = selected_text.clone() {
        let excerpt = derive_anchor_excerpt(
            direct_text.as_deref().unwrap_or(&selected_text),
            Some(&selected_text),
            true,
        );
        return Some(build_text_anchor(
            element,
            app_name,
            window_title,
            TextAnchorKind::SelectedText,
            if focused || editable {
                TextAnchorConfidence::High
            } else {
                TextAnchorConfidence::Medium
            },
            selected_text,
            excerpt,
        ));
    }

    if (editable || entry_role) && direct_text.is_some() {
        let excerpt = derive_anchor_excerpt(direct_text.as_deref().unwrap_or_default(), None, true);
        return Some(build_text_anchor(
            element,
            app_name,
            window_title,
            TextAnchorKind::FocusedEdit,
            if focused || editable {
                TextAnchorConfidence::High
            } else {
                TextAnchorConfidence::Medium
            },
            String::new(),
            excerpt,
        ));
    }

    let direct_text = direct_text?;
    let excerpt = derive_anchor_excerpt(&direct_text, None, false);
    Some(build_text_anchor(
        element,
        app_name,
        window_title,
        TextAnchorKind::FocusedElement,
        if focused {
            TextAnchorConfidence::Medium
        } else {
            TextAnchorConfidence::Low
        },
        String::new(),
        excerpt,
    ))
}

fn document_anchor_candidate(
    element: &AXUIElement,
    app_name: &str,
    window_title: &str,
) -> Option<TextAnchorCapture> {
    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    if !DOCUMENT_ROLES.contains(&role.as_str()) || is_secure_role(&role) {
        return None;
    }

    let text = collect_visible_text(element, 0, FOCUSED_NODE_LIMIT, FOCUSED_CHARACTER_LIMIT)
        .ok()
        .map(|value| normalize_text(&value))
        .filter(|value| !value.is_empty())?;
    let excerpt = derive_anchor_excerpt(&text, None, false);

    Some(build_text_anchor(
        element,
        app_name,
        window_title,
        TextAnchorKind::Document,
        TextAnchorConfidence::Low,
        String::new(),
        excerpt,
    ))
}

fn build_text_anchor(
    element: &AXUIElement,
    app_name: &str,
    window_title: &str,
    kind: TextAnchorKind,
    confidence: TextAnchorConfidence,
    selected_text: String,
    excerpt: AnchorExcerpt,
) -> TextAnchorCapture {
    TextAnchorCapture {
        kind,
        identity: anchor_identity(element, app_name, window_title, kind),
        text: Some(excerpt.text).filter(|value| !value.is_empty()),
        prefix: excerpt.prefix,
        suffix: excerpt.suffix,
        selected_text: Some(selected_text).filter(|value| !value.is_empty()),
        confidence,
    }
}

fn collect_direct_text(element: &AXUIElement) -> Option<String> {
    let fragments = ANCHOR_DIRECT_TEXT_ATTRIBUTES
        .iter()
        .filter_map(|attribute| best_effort_string_attribute(element, attribute))
        .map(|value| normalize_text(&truncate_chars(&value, WINDOW_CHARACTER_LIMIT)));
    let merged = merge_fragments(fragments);
    (!merged.is_empty()).then_some(merged)
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

    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    if is_secure_role(&role) {
        return Ok(String::new());
    }

    let mut pieces = Vec::new();
    if !BLOCKED_ROLES.contains(&role.as_str()) {
        for attribute in TEXT_ATTRIBUTES {
            let Some(text) = best_effort_string_attribute(element, attribute) else {
                continue;
            };
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.chars().count() >= ATTRIBUTE_TEXT_LIMIT {
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
    if merged.chars().count() <= remaining_characters {
        Ok(merged)
    } else {
        Ok(truncate_chars(&merged, remaining_characters))
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
    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    if PREFERRED_CONTENT_ROLES.contains(&role.as_str()) {
        3
    } else if BLOCKED_ROLES.contains(&role.as_str()) {
        0
    } else {
        1
    }
}

fn anchor_identity(
    element: &AXUIElement,
    app_name: &str,
    _window_title: &str,
    kind: TextAnchorKind,
) -> String {
    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    let subrole = best_effort_string_attribute(element, "AXSubrole").unwrap_or_default();
    let identifier = best_effort_string_attribute(element, "AXIdentifier").unwrap_or_default();
    let help = best_effort_string_attribute(element, "AXHelp").unwrap_or_default();
    let placeholder =
        best_effort_string_attribute(element, "AXPlaceholderValue").unwrap_or_default();
    let role_path = anchor_role_path(element);

    let mut hasher = DefaultHasher::new();
    normalize_identity_component(app_name).hash(&mut hasher);
    normalize_identity_component(&role).hash(&mut hasher);
    normalize_identity_component(&subrole).hash(&mut hasher);
    normalize_identity_component(&identifier).hash(&mut hasher);
    normalize_identity_component(&help).hash(&mut hasher);
    normalize_identity_component(&placeholder).hash(&mut hasher);
    normalize_identity_component(&role_path).hash(&mut hasher);
    kind.hash(&mut hasher);

    format!(
        "{}:{}:{}:{:016x}",
        normalize_identity_component(app_name),
        normalize_identity_component(&role),
        normalize_identity_component(&identifier),
        hasher.finish(),
    )
}

fn anchor_role_path(element: &AXUIElement) -> String {
    let mut parts = Vec::new();
    let role = best_effort_string_attribute(element, "AXRole").unwrap_or_default();
    let subrole = best_effort_string_attribute(element, "AXSubrole").unwrap_or_default();
    if !role.is_empty() || !subrole.is_empty() {
        parts.push(format!("{role}:{subrole}"));
    }

    let mut current = best_effort_element_attribute(element, "AXParent");
    let mut depth = 1;
    while let Some(node) = current {
        if depth >= ANCHOR_ANCESTOR_LIMIT {
            break;
        }
        let role = best_effort_string_attribute(&node, "AXRole").unwrap_or_default();
        let subrole = best_effort_string_attribute(&node, "AXSubrole").unwrap_or_default();
        if !role.is_empty() || !subrole.is_empty() {
            parts.push(format!("{role}:{subrole}"));
        }
        current = best_effort_element_attribute(&node, "AXParent");
        depth += 1;
    }

    parts.join(">")
}

fn normalize_identity_component(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn normalize_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_secure_role(role: &str) -> bool {
    role.to_ascii_lowercase().contains("secure")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnchorExcerpt {
    text: String,
    prefix: Option<String>,
    suffix: Option<String>,
}

fn derive_anchor_excerpt(
    full_text: &str,
    selected_text: Option<&str>,
    prefer_tail: bool,
) -> AnchorExcerpt {
    let full_text = normalize_text(full_text);
    let selected_text = selected_text
        .map(normalize_text)
        .filter(|value| !value.is_empty());

    if let Some(selected_text) = selected_text {
        let (prefix, suffix) = surrounding_context(&full_text, &selected_text);
        return AnchorExcerpt {
            text: truncate_chars(&selected_text, ANCHOR_TEXT_LIMIT),
            prefix,
            suffix,
        };
    }

    if full_text.is_empty() {
        return AnchorExcerpt {
            text: String::new(),
            prefix: None,
            suffix: None,
        };
    }

    let lines = full_text.lines().collect::<Vec<_>>();
    let candidate = if prefer_tail {
        lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .copied()
            .unwrap_or(full_text.as_str())
    } else {
        lines
            .iter()
            .find(|line| !line.trim().is_empty())
            .copied()
            .unwrap_or(full_text.as_str())
    };

    if candidate.chars().count() <= ANCHOR_TEXT_LIMIT {
        let (prefix, suffix) = surrounding_context(&full_text, candidate);
        return AnchorExcerpt {
            text: candidate.to_string(),
            prefix,
            suffix,
        };
    }

    if prefer_tail {
        let tail = take_tail_chars(&full_text, ANCHOR_TEXT_LIMIT);
        let prefix = Some(take_tail_chars(
            &full_text,
            ANCHOR_TEXT_LIMIT + ANCHOR_CONTEXT_LIMIT,
        ))
        .map(|value| take_head_chars(&value, ANCHOR_CONTEXT_LIMIT))
        .map(|value| normalize_text(&value))
        .filter(|value| !value.is_empty());
        AnchorExcerpt {
            text: tail,
            prefix,
            suffix: None,
        }
    } else {
        let head = take_head_chars(&full_text, ANCHOR_TEXT_LIMIT);
        let suffix = Some(take_head_chars(
            &skip_head_chars(&full_text, ANCHOR_TEXT_LIMIT),
            ANCHOR_CONTEXT_LIMIT,
        ))
        .map(|value| normalize_text(&value))
        .filter(|value| !value.is_empty());
        AnchorExcerpt {
            text: head,
            prefix: None,
            suffix,
        }
    }
}

fn surrounding_context(full_text: &str, needle: &str) -> (Option<String>, Option<String>) {
    let Some(byte_index) = full_text.find(needle) else {
        return (None, None);
    };

    let prefix = take_tail_chars(&full_text[..byte_index], ANCHOR_CONTEXT_LIMIT);
    let suffix = take_head_chars(
        &full_text[byte_index + needle.len()..],
        ANCHOR_CONTEXT_LIMIT,
    );

    (
        (!prefix.is_empty()).then_some(normalize_text(&prefix)),
        (!suffix.is_empty()).then_some(normalize_text(&suffix)),
    )
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_string()
    } else {
        value.chars().take(limit).collect()
    }
}

fn take_head_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn take_tail_chars(value: &str, limit: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(limit);
    chars[start..].iter().collect()
}

fn skip_head_chars(value: &str, count: usize) -> String {
    value.chars().skip(count).collect()
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
    use hypr_activity_capture_interface::{TextAnchorConfidence, TextAnchorKind};

    use super::{
        AnchorExcerpt, TextAnchorCapture, derive_anchor_excerpt, merge_fragments, normalize_text,
    };

    #[test]
    fn merge_fragments_deduplicates_trimmed_lines() {
        assert_eq!(
            merge_fragments([" hello\nworld ".to_string(), "world\nanother".to_string()]),
            "hello\nworld\nanother"
        );
    }

    #[test]
    fn derive_anchor_excerpt_prefers_trailing_line_for_focused_edit_text() {
        assert_eq!(
            derive_anchor_excerpt("메일 본문\nsure!", None, true),
            AnchorExcerpt {
                text: "sure!".to_string(),
                prefix: Some("메일 본문\n".trim().to_string()),
                suffix: None,
            }
        );
    }

    #[test]
    fn derive_anchor_excerpt_uses_selected_text_when_present() {
        let excerpt = derive_anchor_excerpt("before selected after", Some("selected"), true);

        assert_eq!(excerpt.text, "selected");
        assert_eq!(excerpt.prefix.as_deref(), Some("before"));
        assert_eq!(excerpt.suffix.as_deref(), Some("after"));
    }

    #[test]
    fn derive_anchor_excerpt_uses_head_for_document_context() {
        let excerpt = derive_anchor_excerpt("Hacker News\nnew | threads | past", None, false);

        assert_eq!(excerpt.text, "Hacker News");
        assert_eq!(excerpt.suffix.as_deref(), Some("new | threads | past"));
    }

    #[test]
    fn normalize_text_removes_empty_lines_but_preserves_short_fragments() {
        assert_eq!(normalize_text(" \nsu \n\n sure! "), "su\nsure!");
    }

    #[test]
    fn anchor_capture_can_represent_high_confidence_edit_anchor() {
        let capture = TextAnchorCapture {
            kind: TextAnchorKind::FocusedEdit,
            identity: "app:window:role".to_string(),
            text: Some("sure!".to_string()),
            prefix: Some("메일 본문".to_string()),
            suffix: None,
            selected_text: None,
            confidence: TextAnchorConfidence::High,
        };

        assert_eq!(capture.kind, TextAnchorKind::FocusedEdit);
        assert_eq!(capture.confidence, TextAnchorConfidence::High);
        assert_eq!(capture.text.as_deref(), Some("sure!"));
    }
}
