//! Shipping's editor task storage (`editor-bridge/task-storage.ts`, `@anlg/editor/tasks`):
//! each saved memo or summary mirrors its task items into `action_items`, which automations,
//! the Cloud API and MCP read.

use std::collections::HashSet;

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{DocumentSnapshot, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

/// `resolvedOwnerSql`: the owner, else the CloudSync binding's account or workspace.
const RESOLVED_OWNER: &str = "COALESCE(NULLIF(?, ?), (SELECT NULLIF(json_extract(value_json, '$.account_user_id'), '') FROM app_settings WHERE id = 'cloudsync_workspace_binding'), (SELECT NULLIF(json_extract(value_json, '$.workspace_id'), '') FROM app_settings WHERE id = 'cloudsync_workspace_binding'))";

/// `useOwnerUserId`.
const OWNER_SQL: &str = "SELECT user_id FROM (SELECT owner_user_id AS user_id, updated_at, 0 AS source_priority FROM sessions WHERE owner_user_id <> '' AND deleted_at IS NULL UNION ALL SELECT id AS user_id, updated_at, 1 AS source_priority FROM humans WHERE id = owner_user_id AND id <> '' AND deleted_at IS NULL UNION ALL SELECT owner_user_id AS user_id, updated_at, 2 AS source_priority FROM chat_groups WHERE owner_user_id <> '' AND deleted_at IS NULL) ORDER BY source_priority, updated_at DESC, user_id LIMIT 1";

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub order: i64,
    pub status: &'static str,
    pub text: String,
    /// The task item's content, as `JSON.stringify(task.body)`.
    pub body: String,
}

/// `normalizeTaskStatus`.
fn status(attrs: &Value) -> &'static str {
    match attrs["status"].as_str() {
        _ if attrs["status"] == true => "done",
        _ if attrs["status"] == false => "todo",
        Some("todo") => "todo",
        Some("in_progress") => "in_progress",
        Some("done") => "done",
        _ if attrs["checked"] == true => "done",
        _ => "todo",
    }
}

/// `getNodeTextContent`: leaves keep their text; inner nodes join children with spaces.
fn node_text(node: &Value) -> String {
    if let Some(text) = node["text"].as_str() {
        return text.to_owned();
    }
    let joined = node["content"]
        .as_array()
        .map(|children| children.iter().map(node_text).collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `extractTasksFromContent`: task items with an id, in document order.
pub fn extract(doc: &Value) -> Vec<Task> {
    fn walk(node: &Value, seen: &mut HashSet<String>, tasks: &mut Vec<Task>) {
        if node["type"] == "taskItem"
            && let Some(id) = node["attrs"]["taskId"]
                .as_str()
                .filter(|id| !id.trim().is_empty())
            && seen.insert(id.to_owned())
        {
            let content = node["content"].as_array().cloned().unwrap_or_default();
            let text = content
                .iter()
                .find(|child| child["type"] == "paragraph")
                .map(node_text)
                .unwrap_or_default();
            tasks.push(Task {
                id: id.to_owned(),
                order: tasks.len() as i64,
                status: status(&node["attrs"]),
                text,
                body: Value::Array(content).to_string(),
            });
        }
        for child in node["content"].as_array().into_iter().flatten() {
            walk(child, seen, tasks);
        }
    }
    let mut tasks = Vec::new();
    walk(doc, &mut HashSet::new(), &mut tasks);
    tasks
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

fn statement(sql: String, params: Vec<Value>) -> TransactionStatement {
    TransactionStatement {
        sql,
        params,
        expected_rows_affected: None,
    }
}

/// Mirrors the tasks of a saved memo (`session_raw_note`) or summary (`enhanced_note`) into
/// `action_items`, like shipping's `upsertTasksForSource`: unchanged tasks write nothing;
/// otherwise removed tasks are soft-deleted and every current task is upserted.
pub fn sync(runtime: &RuntimeHandle, saved: &DocumentSnapshot, body: &str) -> Result<()> {
    let doc: Value = serde_json::from_str(body).map_err(failure)?;
    let tasks = extract(&doc);
    let document = saved.id.0.to_string();
    let session = saved.session_id.0.to_string();
    let reply = runtime.submit(move |services| async move {
        let executor = &services.executor;
        let kind = executor
            .execute(
                "SELECT kind FROM session_documents WHERE id = ? AND deleted_at IS NULL".into(),
                vec![json!(document)],
            )
            .await
            .map_err(failure)?;
        let (source_type, source_id) = match kind.first().and_then(|row| row["kind"].as_str()) {
            Some("note") => ("session_raw_note", session),
            Some("summary" | "template_output") => ("enhanced_note", document),
            _ => return Ok(()),
        };
        let current = executor
            .execute(
                "SELECT id, source_order, status, text, body_json FROM action_items WHERE source_type = ? AND source_id = ? AND deleted_at IS NULL ORDER BY source_order, id".into(),
                vec![json!(source_type), json!(source_id)],
            )
            .await
            .map_err(failure)?;
        let unchanged = current.len() == tasks.len()
            && current.iter().zip(&tasks).all(|(row, task)| {
                row["id"] == task.id.as_str()
                    && row["source_order"].as_i64() == Some(task.order)
                    && row["status"] == task.status
                    && row["text"] == task.text.as_str()
                    && row["body_json"] == task.body.as_str()
            });
        if unchanged {
            return Ok(());
        }
        let owner = executor
            .execute(OWNER_SQL.into(), vec![])
            .await
            .map_err(failure)?
            .first()
            .and_then(|row| row["user_id"].as_str())
            .map(str::trim)
            .filter(|owner| !owner.is_empty())
            .unwrap_or(DEFAULT_USER_ID)
            .to_owned();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let retained = json!(tasks.iter().map(|task| task.id.as_str()).collect::<Vec<_>>()).to_string();
        let mut statements = vec![statement(
            format!("UPDATE action_items SET deleted_at = ?, updated_at = ?, updated_by = {RESOLVED_OWNER} WHERE source_type = ? AND source_id = ? AND deleted_at IS NULL AND id NOT IN (SELECT value FROM json_each(?))"),
            vec![json!(now), json!(now), json!(owner), json!(DEFAULT_USER_ID), json!(source_type), json!(source_id), json!(retained)],
        )];
        // `session_id` stays empty: shipping sets it only for `session` sources. A due date
        // set elsewhere is kept, as shipping carries the stored one forward.
        let session_id = if source_type == "session" { source_id.as_str() } else { "" };
        for task in &tasks {
            statements.push(statement(
                format!("INSERT INTO action_items (id, workspace_id, session_id, source_type, source_id, source_order, assignee_human_id, status, text, body_json, due_at, created_by, updated_by, metadata_json, created_at, updated_at, deleted_at) VALUES (?, COALESCE((SELECT NULLIF(workspace_id, '') FROM sessions WHERE id = ? AND deleted_at IS NULL), (SELECT NULLIF(json_extract(value_json, '$.workspace_id'), '') FROM app_settings WHERE id = 'cloudsync_workspace_binding')), ?, ?, ?, ?, '', ?, ?, ?, '', {RESOLVED_OWNER}, {RESOLVED_OWNER}, '{{}}', ?, ?, NULL) ON CONFLICT(id) DO UPDATE SET session_id = excluded.session_id, source_type = excluded.source_type, source_id = excluded.source_id, source_order = excluded.source_order, status = excluded.status, text = excluded.text, body_json = excluded.body_json, due_at = CASE WHEN action_items.deleted_at IS NULL THEN action_items.due_at ELSE excluded.due_at END, updated_by = excluded.updated_by, updated_at = excluded.updated_at, deleted_at = NULL"),
                vec![
                    json!(task.id),
                    json!(session_id),
                    json!(session_id),
                    json!(source_type),
                    json!(source_id),
                    json!(task.order),
                    json!(task.status),
                    json!(task.text),
                    json!(task.body),
                    json!(owner),
                    json!(DEFAULT_USER_ID),
                    json!(owner),
                    json!(DEFAULT_USER_ID),
                    json!(now),
                    json!(now),
                ],
            ));
        }
        executor
            .execute_transaction(statements)
            .await
            .map(|_| ())
            .map_err(failure)
    })?;
    futures::executor::block_on(reply.receive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{document::Document, persistence::SaveJournal};
    use desktop_runtime::Profile;
    use futures::executor::block_on;

    fn item(id: &str, attrs: Value, text: &[&str]) -> Value {
        let mut attrs = attrs;
        attrs["taskId"] = json!(id);
        attrs["taskItemId"] = json!(format!("{id}-item"));
        json!({"type": "taskItem", "attrs": attrs, "content": [{
            "type": "paragraph",
            "content": text.iter().map(|text| json!({"type": "text", "text": text})).collect::<Vec<_>>(),
        }]})
    }

    #[test]
    fn task_items_extract_like_shipping() {
        let doc = json!({"type": "doc", "content": [{"type": "taskList", "content": [
            item("a", json!({"status": "in_progress"}), &["Ship ", "beta"]),
            item("b", json!({"status": "pending", "checked": true}), &["Reply"]),
            item("a", json!({}), &["Duplicate"]),
            {"type": "taskItem", "attrs": {"checked": false}, "content": []},
        ]}]});
        let tasks = extract(&doc);
        assert_eq!(tasks.len(), 2);
        assert_eq!(
            (tasks[0].status, tasks[0].text.as_str()),
            ("in_progress", "Ship beta")
        );
        assert_eq!((tasks[1].order, tasks[1].status), (1, "done"));
        assert!(
            tasks[0]
                .body
                .starts_with(r#"[{"type":"paragraph","content":[{"type":"text","text":"Ship "#)
        );
    }

    #[test]
    fn saving_a_memo_mirrors_its_tasks_into_action_items() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("tasks.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let id = session.summary.id.0.to_string();
        let (journal, _events) =
            block_on(SaveJournal::start(runtime.clone(), session.note.unwrap())).unwrap();
        let save = |revision: u64, items: Vec<Value>| {
            let doc = json!({"type": "doc", "content": [{"type": "taskList", "content": items}]});
            journal.publish(revision, Document::parse(doc.to_string().into()).unwrap());
            block_on(journal.flush(revision)).unwrap().unwrap();
        };
        let rows = || {
            block_on(
                runtime
                    .submit(|services| async move {
                        services
                            .executor
                            .execute("SELECT id, source_type, source_id, session_id, status, text, updated_at, deleted_at IS NOT NULL AS deleted FROM action_items ORDER BY id".into(), vec![])
                            .await
                            .map_err(failure)
                    })
                    .unwrap()
                    .receive(),
            )
            .unwrap()
        };
        save(
            1,
            vec![
                item("t1", json!({"status": "todo"}), &["Draft agenda"]),
                item("t2", json!({"status": "done"}), &["Book room"]),
            ],
        );
        let first = rows();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0]["source_type"], "session_raw_note");
        assert_eq!(first[0]["source_id"], id.as_str());
        assert_eq!(first[0]["session_id"], "");
        assert_eq!(
            (&first[1]["status"], &first[1]["text"]),
            (&json!("done"), &json!("Book room"))
        );
        // Typing elsewhere in the note does not rewrite unchanged tasks.
        save(
            2,
            vec![
                item("t1", json!({"status": "todo"}), &["Draft agenda"]),
                item("t2", json!({"status": "done"}), &["Book room"]),
            ],
        );
        assert_eq!(rows()[0]["updated_at"], first[0]["updated_at"]);
        save(
            3,
            vec![item("t1", json!({"status": "done"}), &["Draft agenda"])],
        );
        let last = rows();
        assert_eq!(
            (&last[0]["status"], &last[0]["deleted"]),
            (&json!("done"), &json!(0))
        );
        assert_eq!(last[1]["deleted"], 1);
        drop(journal);
        block_on(runtime.shutdown()).unwrap();
    }
}
