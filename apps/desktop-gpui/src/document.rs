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
    /// The `highlight` mark (`<mark>`).
    pub highlight: bool,
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
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    List {
        ordered: bool,
        /// `orderedList`'s `start` (`counter-reset: ol-counter start-1`).
        start: u64,
        items: Vec<ListItem>,
    },
    Blockquote(Vec<Block>),
    Code(String),
    HorizontalRule,
    Image(Image),
    FileAttachment(FileAttachment),
    /// The `clip` embed: no view or styles in the desktop app, so only the
    /// block padding shows.
    Clip,
}

/// The `image` node's attributes (`ResizableImageView`): the attachment id
/// (a shared one takes precedence) resolves to a local file, `src` is the
/// stored fallback, and `editorWidth` is the percentage of the editor width.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Image {
    pub src: Option<String>,
    pub alt: String,
    pub title: Option<String>,
    pub attachment_id: Option<String>,
    pub editor_width: u8,
}

/// The `fileAttachment` node's attributes (`FileAttachmentView`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileAttachment {
    pub attachment_id: Option<String>,
    pub name: String,
    pub mime_type: String,
    pub src: Option<String>,
    pub path: Option<String>,
    pub size: Option<u64>,
}

pub const MIN_IMAGE_WIDTH: u8 = 15;
pub const MAX_IMAGE_WIDTH: u8 = 100;
pub const DEFAULT_IMAGE_WIDTH: u8 = 80;

/// `clampImageWidth`
pub fn clamp_image_width(value: Option<f64>) -> u8 {
    match value {
        Some(value) if value.is_finite() => {
            (value.round() as i64).clamp(MIN_IMAGE_WIDTH as i64, MAX_IMAGE_WIDTH as i64) as u8
        }
        _ => DEFAULT_IMAGE_WIDTH,
    }
}

/// `parseImageMetadata(title).title`: the `<img title>` the node view shows —
/// a stored `char-editor-width=N|caption` keeps the caption alone, a bare
/// `char-editor-width=N` has none, anything else is the title as stored.
pub fn image_title(title: &str) -> Option<String> {
    // `/^char-editor-width=(\d{1,3})(?:\|(.*))?$/s`
    let shown = match title.strip_prefix("char-editor-width=") {
        Some(rest) => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            match &rest[digits..] {
                _ if !(1..=3).contains(&digits) => title,
                "" => "",
                tail => tail.strip_prefix('|').unwrap_or(title),
            }
        }
        None => title,
    };
    // An empty `title` attribute shows no tooltip.
    (!shown.is_empty()).then(|| shown.to_string())
}

/// `md2json(markdown)` serialised: the ProseMirror parser's JSON, an empty
/// document becoming one empty paragraph.
pub fn md2json(markdown: &str) -> String {
    anlg_tiptap::md_to_tiptap_json(markdown)
        .ok()
        .filter(|doc| {
            doc.get("content")
                .and_then(|c| c.as_array())
                .is_some_and(|c| !c.is_empty())
        })
        .unwrap_or_else(
            || serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph" }] }),
        )
        .to_string()
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
            start: 1,
            items: children(node).iter().map(list_item).collect(),
        }),
        "orderedList" => Some(Block::List {
            ordered: true,
            start: attr(node, "start").and_then(Value::as_u64).unwrap_or(1),
            items: children(node).iter().map(list_item).collect(),
        }),
        "taskList" => Some(Block::List {
            ordered: false,
            start: 1,
            items: children(node).iter().map(list_item).collect(),
        }),
        "blockquote" => Some(Block::Blockquote(
            children(node).iter().filter_map(block).collect(),
        )),
        "codeBlock" => Some(Block::Code(
            inline(node).into_iter().map(|span| span.text).collect(),
        )),
        "horizontalRule" => Some(Block::HorizontalRule),
        "clip" => Some(Block::Clip),
        "image" => Some(Block::Image(Image {
            src: attr(node, "src")
                .and_then(Value::as_str)
                .map(str::to_string),
            alt: attr(node, "alt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            title: attr(node, "title")
                .and_then(Value::as_str)
                .map(str::to_string),
            attachment_id: attr(node, "sharedAttachmentId")
                .and_then(Value::as_str)
                .or_else(|| attr(node, "attachmentId").and_then(Value::as_str))
                .map(str::to_string),
            editor_width: clamp_image_width(attr(node, "editorWidth").and_then(Value::as_f64)),
        })),
        "fileAttachment" => Some(Block::FileAttachment(FileAttachment {
            attachment_id: attr(node, "sharedAttachmentId")
                .and_then(Value::as_str)
                .or_else(|| attr(node, "attachmentId").and_then(Value::as_str))
                .map(str::to_string),
            name: attr(node, "name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            mime_type: attr(node, "mimeType")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            src: attr(node, "src")
                .and_then(Value::as_str)
                .map(str::to_string),
            path: attr(node, "path")
                .and_then(Value::as_str)
                .map(str::to_string),
            size: attr(node, "size").and_then(Value::as_u64),
        })),
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
                        Some("highlight") => span.highlight = true,
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
    fn image_titles_follow_parse_image_metadata() {
        assert_eq!(
            image_title("Quarterly chart"),
            Some("Quarterly chart".into())
        );
        assert_eq!(image_title("char-editor-width=80"), None);
        assert_eq!(
            image_title("char-editor-width=80|Quarterly chart"),
            Some("Quarterly chart".into())
        );
        assert_eq!(image_title("char-editor-width=80|"), None);
        assert_eq!(
            image_title("char-editor-width=8000"),
            Some("char-editor-width=8000".into())
        );
        assert_eq!(
            image_title("char-editor-width=80x"),
            Some("char-editor-width=80x".into())
        );
        assert_eq!(image_title(""), None);
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
            matches!(&blocks[3], Block::List { ordered: false, items, .. } if items[0].checked.is_none())
        );
        assert!(matches!(&blocks[4], Block::List { items, .. } if items[0].checked == Some(true)));
        assert_eq!(blocks[5], Block::Code("let x = 1;".into()));
        assert!(matches!(&blocks[6], Block::Blockquote(inner) if inner.len() == 1));
        assert_eq!(blocks[7], Block::HorizontalRule);
        assert_eq!(
            blocks[8],
            Block::Image(Image {
                src: Some("x.png".into()),
                alt: "Diagram".into(),
                title: None,
                attachment_id: None,
                editor_width: DEFAULT_IMAGE_WIDTH,
            })
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

/// `ensureFirstLineTitle` + `titleHeadingPlugin` over parsed blocks: the
/// enhanced editor always opens on an h1 carrying the session title (an empty
/// one shows the `Untitled` placeholder).
pub fn with_title_heading(blocks: &[Block], title: &str) -> Vec<Block> {
    let title = title.trim();
    let title_block = Block::Heading {
        level: 1,
        spans: if title.is_empty() {
            Vec::new()
        } else {
            vec![Span {
                text: title.to_string(),
                ..Span::default()
            }]
        },
    };
    let block_text = |spans: &[Span]| {
        spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>()
    };
    match blocks.split_first() {
        Some((Block::Heading { level: 1, spans }, rest)) => {
            if !title.is_empty() && block_text(spans).trim().is_empty() {
                std::iter::once(title_block)
                    .chain(rest.iter().cloned())
                    .collect()
            } else {
                blocks.to_vec()
            }
        }
        Some((Block::Paragraph(spans), rest))
            if !title.is_empty() && block_text(spans).trim() == title =>
        {
            std::iter::once(title_block)
                .chain(rest.iter().cloned())
                .collect()
        }
        _ => std::iter::once(title_block)
            .chain(blocks.iter().cloned())
            .collect(),
    }
}

fn collect_text(node: &Value) -> String {
    let mut text = node
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
        for child in children {
            text.push_str(&collect_text(child));
        }
    }
    text
}

fn is_h1(node: &Value) -> bool {
    node.get("type").and_then(|t| t.as_str()) == Some("heading")
        && node
            .get("attrs")
            .and_then(|attrs| attrs.get("level"))
            .and_then(|level| level.as_u64())
            == Some(1)
}

/// `extractFirstLineTitle`: the first block's text, `Some("")` when only the
/// body has text, `None` for an empty document.
pub fn extract_first_line_title(content: &Value) -> Option<String> {
    let first = content
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|blocks| blocks.first());
    let title = first.map(collect_text).unwrap_or_default();
    let title = title.trim();
    if !title.is_empty() {
        return Some(title.to_string());
    }
    (!collect_text(content).trim().is_empty()).then(String::new)
}

/// `hasStoredNoteContent`: a stored body with visible text.
pub fn has_stored_note_content(body: &str) -> bool {
    if body.trim().is_empty() {
        return false;
    }
    match serde_json::from_str::<Value>(body) {
        Ok(json) => !collect_text(&json).trim().is_empty(),
        Err(_) => true,
    }
}

/// `isCanonicalEmptyDocument`: the title heading (matching the session title,
/// or empty without one) followed by one empty paragraph and nothing else.
pub fn is_canonical_empty_document(content: &Value, session_title: &str) -> bool {
    let blocks: Vec<Value> = content
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let [title, body] = blocks.as_slice() else {
        return false;
    };
    let expected = session_title.trim();
    let title_content: Vec<Value> = title
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let has_expected_title = if expected.is_empty() {
        title_content.is_empty()
    } else {
        title_content.len() == 1
            && title_content[0].get("type").and_then(|t| t.as_str()) == Some("text")
            && title_content[0].get("text").and_then(|t| t.as_str()) == Some(expected)
            && title_content[0]
                .get("marks")
                .and_then(|m| m.as_array())
                .is_none_or(|marks| marks.is_empty())
    };
    let title_attrs = title.get("attrs").and_then(|a| a.as_object());
    content.get("type").and_then(|t| t.as_str()) == Some("doc")
        && is_h1(title)
        && title_attrs.is_some_and(|attrs| attrs.len() == 1)
        && has_expected_title
        && body.get("type").and_then(|t| t.as_str()) == Some("paragraph")
        && body
            .get("attrs")
            .and_then(|a| a.as_object())
            .is_none_or(|attrs| attrs.is_empty())
        && body
            .get("content")
            .and_then(|c| c.as_array())
            .is_none_or(|children| children.is_empty())
}

/// `ensureFirstLineTitle` in `session/title-content.ts`: the document starts
/// with an h1 carrying the title, replacing a first paragraph or an empty h1
/// (or an h1 / paragraph that already holds the title's text).
pub fn ensure_first_line_title(mut content: Value, title: &str) -> Value {
    let title = title.trim();
    if title.is_empty() {
        return content;
    }
    let title_block = serde_json::json!({
        "type": "heading",
        "attrs": { "level": 1 },
        "content": [{ "type": "text", "text": title }]
    });
    let blocks: Vec<Value> = content
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let first = blocks.first();
    let is_paragraph =
        first.is_some_and(|node| node.get("type").and_then(|t| t.as_str()) == Some("paragraph"));
    let next = match first {
        Some(node) if (is_h1(node) || is_paragraph) && collect_text(node).trim() == title => {
            if is_h1(node) {
                return content;
            }
            std::iter::once(title_block)
                .chain(blocks[1..].iter().cloned())
                .collect::<Vec<_>>()
        }
        Some(node) if is_h1(node) && collect_text(node).trim().is_empty() => {
            std::iter::once(title_block)
                .chain(blocks[1..].iter().cloned())
                .collect::<Vec<_>>()
        }
        _ => std::iter::once(title_block)
            .chain(blocks.iter().cloned())
            .collect::<Vec<_>>(),
    };
    if let Some(object) = content.as_object_mut() {
        object.insert("content".to_string(), Value::Array(next));
    }
    content
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn enhanced_editor_helpers_follow_title_content() {
        let doc: Value = serde_json::from_str(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Sync"}]},{"type":"paragraph"}]}"#,
        )
        .unwrap();
        assert_eq!(extract_first_line_title(&doc), Some("Sync".into()));
        assert!(is_canonical_empty_document(&doc, "Sync"));
        assert!(!is_canonical_empty_document(&doc, "Other"));
        let empty: Value = serde_json::from_str(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}},{"type":"paragraph"}]}"#,
        )
        .unwrap();
        assert_eq!(extract_first_line_title(&empty), None);
        assert!(is_canonical_empty_document(&empty, ""));
        let body_only: Value = serde_json::from_str(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}},{"type":"paragraph","content":[{"type":"text","text":"x"}]}]}"#,
        )
        .unwrap();
        assert_eq!(extract_first_line_title(&body_only), Some(String::new()));
        assert!(!is_canonical_empty_document(&body_only, ""));
        assert!(has_stored_note_content(&body_only.to_string()));
        assert!(!has_stored_note_content(&empty.to_string()));
        assert!(!has_stored_note_content(""));
    }

    #[test]
    fn first_line_title_replaces_placeholders_and_prepends_otherwise() {
        let doc = |blocks: &str| -> Value {
            serde_json::from_str(&format!(r#"{{"type":"doc","content":[{blocks}]}}"#)).unwrap()
        };
        let h1 =
            r#"{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Sync"}]}"#;
        let para = r#"{"type":"paragraph","content":[{"type":"text","text":"Sync"}]}"#;
        let empty_h1 = r#"{"type":"heading","attrs":{"level":1}}"#;
        let body = r#"{"type":"paragraph","content":[{"type":"text","text":"body"}]}"#;
        assert_eq!(ensure_first_line_title(doc(h1), "Sync"), doc(h1));
        assert_eq!(
            ensure_first_line_title(doc(&format!("{para},{body}")), "Sync"),
            doc(&format!("{h1},{body}"))
        );
        assert_eq!(
            ensure_first_line_title(doc(&format!("{empty_h1},{body}")), "Sync"),
            doc(&format!("{h1},{body}"))
        );
        assert_eq!(
            ensure_first_line_title(doc(body), "Sync"),
            doc(&format!("{h1},{body}"))
        );
        assert_eq!(ensure_first_line_title(doc(body), "  "), doc(body));
    }
}
