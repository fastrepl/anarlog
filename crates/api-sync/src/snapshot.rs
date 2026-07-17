use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::error::{Result, SyncError};

pub(crate) const MAX_SNAPSHOT_BODY_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_SNAPSHOT_TITLE_BYTES: usize = 4096;

const MAX_SNAPSHOT_DEPTH: usize = 64;
const MAX_SNAPSHOT_NODES: usize = 50_000;

pub(crate) fn sanitize_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.len() > MAX_SNAPSHOT_TITLE_BYTES {
        return Err(SyncError::BadRequest(
            "Shared note title is too large".to_string(),
        ));
    }

    Ok(title.to_string())
}

#[cfg(test)]
fn sanitize_document(body: &Value) -> Result<Value> {
    sanitize_document_with_attachments(body, &HashSet::new())
}

pub(crate) fn sanitize_document_with_attachments(
    body: &Value,
    allowed_attachments: &HashSet<String>,
) -> Result<Value> {
    let input_size = serde_json::to_vec(body)
        .map_err(|_| SyncError::BadRequest("Shared note body is invalid".to_string()))?
        .len();
    if input_size > MAX_SNAPSHOT_BODY_BYTES {
        return Err(SyncError::BadRequest(
            "Shared note body is too large".to_string(),
        ));
    }

    let root = body
        .as_object()
        .ok_or_else(|| SyncError::BadRequest("Shared note body must be a document".to_string()))?;
    if root.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(SyncError::BadRequest(
            "Shared note body must be a document".to_string(),
        ));
    }

    let mut budget = SanitizeBudget {
        nodes: 0,
        allowed_attachments,
    };
    budget.visit(0)?;
    let mut content = sanitize_blocks(node_content(root)?, 1, &mut budget)?;
    if content.is_empty() {
        content.push(json!({ "type": "paragraph" }));
    }

    let body = json!({
        "type": "doc",
        "content": content,
    });
    let output_size = serde_json::to_vec(&body)
        .map_err(|_| SyncError::BadRequest("Shared note body is invalid".to_string()))?
        .len();
    if output_size > MAX_SNAPSHOT_BODY_BYTES {
        return Err(SyncError::BadRequest(
            "Shared note body is too large".to_string(),
        ));
    }

    Ok(body)
}

struct SanitizeBudget<'a> {
    nodes: usize,
    allowed_attachments: &'a HashSet<String>,
}

impl SanitizeBudget<'_> {
    fn visit(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_SNAPSHOT_DEPTH {
            return Err(SyncError::BadRequest(
                "Shared note body is nested too deeply".to_string(),
            ));
        }
        self.nodes += 1;
        if self.nodes > MAX_SNAPSHOT_NODES {
            return Err(SyncError::BadRequest(
                "Shared note body has too many nodes".to_string(),
            ));
        }
        Ok(())
    }
}

fn node_content(node: &Map<String, Value>) -> Result<&[Value]> {
    match node.get("content") {
        None => Ok(&[]),
        Some(Value::Array(content)) => Ok(content),
        Some(_) => Err(SyncError::BadRequest(
            "Shared note body contains invalid content".to_string(),
        )),
    }
}

fn sanitize_blocks(
    content: &[Value],
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Vec<Value>> {
    let mut sanitized = Vec::new();
    for node in content {
        if let Some(node) = sanitize_block(node, depth, budget)? {
            sanitized.push(node);
        }
    }
    Ok(sanitized)
}

fn sanitize_block(
    node: &Value,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    budget.visit(depth)?;
    let node = node.as_object().ok_or_else(|| {
        SyncError::BadRequest("Shared note body contains an invalid node".to_string())
    })?;
    let node_type = node.get("type").and_then(Value::as_str).ok_or_else(|| {
        SyncError::BadRequest("Shared note body contains an invalid node".to_string())
    })?;

    let sanitized = match node_type {
        "paragraph" => Some(content_node(
            "paragraph",
            None,
            sanitize_inlines(node_content(node)?, depth + 1, budget)?,
        )),
        "heading" => {
            let level = positive_integer_attr(node, "level", 6).unwrap_or(1);
            Some(content_node(
                "heading",
                Some(json!({ "level": level })),
                sanitize_inlines(node_content(node)?, depth + 1, budget)?,
            ))
        }
        "blockquote" => {
            let mut content = sanitize_blocks(node_content(node)?, depth + 1, budget)?;
            ensure_block_content(&mut content);
            Some(content_node("blockquote", None, content))
        }
        "codeBlock" => Some(content_node(
            "codeBlock",
            None,
            sanitize_code(node_content(node)?, depth + 1, budget)?,
        )),
        "horizontalRule" => Some(json!({ "type": "horizontalRule" })),
        "bulletList" => sanitize_list(node, "bulletList", depth, budget)?,
        "orderedList" => {
            let start = positive_integer_attr(node, "start", 1_000_000).unwrap_or(1);
            sanitize_list_with_attrs(
                node,
                "orderedList",
                Some(json!({ "start": start })),
                depth,
                budget,
            )?
        }
        "taskList" => sanitize_task_list(node, depth, budget)?,
        "table" => sanitize_table(node, depth, budget)?,
        "session" => {
            let mut content = sanitize_blocks(node_content(node)?, depth + 1, budget)?;
            ensure_block_content(&mut content);
            Some(content_node("blockquote", None, content))
        }
        "image" | "fileAttachment" | "clip" => Some(sanitize_attachment(
            node,
            node_type,
            budget.allowed_attachments,
        )),
        _ => None,
    };

    Ok(sanitized)
}

fn sanitize_attachment(
    node: &Map<String, Value>,
    node_type: &str,
    allowed_attachments: &HashSet<String>,
) -> Value {
    let attachment_id = node
        .get("attrs")
        .and_then(Value::as_object)
        .and_then(|attrs| string_attr(attrs, "sharedAttachmentId"))
        .filter(|id| allowed_attachments.contains(*id));
    match attachment_id {
        Some(attachment_id) => json!({
            "type": node_type,
            "attrs": { "sharedAttachmentId": attachment_id },
        }),
        None => placeholder("Attachment omitted"),
    }
}

fn sanitize_inlines(
    content: &[Value],
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Vec<Value>> {
    let mut sanitized = Vec::new();
    for node in content {
        if let Some(node) = sanitize_inline(node, depth, budget)? {
            sanitized.push(node);
        }
    }
    Ok(sanitized)
}

fn sanitize_inline(
    node: &Value,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    budget.visit(depth)?;
    let node = node.as_object().ok_or_else(|| {
        SyncError::BadRequest("Shared note body contains an invalid node".to_string())
    })?;
    let node_type = node.get("type").and_then(Value::as_str).ok_or_else(|| {
        SyncError::BadRequest("Shared note body contains an invalid node".to_string())
    })?;

    match node_type {
        "text" => sanitize_text_node(node),
        "hardBreak" => Ok(Some(json!({ "type": "hardBreak" }))),
        "mention-@" => Ok(string_attr(node, "label")
            .filter(|label| !label.is_empty())
            .map(text_node)),
        "appLink" => Ok(Some(sanitize_app_link(node))),
        _ => Ok(None),
    }
}

fn sanitize_code(
    content: &[Value],
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Vec<Value>> {
    let mut sanitized = Vec::new();
    for node in content {
        budget.visit(depth)?;
        let node = node.as_object().ok_or_else(|| {
            SyncError::BadRequest("Shared note body contains an invalid node".to_string())
        })?;
        if node.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        if let Some(text) = string_attr(node, "text").filter(|text| !text.is_empty()) {
            sanitized.push(text_node(text));
        }
    }
    Ok(sanitized)
}

fn sanitize_text_node(node: &Map<String, Value>) -> Result<Option<Value>> {
    let Some(text) = string_attr(node, "text").filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let marks = sanitize_marks(node.get("marks"))?;
    let mut sanitized = Map::from_iter([
        ("type".to_string(), Value::String("text".to_string())),
        ("text".to_string(), Value::String(text.to_string())),
    ]);
    if !marks.is_empty() {
        sanitized.insert("marks".to_string(), Value::Array(marks));
    }
    Ok(Some(Value::Object(sanitized)))
}

fn sanitize_marks(marks: Option<&Value>) -> Result<Vec<Value>> {
    let Some(marks) = marks else {
        return Ok(Vec::new());
    };
    let marks = marks.as_array().ok_or_else(|| {
        SyncError::BadRequest("Shared note body contains invalid formatting".to_string())
    })?;
    let mut sanitized = Vec::new();
    let mut seen = HashSet::new();

    for mark in marks {
        let Some(mark) = mark.as_object() else {
            continue;
        };
        let Some(mark_type) = mark.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(mark_type) {
            continue;
        }
        match mark_type {
            "bold" | "italic" | "strike" | "highlight" => {
                sanitized.push(json!({ "type": mark_type }));
            }
            "code" => return Ok(vec![json!({ "type": "code" })]),
            "link" => {
                if let Some(href) = mark
                    .get("attrs")
                    .and_then(Value::as_object)
                    .and_then(|attrs| string_attr(attrs, "href"))
                    .and_then(safe_link)
                {
                    sanitized.push(json!({
                        "type": "link",
                        "attrs": { "href": href },
                    }));
                }
            }
            _ => {}
        }
    }

    Ok(sanitized)
}

fn sanitize_app_link(node: &Map<String, Value>) -> Value {
    let href = string_attr(node, "url").and_then(safe_link);
    let label = string_attr(node, "resourceTitle")
        .filter(|label| !label.is_empty())
        .map(ToString::to_string)
        .or_else(|| href.clone())
        .unwrap_or_else(|| "Link omitted".to_string());
    let mut node = text_node(&label);
    if let (Some(href), Some(node)) = (href, node.as_object_mut()) {
        node.insert(
            "marks".to_string(),
            json!([{ "type": "link", "attrs": { "href": href } }]),
        );
    }
    node
}

fn sanitize_list(
    node: &Map<String, Value>,
    node_type: &str,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    sanitize_list_with_attrs(node, node_type, None, depth, budget)
}

fn sanitize_list_with_attrs(
    node: &Map<String, Value>,
    node_type: &str,
    attrs: Option<Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    let mut content = Vec::new();
    for child in node_content(node)? {
        let Some(child) = child.as_object() else {
            return Err(SyncError::BadRequest(
                "Shared note body contains an invalid node".to_string(),
            ));
        };
        if child.get("type").and_then(Value::as_str) != Some("listItem") {
            budget.visit(depth + 1)?;
            continue;
        }
        if let Some(item) = sanitize_list_item(child, "listItem", None, depth + 1, budget)? {
            content.push(item);
        }
    }
    Ok((!content.is_empty()).then(|| content_node(node_type, attrs, content)))
}

fn sanitize_list_item(
    node: &Map<String, Value>,
    node_type: &str,
    attrs: Option<Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    budget.visit(depth)?;
    let mut content = sanitize_blocks(node_content(node)?, depth + 1, budget)?;
    ensure_paragraph_first(&mut content);
    Ok(Some(content_node(node_type, attrs, content)))
}

fn sanitize_task_list(
    node: &Map<String, Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    let mut content = Vec::new();
    for child in node_content(node)? {
        let Some(child) = child.as_object() else {
            return Err(SyncError::BadRequest(
                "Shared note body contains an invalid node".to_string(),
            ));
        };
        if child.get("type").and_then(Value::as_str) != Some("taskItem") {
            budget.visit(depth + 1)?;
            continue;
        }
        if let Some(item) = sanitize_task_item(child, depth + 1, budget)? {
            content.push(item);
        }
    }
    Ok((!content.is_empty()).then(|| content_node("taskList", None, content)))
}

fn sanitize_task_item(
    node: &Map<String, Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    let status = match string_attr(node, "status") {
        Some("todo") => "todo",
        Some("in_progress") => "in_progress",
        Some("done") => "done",
        _ if bool_attr(node, "checked") == Some(true) => "done",
        _ => "todo",
    };
    sanitize_list_item(
        node,
        "taskItem",
        Some(json!({
            "status": status,
            "checked": status == "done",
        })),
        depth,
        budget,
    )
}

fn sanitize_table(
    node: &Map<String, Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    let mut content = Vec::new();
    for child in node_content(node)? {
        let Some(child) = child.as_object() else {
            return Err(SyncError::BadRequest(
                "Shared note body contains an invalid node".to_string(),
            ));
        };
        if child.get("type").and_then(Value::as_str) != Some("tableRow") {
            budget.visit(depth + 1)?;
            continue;
        }
        if let Some(row) = sanitize_table_row(child, depth + 1, budget)? {
            content.push(row);
        }
    }
    Ok((!content.is_empty()).then(|| content_node("table", None, content)))
}

fn sanitize_table_row(
    node: &Map<String, Value>,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    budget.visit(depth)?;
    let mut content = Vec::new();
    for child in node_content(node)? {
        let Some(child) = child.as_object() else {
            return Err(SyncError::BadRequest(
                "Shared note body contains an invalid node".to_string(),
            ));
        };
        let Some(child_type @ ("tableCell" | "tableHeader")) =
            child.get("type").and_then(Value::as_str)
        else {
            budget.visit(depth + 1)?;
            continue;
        };
        if let Some(cell) = sanitize_table_cell(child, child_type, depth + 1, budget)? {
            content.push(cell);
        }
    }
    Ok((!content.is_empty()).then(|| content_node("tableRow", None, content)))
}

fn sanitize_table_cell(
    node: &Map<String, Value>,
    node_type: &str,
    depth: usize,
    budget: &mut SanitizeBudget,
) -> Result<Option<Value>> {
    budget.visit(depth)?;
    let colspan = positive_integer_attr(node, "colspan", 1000).unwrap_or(1);
    let rowspan = positive_integer_attr(node, "rowspan", 1000).unwrap_or(1);
    let mut content = sanitize_blocks(node_content(node)?, depth + 1, budget)?;
    ensure_block_content(&mut content);
    Ok(Some(content_node(
        node_type,
        Some(json!({
            "colspan": colspan,
            "rowspan": rowspan,
        })),
        content,
    )))
}

fn content_node(node_type: &str, attrs: Option<Value>, content: Vec<Value>) -> Value {
    let mut node = Map::new();
    node.insert("type".to_string(), Value::String(node_type.to_string()));
    if let Some(attrs) = attrs {
        node.insert("attrs".to_string(), attrs);
    }
    if !content.is_empty() {
        node.insert("content".to_string(), Value::Array(content));
    }
    Value::Object(node)
}

fn placeholder(label: &str) -> Value {
    content_node("paragraph", None, vec![text_node(label)])
}

fn text_node(text: &str) -> Value {
    json!({
        "type": "text",
        "text": text,
    })
}

fn ensure_block_content(content: &mut Vec<Value>) {
    if content.is_empty() {
        content.push(json!({ "type": "paragraph" }));
    }
}

fn ensure_paragraph_first(content: &mut Vec<Value>) {
    if content
        .first()
        .and_then(|node| node.get("type"))
        .and_then(Value::as_str)
        != Some("paragraph")
    {
        content.insert(0, json!({ "type": "paragraph" }));
    }
}

fn positive_integer_attr(node: &Map<String, Value>, name: &str, max: u64) -> Option<u64> {
    node.get("attrs")
        .and_then(Value::as_object)
        .and_then(|attrs| attrs.get(name))
        .and_then(Value::as_u64)
        .filter(|value| (1..=max).contains(value))
}

fn string_attr<'a>(node: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    node.get(name)
        .or_else(|| node.get("attrs").and_then(Value::as_object)?.get(name))
        .and_then(Value::as_str)
}

fn bool_attr(node: &Map<String, Value>, name: &str) -> Option<bool> {
    node.get(name)
        .or_else(|| node.get("attrs").and_then(Value::as_object)?.get(name))
        .and_then(Value::as_bool)
}

fn safe_link(href: &str) -> Option<String> {
    let url = reqwest::Url::parse(href.trim()).ok()?;
    match url.scheme() {
        "http" | "https"
            if url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none() => {}
        "mailto" if !url.path().is_empty() => {}
        _ => return None,
    }
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn keeps_safe_structure_and_formatting() {
        let sanitized = sanitize_document(&json!({
            "type": "doc",
            "content": [
                {
                    "type": "heading",
                    "attrs": { "level": 2, "secret": "drop" },
                    "content": [{
                        "type": "text",
                        "text": "Shared",
                        "marks": [
                            { "type": "bold", "attrs": { "secret": true } },
                            {
                                "type": "link",
                                "attrs": {
                                    "href": "https://example.com/note?q=1",
                                    "target": "_blank"
                                }
                            }
                        ]
                    }]
                },
                {
                    "type": "taskList",
                    "content": [{
                        "type": "taskItem",
                        "attrs": {
                            "status": "in_progress",
                            "checked": true,
                            "taskId": "private-task",
                            "taskItemId": "private-item"
                        },
                        "content": [{ "type": "paragraph" }]
                    }]
                },
                {
                    "type": "table",
                    "content": [{
                        "type": "tableRow",
                        "content": [{
                            "type": "tableCell",
                            "attrs": { "colspan": 2, "rowspan": 1, "colwidth": [200, 200] },
                            "content": [{ "type": "paragraph" }]
                        }]
                    }]
                }
            ]
        }))
        .unwrap();

        assert_eq!(sanitized["content"][0]["attrs"], json!({ "level": 2 }));
        assert_eq!(
            sanitized["content"][0]["content"][0]["marks"],
            json!([
                { "type": "bold" },
                {
                    "type": "link",
                    "attrs": { "href": "https://example.com/note?q=1" }
                }
            ])
        );
        assert_eq!(
            sanitized["content"][1]["content"][0]["attrs"],
            json!({ "status": "in_progress", "checked": false })
        );
        assert_eq!(
            sanitized["content"][2]["content"][0]["content"][0]["attrs"],
            json!({ "colspan": 2, "rowspan": 1 })
        );
        let serialized = sanitized.to_string();
        assert!(!serialized.contains("private-task"));
        assert!(!serialized.contains("private-item"));
        assert!(!serialized.contains("colwidth"));
        assert!(!serialized.contains("target"));
        assert!(!serialized.contains("secret"));
    }

    #[test]
    fn removes_private_references_and_unsafe_links() {
        let sanitized = sanitize_document(&json!({
            "type": "doc",
            "attrs": { "workspaceId": "private-workspace" },
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {
                            "type": "mention-@",
                            "attrs": {
                                "id": "private-mention-id",
                                "type": "session",
                                "label": "Planning"
                            }
                        },
                        {
                            "type": "text",
                            "text": " unsafe",
                            "marks": [{
                                "type": "link",
                                "attrs": { "href": "javascript:alert(1)" }
                            }, { "type": "privateMark" }]
                        },
                        {
                            "type": "appLink",
                            "attrs": {
                                "url": "file:///Users/alice/secret.txt",
                                "resourceTitle": "Repository",
                                "workspace": "private-connector-workspace",
                                "channelId": "private-channel",
                                "resourceId": "private-resource"
                            }
                        }
                    ]
                },
                {
                    "type": "image",
                    "attrs": {
                        "src": "asset://localhost/Users/alice/note/secret.png",
                        "attachmentId": "private-image-id"
                    }
                },
                {
                    "type": "fileAttachment",
                    "attrs": {
                        "path": "/Users/alice/note/secret.wav",
                        "attachmentId": "private-audio-id"
                    }
                },
                {
                    "type": "clip",
                    "attrs": { "src": "https://youtube.com/private-video" }
                },
                {
                    "type": "session",
                    "attrs": { "sessionId": "private-session-id" },
                    "content": [{
                        "type": "paragraph",
                        "content": [{ "type": "text", "text": "Linked session" }]
                    }]
                },
                {
                    "type": "unknownPrivateNode",
                    "attrs": { "id": "private-unknown-id" }
                }
            ]
        }))
        .unwrap();

        let serialized = sanitized.to_string();
        assert!(serialized.contains("Planning"));
        assert!(serialized.contains("Repository"));
        assert!(serialized.contains("Linked session"));
        assert!(serialized.contains("Attachment omitted"));
        for private_value in [
            "private-workspace",
            "private-mention-id",
            "javascript",
            "/Users/alice",
            "private-image-id",
            "private-audio-id",
            "youtube",
            "private-session-id",
            "private-connector-workspace",
            "private-channel",
            "private-resource",
            "private-unknown-id",
            "privateMark",
        ] {
            assert!(
                !serialized.contains(private_value),
                "{private_value} leaked"
            );
        }
    }

    #[test]
    fn keeps_only_explicitly_selected_server_attachments() {
        let selected = "22222222-2222-4222-8222-222222222222".to_string();
        let allowed = HashSet::from([selected.clone()]);
        let sanitized = sanitize_document_with_attachments(
            &json!({
                "type": "doc",
                "content": [
                    {
                        "type": "image",
                        "attrs": {
                            "sharedAttachmentId": selected,
                            "src": "asset://localhost/private.png",
                            "attachmentId": "local-private.png"
                        }
                    },
                    {
                        "type": "fileAttachment",
                        "attrs": {
                            "sharedAttachmentId": "33333333-3333-4333-8333-333333333333",
                            "path": "/Users/alice/private.pdf"
                        }
                    }
                ]
            }),
            &allowed,
        )
        .unwrap();

        assert_eq!(
            sanitized["content"][0],
            json!({
                "type": "image",
                "attrs": { "sharedAttachmentId": selected }
            })
        );
        let serialized = sanitized.to_string();
        assert!(serialized.contains("Attachment omitted"));
        assert!(!serialized.contains("asset://"));
        assert!(!serialized.contains("/Users/alice"));
        assert!(!serialized.contains("local-private"));
    }

    #[test]
    fn accepts_only_safe_absolute_links() {
        let sanitized = sanitize_document(&json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    {
                        "type": "text",
                        "text": "web",
                        "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }]
                    },
                    {
                        "type": "text",
                        "text": " mail",
                        "marks": [{ "type": "link", "attrs": { "href": "mailto:hi@example.com" } }]
                    },
                    {
                        "type": "text",
                        "text": " local",
                        "marks": [{ "type": "link", "attrs": { "href": "/local/path" } }]
                    }
                ]
            }]
        }))
        .unwrap();

        assert_eq!(
            sanitized.to_string().matches("\"type\":\"link\"").count(),
            2
        );
        assert!(!sanitized.to_string().contains("/local/path"));
    }

    #[test]
    fn rejects_invalid_or_oversized_documents() {
        assert!(sanitize_document(&json!({ "type": "paragraph" })).is_err());
        assert!(sanitize_document(&json!({ "type": "doc", "content": {} })).is_err());
        assert!(sanitize_title(&"x".repeat(MAX_SNAPSHOT_TITLE_BYTES + 1)).is_err());
        assert!(
            sanitize_document(&json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": "x".repeat(MAX_SNAPSHOT_BODY_BYTES)
                    }]
                }]
            }))
            .is_err()
        );
    }
}
