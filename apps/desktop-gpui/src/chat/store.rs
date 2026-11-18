use std::sync::Arc;

use chrono::{DateTime, Utc};
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

use crate::meeting::config::{ProviderKind, SecretResolver};

use super::{composer::ContextRef, search::MentionKind};
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

// Parts are short-lived per message; boxing tool parts would only add indirection.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Display {
    Text(String),
    Reasoning(String),
    Tool(ToolPart),
}

/// A tool call as shipping's AI SDK part keeps it: call id, state, input and output.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolPart {
    pub id: String,
    pub name: String,
    pub state: String,
    pub input: Value,
    pub output: Value,
    pub error: Option<String>,
}

impl ToolPart {
    pub fn from_part(part: &Part) -> Option<Self> {
        let Part::Tool {
            call_id,
            name,
            state,
            input,
            output,
            error,
        } = part
        else {
            return None;
        };
        Some(Self {
            id: call_id.clone(),
            name: name.clone(),
            state: state.clone(),
            input: input.clone(),
            output: output.clone(),
            error: error.clone(),
        })
    }

    pub fn running(&self) -> bool {
        matches!(self.state.as_str(), "input-streaming" | "input-available")
    }

    pub fn failed(&self) -> bool {
        self.state == "output-error"
    }

    pub fn done(&self) -> bool {
        self.state == "output-available"
    }

    /// Shipping renders memo and summary edits as cards outside the activity list.
    pub fn edit(&self) -> bool {
        matches!(self.name.as_str(), "edit_memo" | "edit_summary")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub user: bool,
    pub parts: Vec<Display>,
    pub created_at: Option<DateTime<Utc>>,
    /// Mentions sent with a user message, from its `metadata.contextRefs`.
    pub refs: Vec<ContextRef>,
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
                    Part::Tool { .. } => Display::Tool(ToolPart::from_part(part).unwrap()),
                })
                .collect(),
            created_at: Some(Utc::now()),
            refs: Vec::new(),
        }
    }

    fn renderable(&self) -> bool {
        self.parts.iter().any(|part| match part {
            Display::Text(text) | Display::Reasoning(text) => !text.trim().is_empty(),
            Display::Tool(_) => true,
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
                "SELECT id, role, content, parts_json, metadata_json, created_at FROM chat_messages WHERE chat_group_id = ? AND deleted_at IS NULL ORDER BY created_at, id".into(),
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
    let metadata = serde_json::from_str::<Value>(row["metadata_json"].as_str().unwrap_or("{}"))
        .unwrap_or_default();
    let refs = metadata["contextRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|reference| reference["source"] == "manual")
        .filter_map(|reference| {
            let kind = MentionKind::parse(reference["kind"].as_str()?)?;
            let field = match kind {
                MentionKind::Session => "sessionId",
                MentionKind::Human => "humanId",
                MentionKind::Organization => "organizationId",
                MentionKind::Folder => "folderId",
            };
            Some(ContextRef {
                kind,
                id: reference[field].as_str()?.to_owned(),
            })
        })
        .collect();
    let mut message = ChatMessage {
        id: row["id"].as_str()?.to_owned(),
        user,
        parts,
        created_at: timestamp(&row["created_at"]),
        refs,
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
            // Template edits belong to the templates chat, which this app does not offer.
            if name == "update_prompt_template" {
                return None;
            }
            Some(Display::Tool(ToolPart {
                id: part["toolCallId"].as_str().unwrap_or_default().to_owned(),
                name: name.to_owned(),
                state: part["state"].as_str().unwrap_or_default().to_owned(),
                input: part["input"].clone(),
                output: part["output"].clone(),
                error: part["errorText"].as_str().map(str::to_owned),
            }))
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

/// Whether a usable language model is chosen, and whether it is Anarlog's hosted one.
/// Shipping's `useLanguageModel` yields none while the provider lacks a required API key or
/// base URL, so chat shows its setup prompt instead of failing on send.
pub fn model_configured(runtime: &RuntimeHandle) -> Result<Reply<(bool, bool)>> {
    model_status(runtime, None)
}

pub fn model_status(
    runtime: &RuntimeHandle,
    secret: Option<SecretResolver>,
) -> Result<Reply<(bool, bool)>> {
    let runtime_for_read = runtime.clone();
    runtime.service(move |_| async move {
        let settings = crate::meeting::config::preferences(&runtime_for_read).await?;
        let provider = settings.text("current_llm_provider");
        let hosted = matches!(provider.as_str(), "anarlog" | "hyprnote");
        let chosen = settings.value("intelligence_disabled") != &Value::Bool(true)
            && !provider.is_empty()
            && !settings.text("current_llm_model").is_empty();
        if !chosen || hosted {
            return Ok((chosen, hosted));
        }
        let Some(info) = crate::product::provider_catalog::LLM
            .iter()
            .find(|info| info.id == provider)
        else {
            return Ok((false, false));
        };
        let config = provider_config(&runtime_for_read, &provider).await?;
        let base_url = config["base_url"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if info.needs_base_url && base_url.is_empty() && info.base_url.is_empty() {
            return Ok((false, false));
        }
        if info.needs_api_key && !info.subscription {
            let stored = match &secret {
                Some(secret) => secret(ProviderKind::Llm, provider.clone())
                    .await
                    .ok()
                    .flatten(),
                None => None,
            };
            let key =
                stored.unwrap_or_else(|| config["api_key"].as_str().unwrap_or_default().into());
            if key.trim().is_empty() {
                return Ok((false, false));
            }
        }
        Ok((true, false))
    })
}

async fn provider_config(runtime: &RuntimeHandle, provider: &str) -> Result<Value> {
    let id = format!("ai_provider:llm:{provider}");
    runtime
        .read(CancellationToken::new(), move |services| async move {
            let rows = services
                .executor
                .execute(
                    "SELECT value_json FROM synced_preferences WHERE id = ?1 UNION ALL SELECT value_json FROM app_settings WHERE id = ?1".into(),
                    vec![json!(id)],
                )
                .await
                .map_err(failure)?;
            Ok(rows
                .first()
                .and_then(|row| row["value_json"].as_str())
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or(Value::Null))
        })?
        .receive()
        .await
}

/// Records the user message's scope, time and mentions like shipping's message metadata.
pub fn set_metadata(
    runtime: &RuntimeHandle,
    message: String,
    refs: &[ContextRef],
) -> Result<Reply<()>> {
    let mut metadata = json!({"chatScope": "general", "createdAt": Utc::now().timestamp_millis()});
    if !refs.is_empty() {
        metadata["contextRefs"] = Value::Array(refs.iter().map(ContextRef::to_json).collect());
    }
    runtime.submit(move |services| async move {
        services
            .executor
            .execute(
                "UPDATE chat_messages SET metadata_json = ? WHERE id = ? AND role = 'user'".into(),
                vec![json!(metadata.to_string()), json!(message)],
            )
            .await
            .map_err(failure)?;
        Ok(())
    })
}

/// Shipping's text contexts for mentioned people, organizations and folders.
pub fn mention_context(
    runtime: &RuntimeHandle,
    reference: ContextRef,
) -> Result<Reply<Option<String>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let run = |sql: &str, params: Vec<Value>| services.executor.execute(sql.into(), params);
        let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().trim().to_owned();
        Ok(match reference.kind {
            MentionKind::Human => {
                let rows = run("SELECT h.name, h.email, h.job_title, h.memo, o.name AS organization FROM humans AS h LEFT JOIN organizations AS o ON o.id = h.organization_id AND o.deleted_at IS NULL WHERE h.id = ? AND h.deleted_at IS NULL", vec![json!(reference.id)]).await.map_err(failure)?;
                rows.first().and_then(|row| {
                    let (name, email) = (text(row, "name"), text(row, "email"));
                    if name.is_empty() && email.is_empty() {
                        return None;
                    }
                    let mut lines = vec![format!("Referenced contact: {}", if name.is_empty() { &email } else { &name })];
                    lines.extend([
                        Some(text(row, "job_title")).filter(|value| !value.is_empty()),
                        Some(text(row, "organization")).filter(|value| !value.is_empty()).map(|name| format!("Organization: {name}")),
                        Some(email.clone()).filter(|value| !value.is_empty()).map(|email| format!("Email: {email}")),
                        Some(text(row, "memo")).filter(|value| !value.is_empty()).map(|memo| format!("Notes: {memo}")),
                    ].into_iter().flatten());
                    Some(lines.join("\n"))
                })
            }
            MentionKind::Organization => {
                let rows = run("SELECT name FROM organizations WHERE id = ? AND deleted_at IS NULL", vec![json!(reference.id)]).await.map_err(failure)?;
                rows.first()
                    .map(|row| text(row, "name"))
                    .filter(|name| !name.is_empty())
                    .map(|name| format!("Referenced organization: {name}"))
            }
            MentionKind::Folder => {
                const LIMIT: usize = 50;
                let folder = reference.id.clone();
                let sessions = run("SELECT id, title, created_at FROM sessions WHERE deleted_at IS NULL AND (folder_path = ?1 OR substr(folder_path, 1, length(?1) + 1) = ?1 || '/') ORDER BY created_at DESC, id", vec![json!(folder)]).await.map_err(failure)?;
                let materials = run("SELECT id, filename FROM folder_attachments WHERE folder_path = ? AND deleted_at IS NULL ORDER BY filename, id LIMIT 100", vec![json!(folder)]).await.map_err(failure)?;
                let instructions = run("SELECT instructions FROM folders WHERE path = ? AND deleted_at IS NULL", vec![json!(folder)]).await.map(|rows| rows.first().map(|row| text(row, "instructions")).unwrap_or_default()).unwrap_or_default();
                let count = sessions.len();
                let mut lines = vec![
                    format!("Folder context: {folder} ({count} {})", if count == 1 { "note" } else { "notes" }),
                    "Answer from notes in this folder. Use get_meeting or search_meetings with the listed IDs when you need full notes or transcripts. Use list_meetings with this folder_path to enumerate every note. Use read_folder_material with a listed material ID to read a syllabus or other folder file.".into(),
                ];
                if !instructions.is_empty() {
                    lines.extend([String::new(), "Folder instructions:".into(), instructions]);
                }
                if !materials.is_empty() {
                    lines.extend([String::new(), "Folder materials:".into()]);
                    lines.extend(materials.iter().map(|row| format!("- {} [{}]", text(row, "filename"), text(row, "id"))));
                }
                lines.push(String::new());
                if sessions.is_empty() {
                    lines.push("This folder has no notes yet.".into());
                } else {
                    for row in sessions.iter().take(LIMIT) {
                        let title = Some(text(row, "title")).filter(|title| !title.is_empty()).unwrap_or_else(|| "Untitled".into());
                        let date = text(row, "created_at");
                        lines.push(format!("- {title}{} [{}]", if date.is_empty() { String::new() } else { format!(" ({date})") }, text(row, "id")));
                    }
                    if count > LIMIT {
                        lines.push(format!("- and {} more (use list_meetings with folder_path \"{folder}\" to enumerate every note)", count - LIMIT));
                    }
                }
                Some(lines.join("\n"))
            }
            MentionKind::Session => None,
        })
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
            r#"[{"type":"step-start"},{"type":"reasoning","text":"think"},{"type":"tool-search_meetings","toolCallId":"c1","state":"output-available","input":{"query":"sync"},"output":{"results":[]}},{"type":"tool-edit_summary","toolCallId":"c2","state":"output-available","input":{"content":"New"},"output":{"status":"applied"}},{"type":"tool-update_prompt_template","state":"output-available"},{"type":"text","text":"Done"}]"#,
        ))
        .unwrap();
        assert_eq!(
            decoded.parts,
            vec![
                Display::Reasoning("think".into()),
                Display::Tool(ToolPart {
                    id: "c1".into(),
                    name: "search_meetings".into(),
                    state: "output-available".into(),
                    input: json!({"query": "sync"}),
                    output: json!({"results": []}),
                    error: None,
                }),
                Display::Tool(ToolPart {
                    id: "c2".into(),
                    name: "edit_summary".into(),
                    state: "output-available".into(),
                    input: json!({"content": "New"}),
                    output: json!({"status": "applied"}),
                    error: None,
                }),
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
            assert_eq!(
                model_configured(&runtime).unwrap().receive().await.unwrap(),
                (false, false)
            );
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
            assert_eq!(
                model_configured(&runtime).unwrap().receive().await.unwrap(),
                (true, false)
            );
            // A provider that needs an API key is not ready without one, like shipping's
            // `useLanguageModel`, so chat shows its setup prompt instead of a send error.
            runtime
                .submit(|services| async move {
                    services.executor.execute_transaction(vec![statement("UPDATE app_settings SET value_json = '\"openai\"' WHERE id = 'current_llm_provider'", vec![], Some(1))]).await.map_err(failure)
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            assert_eq!(
                model_configured(&runtime).unwrap().receive().await.unwrap(),
                (false, false)
            );
            let keyed: SecretResolver =
                std::sync::Arc::new(|_, _| Box::pin(async { Ok(Some("sk-test".into())) }));
            assert_eq!(
                model_status(&runtime, Some(keyed))
                    .unwrap()
                    .receive()
                    .await
                    .unwrap(),
                (true, false)
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
