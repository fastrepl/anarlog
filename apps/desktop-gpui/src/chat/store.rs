use std::sync::Arc;

use chrono::{DateTime, Utc};
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

use crate::meeting::{
    ai::{Message, Part, Role},
    model::failure,
    store::statement,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ChatGroup {
    pub id: Arc<str>,
    pub title: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Display {
    Text(String),
    Reasoning(String),
    Tool { name: String, running: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub user: bool,
    pub parts: Vec<Display>,
    pub created_at: Option<DateTime<Utc>>,
}

impl ChatMessage {
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                Display::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Text-only history for the provider; tool and reasoning parts stay in storage.
    pub fn to_request(&self) -> Message {
        Message {
            id: self.id.clone(),
            role: if self.user {
                Role::User
            } else {
                Role::Assistant
            },
            parts: vec![Part::Text { text: self.text() }],
        }
    }

    pub fn from_reply(message: &Message) -> Self {
        Self {
            id: message.id.clone(),
            user: message.role == Role::User,
            parts: message
                .parts
                .iter()
                .map(|part| match part {
                    Part::Text { text } => Display::Text(text.clone()),
                    Part::Reasoning { text } => Display::Reasoning(text.clone()),
                    Part::Tool { name, .. } => Display::Tool {
                        name: name.clone(),
                        running: false,
                    },
                })
                .collect(),
            created_at: Some(Utc::now()),
        }
    }

    fn renderable(&self) -> bool {
        self.parts.iter().any(|part| match part {
            Display::Text(text) | Display::Reasoning(text) => !text.trim().is_empty(),
            Display::Tool { .. } => true,
        })
    }
}

/// Shipping's general scope excludes groups whose messages were sent from automations.
const GENERAL_SCOPE: &str = "NOT EXISTS (SELECT 1 FROM chat_messages AS m WHERE m.chat_group_id = g.id AND m.deleted_at IS NULL AND CASE WHEN json_valid(m.metadata_json) THEN json_extract(m.metadata_json, '$.chatScope') ELSE NULL END = 'automations')";

pub fn recent_groups(runtime: &RuntimeHandle, limit: u32) -> Result<Reply<Vec<ChatGroup>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                format!("SELECT g.id, g.title, g.created_at FROM chat_groups AS g WHERE g.deleted_at IS NULL AND {GENERAL_SCOPE} ORDER BY g.created_at DESC, g.id DESC LIMIT ?"),
                vec![json!(limit)],
            )
            .await
            .map_err(failure)?;
        Ok(rows
            .iter()
            .map(|row| ChatGroup {
                id: row["id"].as_str().unwrap_or_default().into(),
                title: row["title"].as_str().unwrap_or_default().to_owned(),
                created_at: timestamp(&row["created_at"]),
            })
            .collect())
    })
}

pub fn messages(
    runtime: &RuntimeHandle,
    group: Arc<str>,
    cancellation: CancellationToken,
) -> Result<Reply<Vec<ChatMessage>>> {
    runtime.read(cancellation, move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT id, role, content, parts_json, created_at FROM chat_messages WHERE chat_group_id = ? AND deleted_at IS NULL ORDER BY created_at, id".into(),
                vec![json!(group)],
            )
            .await
            .map_err(failure)?;
        Ok(rows.iter().filter_map(message).collect())
    })
}

fn message(row: &Value) -> Option<ChatMessage> {
    let parts = serde_json::from_str::<Vec<Value>>(row["parts_json"].as_str().unwrap_or("[]"))
        .unwrap_or_default()
        .iter()
        .filter_map(display)
        .collect::<Vec<_>>();
    let user = row["role"] == "user";
    let content = row["content"].as_str().unwrap_or_default();
    let mut message = ChatMessage {
        id: row["id"].as_str()?.to_owned(),
        user,
        parts,
        created_at: timestamp(&row["created_at"]),
    };
    if !message.renderable() {
        if content.trim().is_empty() {
            // Shipping hides empty assistant placeholders left by interrupted streams.
            return (user).then_some(message);
        }
        message.parts = vec![Display::Text(content.to_owned())];
    }
    Some(message)
}

fn display(part: &Value) -> Option<Display> {
    let kind = part["type"].as_str()?;
    let text = || part["text"].as_str().unwrap_or_default().to_owned();
    match kind {
        "text" => Some(Display::Text(text())),
        "reasoning" => Some(Display::Reasoning(text())),
        "step-start" => None,
        _ => {
            let name = kind
                .strip_prefix("tool-")
                .or_else(|| part["toolName"].as_str())
                .unwrap_or(kind);
            // Edits render as their own tools in shipping, not inside the activity list.
            if matches!(
                name,
                "edit_memo" | "edit_summary" | "update_prompt_template"
            ) {
                return None;
            }
            Some(Display::Tool {
                name: name.to_owned(),
                running: matches!(
                    part["state"].as_str(),
                    Some("input-streaming" | "input-available")
                ),
            })
        }
    }
}

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// Creates the group a new conversation is saved into; the owner follows shipping's
/// owner-user lookup.
pub fn create_group(runtime: &RuntimeHandle, id: Arc<str>, title: String) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        services
            .executor
            .execute_transaction(vec![statement(
                "INSERT INTO chat_groups (id, workspace_id, owner_user_id, title) SELECT ?, '', COALESCE((SELECT user_id FROM (SELECT owner_user_id AS user_id, updated_at, 0 AS source_priority FROM sessions WHERE owner_user_id <> '' AND deleted_at IS NULL UNION ALL SELECT id AS user_id, updated_at, 1 FROM humans WHERE id = owner_user_id AND id <> '' AND deleted_at IS NULL UNION ALL SELECT owner_user_id AS user_id, updated_at, 2 FROM chat_groups WHERE owner_user_id <> '' AND deleted_at IS NULL) ORDER BY source_priority, updated_at DESC, user_id LIMIT 1), '00000000-0000-0000-0000-000000000000'), ?",
                vec![json!(id), json!(title)],
                Some(1),
            )])
            .await
            .map_err(failure)?;
        Ok(())
    })
}

/// Whether chat has a language model to talk to; shipping hides the composer otherwise.
pub fn model_configured(runtime: &RuntimeHandle) -> Result<Reply<bool>> {
    let runtime_for_read = runtime.clone();
    runtime.service(move |_| async move {
        let settings = crate::meeting::config::preferences(&runtime_for_read).await?;
        Ok(
            settings.value("intelligence_disabled") != &Value::Bool(true)
                && !settings.text("current_llm_provider").is_empty()
                && !settings.text("current_llm_model").is_empty(),
        )
    })
}

pub fn closed(error: ServiceError) -> String {
    match error {
        ServiceError::Cancelled => String::new(),
        error => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_decode_source_parts_and_hide_empty_assistant_placeholders() {
        let row = |role: &str, content: &str, parts: &str| json!({"id": "m", "role": role, "content": content, "parts_json": parts, "created_at": "2026-11-16T10:00:00.000Z"});
        let decoded = message(&row(
            "assistant",
            "",
            r#"[{"type":"step-start"},{"type":"reasoning","text":"think"},{"type":"tool-search_meetings","state":"output-available"},{"type":"tool-edit_summary"},{"type":"text","text":"Done"}]"#,
        ))
        .unwrap();
        assert_eq!(
            decoded.parts,
            vec![
                Display::Reasoning("think".into()),
                Display::Tool {
                    name: "search_meetings".into(),
                    running: false
                },
                Display::Text("Done".into()),
            ]
        );
        assert!(decoded.created_at.is_some());
        assert!(message(&row("assistant", " ", r#"[{"type":"step-start"}]"#)).is_none());
        let fallback = message(&row("assistant", "Saved text", "not json")).unwrap();
        assert_eq!(fallback.text(), "Saved text");
    }

    #[test]
    fn groups_exclude_automation_chats_and_new_groups_inherit_the_library_owner() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
                database: directory.path().join("chat.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            assert!(!model_configured(&runtime).unwrap().receive().await.unwrap());
            runtime
                .submit(|services| async move {
                    services.executor.execute_transaction(vec![
                        statement("INSERT INTO sessions (id, owner_user_id, title) VALUES ('s', 'owner-1', 'Note')", vec![], Some(1)),
                        statement("INSERT INTO chat_groups (id, title, created_at) VALUES ('old', 'Old', '2026-01-01T00:00:00.000Z'), ('auto', 'Auto', '2026-01-03T00:00:00.000Z')", vec![], Some(2)),
                        statement("INSERT INTO chat_messages (id, chat_group_id, role, parts_json, metadata_json, created_at) VALUES ('m2', 'old', 'assistant', '[{\"type\":\"text\",\"text\":\"Hi\"}]', '{}', '2026-01-01T00:00:02.000Z'), ('m1', 'old', 'user', '[{\"type\":\"text\",\"text\":\"Hello\"}]', '{}', '2026-01-01T00:00:01.000Z'), ('a1', 'auto', 'user', '[]', '{\"chatScope\":\"automations\"}', '2026-01-03T00:00:00.000Z')", vec![], Some(3)),
                        statement("INSERT INTO app_settings (id, value_json) VALUES ('current_llm_provider', '\"ollama\"'), ('current_llm_model', '\"llama3.2\"')", vec![], Some(2)),
                    ]).await.map_err(failure)
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            create_group(&runtime, "new".into(), "New chat".into())
                .unwrap()
                .receive()
                .await
                .unwrap();
            let groups = recent_groups(&runtime, 5).unwrap().receive().await.unwrap();
            assert_eq!(
                groups
                    .iter()
                    .map(|group| group.id.as_ref())
                    .collect::<Vec<_>>(),
                ["new", "old"]
            );
            assert_eq!(
                recent_groups(&runtime, 1)
                    .unwrap()
                    .receive()
                    .await
                    .unwrap()
                    .len(),
                1
            );
            let owner = runtime
                .read(CancellationToken::new(), |services| async move {
                    services
                        .executor
                        .execute(
                            "SELECT owner_user_id FROM chat_groups WHERE id = 'new'".into(),
                            vec![],
                        )
                        .await
                        .map_err(failure)
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            assert_eq!(owner[0]["owner_user_id"], "owner-1");
            let thread = messages(&runtime, "old".into(), CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap();
            assert_eq!(
                thread.iter().map(ChatMessage::text).collect::<Vec<_>>(),
                ["Hello", "Hi"]
            );
            assert!(thread[0].user);
            assert!(model_configured(&runtime).unwrap().receive().await.unwrap());
            runtime.shutdown().await.unwrap();
        });
    }
}
