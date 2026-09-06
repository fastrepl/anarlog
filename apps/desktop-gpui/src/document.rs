//! Read model for TipTap-dialect ProseMirror JSON, the format
//! `session_documents.body` is stored in. Mirrors the node and mark set that
//! `crates/tiptap` understands so both shells agree on what a document means.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub underline: bool,
    pub link: Option<String>,
    /// A `mention-@` chip: `(type, id, label)`; `text` is its display text.
    pub mention: Option<(String, String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// `Some` for task items.
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Span>),
    Heading { level: u8, spans: Vec<Span> },
    List { ordered: bool, items: Vec<ListItem> },
    Blockquote(Vec<Block>),
    Code(String),
    HorizontalRule,
    Image { alt: String },
}

/// Parses a stored body. Markdown bodies go through the same converter the
/// app uses so they render identically to ProseMirror ones.
pub fn from_body(body_format: &str, body: &str) -> Vec<Block> {
    if body.trim().is_empty() {
        return Vec::new();
    }
    let json = match body_format {
        "markdown" => anlg_tiptap::md_to_tiptap_json(body).ok(),
        _ => serde_json::from_str::<Value>(body)
            .ok()
            .or_else(|| anlg_tiptap::md_to_tiptap_json(body).ok()),
    };
    match json {
        Some(json) => parse(&json),
        // Unparseable bodies still need to be readable.
        None => vec![Block::Paragraph(vec![Span {
            text: body.to_string(),
            ..Span::default()
        }])],
    }
}

pub fn parse(doc: &Value) -> Vec<Block> {
    children(doc).iter().filter_map(block).collect()
}

fn children(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn attr<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("attrs").and_then(|attrs| attrs.get(name))
}

fn block(node: &Value) -> Option<Block> {
    match node.get("type").and_then(Value::as_str)? {
        "paragraph" => Some(Block::Paragraph(inline(node))),
        "heading" => Some(Block::Heading {
            level: attr(node, "level")
                .and_then(Value::as_u64)
                .map_or(1, |level| level.clamp(1, 6) as u8),
            spans: inline(node),
        }),
        "bulletList" => Some(Block::List {
            ordered: false,
            items: children(node).iter().map(list_item).collect(),
        }),
        "orderedList" => Some(Block::List {
            ordered: true,
            items: children(node).iter().map(list_item).collect(),
        }),
        "taskList" => Some(Block::List {
            ordered: false,
            items: children(node).iter().map(list_item).collect(),
        }),
        "blockquote" => Some(Block::Blockquote(
            children(node).iter().filter_map(block).collect(),
        )),
        "codeBlock" => Some(Block::Code(
            inline(node).into_iter().map(|span| span.text).collect(),
        )),
        "horizontalRule" => Some(Block::HorizontalRule),
        "image" => Some(Block::Image {
            alt: attr(node, "alt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        // Stray inline content at block level still has to show up somewhere.
        "text" | "hardBreak" => Some(Block::Paragraph(inline_nodes(std::slice::from_ref(node)))),
        _ => None,
    }
}

fn list_item(node: &Value) -> ListItem {
    let checked = match node.get("type").and_then(Value::as_str) {
        Some("taskItem") => Some(
            attr(node, "checked")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        _ => None,
    };
    ListItem {
        checked,
        blocks: children(node).iter().filter_map(block).collect(),
    }
}

fn inline(node: &Value) -> Vec<Span> {
    inline_nodes(children(node))
}

fn inline_nodes(nodes: &[Value]) -> Vec<Span> {
    let mut spans = Vec::new();
    for node in nodes {
        match node.get("type").and_then(Value::as_str) {
            Some("text") => {
                let Some(text) = node.get("text").and_then(Value::as_str) else {
                    continue;
                };
                let mut span = Span {
                    text: text.to_string(),
                    ..Span::default()
                };
                for mark in node
                    .get("marks")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                {
                    match mark.get("type").and_then(Value::as_str) {
                        Some("bold" | "strong") => span.bold = true,
                        Some("italic" | "em") => span.italic = true,
                        Some("strike") => span.strike = true,
                        Some("code") => span.code = true,
                        Some("underline") => span.underline = true,
                        Some("link") => {
                            span.link = attr(mark, "href")
                                .and_then(Value::as_str)
                                .map(str::to_string);
                        }
                        _ => {}
                    }
                }
                spans.push(span);
            }
            Some("hardBreak") => spans.push(Span {
                text: "\n".to_string(),
                ..Span::default()
            }),
            Some(kind) if kind.starts_with("mention-") => {
                let label = attr(node, "label")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let kind = attr(node, "type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let id = attr(node, "id").and_then(Value::as_str).unwrap_or_default();
                spans.push(Span {
                    text: crate::mention::display_text(label),
                    mention: Some((kind.to_string(), id.to_string(), label.to_string())),
                    ..Span::default()
                });
            }
            _ => {}
        }
    }
    spans
}

/// Plain text of a span list.
#[cfg(test)]
pub fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Value {
        serde_json::json!({ "type": "text", "text": t })
    }

    #[test]
    fn parses_the_onboarding_note_shapes() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "heading", "attrs": { "level": 2 }, "content": [text("Agenda")] },
                { "type": "paragraph", "content": [
                    text("Click "),
                    { "type": "text", "marks": [{ "type": "bold" }], "text": "Join & record" },
                    text(" in "),
                    { "type": "text", "marks": [{ "type": "link", "attrs": { "href": "https://anarlog.so" } }, { "type": "italic" }], "text": "Settings" }
                ]},
                { "type": "paragraph" },
                { "type": "bulletList", "content": [
                    { "type": "listItem", "content": [{ "type": "paragraph", "content": [text("First")] }] }
                ]},
                { "type": "taskList", "content": [
                    { "type": "taskItem", "attrs": { "checked": true }, "content": [{ "type": "paragraph", "content": [text("Done")] }] }
                ]},
                { "type": "codeBlock", "content": [text("let x = 1;")] },
                { "type": "blockquote", "content": [{ "type": "paragraph", "content": [text("Quote")] }] },
                { "type": "horizontalRule" },
                { "type": "image", "attrs": { "src": "x.png", "alt": "Diagram" } },
                { "type": "paragraph", "content": [
                    { "type": "mention-human", "attrs": { "id": "h1", "label": "Ada" } },
                    { "type": "hardBreak" },
                    text("after")
                ]}
            ]
        });

        let blocks = parse(&doc);
        assert_eq!(blocks.len(), 10);
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 2,
                spans: vec![Span {
                    text: "Agenda".into(),
                    ..Span::default()
                }]
            }
        );
        let Block::Paragraph(spans) = &blocks[1] else {
            panic!("expected paragraph");
        };
        assert_eq!(plain_text(spans), "Click Join & record in Settings");
        assert!(spans[1].bold && !spans[0].bold);
        assert!(spans[3].italic);
        assert_eq!(spans[3].link.as_deref(), Some("https://anarlog.so"));
        assert_eq!(blocks[2], Block::Paragraph(vec![]));
        assert!(
            matches!(&blocks[3], Block::List { ordered: false, items } if items[0].checked.is_none())
        );
        assert!(matches!(&blocks[4], Block::List { items, .. } if items[0].checked == Some(true)));
        assert_eq!(blocks[5], Block::Code("let x = 1;".into()));
        assert!(matches!(&blocks[6], Block::Blockquote(inner) if inner.len() == 1));
        assert_eq!(blocks[7], Block::HorizontalRule);
        assert_eq!(
            blocks[8],
            Block::Image {
                alt: "Diagram".into()
            }
        );
        let Block::Paragraph(spans) = &blocks[9] else {
            panic!("expected paragraph");
        };
        assert_eq!(
            plain_text(spans),
            format!("{}\nafter", crate::mention::display_text("Ada"))
        );
    }

    #[test]
    fn markdown_bodies_use_the_shared_converter() {
        let blocks = from_body("markdown", "## Title\n\n- item **bold**\n");
        assert!(matches!(&blocks[0], Block::Heading { level: 2, .. }));
        let Block::List { items, .. } = &blocks[1] else {
            panic!("expected list, got {blocks:?}");
        };
        let Block::Paragraph(spans) = &items[0].blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(plain_text(spans), "item bold");
        assert!(spans.iter().any(|span| span.bold && span.text == "bold"));
        assert!(from_body("prosemirror_json", "  ").is_empty());
    }
}
