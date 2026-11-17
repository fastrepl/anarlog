use serde_json::{Value, json};

use super::{
    document::{Document, EditResult, NodeRef, inline_text, split_inline},
    model::Selection,
    sequence::Measured,
};

pub struct CopyPayload {
    pub text: String,
    pub metadata: String,
}

pub fn copy(document: &Document, selection: Selection) -> EditResult<CopyPayload> {
    let range = selection.range();
    if range.start == 0 && range.end == document.units() {
        let mut blocks = Vec::new();
        document
            .root
            .children
            .visit(&mut |node| blocks.push(node.clone()));
        return payload(blocks, 0);
    }
    let from = document.resolve(range.start)?;
    let to = document.resolve(range.end)?;
    let mut blocks = Vec::new();
    if from.path == to.path {
        let (through, _) = split_inline(&from.node.children, to.offset)?;
        let (_, selected) = split_inline(&through, from.offset)?;
        blocks.push(from.node.with_children(selected));
    } else {
        super::transform::copy_range(document, range)?.visit(&mut |node| blocks.push(node.clone()));
    }
    payload(blocks, 1)
}

fn payload(blocks: Vec<NodeRef>, open: u8) -> EditResult<CopyPayload> {
    let text = blocks
        .iter()
        .map(plain_block)
        .collect::<Vec<_>>()
        .join("\n\n");
    let metadata = json!({
        "anarlog-native-slice": 1,
        "openStart": open, "openEnd": open,
        "content": blocks.iter().map(|block| block.value()).collect::<Vec<_>>(),
    })
    .to_string();
    Ok(CopyPayload { text, metadata })
}

pub fn parse_slice(metadata: &str) -> EditResult<(Document, bool)> {
    if metadata.trim_start().starts_with('<') {
        return super::html::parse(metadata);
    }
    if metadata.len() > super::document::MAX_BYTES {
        return Err("Clipboard slice exceeds 16 MiB".into());
    }
    let value: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    if value["anarlog-native-slice"] != 1 {
        return Err("Unsupported rich clipboard format. Use explicit plain-text paste.".into());
    }
    let open = value["openStart"] == 1 && value["openEnd"] == 1;
    let document = Document::parse(
        json!({"type":"doc", "content": value["content"]})
            .to_string()
            .into(),
    )?;
    Ok((document, open))
}

fn escape_image(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn plain_block(node: &NodeRef) -> String {
    if node.kind() == "image" {
        let alt = node.attr("alt").and_then(Value::as_str).unwrap_or("");
        let src = node.attr("src").and_then(Value::as_str).unwrap_or("");
        return format!("![{}]({})", escape_image(alt), escape_image(src));
    }
    if node.is_textblock() {
        let mut result = String::new();
        node.children.visit(&mut |child| {
            if child.text.is_some() || child.kind() == "hardBreak" {
                result.push_str(&inline_text(&super::sequence::Sequence::one(child.clone())));
            } else {
                result.push_str(
                    child
                        .attr("label")
                        .or_else(|| child.attr("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("\u{fffc}"),
                );
            }
        });
        return result;
    }
    let mut children = Vec::new();
    node.children
        .visit(&mut |child| children.push(plain_block(child)));
    children.join("\n\n")
}

pub fn text_for_range(document: &Document, range: std::ops::Range<usize>) -> EditResult<String> {
    if range.len() > 65_536 {
        return Err("Native text request exceeds the 64K window".into());
    }
    fn append(
        node: &NodeRef,
        start: usize,
        range: &std::ops::Range<usize>,
        output: &mut String,
    ) -> EditResult<()> {
        if range.start >= start + node.units() || range.end <= start {
            return Ok(());
        }
        if let Some(text) = &node.text {
            output.push_str(
                &text
                    .slice(
                        range.start.saturating_sub(start)..(range.end - start).min(text.units()),
                    )?
                    .as_string(),
            );
        } else if node.is_atom() {
            output.push(if node.kind() == "hardBreak" {
                '\n'
            } else {
                '\u{fffc}'
            });
        } else {
            let pad = usize::from(node.kind() != "doc");
            if pad == 1 && range.contains(&start) {
                output.push('\n');
            }
            let content_start = start + pad;
            if let Some((mut index, _, _)) = node
                .children
                .locate(range.start.saturating_sub(content_start))
            {
                while let Some(child) = node.children.get(index) {
                    let child_start = content_start + node.children.prefix(index);
                    if child_start >= range.end {
                        break;
                    }
                    append(child, child_start, range, output)?;
                    index += 1;
                }
            }
            if pad == 1 && range.contains(&(start + node.units() - 1)) {
                output.push('\n');
            }
        }
        Ok(())
    }
    if range.start > range.end || range.end > document.units() {
        return Err("Native range is outside document".into());
    }
    let mut output = String::new();
    append(&document.root, 0, &range, &mut output)?;
    Ok(output)
}

pub fn openable_link(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !href.chars().any(char::is_control)
        && href
            .split_once("://")
            .is_some_and(|(_, tail)| !tail.is_empty())
}
