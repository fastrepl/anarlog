use serde_json::{Value, json};

/// Notion caps children at 100 blocks per append request; leave room for the
/// heading block prepended by the update route.
pub(crate) const MAX_CONTENT_BLOCKS: usize = 90;
const MAX_RICH_TEXT_CHARS: usize = 2000;

pub(crate) fn heading_block(text: &str) -> Value {
    json!({
        "object": "block",
        "type": "heading_3",
        "heading_3": { "rich_text": rich_text(text) }
    })
}

/// Line-based markdown-to-blocks conversion. Notion has no markdown import
/// API, so headings, list items, and to-dos map to their block types and
/// everything else becomes paragraphs; inline formatting stays literal.
pub(crate) fn markdown_to_blocks(markdown: &str) -> Vec<Value> {
    let mut blocks = Vec::new();
    for line in markdown.lines() {
        if blocks.len() >= MAX_CONTENT_BLOCKS {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let block = if let Some(text) = trimmed.strip_prefix("### ") {
            typed_block("heading_3", text, None)
        } else if let Some(text) = trimmed.strip_prefix("## ") {
            typed_block("heading_2", text, None)
        } else if let Some(text) = trimmed.strip_prefix("# ") {
            typed_block("heading_1", text, None)
        } else if let Some(text) = strip_task_prefix(trimmed, false) {
            typed_block("to_do", text, Some(false))
        } else if let Some(text) = strip_task_prefix(trimmed, true) {
            typed_block("to_do", text, Some(true))
        } else if let Some(text) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            typed_block("bulleted_list_item", text, None)
        } else if let Some(text) = strip_ordered_prefix(trimmed) {
            typed_block("numbered_list_item", text, None)
        } else {
            typed_block("paragraph", trimmed, None)
        };
        blocks.push(block);
    }
    blocks
}

fn typed_block(block_type: &str, text: &str, checked: Option<bool>) -> Value {
    let mut body = json!({ "rich_text": rich_text(text) });
    if let Some(checked) = checked {
        body["checked"] = json!(checked);
    }
    json!({
        "object": "block",
        "type": block_type,
        block_type: body
    })
}

fn rich_text(text: &str) -> Value {
    let truncated = if text.chars().count() > MAX_RICH_TEXT_CHARS {
        text.chars().take(MAX_RICH_TEXT_CHARS).collect::<String>()
    } else {
        text.to_string()
    };
    json!([{ "type": "text", "text": { "content": truncated } }])
}

fn strip_task_prefix(line: &str, checked: bool) -> Option<&str> {
    let prefixes: &[&str] = if checked {
        &["- [x] ", "- [X] ", "* [x] ", "* [X] "]
    } else {
        &["- [ ] ", "* [ ] "]
    };
    prefixes
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))
        .map(str::trim)
}

fn strip_ordered_prefix(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &line[digits..];
    rest.strip_prefix(". ").map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_markdown_lines_into_typed_blocks() {
        let blocks = markdown_to_blocks(
            "## Decisions\n\nShip it.\n- [ ] Follow up with legal\n- [x] Book the room\n- plain bullet\n1. first step",
        );

        let types: Vec<&str> = blocks
            .iter()
            .map(|block| block["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            vec![
                "heading_2",
                "paragraph",
                "to_do",
                "to_do",
                "bulleted_list_item",
                "numbered_list_item"
            ]
        );
        assert_eq!(blocks[2]["to_do"]["checked"], json!(false));
        assert_eq!(blocks[3]["to_do"]["checked"], json!(true));
        assert_eq!(
            blocks[5]["numbered_list_item"]["rich_text"][0]["text"]["content"],
            json!("first step")
        );
    }

    #[test]
    fn caps_block_count_and_rich_text_length() {
        let long_markdown = (0..200)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(markdown_to_blocks(&long_markdown).len(), MAX_CONTENT_BLOCKS);

        let long_line = "x".repeat(5000);
        let blocks = markdown_to_blocks(&long_line);
        let content = blocks[0]["paragraph"]["rich_text"][0]["text"]["content"]
            .as_str()
            .unwrap();
        assert_eq!(content.chars().count(), 2000);
    }
}
