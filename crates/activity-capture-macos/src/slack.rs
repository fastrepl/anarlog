#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::CaptureError;
use objc2_application_services::AXUIElement;
use objc2_core_foundation::CFRetained;

use crate::ax::{child_elements, merge_fragments, string_attribute};

const SEARCH_DEPTH_LIMIT: usize = 14;
const TEXT_DEPTH_LIMIT: usize = 12;
const NODE_LIMIT: usize = 240;
const CHARACTER_LIMIT: usize = 4000;
const ATTRIBUTE_TEXT_LIMIT: usize = 600;
const SEARCH_CHILD_LIMIT: usize = 24;
const MIN_CONTENT_SCORE: usize = 8;

const TEXT_ATTRIBUTES: [&str; 3] = ["AXValue", "AXTitle", "AXDescription"];
const BLOCKED_ROLES: [&str; 11] = [
    "AXButton",
    "AXCheckBox",
    "AXImage",
    "AXMenu",
    "AXMenuBar",
    "AXPopUpButton",
    "AXRadioButton",
    "AXSlider",
    "AXTab",
    "AXTabGroup",
    "AXToolbar",
];
const CONTENT_CANDIDATE_ROLES: [&str; 6] = [
    "AXDocument",
    "AXGroup",
    "AXLayoutArea",
    "AXList",
    "AXScrollArea",
    "AXTable",
];

struct ContentSelection {
    total_score: usize,
    best: Option<ScoredElement>,
}

struct ScoredElement {
    element: CFRetained<AXUIElement>,
    score: usize,
    depth: usize,
}

pub(crate) fn collect_visible_text(focused_window: &AXUIElement) -> Result<String, CaptureError> {
    let Some(web_area) = find_first_descendant(focused_window, 0, |element| {
        matches_role(element, "AXWebArea")
    })?
    else {
        return Ok(String::new());
    };

    let content_root = find_content_root(web_area.as_ref())?;
    let content_root = content_root
        .as_deref()
        .map(|value| value as &AXUIElement)
        .unwrap_or_else(|| web_area.as_ref());

    collect_content_text(content_root, 0, NODE_LIMIT, CHARACTER_LIMIT)
}

fn find_first_descendant<F>(
    element: &AXUIElement,
    depth: usize,
    predicate: F,
) -> Result<Option<CFRetained<AXUIElement>>, CaptureError>
where
    F: Copy + Fn(&AXUIElement) -> bool,
{
    if depth >= SEARCH_DEPTH_LIMIT {
        return Ok(None);
    }

    for child in child_elements(element)? {
        if predicate(child.as_ref()) {
            return Ok(Some(child));
        }
        if let Some(found) = find_first_descendant(child.as_ref(), depth + 1, predicate)? {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

fn find_content_root(
    web_area: &AXUIElement,
) -> Result<Option<CFRetained<AXUIElement>>, CaptureError> {
    let children = child_elements(web_area)?;
    let child_budget = (NODE_LIMIT / children.len().max(1)).max(1);
    let mut best = None;

    for child in children
        .into_iter()
        .take(NODE_LIMIT.min(SEARCH_CHILD_LIMIT))
    {
        let selection = score_content_subtree(child, 1, child_budget)?;
        best = choose_better_candidate(best, selection.best);
    }

    Ok(best.map(|value| value.element))
}

fn collect_content_text(
    element: &AXUIElement,
    depth: usize,
    remaining_nodes: usize,
    remaining_characters: usize,
) -> Result<String, CaptureError> {
    if depth >= TEXT_DEPTH_LIMIT || remaining_nodes == 0 || remaining_characters == 0 {
        return Ok(String::new());
    }

    if should_skip_subtree(element)? {
        return Ok(String::new());
    }

    let role = string_attribute(element, "AXRole")?.unwrap_or_default();
    let mut pieces = Vec::new();
    if !BLOCKED_ROLES.contains(&role.as_str()) {
        for attribute in TEXT_ATTRIBUTES {
            let Some(text) = string_attribute(element, attribute)? else {
                continue;
            };
            if let Some(text) = sanitize_text(&text) {
                pieces.push(text);
            }
        }
    }

    let children = child_elements(element)?;
    let child_budget = (remaining_nodes / children.len().max(1)).max(1);
    let char_budget = (remaining_characters / (children.len() + 1).max(1)).max(80);

    for child in children.into_iter().take(remaining_nodes.min(32)) {
        let child_text = collect_content_text(&child, depth + 1, child_budget, char_budget)?;
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

fn score_content_subtree(
    element: CFRetained<AXUIElement>,
    depth: usize,
    remaining_nodes: usize,
) -> Result<ContentSelection, CaptureError> {
    if depth >= SEARCH_DEPTH_LIMIT || remaining_nodes == 0 {
        return Ok(ContentSelection {
            total_score: 0,
            best: None,
        });
    }
    if should_skip_subtree(element.as_ref())? {
        return Ok(ContentSelection {
            total_score: 0,
            best: None,
        });
    }

    let mut selection = ContentSelection {
        total_score: element_text_score(element.as_ref())?,
        best: None,
    };
    let children = child_elements(element.as_ref())?;
    let child_budget = (remaining_nodes.saturating_sub(1) / children.len().max(1)).max(1);

    for child in children
        .into_iter()
        .take(remaining_nodes.min(SEARCH_CHILD_LIMIT))
    {
        let child_selection = score_content_subtree(child, depth + 1, child_budget)?;
        selection.total_score += child_selection.total_score;
        selection.best = choose_better_candidate(selection.best, child_selection.best);
    }

    if is_content_candidate(element.as_ref())
        && selection.total_score >= MIN_CONTENT_SCORE
        && should_prefer_current_candidate(selection.total_score, selection.best.as_ref())
    {
        selection.best = Some(ScoredElement {
            element,
            score: selection.total_score,
            depth,
        });
    }

    Ok(selection)
}

fn element_text_score(element: &AXUIElement) -> Result<usize, CaptureError> {
    let role = string_attribute(element, "AXRole")?.unwrap_or_default();
    if BLOCKED_ROLES.contains(&role.as_str()) {
        return Ok(0);
    }

    let mut score = 0;
    for attribute in TEXT_ATTRIBUTES {
        let Some(text) = string_attribute(element, attribute)? else {
            continue;
        };
        let Some(text) = sanitize_text(&text) else {
            continue;
        };
        score += text_weight(&text);
    }

    Ok(score)
}

fn text_weight(text: &str) -> usize {
    let length = text.chars().count();
    if length >= 80 {
        6
    } else if length >= 24 {
        4
    } else if length >= 8 {
        2
    } else {
        1
    }
}

fn matches_role(element: &AXUIElement, expected: &str) -> bool {
    string_attribute(element, "AXRole")
        .ok()
        .flatten()
        .as_deref()
        == Some(expected)
}

fn is_content_candidate(element: &AXUIElement) -> bool {
    let role = string_attribute(element, "AXRole")
        .ok()
        .flatten()
        .unwrap_or_default();
    CONTENT_CANDIDATE_ROLES.contains(&role.as_str())
}

fn should_skip_subtree(element: &AXUIElement) -> Result<bool, CaptureError> {
    let role = string_attribute(element, "AXRole")?.unwrap_or_default();
    if BLOCKED_ROLES.contains(&role.as_str()) {
        return Ok(true);
    }

    let identifier = string_attribute(element, "AXIdentifier")?.unwrap_or_default();
    Ok(identifier.eq_ignore_ascii_case("composer"))
}

fn sanitize_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().count() > ATTRIBUTE_TEXT_LIMIT {
        return Some(trimmed.chars().take(ATTRIBUTE_TEXT_LIMIT).collect());
    }

    Some(trimmed.to_string())
}

fn choose_better_candidate(
    current: Option<ScoredElement>,
    next: Option<ScoredElement>,
) -> Option<ScoredElement> {
    match (current, next) {
        (None, value) | (value, None) => value,
        (Some(current), Some(next)) => {
            if next.score > current.score
                || (next.score == current.score && next.depth < current.depth)
            {
                Some(next)
            } else {
                Some(current)
            }
        }
    }
}

fn should_prefer_current_candidate(
    current_score: usize,
    best_child: Option<&ScoredElement>,
) -> bool {
    let Some(best_child) = best_child else {
        return true;
    };

    current_score.saturating_mul(4) >= best_child.score.saturating_mul(5)
}

#[cfg(test)]
mod tests {
    use super::{ATTRIBUTE_TEXT_LIMIT, sanitize_text, text_weight};

    #[test]
    fn sanitize_text_trims_and_truncates_unicode_safely() {
        assert_eq!(
            sanitize_text("  hello world  "),
            Some("hello world".to_string())
        );
        assert_eq!(
            sanitize_text(&"界".repeat(ATTRIBUTE_TEXT_LIMIT + 5)),
            Some("界".repeat(ATTRIBUTE_TEXT_LIMIT))
        );
    }

    #[test]
    fn text_weight_prefers_longer_content() {
        assert!(text_weight("short") < text_weight("this is a longer line of message text"));
    }
}
