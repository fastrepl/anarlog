//! Shipping's first-line session title for summaries (`session/title-content.ts`): the summary
//! editor shows the note title as its first H1, and editing that line renames the note.

use serde_json::{Value, json};

use super::{document::NodeRef, sequence::Sequence};

fn text(node: &Value) -> String {
    node["text"].as_str().unwrap_or_default().to_owned()
        + &node["content"]
            .as_array()
            .map(|children| children.iter().map(text).collect::<String>())
            .unwrap_or_default()
}

fn is_title_heading(node: &Value) -> bool {
    node["type"] == "heading" && node["attrs"]["level"] == 1
}

/// `ensureFirstLineTitle`: the body with `title` as its first H1, or `None` when the stored
/// body already starts with it and needs no change.
pub fn ensure_first_line_title(body: &str, title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    let mut document: Value = if body.trim().is_empty() {
        json!({"type": "doc", "content": []})
    } else {
        serde_json::from_str(body).ok()?
    };
    let blocks = document["content"].as_array_mut()?;
    let heading = json!({
        "type": "heading",
        "attrs": {"level": 1},
        "content": [{"type": "text", "text": title}],
    });
    match blocks.first() {
        Some(first) if is_title_heading(first) && text(first).trim() == title => return None,
        Some(first)
            if (first["type"] == "paragraph" && text(first).trim() == title)
                || (is_title_heading(first) && text(first).trim().is_empty()) =>
        {
            blocks[0] = heading
        }
        _ => blocks.insert(0, heading),
    }
    Some(document.to_string())
}

/// `extractFirstLineTitle`: the first block's text; `""` when it is blank but the document has
/// text, `None` when the document has none.
pub fn first_line_title(body: &str) -> Option<String> {
    let document: Value = serde_json::from_str(body).ok()?;
    let title = document["content"]
        .get(0)
        .map(text)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if !title.is_empty() {
        Some(title)
    } else if text(&document).trim().is_empty() {
        None
    } else {
        Some(String::new())
    }
}

/// Shipping's `isPristineNoteDoc`: no blocks, or one paragraph without content. Checked on
/// the editor's nodes every frame, so it never serializes the document.
pub(super) fn is_pristine(blocks: &Sequence<NodeRef>) -> bool {
    match blocks.len() {
        0 => true,
        1 => blocks
            .get(0)
            .is_some_and(|block| block.kind() == "paragraph" && block.children.len() == 0),
        _ => false,
    }
}

/// Shipping's `handleApplyTemplate` content: titled sections as H2s, each over an empty
/// paragraph; `None` when no section has a title.
pub fn template_body(sections: &[&str]) -> Option<Value> {
    let content: Vec<Value> = sections
        .iter()
        .map(|title| title.trim())
        .filter(|title| !title.is_empty())
        .flat_map(|title| {
            [
                json!({"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": title}]}),
                json!({"type": "paragraph"}),
            ]
        })
        .collect();
    (!content.is_empty()).then(|| json!({"type": "doc", "content": content}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_gain_the_title_line_only_when_missing() {
        let titled = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Sync"}]}]}"#;
        assert_eq!(ensure_first_line_title(titled, "Sync"), None);
        let body = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Decisions"}]}]}"#;
        let ensured: Value =
            serde_json::from_str(&ensure_first_line_title(body, "Sync").unwrap()).unwrap();
        assert_eq!(text(&ensured["content"][0]), "Sync");
        assert_eq!(text(&ensured["content"][1]), "Decisions");
        let empty = ensure_first_line_title("", "Sync").unwrap();
        assert_eq!(first_line_title(&empty).as_deref(), Some("Sync"));
        assert_eq!(ensure_first_line_title(body, "  "), None);
    }

    #[test]
    fn templates_fill_only_pristine_memos_with_their_sections() {
        let pristine = |body: Value| {
            let document =
                super::super::document::Document::parse(body.to_string().into()).unwrap();
            is_pristine(&document.root.children)
        };
        assert!(pristine(
            json!({"type": "doc", "content": [{"type": "paragraph"}]})
        ));
        assert!(!pristine(
            json!({"type": "doc", "content": [{"type": "paragraph"}, {"type": "paragraph"}]})
        ));
        assert!(!pristine(
            json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "x"}]}]})
        ));
        let body = template_body(&["Goals", " ", "Risks"]).unwrap();
        assert_eq!(body["content"].as_array().unwrap().len(), 4);
        assert_eq!(text(&body["content"][2]), "Risks");
        assert_eq!(template_body(&["", "  "]), None);
    }

    #[test]
    fn the_first_line_names_the_note() {
        let body = |first: &str| {
            format!(
                r#"{{"type":"doc","content":[{{"type":"heading","attrs":{{"level":1}},"content":[{first}]}},{{"type":"paragraph","content":[{{"type":"text","text":"Body"}}]}}]}}"#
            )
        };
        assert_eq!(
            first_line_title(&body(r#"{"type":"text","text":" Renamed "}"#)).as_deref(),
            Some("Renamed")
        );
        assert_eq!(first_line_title(&body("")).as_deref(), Some(""));
        assert_eq!(
            first_line_title(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#),
            None
        );
    }
}
