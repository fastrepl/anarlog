use crate::api::{ProseMirrorDoc, ProseMirrorNode};
use regex::Regex;
use std::sync::OnceLock;

static NEWLINE_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_newline_regex() -> &'static Regex {
    NEWLINE_REGEX.get_or_init(|| Regex::new(r"\n{3,}").unwrap())
}

pub fn convert_to_markdown(doc: &ProseMirrorDoc) -> String {
    if doc.doc_type != "doc" || doc.content.is_empty() {
        return String::new();
    }

    let mut output = Vec::new();
    for node in &doc.content {
        output.push(process_node(node, 0, true));
    }

    let result = output.join("");
    let result = get_newline_regex().replace_all(&result, "\n\n");

    format!("{}\n", result.trim())
}

fn process_node(node: &ProseMirrorNode, indent_level: usize, is_top_level: bool) -> String {
    let text_content = if !node.content.is_empty() {
        match node.node_type.as_str() {
            "bulletList" => node
                .content
                .iter()
                .map(|child| process_node(child, indent_level, false))
                .collect::<Vec<_>>()
                .join(""),
            "listItem" => node
                .content
                .iter()
                .map(|child| {
                    if child.node_type == "bulletList" {
                        process_node(child, indent_level + 1, false)
                    } else {
                        process_node(child, indent_level, false)
                    }
                })
                .collect::<Vec<_>>()
                .join(""),
            _ => node
                .content
                .iter()
                .map(|child| process_node(child, indent_level, false))
                .collect::<Vec<_>>()
                .join(""),
        }
    } else if !node.text.is_empty() {
        node.text.clone()
    } else {
        String::new()
    };

    match node.node_type.as_str() {
        "heading" => {
            let level = node
                .attrs
                .as_ref()
                .and_then(|attrs: &serde_json::Map<String, serde_json::Value>| attrs.get("level"))
                .and_then(|v: &serde_json::Value| v.as_f64())
                .map(|v: f64| v as usize)
                .unwrap_or(1);

            let suffix = if is_top_level { "\n\n" } else { "\n" };
            format!("{} {}{}", "#".repeat(level), text_content.trim(), suffix)
        }
        "paragraph" => {
            let suffix = if is_top_level { "\n\n" } else { "" };
            format!("{}{}", text_content, suffix)
        }
        "bulletList" => {
            let mut items = Vec::new();
            for item_node in &node.content {
                if item_node.node_type == "listItem" {
                    let mut child_contents = Vec::new();
                    let mut nested_lists = Vec::new();

                    for child in &item_node.content {
                        if child.node_type == "bulletList" {
                            nested_lists.push(format!(
                                "\n{}",
                                process_node(child, indent_level + 1, false)
                            ));
                        } else {
                            child_contents.push(process_node(child, indent_level, false));
                        }
                    }

                    let first_text = child_contents
                        .iter()
                        .find(|c| !c.starts_with('\n'))
                        .cloned()
                        .unwrap_or_default();

                    let indent = "\t".repeat(indent_level);
                    let rest = nested_lists.join("");
                    items.push(format!("{}- {}{}", indent, first_text.trim(), rest));
                }
            }

            let suffix = if is_top_level { "\n\n" } else { "" };
            format!("{}{}", items.join("\n"), suffix)
        }
        "text" => node.text.clone(),
        _ => text_content,
    }
}

pub fn convert_to_plain_text(doc: &ProseMirrorDoc) -> String {
    if doc.doc_type != "doc" || doc.content.is_empty() {
        return String::new();
    }

    let mut output = Vec::new();
    for node in &doc.content {
        let text = extract_text(node);
        if !text.is_empty() {
            output.push(text);
        }
    }

    output.join("\n\n").trim().to_string()
}

fn extract_text(node: &ProseMirrorNode) -> String {
    if !node.text.is_empty() {
        return node.text.clone();
    }

    if node.content.is_empty() {
        return String::new();
    }

    let texts: Vec<String> = node
        .content
        .iter()
        .map(extract_text)
        .filter(|t: &String| !t.is_empty())
        .collect();

    let separator = match node.node_type.as_str() {
        "paragraph" | "heading" | "listItem" => "\n",
        _ => " ",
    };

    texts.join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ProseMirrorDoc, ProseMirrorNode};
    use serde_json::json;

    #[test]
    fn test_convert_heading_and_paragraph() {
        let doc = ProseMirrorDoc {
            doc_type: "doc".to_string(),
            content: vec![
                ProseMirrorNode {
                    node_type: "heading".to_string(),
                    content: vec![ProseMirrorNode {
                        node_type: "text".to_string(),
                        text: "Meeting Notes".to_string(),
                        content: vec![],
                        attrs: None,
                    }],
                    text: String::new(),
                    attrs: Some(serde_json::from_value(json!({"level": 1})).unwrap()),
                },
                ProseMirrorNode {
                    node_type: "paragraph".to_string(),
                    content: vec![ProseMirrorNode {
                        node_type: "text".to_string(),
                        text: "This is a paragraph.".to_string(),
                        content: vec![],
                        attrs: None,
                    }],
                    text: String::new(),
                    attrs: None,
                },
            ],
        };

        let result = convert_to_markdown(&doc);
        assert!(result.contains("# Meeting Notes"));
        assert!(result.contains("This is a paragraph."));
    }

    #[test]
    fn test_convert_bullet_list() {
        let doc = ProseMirrorDoc {
            doc_type: "doc".to_string(),
            content: vec![ProseMirrorNode {
                node_type: "bulletList".to_string(),
                content: vec![
                    ProseMirrorNode {
                        node_type: "listItem".to_string(),
                        content: vec![ProseMirrorNode {
                            node_type: "paragraph".to_string(),
                            content: vec![ProseMirrorNode {
                                node_type: "text".to_string(),
                                text: "First item".to_string(),
                                content: vec![],
                                attrs: None,
                            }],
                            text: String::new(),
                            attrs: None,
                        }],
                        text: String::new(),
                        attrs: None,
                    },
                    ProseMirrorNode {
                        node_type: "listItem".to_string(),
                        content: vec![ProseMirrorNode {
                            node_type: "paragraph".to_string(),
                            content: vec![ProseMirrorNode {
                                node_type: "text".to_string(),
                                text: "Second item".to_string(),
                                content: vec![],
                                attrs: None,
                            }],
                            text: String::new(),
                            attrs: None,
                        }],
                        text: String::new(),
                        attrs: None,
                    },
                ],
                text: String::new(),
                attrs: None,
            }],
        };

        let result = convert_to_markdown(&doc);
        assert!(result.contains("- First item"));
        assert!(result.contains("- Second item"));
    }

    #[test]
    fn test_empty_doc() {
        let doc = ProseMirrorDoc {
            doc_type: "doc".to_string(),
            content: vec![],
        };
        assert_eq!(convert_to_markdown(&doc), "");
    }
}
