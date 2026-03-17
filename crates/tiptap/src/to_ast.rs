use markdown::mdast;

pub fn tiptap_json_to_mdast(json: &serde_json::Value) -> mdast::Node {
    let children = convert_content(json);
    mdast::Node::Root(mdast::Root {
        children,
        position: None,
    })
}

fn convert_content(json: &serde_json::Value) -> Vec<mdast::Node> {
    let Some(content) = json.get("content").and_then(|c| c.as_array()) else {
        return vec![];
    };

    content.iter().filter_map(convert_node).collect()
}

fn convert_node(node: &serde_json::Value) -> Option<mdast::Node> {
    let node_type = node.get("type")?.as_str()?;

    match node_type {
        "paragraph" => Some(convert_paragraph(node)),
        "heading" => Some(convert_heading(node)),
        "bulletList" => Some(convert_bullet_list(node)),
        "orderedList" => Some(convert_ordered_list(node)),
        "taskList" => Some(convert_task_list(node)),
        "listItem" => Some(convert_list_item(node, None)),
        "taskItem" => Some(convert_task_item(node)),
        "codeBlock" => Some(convert_code_block(node)),
        "blockquote" => Some(convert_blockquote(node)),
        "horizontalRule" => Some(convert_horizontal_rule()),
        "hardBreak" => Some(convert_hard_break()),
        "image" => Some(convert_image(node)),
        "text" => convert_text(node),
        t if t.starts_with("mention-") => Some(convert_mention(node)),
        _ => None,
    }
}

fn convert_paragraph(node: &serde_json::Value) -> mdast::Node {
    let children = convert_inline_content(node);
    mdast::Node::Paragraph(mdast::Paragraph {
        children,
        position: None,
    })
}

fn convert_heading(node: &serde_json::Value) -> mdast::Node {
    let depth = node
        .get("attrs")
        .and_then(|a| a.get("level"))
        .and_then(|l| l.as_u64())
        .unwrap_or(1) as u8;

    let children = convert_inline_content(node);
    mdast::Node::Heading(mdast::Heading {
        depth,
        children,
        position: None,
    })
}

fn convert_bullet_list(node: &serde_json::Value) -> mdast::Node {
    let children = convert_list_items(node);
    mdast::Node::List(mdast::List {
        ordered: false,
        start: None,
        spread: false,
        children,
        position: None,
    })
}

fn convert_ordered_list(node: &serde_json::Value) -> mdast::Node {
    let start = node
        .get("attrs")
        .and_then(|a| a.get("start"))
        .and_then(|s| s.as_u64())
        .map(|s| s as u32);

    let children = convert_list_items(node);
    mdast::Node::List(mdast::List {
        ordered: true,
        start,
        spread: false,
        children,
        position: None,
    })
}

fn convert_list_items(node: &serde_json::Value) -> Vec<mdast::Node> {
    let Some(content) = node.get("content").and_then(|c| c.as_array()) else {
        return vec![];
    };

    content
        .iter()
        .filter_map(|item| {
            let item_type = item.get("type")?.as_str()?;
            if item_type == "listItem" {
                Some(convert_list_item(item, None))
            } else {
                None
            }
        })
        .collect()
}

fn convert_task_list(node: &serde_json::Value) -> mdast::Node {
    let children = convert_task_items(node);
    mdast::Node::List(mdast::List {
        ordered: false,
        start: None,
        spread: false,
        children,
        position: None,
    })
}

fn convert_task_items(node: &serde_json::Value) -> Vec<mdast::Node> {
    let Some(content) = node.get("content").and_then(|c| c.as_array()) else {
        return vec![];
    };

    content
        .iter()
        .filter_map(|item| {
            let item_type = item.get("type")?.as_str()?;
            if item_type == "taskItem" {
                Some(convert_task_item(item))
            } else {
                None
            }
        })
        .collect()
}

fn convert_task_item(node: &serde_json::Value) -> mdast::Node {
    let checked = node
        .get("attrs")
        .and_then(|a| a.get("checked"))
        .and_then(|c| c.as_bool());
    convert_list_item(node, checked)
}

fn convert_list_item(node: &serde_json::Value, checked: Option<bool>) -> mdast::Node {
    let children = convert_content(node);
    mdast::Node::ListItem(mdast::ListItem {
        checked,
        spread: false,
        children,
        position: None,
    })
}

fn convert_code_block(node: &serde_json::Value) -> mdast::Node {
    let lang = node
        .get("attrs")
        .and_then(|a| a.get("language"))
        .and_then(|l| l.as_str())
        .map(|s| s.to_string());

    let value = node
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    mdast::Node::Code(mdast::Code {
        value,
        lang,
        meta: None,
        position: None,
    })
}

fn convert_blockquote(node: &serde_json::Value) -> mdast::Node {
    let children = convert_content(node);
    mdast::Node::Blockquote(mdast::Blockquote {
        children,
        position: None,
    })
}

fn convert_horizontal_rule() -> mdast::Node {
    mdast::Node::ThematicBreak(mdast::ThematicBreak { position: None })
}

fn convert_hard_break() -> mdast::Node {
    mdast::Node::Break(mdast::Break { position: None })
}

fn convert_image(node: &serde_json::Value) -> mdast::Node {
    let attrs = node.get("attrs");
    let url = attrs
        .and_then(|a| a.get("src"))
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let alt = attrs
        .and_then(|a| a.get("alt"))
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    let title = attrs
        .and_then(|a| a.get("title"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    mdast::Node::Image(mdast::Image {
        url,
        alt,
        title,
        position: None,
    })
}

fn convert_mention(node: &serde_json::Value) -> mdast::Node {
    let attrs = node.get("attrs");
    let id = attrs
        .and_then(|a| a.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let typ = attrs
        .and_then(|a| a.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let label = attrs
        .and_then(|a| a.get("label"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    mdast::Node::Html(mdast::Html {
        value: format!(
            r#"<mention data-id="{}" data-type="{}" data-label="{}"></mention>"#,
            id, typ, label
        ),
        position: None,
    })
}

fn convert_text(node: &serde_json::Value) -> Option<mdast::Node> {
    let text = node.get("text")?.as_str()?;
    Some(mdast::Node::Text(mdast::Text {
        value: text.to_string(),
        position: None,
    }))
}

fn convert_inline_content(node: &serde_json::Value) -> Vec<mdast::Node> {
    let Some(content) = node.get("content").and_then(|c| c.as_array()) else {
        return vec![];
    };

    content.iter().filter_map(convert_inline_node).collect()
}

fn convert_inline_node(node: &serde_json::Value) -> Option<mdast::Node> {
    let node_type = node.get("type")?.as_str()?;

    match node_type {
        "text" => convert_text_with_marks(node),
        "hardBreak" => Some(convert_hard_break()),
        "image" => Some(convert_image(node)),
        t if t.starts_with("mention-") => Some(convert_mention(node)),
        _ => None,
    }
}

fn convert_text_with_marks(node: &serde_json::Value) -> Option<mdast::Node> {
    let text = node.get("text")?.as_str()?;
    let marks = node.get("marks").and_then(|m| m.as_array());

    let text_node = mdast::Node::Text(mdast::Text {
        value: text.to_string(),
        position: None,
    });

    let Some(marks) = marks else {
        return Some(text_node);
    };

    let mut result = text_node;

    for mark in marks.iter().rev() {
        let mark_type = mark.get("type").and_then(|t| t.as_str());
        result = match mark_type {
            Some("bold") | Some("strong") => mdast::Node::Strong(mdast::Strong {
                children: vec![result],
                position: None,
            }),
            Some("italic") | Some("em") => mdast::Node::Emphasis(mdast::Emphasis {
                children: vec![result],
                position: None,
            }),
            Some("code") => {
                if let mdast::Node::Text(t) = result {
                    mdast::Node::InlineCode(mdast::InlineCode {
                        value: t.value,
                        position: None,
                    })
                } else {
                    result
                }
            }
            Some("link") => {
                let url = mark
                    .get("attrs")
                    .and_then(|a| a.get("href"))
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = mark
                    .get("attrs")
                    .and_then(|a| a.get("title"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());

                mdast::Node::Link(mdast::Link {
                    url,
                    title,
                    children: vec![result],
                    position: None,
                })
            }
            Some("strike") => mdast::Node::Delete(mdast::Delete {
                children: vec![result],
                position: None,
            }),
            _ => result,
        };
    }

    Some(result)
}
