//! `packages/editor/src/tasks.ts` and `plugins/task-identity.ts`: task items
//! carry `{ status, checked, taskId, taskItemId }`, every item in a document
//! has unique non-empty ids, and the items of a note are mirrored into
//! `action_items` rows (`apps/desktop/src/editor-bridge/task-storage.ts`).
//! `hydrateTaskContent` is not ported: the web editor computes it from the
//! task store's snapshot at mount, which is still empty when a note opens, so
//! the shipping app opens notes as stored (verified: a nested task list
//! reopens with its three items and no appended copy).

use serde_json::{Map, Value, json};

pub const DEFAULT_STATUS: &str = "todo";

pub fn is_task_status(value: &str) -> bool {
    matches!(value, "todo" | "in_progress" | "done")
}

/// `normalizeTaskStatus(status, checked)`
pub fn normalize_status(status: Option<&Value>, checked: Option<&Value>) -> &'static str {
    match status {
        Some(Value::Bool(true)) => return "done",
        Some(Value::Bool(false)) => return "todo",
        Some(Value::String(status)) if is_task_status(status) => {
            return match status.as_str() {
                "done" => "done",
                "in_progress" => "in_progress",
                _ => "todo",
            };
        }
        _ => {}
    }
    match checked {
        Some(Value::Bool(true)) => "done",
        Some(Value::Bool(false)) => "todo",
        _ => DEFAULT_STATUS,
    }
}

pub fn next_status(status: &str) -> &'static str {
    if status == "done" { "todo" } else { "done" }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// `createTaskItemAttrs`: attrs in the schema's order (`status`, `checked`,
/// `taskId`, `taskItemId`), which is the order `node.toJSON()` emits.
pub fn item_attrs(status: &str, task_id: &str, task_item_id: &str) -> Value {
    json!({
        "status": status,
        "checked": status == "done",
        "taskId": task_id,
        "taskItemId": task_item_id,
    })
}

/// `createTaskStatusAttrs` applied over existing attrs (`setNodeMarkup` with
/// `{ ...node.attrs, status, checked }`).
pub fn set_status(attrs: &mut Map<String, Value>, status: &str) {
    attrs.insert("status".into(), Value::String(status.to_string()));
    attrs.insert("checked".into(), Value::Bool(status == "done"));
    *attrs = in_schema_order(std::mem::take(attrs));
}

/// `computeAttrs`: ProseMirror emits a node's attrs in the spec's order
/// (`status`, `checked`, `taskId`, `taskItemId`) whatever the input held.
pub fn in_schema_order(mut attrs: Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    for key in ["status", "checked", "taskId", "taskItemId"] {
        if let Some(value) = attrs.remove(key) {
            ordered.insert(key.into(), value);
        }
    }
    ordered.extend(attrs);
    ordered
}

pub fn item_status(node: &Value) -> &'static str {
    let attrs = node.get("attrs");
    normalize_status(
        attrs.and_then(|a| a.get("status")),
        attrs.and_then(|a| a.get("checked")),
    )
}

fn non_empty_id(attrs: Option<&Value>, key: &str) -> Option<String> {
    attrs
        .and_then(|attrs| attrs.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// `taskIdentityPlugin` / `normalizeTaskContent`: walk the tree once and give
/// every `taskItem` unique, non-empty `taskId` / `taskItemId` values, keeping
/// the first occurrence of a duplicate. Returns whether anything changed.
pub fn ensure_identity(node: &mut Value) -> bool {
    let mut seen_tasks = std::collections::HashSet::new();
    let mut seen_items = std::collections::HashSet::new();
    ensure_identity_inner(node, &mut seen_tasks, &mut seen_items, &mut new_id)
}

fn ensure_identity_inner(
    node: &mut Value,
    seen_tasks: &mut std::collections::HashSet<String>,
    seen_items: &mut std::collections::HashSet<String>,
    fresh: &mut dyn FnMut() -> String,
) -> bool {
    let mut changed = false;
    if node.get("type").and_then(Value::as_str) == Some("taskItem") {
        let attrs = node.get("attrs");
        let mut task_id = non_empty_id(attrs, "taskId").unwrap_or_default();
        while task_id.is_empty() || seen_tasks.contains(&task_id) {
            task_id = fresh();
        }
        let mut item_id = non_empty_id(attrs, "taskItemId").unwrap_or_default();
        while item_id.is_empty() || seen_items.contains(&item_id) {
            item_id = fresh();
        }
        seen_tasks.insert(task_id.clone());
        seen_items.insert(item_id.clone());
        let current_task = attrs.and_then(|a| a.get("taskId")).and_then(Value::as_str);
        let current_item = attrs
            .and_then(|a| a.get("taskItemId"))
            .and_then(Value::as_str);
        if current_task != Some(task_id.as_str()) || current_item != Some(item_id.as_str()) {
            let object = node.as_object_mut().expect("node object");
            let mut attrs = object
                .get("attrs")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            // A missing attr set still serialises in schema order.
            if !attrs.contains_key("status") {
                attrs.insert("status".into(), Value::String(DEFAULT_STATUS.into()));
            }
            if !attrs.contains_key("checked") {
                attrs.insert("checked".into(), Value::Bool(false));
            }
            attrs.insert("taskId".into(), Value::String(task_id));
            attrs.insert("taskItemId".into(), Value::String(item_id));
            // Keep TipTap's key order: type, attrs, content.
            let content = object.remove("content");
            object.remove("attrs");
            object.insert("attrs".into(), Value::Object(in_schema_order(attrs)));
            if let Some(content) = content {
                object.insert("content".into(), content);
            }
            changed = true;
        }
    }
    if let Some(children) = node.get_mut("content").and_then(Value::as_array_mut) {
        for child in children {
            changed |= ensure_identity_inner(child, seen_tasks, seen_items, fresh);
        }
    }
    changed
}

/// `TaskRecord` as `extractTasksFromContent` builds it for one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub task_id: String,
    pub source_order: usize,
    pub status: String,
    pub text_preview: String,
    /// The item's content array, serialised.
    pub body_json: String,
}

/// `extractTasksFromContent`: every `taskItem` with a `taskId`, in document
/// order, with `getTaskItemTextContent` (the first paragraph's text).
pub fn extract_tasks(doc: &Value) -> Vec<TaskRecord> {
    let mut tasks = Vec::new();
    walk(doc, &mut |node| {
        if node.get("type").and_then(Value::as_str) != Some("taskItem") {
            return;
        }
        let Some(task_id) = non_empty_id(node.get("attrs"), "taskId") else {
            return;
        };
        let content = node
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let paragraph = content
            .iter()
            .find(|child| child.get("type").and_then(Value::as_str) == Some("paragraph"));
        tasks.push(TaskRecord {
            task_id,
            source_order: tasks.len(),
            status: item_status(node).to_string(),
            text_preview: paragraph.map(text_content).unwrap_or_default(),
            body_json: Value::Array(content).to_string(),
        });
    });
    tasks
}

fn walk(node: &Value, visit: &mut dyn FnMut(&Value)) {
    visit(node);
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            walk(child, visit);
        }
    }
}

/// `getNodeTextContent`
fn text_content(node: &Value) -> String {
    let mut out = String::new();
    walk(node, &mut |child| {
        if let Some(text) = child.get("text").and_then(Value::as_str) {
            out.push_str(text);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_normalises_like_the_editor() {
        assert_eq!(normalize_status(Some(&json!(true)), None), "done");
        assert_eq!(
            normalize_status(Some(&json!("in_progress")), None),
            "in_progress"
        );
        assert_eq!(
            normalize_status(Some(&json!("bogus")), Some(&json!(true))),
            "done"
        );
        assert_eq!(normalize_status(None, Some(&json!(false))), "todo");
        assert_eq!(normalize_status(None, None), "todo");
        assert_eq!(next_status("done"), "todo");
        assert_eq!(next_status("in_progress"), "done");
    }

    #[test]
    fn identity_fills_missing_and_duplicate_ids_keeping_the_first() {
        let mut doc = json!({ "type": "doc", "content": [{ "type": "taskList", "content": [
            { "type": "taskItem", "attrs": { "status": "todo", "checked": false, "taskId": "a", "taskItemId": "i" }, "content": [{ "type": "paragraph" }] },
            { "type": "taskItem", "attrs": { "status": "done", "checked": true, "taskId": "a", "taskItemId": "i" }, "content": [{ "type": "paragraph" }] },
            { "type": "taskItem", "attrs": { "checked": true }, "content": [{ "type": "paragraph" }] },
        ]}]});
        let mut counter = 0;
        let mut fresh = || {
            counter += 1;
            format!("id-{counter}")
        };
        let changed = ensure_identity_inner(
            &mut doc,
            &mut Default::default(),
            &mut Default::default(),
            &mut fresh,
        );
        assert!(changed);
        let items = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(items[0]["attrs"]["taskId"], "a");
        assert_eq!(items[1]["attrs"]["taskId"], "id-1");
        assert_eq!(items[1]["attrs"]["taskItemId"], "id-2");
        assert_eq!(items[1]["attrs"]["status"], "done");
        assert_eq!(items[2]["attrs"]["taskId"], "id-3");
        // Attr key order follows the schema.
        assert_eq!(
            items[2]["attrs"].to_string(),
            r#"{"status":"todo","checked":true,"taskId":"id-3","taskItemId":"id-4"}"#
        );
        assert!(!ensure_identity(&mut doc.clone()));
    }

    #[test]
    fn extract_lists_items_in_order_with_previews() {
        let doc = json!({ "type": "doc", "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "Plan" }] },
            { "type": "taskList", "content": [
                { "type": "taskItem", "attrs": item_attrs("done", "t1", "i1"), "content": [
                    { "type": "paragraph", "content": [{ "type": "text", "text": "Ship " }, { "type": "text", "marks": [{ "type": "bold" }], "text": "it" }] }
                ]},
                { "type": "taskItem", "attrs": item_attrs("todo", "t2", "i2"), "content": [{ "type": "paragraph" }] },
                { "type": "taskItem", "attrs": { "status": "todo", "checked": false, "taskId": "", "taskItemId": "x" }, "content": [{ "type": "paragraph" }] }
            ]}
        ]});
        let tasks = extract_tasks(&doc);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].task_id, "t1");
        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].text_preview, "Ship it");
        assert_eq!(tasks[0].source_order, 0);
        assert!(tasks[0].body_json.starts_with(r#"[{"type":"paragraph""#));
        assert_eq!(tasks[1].source_order, 1);
        assert_eq!(tasks[1].text_preview, "");
        assert_eq!(tasks[1].body_json, r#"[{"type":"paragraph"}]"#);
    }
}
