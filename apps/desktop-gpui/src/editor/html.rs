use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde_json::{Map, Value, json};

use super::document::{Document, EditResult};

pub fn parse(html: &str) -> EditResult<(Document, bool)> {
    if html.len() > 1024 * 1024 {
        return Err("HTML clipboard exceeds 1 MiB".into());
    }
    let dom = parse_document(RcDom::default(), Default::default()).one(html);
    let mut budget = 50_000;
    let content = convert(&dom.document, &[], 0, &mut budget)?;
    let content = blocks(content);
    Document::parse(json!({"type":"doc","content":content}).to_string().into())
        .map(|document| (document, true))
}

fn blocks(content: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::new();
    let mut inline = Vec::new();
    for child in content {
        if matches!(
            child["type"].as_str(),
            Some("text" | "hardBreak" | "mention-@" | "appLink")
        ) {
            inline.push(child);
        } else {
            if !inline.is_empty() {
                result.push(json!({"type":"paragraph","content":std::mem::take(&mut inline)}));
            }
            result.push(child);
        }
    }
    if !inline.is_empty() || result.is_empty() {
        result.push(json!({"type":"paragraph","content":inline}));
    }
    result
}

fn convert(
    handle: &Handle,
    marks: &[Value],
    depth: usize,
    budget: &mut usize,
) -> EditResult<Vec<Value>> {
    if depth > 64 || *budget == 0 {
        return Err("HTML clipboard exceeds structural limits".into());
    }
    *budget -= 1;
    if let NodeData::Text { contents } = &handle.data {
        let text = contents.borrow().to_string();
        return Ok(if text.is_empty() {
            vec![]
        } else {
            vec![json!({"type":"text","text":text,"marks":marks})]
        });
    }
    let (tag, attrs) = if let NodeData::Element { name, attrs, .. } = &handle.data {
        (
            name.local.as_ref(),
            attrs
                .borrow()
                .iter()
                .map(|attr| {
                    (
                        attr.name.local.to_string(),
                        Value::String(attr.value.to_string()),
                    )
                })
                .collect::<Map<_, _>>(),
        )
    } else {
        ("", Map::new())
    };
    let attr = |name: &str| attrs.get(name).and_then(Value::as_str);
    if matches!(
        tag,
        "script" | "style" | "meta" | "link" | "head" | "noscript"
    ) {
        return Ok(vec![]);
    }
    if matches!(
        tag,
        "iframe" | "object" | "embed" | "svg" | "video" | "audio"
    ) {
        return Err(format!(
            "HTML contains {tag}; paste as plain text to omit it"
        ));
    }
    let mut marks = marks.to_vec();
    let mark = match tag {
        "b" | "strong" => Some("bold"),
        "i" | "em" => Some("italic"),
        "s" | "del" | "strike" => Some("strike"),
        "u" => Some("underline"),
        "mark" => Some("highlight"),
        "code" => Some("code"),
        _ => None,
    };
    if let Some(mark) = mark {
        marks.push(json!({"type":mark}));
    }
    if tag == "a"
        && let Some(href) = attr("href")
    {
        if !super::clipboard::openable_link(href) {
            return Err("HTML link uses an unsupported scheme".into());
        }
        marks.push(json!({"type":"link","attrs":{"href":href,"target":attr("target")}}));
    }
    let mut children = Vec::new();
    for child in handle.children.borrow().iter() {
        children.extend(convert(child, &marks, depth + 1, budget)?);
    }
    let kind = match tag {
        "p" => "paragraph",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "heading",
        "blockquote" => "blockquote",
        "pre" => "codeBlock",
        "ul" if attr("data-type") == Some("taskList") => "taskList",
        "ul" => "bulletList",
        "ol" => "orderedList",
        "li" if attr("data-type") == Some("taskItem") => "taskItem",
        "li" => "listItem",
        "table" => "table",
        "tr" => "tableRow",
        "td" => "tableCell",
        "th" => "tableHeader",
        "br" => "hardBreak",
        "hr" => "horizontalRule",
        "img" => "image",
        "span" if attr("data-mention") == Some("true") => "mention-@",
        "div" if attr("data-type") == Some("file-attachment") => "fileAttachment",
        "div" | "section" | "article" => return Ok(blocks(children)),
        _ => return Ok(children),
    };
    let mut node_attrs = Map::new();
    match kind {
        "heading" => {
            node_attrs.insert("level".into(), json!(tag[1..].parse::<u8>().unwrap_or(1)));
        }
        "orderedList" => {
            node_attrs.insert(
                "start".into(),
                json!(
                    attr("start")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(1)
                ),
            );
        }
        "tableCell" | "tableHeader" => {
            for key in ["colspan", "rowspan"] {
                node_attrs.insert(
                    key.into(),
                    json!(
                        attr(key)
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(1)
                            .clamp(1, 1000)
                    ),
                );
            }
            if let Some(widths) = attr("data-colwidth") {
                node_attrs.insert(
                    "colwidth".into(),
                    widths
                        .split(',')
                        .filter_map(|s| s.parse::<u64>().ok())
                        .collect::<Vec<_>>()
                        .into(),
                );
            }
        }
        "taskItem" => {
            let status = match attr("data-status") {
                Some(status @ ("todo" | "in_progress" | "done")) => status,
                _ if attr("data-checked") == Some("true") => "done",
                _ => "todo",
            };
            node_attrs = json!({
                "taskId": attr("data-task-id").map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                "taskItemId": uuid::Uuid::new_v4().to_string(),
                "checked": status == "done", "status": status
            }).as_object().expect("attrs").clone();
        }
        "image" => {
            for (html, name) in [
                ("data-attachment-id", "attachmentId"),
                ("data-shared-attachment-id", "sharedAttachmentId"),
            ] {
                if let Some(id) = attr(html) {
                    node_attrs.insert(name.into(), json!(id));
                }
            }
            if let Some(width) =
                attr("data-editor-width").and_then(|width| width.parse::<u64>().ok())
            {
                node_attrs.insert("editorWidth".into(), json!(width.clamp(10, 100)));
            }
            for key in ["src", "alt", "title"] {
                if let Some(value) = attr(key) {
                    if key == "src"
                        && (node_attrs.contains_key("attachmentId")
                            || node_attrs.contains_key("sharedAttachmentId"))
                    {
                        continue;
                    }
                    if key == "src" && !super::clipboard::openable_link(value) {
                        return Err(
                            "HTML image requires an HTTP(S) source or attachment import".into()
                        );
                    }
                    node_attrs.insert(key.into(), json!(value));
                }
            }
        }
        "fileAttachment" => {
            for (html, name) in [
                ("data-attachment-id", "attachmentId"),
                ("data-shared-attachment-id", "sharedAttachmentId"),
                ("data-name", "name"),
                ("data-mime-type", "mimeType"),
                ("data-src", "src"),
            ] {
                if let Some(value) = attr(html) {
                    node_attrs.insert(name.into(), json!(value));
                }
            }
            if let Some(size) = attr("data-size").and_then(|s| s.parse::<u64>().ok()) {
                node_attrs.insert("size".into(), json!(size));
            }
        }
        "mention-@" => {
            for key in ["id", "label", "type"] {
                if let Some(value) = attr(&format!("data-{key}")) {
                    node_attrs.insert(key.into(), json!(value));
                }
            }
        }
        _ => {}
    }
    if matches!(
        kind,
        "tableCell" | "tableHeader" | "listItem" | "taskItem" | "blockquote"
    ) {
        children = blocks(children);
    }
    if matches!(
        kind,
        "table" | "tableRow" | "bulletList" | "orderedList" | "taskList"
    ) {
        children.retain(|child| {
            !(child["type"] == "text"
                && child["text"]
                    .as_str()
                    .is_some_and(|text| text.trim().is_empty()))
        });
    }
    if kind == "codeBlock" {
        for child in &mut children {
            if let Some(object) = child.as_object_mut() {
                object.remove("marks");
            }
        }
    }
    if matches!(
        kind,
        "hardBreak" | "horizontalRule" | "image" | "fileAttachment" | "mention-@"
    ) {
        return Ok(vec![json!({"type":kind,"attrs":node_attrs})]);
    }
    Ok(vec![
        json!({"type":kind,"attrs":node_attrs,"content":children}),
    ])
}
