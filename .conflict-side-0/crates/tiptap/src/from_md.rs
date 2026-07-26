use markdown::mdast;
use serde_json::{Value, json};

pub fn md_to_tiptap_json(md: &str) -> Result<Value, String> {
    let mdast = markdown::to_mdast(md, &markdown::ParseOptions::gfm())
        .map_err(|e| format!("Failed to parse markdown: {}", e))?;
    Ok(mdast_to_tiptap(&mdast))
}

fn mdast_to_tiptap(node: &mdast::Node) -> Value {
    match node {
        mdast::Node::Root(root) => {
            let content: Vec<Value> = root
                .children
                .iter()
                .filter_map(convert_block_node)
                .collect();
            json!({
                "type": "doc",
                "content": content
            })
        }
        _ => json!({
            "type": "doc",
            "content": []
        }),
    }
}

fn convert_block_node(node: &mdast::Node) -> Option<Value> {
    match node {
        mdast::Node::Paragraph(p) => Some(convert_paragraph(p)),
        mdast::Node::Heading(h) => Some(convert_heading(h)),
        mdast::Node::List(l) => Some(convert_list(l)),
        mdast::Node::Code(c) => Some(convert_code_block(c)),
        mdast::Node::Blockquote(b) => Some(convert_blockquote(b)),
        mdast::Node::ThematicBreak(_) => Some(json!({ "type": "horizontalRule" })),
        mdast::Node::Image(img) => Some(convert_image(img)),
        mdast::Node::Html(h) => convert_html_mention(&h.value),
        _ => None,
    }
}

fn convert_paragraph(p: &mdast::Paragraph) -> Value {
    let content = convert_inline_nodes(&p.children);
    if content.is_empty() {
        json!({ "type": "paragraph" })
    } else {
        json!({
            "type": "paragraph",
            "content": content
        })
    }
}

fn convert_heading(h: &mdast::Heading) -> Value {
    let content = convert_inline_nodes(&h.children);
    let mut result = json!({
        "type": "heading",
        "attrs": { "level": h.depth }
    });
    if !content.is_empty() {
        result["content"] = json!(content);
    }
    result
}

fn convert_list(l: &mdast::List) -> Value {
    let has_checked = l
        .children
        .iter()
        .any(|child| matches!(child, mdast::Node::ListItem(item) if item.checked.is_some()));

    if has_checked {
        let content: Vec<Value> = l
            .children
            .iter()
            .filter_map(|child| {
                if let mdast::Node::ListItem(item) = child {
                    Some(convert_task_item(item))
                } else {
                    None
                }
            })
            .collect();
        json!({
            "type": "taskList",
            "content": content
        })
    } else if l.ordered {
        let content: Vec<Value> = l
            .children
            .iter()
            .filter_map(|child| {
                if let mdast::Node::ListItem(item) = child {
                    Some(convert_list_item(item))
                } else {
                    None
                }
            })
            .collect();
        let mut result = json!({
            "type": "orderedList",
            "content": content
        });
        if let Some(start) = l.start {
            result["attrs"] = json!({ "start": start });
        }
        result
    } else {
        let content: Vec<Value> = l
            .children
            .iter()
            .filter_map(|child| {
                if let mdast::Node::ListItem(item) = child {
                    Some(convert_list_item(item))
                } else {
                    None
                }
            })
            .collect();
        json!({
            "type": "bulletList",
            "content": content
        })
    }
}

fn ensure_starts_with_paragraph(content: Vec<Value>) -> Vec<Value> {
    if content.is_empty() {
        return vec![json!({ "type": "paragraph" })];
    }

    let first_is_paragraph = content
        .first()
        .and_then(|v| v.get("type"))
        .and_then(|t| t.as_str())
        .map(|t| t == "paragraph")
        .unwrap_or(false);

    if first_is_paragraph {
        content
    } else {
        let mut result = vec![json!({ "type": "paragraph" })];
        result.extend(content);
        result
    }
}

fn convert_list_item(item: &mdast::ListItem) -> Value {
    let content: Vec<Value> = item
        .children
        .iter()
        .filter_map(convert_block_node)
        .collect();

    let content = ensure_starts_with_paragraph(content);

    json!({
        "type": "listItem",
        "content": content
    })
}

fn convert_task_item(item: &mdast::ListItem) -> Value {
    let content: Vec<Value> = item
        .children
        .iter()
        .filter_map(convert_block_node)
        .collect();

    let content = ensure_starts_with_paragraph(content);

    json!({
        "type": "taskItem",
        "attrs": { "checked": item.checked.unwrap_or(false) },
        "content": content
    })
}

fn convert_code_block(c: &mdast::Code) -> Value {
    let mut result = json!({
        "type": "codeBlock"
    });
    if let Some(lang) = &c.lang {
        result["attrs"] = json!({ "language": lang });
    }
    if !c.value.is_empty() {
        result["content"] = json!([{ "type": "text", "text": c.value }]);
    }
    result
}

fn convert_blockquote(b: &mdast::Blockquote) -> Value {
    let content: Vec<Value> = b.children.iter().filter_map(convert_block_node).collect();
    json!({
        "type": "blockquote",
        "content": content
    })
}

fn convert_image(img: &mdast::Image) -> Value {
    let mut attrs = json!({ "src": img.url });
    if !img.alt.is_empty() {
        attrs["alt"] = json!(img.alt);
    }
    if let Some(title) = &img.title {
        attrs["title"] = json!(title);
    }
    json!({
        "type": "image",
        "attrs": attrs
    })
}

fn convert_inline_nodes(nodes: &[mdast::Node]) -> Vec<Value> {
    nodes.iter().filter_map(convert_inline_node).collect()
}

fn convert_inline_node(node: &mdast::Node) -> Option<Value> {
    match node {
        mdast::Node::Text(t) => Some(json!({ "type": "text", "text": t.value })),
        mdast::Node::Strong(s) => Some(convert_marked_text(&s.children, "bold")),
        mdast::Node::Emphasis(e) => Some(convert_marked_text(&e.children, "italic")),
        mdast::Node::InlineCode(c) => Some(json!({
            "type": "text",
            "text": c.value,
            "marks": [{ "type": "code" }]
        })),
        mdast::Node::Link(l) => Some(convert_link(l)),
        mdast::Node::Delete(d) => Some(convert_marked_text(&d.children, "strike")),
        mdast::Node::Break(_) => Some(json!({ "type": "hardBreak" })),
        mdast::Node::Image(img) => Some(convert_image(img)),
        mdast::Node::Html(h) => convert_html_mention(&h.value),
        _ => None,
    }
}

fn convert_marked_text(children: &[mdast::Node], mark_type: &str) -> Value {
    let text = extract_text(children);
    let mut marks = extract_marks(children);
    marks.push(json!({ "type": mark_type }));

    let marks = sanitize_marks(marks);

    json!({
        "type": "text",
        "text": text,
        "marks": marks
    })
}

/// The `code` mark has `excludes: "_"` in TipTap, meaning it excludes all other marks.
/// When `code` is present, strip all other marks to match ProseMirror's schema rules.
fn sanitize_marks(marks: Vec<Value>) -> Vec<Value> {
    let has_code = marks
        .iter()
        .any(|m| m.get("type").and_then(|t| t.as_str()) == Some("code"));

    if has_code {
        vec![json!({ "type": "code" })]
    } else {
        marks
    }
}

fn convert_link(l: &mdast::Link) -> Value {
    let text = extract_text(&l.children);
    let mut marks = extract_marks(&l.children);

    let mut link_attrs = json!({ "href": l.url });
    if let Some(title) = &l.title {
        link_attrs["title"] = json!(title);
    }
    marks.push(json!({ "type": "link", "attrs": link_attrs }));

    let marks = sanitize_marks(marks);

    json!({
        "type": "text",
        "text": text,
        "marks": marks
    })
}

fn extract_text(nodes: &[mdast::Node]) -> String {
    nodes
        .iter()
        .map(|n| match n {
            mdast::Node::Text(t) => t.value.clone(),
            mdast::Node::Strong(s) => extract_text(&s.children),
            mdast::Node::Emphasis(e) => extract_text(&e.children),
            mdast::Node::InlineCode(c) => c.value.clone(),
            mdast::Node::Link(l) => extract_text(&l.children),
            mdast::Node::Delete(d) => extract_text(&d.children),
            _ => String::new(),
        })
        .collect()
}

fn extract_marks(nodes: &[mdast::Node]) -> Vec<Value> {
    let mut marks = Vec::new();
    for node in nodes {
        match node {
            mdast::Node::Strong(s) => {
                marks.push(json!({ "type": "bold" }));
                marks.extend(extract_marks(&s.children));
            }
            mdast::Node::Emphasis(e) => {
                marks.push(json!({ "type": "italic" }));
                marks.extend(extract_marks(&e.children));
            }
            mdast::Node::Delete(d) => {
                marks.push(json!({ "type": "strike" }));
                marks.extend(extract_marks(&d.children));
            }
            mdast::Node::InlineCode(_) => {
                marks.push(json!({ "type": "code" }));
            }
            _ => {}
        }
    }
    marks
}

fn convert_html_mention(html: &str) -> Option<Value> {
    let trimmed = html.trim();
    if !trimmed.starts_with("<mention ") {
        return None;
    }

    let id = extract_attr(trimmed, "data-id");
    let typ = extract_attr(trimmed, "data-type");
    let label = extract_attr(trimmed, "data-label");

    Some(json!({
        "type": "mention-@",
        "attrs": { "id": id, "type": typ, "label": label }
    }))
}

fn extract_attr(html: &str, attr_name: &str) -> String {
    let pattern = format!("{}=\"", attr_name);
    let Some(start) = html.find(&pattern) else {
        return String::new();
    };
    let value_start = start + pattern.len();
    let rest = &html[value_start..];
    let end = rest.find('"').unwrap_or(rest.len());
    rest[..end].to_string()
}
