use serde_json::Value;

#[derive(Debug)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at {}: {}", self.path, self.message)
    }
}

pub fn validate_tiptap_json(json: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    validate_node(json, "doc", &mut errors);
    errors
}

fn node_type(node: &Value) -> Option<&str> {
    node.get("type").and_then(|t| t.as_str())
}

fn node_content(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(|c| c.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn is_block_type(t: &str) -> bool {
    matches!(
        t,
        "paragraph"
            | "heading"
            | "bulletList"
            | "orderedList"
            | "taskList"
            | "blockquote"
            | "codeBlock"
            | "image"
            | "horizontalRule"
    )
}

fn is_inline_type(t: &str) -> bool {
    matches!(t, "text" | "hardBreak" | "image") || t.starts_with("mention-")
}

fn validate_node(node: &Value, path: &str, errors: &mut Vec<ValidationError>) {
    let Some(typ) = node_type(node) else {
        errors.push(ValidationError {
            path: path.to_string(),
            message: "missing 'type' field".to_string(),
        });
        return;
    };

    let content = node_content(node);

    match typ {
        "doc" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "doc must contain at least one block node (content: 'block+')"
                        .to_string(),
                });
            }
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && !is_block_type(ct)
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("doc child must be a block node, got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "paragraph" => {
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && !is_inline_type(ct)
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("paragraph child must be an inline node, got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "heading" => {
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && !is_inline_type(ct)
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("heading child must be an inline node, got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "bulletList" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "bulletList must contain at least one listItem (content: 'listItem+')"
                        .to_string(),
                });
            }
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && ct != "listItem"
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("bulletList child must be 'listItem', got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "orderedList" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message:
                        "orderedList must contain at least one listItem (content: 'listItem+')"
                            .to_string(),
                });
            }
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && ct != "listItem"
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("orderedList child must be 'listItem', got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "taskList" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "taskList must contain at least one taskItem (content: 'taskItem+')"
                        .to_string(),
                });
            }
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && ct != "taskItem"
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("taskList child must be 'taskItem', got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "listItem" | "taskItem" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: format!(
                        "{typ} must contain at least a paragraph (content: 'paragraph block*')"
                    ),
                });
            } else {
                let first_type = node_type(&content[0]);
                if first_type != Some("paragraph") {
                    errors.push(ValidationError {
                        path: format!("{path}.content[0]"),
                        message: format!(
                            "{typ} must start with a paragraph (content: 'paragraph block*'), got '{}'",
                            first_type.unwrap_or("unknown")
                        ),
                    });
                }
                for (i, child) in content.iter().enumerate() {
                    let child_path = format!("{path}.content[{i}]");
                    if let Some(ct) = node_type(child)
                        && !is_block_type(ct)
                    {
                        errors.push(ValidationError {
                            path: child_path.clone(),
                            message: format!("{typ} child must be a block node, got '{ct}'"),
                        });
                    }
                    validate_node(child, &child_path, errors);
                }
            }
        }

        "blockquote" => {
            if content.is_empty() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "blockquote must contain at least one block node (content: 'block+')"
                        .to_string(),
                });
            }
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && !is_block_type(ct)
                {
                    errors.push(ValidationError {
                        path: child_path.clone(),
                        message: format!("blockquote child must be a block node, got '{ct}'"),
                    });
                }
                validate_node(child, &child_path, errors);
            }
        }

        "codeBlock" => {
            for (i, child) in content.iter().enumerate() {
                let child_path = format!("{path}.content[{i}]");
                if let Some(ct) = node_type(child)
                    && ct != "text"
                {
                    errors.push(ValidationError {
                        path: child_path,
                        message: format!("codeBlock child must be 'text', got '{ct}'"),
                    });
                }
            }
        }

        "text" => {
            validate_marks(node, path, errors);
        }

        "hardBreak" | "horizontalRule" | "image" => {}

        _ => {}
    }
}

fn validate_marks(node: &Value, path: &str, errors: &mut Vec<ValidationError>) {
    let Some(marks) = node.get("marks").and_then(|m| m.as_array()) else {
        return;
    };

    let mark_types: Vec<&str> = marks
        .iter()
        .filter_map(|m| m.get("type").and_then(|t| t.as_str()))
        .collect();

    // The `code` mark has `excludes: "_"` — it excludes all other marks.
    if mark_types.contains(&"code") && mark_types.len() > 1 {
        let other_marks: Vec<&str> = mark_types
            .iter()
            .filter(|&&t| t != "code")
            .copied()
            .collect();
        errors.push(ValidationError {
            path: path.to_string(),
            message: format!(
                "code mark excludes all other marks, but found alongside: {}",
                other_marks.join(", ")
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assert_valid(json: &Value) {
        let errors = validate_tiptap_json(json);
        assert!(
            errors.is_empty(),
            "expected valid, got errors:\n{}",
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    fn assert_invalid(json: &Value, expected_fragment: &str) {
        let errors = validate_tiptap_json(json);
        assert!(
            !errors.is_empty(),
            "expected validation errors but got none"
        );
        let all = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains(expected_fragment),
            "expected error containing '{expected_fragment}', got:\n{all}"
        );
    }

    #[test]
    fn valid_simple_doc() {
        assert_valid(&json!({
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        }));
    }

    #[test]
    fn valid_list_item_with_paragraph() {
        assert_valid(&json!({
            "type": "doc",
            "content": [{
                "type": "bulletList",
                "content": [{
                    "type": "listItem",
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "hello" }]
                    }]
                }]
            }]
        }));
    }

    #[test]
    fn valid_list_item_with_paragraph_then_nested_list() {
        assert_valid(&json!({
            "type": "doc",
            "content": [{
                "type": "bulletList",
                "content": [{
                    "type": "listItem",
                    "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "item" }] },
                        {
                            "type": "bulletList",
                            "content": [{
                                "type": "listItem",
                                "content": [{ "type": "paragraph" }]
                            }]
                        }
                    ]
                }]
            }]
        }));
    }

    #[test]
    fn invalid_list_item_starting_with_list() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": [{
                    "type": "bulletList",
                    "content": [{
                        "type": "listItem",
                        "content": [{
                            "type": "bulletList",
                            "content": [{
                                "type": "listItem",
                                "content": [{ "type": "paragraph" }]
                            }]
                        }]
                    }]
                }]
            }),
            "must start with a paragraph",
        );
    }

    #[test]
    fn invalid_empty_list_item() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": [{
                    "type": "bulletList",
                    "content": [{
                        "type": "listItem",
                        "content": []
                    }]
                }]
            }),
            "must contain at least a paragraph",
        );
    }

    #[test]
    fn invalid_task_item_without_paragraph() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": [{
                    "type": "taskList",
                    "content": [{
                        "type": "taskItem",
                        "attrs": { "checked": false },
                        "content": [{
                            "type": "bulletList",
                            "content": [{
                                "type": "listItem",
                                "content": [{ "type": "paragraph" }]
                            }]
                        }]
                    }]
                }]
            }),
            "must start with a paragraph",
        );
    }

    #[test]
    fn invalid_empty_doc() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": []
            }),
            "must contain at least one block",
        );
    }

    #[test]
    fn invalid_inline_in_doc() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": [{ "type": "text", "text": "hello" }]
            }),
            "doc child must be a block node",
        );
    }

    #[test]
    fn invalid_code_with_bold_marks() {
        assert_invalid(
            &json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": "code",
                        "marks": [{ "type": "bold" }, { "type": "code" }]
                    }]
                }]
            }),
            "code mark excludes all other marks",
        );
    }

    #[test]
    fn valid_code_mark_alone() {
        assert_valid(&json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "code",
                    "marks": [{ "type": "code" }]
                }]
            }]
        }));
    }

    #[test]
    fn valid_bold_italic_marks() {
        assert_valid(&json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "bold italic",
                    "marks": [{ "type": "bold" }, { "type": "italic" }]
                }]
            }]
        }));
    }
}
