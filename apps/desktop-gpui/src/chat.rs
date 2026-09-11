//! The chat's pure logic, ported from `apps/desktop/src/chat`: the system
//! prompt guidance (`transport/use-transport.ts`), the UI message parts and
//! their persisted shapes (`store/persisted-messages.ts`), the model
//! message window (`transport/helpers.ts`), the context block over the
//! session snapshot (`context/session-context-hydrator.ts`), the chat title
//! helpers (`store/chat-title.ts`), and the empty state's suggestions.

use serde::{Deserialize, Serialize};

pub const MEETING_CONTEXT_TOOL_GUIDANCE: &str = "Context and local meeting tool guidance:
- Use list_meetings for recent meetings, title or ID lookup, pagination, and exact recurring-series filtering. Never guess a meeting ID.
- Use search_meetings for open-ended questions about topics, people, decisions, or date ranges across meeting content. Use search_meeting_content when the user needs exact wording from notes or transcripts.
- After resolving an ID, use get_meeting for the canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for bounded transcript pages, following pagination.next_offset only when more context is needed.
- Use get_recurring_meeting_history for meetings in the same recurring series. Use find_related_meetings only for broader relationships such as shared participants or nearby dates.
- When the user refers to the current meeting, prefer the attached meeting context. Do not fetch it again unless the task needs newer structured data.
- When folder context is attached, prefer the notes listed in that folder and follow any folder instructions. Search and content tools stay scoped to that folder. Use read_folder_material for syllabus or other folder files listed in that context. PDF text is extracted when available.
- When the user asks to prepare for a meeting, create an agenda, organize talking points, or add drafted content before or during a meeting, call edit_memo with the complete replacement markdown so they can review and apply it. Preserve relevant existing memo content. Use edit_memo even when the memo is empty; do not use edit_summary for meeting preparation.
- When the user asks to rewrite, revise, refocus, shorten, or restructure an existing summary, call edit_summary with the complete replacement markdown so they can review and apply it. Do not return the rewrite only as a fenced markdown block.
- Use edit_summary only for existing generated post-meeting summaries. Use apply_session_correction for narrow exact old-to-new corrections and edit_summary for broader summary rewrites. Only return a draft without calling edit_memo or edit_summary when the user explicitly asks not to change the meeting content or no target session can be resolved.
- When the user corrects note content with wording like \"it's not X but Y\", use apply_session_correction to update the current session summary, visible session title, and transcript unless they explicitly ask for one target only. Add uncommon names, companies, products, acronyms, or jargon from the correction to dictionaryTerms so future transcription and summaries can prefer them; skip common names. If the tool reports partial, use get_meeting or retry with the exact remaining text instead of claiming both were updated.
- When the user asks to move a recording, transcript, or notes onto a different existing meeting, resolve both meeting IDs with list_meetings or search_meetings, then call move_meeting_contents. Default the source to the current meeting when they are looking at the misplaced recording. Do not guess IDs. If the target already has a recording or transcript, explain that and stop.
- Do not ask the user to open or share a meeting until list_meetings, search_meetings, search_meeting_content, and get_meeting cannot find enough local context.
- Use typed meeting tools instead of constructing shell commands, crawling files, or accessing SQLite directly.
- Do not assume meeting contents from chat history when a typed tool can read the current source of truth.

Web search guidance:
- Use web_search for public websites, URLs, companies, products, people, news, or current facts that may be outside local notes.
- Include source URLs in the final answer when web_search results are used.
- Do not use web_search for questions that only need local notes, contacts, or calendar events.";

/// `appendMeetingContextToolGuidance(prompt)`
pub fn append_meeting_context_tool_guidance(prompt: &str) -> String {
    if prompt.trim().is_empty() {
        return MEETING_CONTEXT_TOOL_GUIDANCE.to_string();
    }
    format!("{}\n\n{MEETING_CONTEXT_TOOL_GUIDANCE}", prompt.trim())
}

/// `prepareStep`: past `MESSAGE_WINDOW_THRESHOLD` model messages only the
/// last `MESSAGE_WINDOW_SIZE` go to the model.
pub const MESSAGE_WINDOW_THRESHOLD: usize = 20;
pub const MESSAGE_WINDOW_SIZE: usize = 10;

pub fn window_messages<T>(messages: Vec<T>) -> Vec<T> {
    if messages.len() > MESSAGE_WINDOW_THRESHOLD {
        let skip = messages.len() - MESSAGE_WINDOW_SIZE;
        messages.into_iter().skip(skip).collect()
    } else {
        messages
    }
}

/// `ChatScope`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    General,
    Automations,
}

/// A `ContextRef` in the message metadata (`context/entities.ts`): the
/// current note and folder `use-chat-context-pipeline.ts` attaches
/// (`session:auto:<id>` / `folder:auto:<id>`, `auto-current`) and the
/// sessions, people, and organizations mentioned or dropped into the
/// composer (`*:manual:<id>`, `manual`). Kinds this shell does not know are
/// kept verbatim so a row written by another build survives a round trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ContextRef {
    Session {
        key: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Human {
        key: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
        #[serde(rename = "humanId")]
        human_id: String,
    },
    Organization {
        key: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
        #[serde(rename = "organizationId")]
        organization_id: String,
    },
    Folder {
        key: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
        #[serde(rename = "folderId")]
        folder_id: String,
    },
    #[serde(untagged)]
    Other(serde_json::Value),
}

impl ContextRef {
    pub fn auto_session(session_id: &str) -> Self {
        Self::Session {
            key: format!("session:auto:{session_id}"),
            source: "auto-current".to_string(),
            session_id: session_id.to_string(),
        }
    }

    /// `extractContextRefsFromTiptapJson` / the chat panel's drop: a mention
    /// or dropped row of the given `type` (`session`, `human`,
    /// `organization`); other types are not context.
    pub fn manual(kind: &str, id: &str) -> Option<Self> {
        let key = format!("{kind}:manual:{id}");
        let source = "manual".to_string();
        Some(match kind {
            "session" => Self::Session {
                key,
                source,
                session_id: id.to_string(),
            },
            "human" => Self::Human {
                key,
                source,
                human_id: id.to_string(),
            },
            "organization" => Self::Organization {
                key,
                source,
                organization_id: id.to_string(),
            },
            _ => return None,
        })
    }

    pub fn key(&self) -> &str {
        match self {
            Self::Session { key, .. }
            | Self::Human { key, .. }
            | Self::Organization { key, .. }
            | Self::Folder { key, .. } => key,
            Self::Other(value) => value.get("key").and_then(|key| key.as_str()).unwrap_or(""),
        }
    }
}

/// `dedupeByKey`: the first ref per key wins.
pub fn dedupe_refs(refs: impl IntoIterator<Item = ContextRef>) -> Vec<ContextRef> {
    let mut seen = std::collections::HashSet::new();
    refs.into_iter()
        .filter(|reference| seen.insert(reference.key().to_string()))
        .collect()
}

/// `AnlgUIMessage["metadata"]`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(rename = "chatScope", default, skip_serializing_if = "Option::is_none")]
    pub chat_scope: Option<Scope>,
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(
        rename = "contextRefs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub context_refs: Option<Vec<ContextRef>>,
}

/// The AI SDK `UIMessage` parts this shell reads and writes. Unknown parts
/// (tool calls written by the Tauri app) are kept verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    #[serde(rename = "text")]
    Text {
        text: String,
        /// The provider's `text-end` metadata (the Responses API's
        /// `{ openai: { itemId } }`), before `state` like the SDK's part.
        #[serde(
            rename = "providerMetadata",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        provider_metadata: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<String>,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        text: String,
        #[serde(
            rename = "providerMetadata",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        provider_metadata: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<String>,
    },
    #[serde(rename = "step-start")]
    StepStart,
    #[serde(untagged)]
    Other(serde_json::Value),
}

/// A `tool-<name>` part read out of `Part::Other`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolView<'a> {
    pub name: &'a str,
    pub call_id: &'a str,
    pub state: &'a str,
    pub input: Option<&'a serde_json::Value>,
    pub output: Option<&'a serde_json::Value>,
    pub error_text: Option<&'a str>,
}

impl Part {
    /// A tool part in the AI SDK's key order: `type`, `toolCallId`, `state`,
    /// `input`, then `output` or `errorText`, then the provider metadata of
    /// the call (`callProviderMetadata`) and, once the tool has answered, of
    /// the result (`resultProviderMetadata`, the call's own metadata for a
    /// locally run tool).
    pub fn tool(
        name: &str,
        call_id: &str,
        state: &str,
        input: serde_json::Value,
        output: Option<serde_json::Value>,
        error_text: Option<String>,
        provider_metadata: Option<serde_json::Value>,
    ) -> Part {
        let mut map = serde_json::Map::new();
        map.insert("type".into(), format!("tool-{name}").into());
        map.insert("toolCallId".into(), call_id.into());
        map.insert("state".into(), state.into());
        map.insert("input".into(), input);
        if let Some(output) = output {
            map.insert("output".into(), output);
        }
        if let Some(error_text) = error_text {
            map.insert("errorText".into(), error_text.into());
        }
        if let Some(metadata) = provider_metadata {
            map.insert("callProviderMetadata".into(), metadata.clone());
            if matches!(state, "output-available" | "output-error") {
                map.insert("resultProviderMetadata".into(), metadata);
            }
        }
        Part::Other(serde_json::Value::Object(map))
    }

    /// The Responses `itemId` a part's provider metadata carries, under the
    /// `openai` or `azure` provider name.
    fn item_id(metadata: Option<&serde_json::Value>) -> Option<String> {
        let metadata = metadata?.as_object()?;
        ["openai", "azure"].iter().find_map(|provider| {
            metadata
                .get(*provider)?
                .get("itemId")?
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_string)
        })
    }

    pub fn tool_view(&self) -> Option<ToolView<'_>> {
        let Part::Other(value) = self else {
            return None;
        };
        let name = value.get("type")?.as_str()?.strip_prefix("tool-")?;
        Some(ToolView {
            name,
            call_id: value
                .get("toolCallId")
                .and_then(|id| id.as_str())
                .unwrap_or_default(),
            state: value
                .get("state")
                .and_then(|s| s.as_str())
                .unwrap_or_default(),
            input: value.get("input"),
            output: value.get("output"),
            error_text: value.get("errorText").and_then(|e| e.as_str()),
        })
    }
}

/// `convertToModelMessages` for one assistant message: each step's text and
/// tool calls become an assistant turn followed by the tool results; parts
/// whose provider metadata names a Responses item (reasoning, the text's
/// message, a tool call) carry the item so that API gets an `item_reference`.
pub fn assistant_turns(parts: &[Part]) -> Vec<crate::llm_stream::Turn> {
    use crate::llm_stream::{StoredItem, ToolCall, Turn};
    let mut turns = Vec::new();
    let mut text = String::new();
    let mut items: Vec<StoredItem> = Vec::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut results: Vec<Turn> = Vec::new();
    let flush = |text: &mut String,
                 items: &mut Vec<StoredItem>,
                 calls: &mut Vec<ToolCall>,
                 results: &mut Vec<Turn>,
                 turns: &mut Vec<Turn>| {
        turns.extend(std::mem::take(items).into_iter().map(Turn::StoredItem));
        if !text.is_empty() || !calls.is_empty() {
            turns.push(Turn::Assistant {
                text: std::mem::take(text),
                tool_calls: std::mem::take(calls),
            });
        }
        turns.append(results);
    };
    for part in parts {
        match part {
            Part::StepStart => flush(&mut text, &mut items, &mut calls, &mut results, &mut turns),
            Part::Text {
                text: t,
                provider_metadata,
                ..
            } => {
                if let Some(id) = Part::item_id(provider_metadata.as_ref()) {
                    items.push(StoredItem::Message { id });
                }
                text.push_str(t);
            }
            Part::Reasoning {
                provider_metadata, ..
            } => {
                if let Some(id) = Part::item_id(provider_metadata.as_ref()) {
                    items.push(StoredItem::Reasoning {
                        id,
                        encrypted_content: None,
                    });
                }
            }
            Part::Other(value) => {
                if let Some(tool) = part.tool_view() {
                    calls.push(ToolCall {
                        id: tool.call_id.to_string(),
                        name: tool.name.to_string(),
                        arguments: tool.input.cloned().unwrap_or_else(|| serde_json::json!({})),
                        item_id: Part::item_id(value.get("callProviderMetadata")),
                    });
                    let (output, is_error) = match (tool.output, tool.error_text) {
                        (Some(output), _) => (output.to_string(), false),
                        (None, Some(error)) => (error.to_string(), true),
                        (None, None) => (String::new(), false),
                    };
                    results.push(Turn::ToolResult {
                        call_id: tool.call_id.to_string(),
                        name: tool.name.to_string(),
                        output,
                        is_error,
                    });
                }
            }
        }
    }
    flush(&mut text, &mut items, &mut calls, &mut results, &mut turns);
    turns
}

/// `getMeetingIdsFromSearchOutput`: the session ids a `search_meetings`
/// output names.
pub fn meeting_ids_from_search_output(output: &serde_json::Value) -> Vec<String> {
    ["results", "meetings"]
        .iter()
        .filter_map(|key| output.get(key).and_then(|v| v.as_array()))
        .flatten()
        .filter_map(|result| result.get("id").and_then(|id| id.as_str()))
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// `AnlgUIMessage`
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub parts: Vec<Part>,
    pub metadata: Metadata,
    /// `PersistedChatMessage.status`
    pub status: Status,
}

/// `ChatMessageStatus`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Streaming,
    Ready,
    Error,
    Aborted,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Streaming => "streaming",
            Status::Ready => "ready",
            Status::Error => "error",
            Status::Aborted => "aborted",
        }
    }
}

/// `extractTextContent(parts)`: the text parts joined by blank lines.
pub fn extract_text_content(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The row shape of `chat_messages` (`ChatMessageRecord`).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub chat_group_id: String,
    pub owner_user_id: String,
    pub role: String,
    pub content: String,
    pub metadata_json: String,
    pub parts_json: String,
    pub status: String,
    pub created_at: String,
}

impl MessageRow {
    /// `buildPersistedChatMessage`: `createdAt` from the metadata's epoch
    /// millis, else now.
    pub fn from_message(
        message: &Message,
        chat_group_id: &str,
        owner_user_id: &str,
        content: Option<String>,
    ) -> Self {
        let created_at = message
            .metadata
            .created_at
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
            .unwrap_or_else(chrono::Utc::now)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        Self {
            id: message.id.clone(),
            chat_group_id: chat_group_id.to_string(),
            owner_user_id: owner_user_id.to_string(),
            role: match message.role {
                Role::User => "user".into(),
                Role::Assistant => "assistant".into(),
            },
            content: content.unwrap_or_else(|| extract_text_content(&message.parts)),
            metadata_json: serde_json::to_string(&message.metadata).unwrap_or_else(|_| "{}".into()),
            parts_json: serde_json::to_string(&message.parts).unwrap_or_else(|_| "[]".into()),
            status: message.status.as_str().to_string(),
            created_at,
        }
    }

    /// `rowToPersistedChatMessage`: unknown roles and bad JSON fall back like
    /// the frontend's `parseJson` defaults.
    pub fn into_message(self) -> Option<Message> {
        let role = match self.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => return None,
        };
        Some(Message {
            id: self.id,
            role,
            parts: serde_json::from_str(&self.parts_json).unwrap_or_default(),
            metadata: serde_json::from_str(&self.metadata_json).unwrap_or_default(),
            status: match self.status.as_str() {
                "streaming" => Status::Streaming,
                "error" => Status::Error,
                "aborted" => Status::Aborted,
                _ => Status::Ready,
            },
        })
    }
}

/// `ChatGroupRecord`
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct GroupRow {
    pub id: String,
    pub owner_user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

const FALLBACK_CHAT_TITLE_MAX_LENGTH: usize = 50;
const GENERATED_CHAT_TITLE_MAX_LENGTH: usize = 60;
pub const INITIAL_REQUEST_MAX_LENGTH: usize = 4000;

/// `generateChatTitle`'s system prompt.
pub const TITLE_SYSTEM_PROMPT: &str = "Write a concise chat title from the user's first message. Use the same language as the request. Return only the title, with no quotes, emoji, markdown, or ending punctuation. Keep it under 6 words.";

/// `createFallbackChatTitle(initialRequest)`
pub fn create_fallback_chat_title(initial_request: &str) -> String {
    let title = normalize_title_text(initial_request);
    if title.is_empty() {
        return "New chat".to_string();
    }
    truncate_title(&title, FALLBACK_CHAT_TITLE_MAX_LENGTH)
}

/// The `Initial request:` prompt for the title model, capped like
/// `INITIAL_REQUEST_MAX_LENGTH`; `None` when there is nothing to title.
pub fn title_request(initial_request: &str) -> Option<String> {
    let request: String = normalize_title_text(initial_request)
        .chars()
        .take(INITIAL_REQUEST_MAX_LENGTH)
        .collect();
    (!request.is_empty()).then(|| format!("Initial request:\n{request}"))
}

/// `normalizeGeneratedChatTitle(text)`
pub fn normalize_generated_chat_title(text: &str) -> Option<String> {
    let first_line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut title = normalize_title_text(first_line);
    // `^\d+[.)]\s*`
    let digits = title.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && matches!(title[digits..].chars().next(), Some('.') | Some(')')) {
        title = title[digits + 1..].trim_start().to_string();
    }
    // `^[-*#]\s*`
    if let Some(rest) = title.strip_prefix(['-', '*', '#']) {
        title = rest.trim_start().to_string();
    }
    let title = title
        .trim_start_matches(['"', '\'', '`'])
        .trim_end_matches(['"', '\'', '`'])
        .trim_end_matches(['.', '!', '?'])
        .trim();
    if title.is_empty() {
        return None;
    }
    Some(truncate_title(title, GENERATED_CHAT_TITLE_MAX_LENGTH))
}

/// `normalizeTitleText`
fn normalize_title_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `truncateTitle`: cut to `max_length - 3` characters, back to the last
/// space when it sits past the middle, then `...`.
fn truncate_title(title: &str, max_length: usize) -> String {
    let chars: Vec<char> = title.chars().collect();
    if chars.len() <= max_length {
        return title.to_string();
    }
    let truncated: String = chars[..max_length - 3].iter().collect();
    let truncated = truncated.trim_end();
    let prefix = match truncated.rfind(' ') {
        Some(last_space) if truncated[..last_space].chars().count() > max_length / 2 => {
            &truncated[..last_space]
        }
        _ => truncated,
    };
    format!("{prefix}...")
}

/// `body/empty.tsx`'s suggestions when a model and the current note exist:
/// the label shown and the prompt sent.
pub const SUGGESTIONS: [(&str, &str); 3] = [
    (
        "List action items.",
        "What are my action items from this meeting?",
    ),
    (
        "Draft follow-up email.",
        "Draft a follow-up email to the participants",
    ),
    (
        "Find key decisions.",
        "What were the key decisions that have been made?",
    ),
];

/// `hydrateSessionContext(sessionId)` over the enhancer's content snapshot:
/// the `SessionContext` the `ContextBlock` template renders.
pub fn session_context(snapshot: &crate::enhancer::Snapshot) -> anlg_template_app::SessionContext {
    let enhanced: Vec<String> = snapshot
        .enhanced_notes
        .iter()
        .filter_map(|note| {
            let markdown =
                crate::db::enhancer::body_to_markdown(&note.content, &note.content_format);
            (!markdown.trim().is_empty()).then_some(markdown)
        })
        .collect();
    // `buildTranscript` is `null` when no transcript has a renderable word
    // (an id, text, and times), which is exactly when nothing rendered.
    let transcript = (!snapshot.segments.is_empty()).then(|| anlg_template_app::Transcript {
        segments: snapshot
            .segments
            .iter()
            .map(|segment| anlg_template_app::Segment {
                text: segment.text.clone(),
                speaker: segment.speaker_label.clone(),
            })
            .collect(),
        started_at: snapshot
            .transcripts
            .iter()
            .map(|transcript| transcript.started_at)
            .min()
            .map(|value| value.max(0) as u64),
        ended_at: snapshot
            .transcripts
            .iter()
            .filter_map(|transcript| transcript.ended_at)
            .max()
            .map(|value| value.max(0) as u64),
    });
    let event_name = serde_json::from_str::<serde_json::Value>(&snapshot.event_json)
        .ok()
        .and_then(|event| {
            ["name", "title"]
                .iter()
                .find_map(|key| {
                    event
                        .get(key)
                        .and_then(|v| v.as_str())
                        .filter(|v| !v.is_empty())
                })
                .map(str::to_string)
        });
    anlg_template_app::SessionContext {
        title: Some(snapshot.title.clone()).filter(|title| !title.is_empty()),
        date: Some(snapshot.created_at.clone()).filter(|date| !date.is_empty()),
        raw_content: Some(snapshot.raw_markdown.clone()).filter(|md| !md.is_empty()),
        enhanced_content: (!enhanced.is_empty()).then(|| enhanced.join("\n\n---\n\n")),
        meeting_chat: Some(snapshot.meeting_chat.clone()).filter(|chat| !chat.is_empty()),
        transcript,
        participants: snapshot
            .participants
            .iter()
            .filter(|participant| !participant.name.is_empty())
            .map(|participant| anlg_template_app::Participant {
                name: participant.name.clone(),
                job_title: Some(participant.job_title.clone()).filter(|title| !title.is_empty()),
            })
            .collect(),
        event: event_name.map(|name| anlg_template_app::Event { name }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_is_appended_after_a_blank_line() {
        assert_eq!(
            append_meeting_context_tool_guidance(" prompt \n"),
            format!("prompt\n\n{MEETING_CONTEXT_TOOL_GUIDANCE}")
        );
        assert_eq!(
            append_meeting_context_tool_guidance("  "),
            MEETING_CONTEXT_TOOL_GUIDANCE
        );
        assert!(
            MEETING_CONTEXT_TOOL_GUIDANCE.starts_with("Context and local meeting tool guidance:")
        );
        assert!(MEETING_CONTEXT_TOOL_GUIDANCE.ends_with("contacts, or calendar events."));
    }

    #[test]
    fn windowing_keeps_the_last_ten_past_twenty() {
        let messages: Vec<usize> = (0..21).collect();
        assert_eq!(window_messages(messages), (11..21).collect::<Vec<_>>());
        let messages: Vec<usize> = (0..20).collect();
        assert_eq!(window_messages(messages.clone()), messages);
    }

    #[test]
    fn fallback_titles_follow_create_fallback_chat_title() {
        assert_eq!(create_fallback_chat_title("   "), "New chat");
        assert_eq!(
            create_fallback_chat_title("  What   are\nmy tasks? "),
            "What are my tasks?"
        );
        let long = "word ".repeat(20);
        let title = create_fallback_chat_title(&long);
        assert!(title.ends_with("..."));
        assert!(title.chars().count() <= 50);
        assert_eq!(title, "word word word word word word word word word...");
        // A long single token cuts mid-word.
        assert_eq!(
            create_fallback_chat_title(&"a".repeat(60)),
            format!("{}...", "a".repeat(47))
        );
    }

    #[test]
    fn generated_titles_are_normalised() {
        assert_eq!(
            normalize_generated_chat_title("\n\n 1. \"Action items for Monday.\" \n"),
            Some("Action items for Monday".to_string())
        );
        assert_eq!(
            normalize_generated_chat_title("- Follow-up email!"),
            Some("Follow-up email".to_string())
        );
        assert_eq!(
            normalize_generated_chat_title("# Key decisions?\nsecond line"),
            Some("Key decisions".to_string())
        );
        assert_eq!(normalize_generated_chat_title("   \n"), None);
        assert_eq!(normalize_generated_chat_title("\"...\""), None);
        assert_eq!(
            title_request("  hello   world "),
            Some("Initial request:\nhello world".into())
        );
        assert_eq!(title_request(" "), None);
    }

    #[test]
    fn tool_parts_keep_the_sdk_key_order_and_become_turns() {
        let part = Part::tool(
            "list_meetings",
            "call_1",
            "output-available",
            serde_json::json!({ "limit": 3 }),
            Some(serde_json::json!({ "meetings": [] })),
            None,
            None,
        );
        assert_eq!(
            serde_json::to_string(&part).unwrap(),
            r#"{"type":"tool-list_meetings","toolCallId":"call_1","state":"output-available","input":{"limit":3},"output":{"meetings":[]}}"#
        );
        // A Responses call carries its item id as call and result metadata,
        // after the output like the SDK's part.
        let stored = Part::tool(
            "list_meetings",
            "call_1",
            "output-available",
            serde_json::json!({ "limit": 3 }),
            Some(serde_json::json!({ "meetings": [] })),
            None,
            Some(serde_json::json!({ "openai": { "itemId": "fc_1" } })),
        );
        assert_eq!(
            serde_json::to_string(&stored).unwrap(),
            r#"{"type":"tool-list_meetings","toolCallId":"call_1","state":"output-available","input":{"limit":3},"output":{"meetings":[]},"callProviderMetadata":{"openai":{"itemId":"fc_1"}},"resultProviderMetadata":{"openai":{"itemId":"fc_1"}}}"#
        );
        let pending = Part::tool(
            "list_meetings",
            "call_1",
            "input-available",
            serde_json::json!({ "limit": 3 }),
            None,
            None,
            Some(serde_json::json!({ "openai": { "itemId": "fc_1" } })),
        );
        assert!(
            !serde_json::to_string(&pending)
                .unwrap()
                .contains("resultProviderMetadata")
        );
        let view = part.tool_view().unwrap();
        assert_eq!(
            (view.name, view.call_id, view.state),
            ("list_meetings", "call_1", "output-available")
        );
        let parts = vec![
            Part::StepStart,
            part,
            Part::StepStart,
            Part::Text {
                text: "Found none.".into(),
                provider_metadata: None,
                state: Some("done".into()),
            },
        ];
        let turns = assistant_turns(&parts);
        assert_eq!(turns.len(), 3);
        assert!(
            matches!(&turns[0], crate::llm_stream::Turn::Assistant { text, tool_calls } if text.is_empty() && tool_calls.len() == 1)
        );
        assert!(
            matches!(&turns[1], crate::llm_stream::Turn::ToolResult { call_id, output, .. } if call_id == "call_1" && output == r#"{"meetings":[]}"#)
        );
        assert!(
            matches!(&turns[2], crate::llm_stream::Turn::Assistant { text, tool_calls } if text == "Found none." && tool_calls.is_empty())
        );
        assert_eq!(
            meeting_ids_from_search_output(
                &serde_json::json!({ "results": [{ "id": "a" }, { "title": "no id" }] })
            ),
            ["a"]
        );
    }

    #[test]
    fn stored_response_items_become_references() {
        use crate::llm_stream::{StoredItem, Turn};
        // The parts the Responses API leaves behind: an (empty) reasoning
        // item, a call with its item id, and the text's message item.
        let parts = vec![
            Part::StepStart,
            Part::Reasoning {
                text: String::new(),
                provider_metadata: Some(serde_json::json!({
                    "openai": { "itemId": "rs_1", "reasoningEncryptedContent": null }
                })),
                state: Some("done".into()),
            },
            Part::tool(
                "list_meetings",
                "call_1",
                "output-available",
                serde_json::json!({ "limit": 3 }),
                Some(serde_json::json!({ "meetings": [] })),
                None,
                Some(serde_json::json!({ "openai": { "itemId": "fc_1" } })),
            ),
            Part::StepStart,
            Part::Text {
                text: "Found none.".into(),
                provider_metadata: Some(serde_json::json!({ "azure": { "itemId": "msg_1" } })),
                state: Some("done".into()),
            },
        ];
        let turns = assistant_turns(&parts);
        assert_eq!(
            turns[0],
            Turn::StoredItem(StoredItem::Reasoning {
                id: "rs_1".into(),
                encrypted_content: None,
            })
        );
        assert!(
            matches!(&turns[1], Turn::Assistant { text, tool_calls } if text.is_empty() && tool_calls[0].item_id.as_deref() == Some("fc_1"))
        );
        assert!(matches!(&turns[2], Turn::ToolResult { call_id, .. } if call_id == "call_1"));
        assert_eq!(
            turns[3],
            Turn::StoredItem(StoredItem::Message { id: "msg_1".into() })
        );
        assert!(
            matches!(&turns[4], Turn::Assistant { text, tool_calls } if text == "Found none." && tool_calls.is_empty())
        );
        // The Responses request refers to every stored item and sends the
        // text of a referenced message no second time; chat completions get
        // the plain turns.
        let mut request = crate::llm_stream::Request::new("sys", "next", 0);
        request.messages = turns;
        request.messages.push(Turn::User("next".into()));
        let conn = |provider: &str| crate::llm_stream::Connection {
            provider_id: provider.into(),
            base_url: "https://api.example/v1".into(),
            api_key: "k".into(),
            model_id: "gpt-5.6".into(),
            reasoning_effort: "default".into(),
        };
        let responses = crate::llm_stream::build_request(&conn("openai"), &request).unwrap();
        assert_eq!(
            responses.body["input"],
            serde_json::json!([
                { "role": "developer", "content": "sys" },
                { "type": "item_reference", "id": "rs_1" },
                { "type": "item_reference", "id": "fc_1" },
                { "type": "function_call_output", "call_id": "call_1", "output": "{\"meetings\":[]}" },
                { "type": "item_reference", "id": "msg_1" },
                { "role": "user", "content": [{ "type": "input_text", "text": "next" }] }
            ])
        );
        let chat = crate::llm_stream::build_request(&conn("openrouter"), &request).unwrap();
        assert_eq!(chat.body["messages"].as_array().unwrap().len(), 5);
        assert_eq!(chat.body["messages"][3]["content"], "Found none.");
    }

    #[test]
    fn session_context_omits_a_transcript_nothing_rendered() {
        let mut snapshot = crate::enhancer::Snapshot {
            session_id: "s".into(),
            owner_user_id: String::new(),
            title: "T".into(),
            created_at: "2026-08-20T10:00:00.000Z".into(),
            event_id: String::new(),
            event_json: String::new(),
            meeting_chat: String::new(),
            raw_note_id: None,
            raw_template_id: String::new(),
            raw_content: String::new(),
            raw_content_format: "prosemirror_json".into(),
            raw_markdown: String::new(),
            enhanced_notes: Vec::new(),
            // A row whose words have no ids renders nothing (`buildTranscript`
            // returns null for it).
            transcripts: vec![crate::enhancer::SnapshotTranscript {
                id: "t".into(),
                started_at: 0,
                ended_at: Some(5_400_000),
                memo: String::new(),
                words: vec!["hello".into()],
            }],
            participants: Vec::new(),
            segments: Vec::new(),
            supplemental_context: String::new(),
        };
        assert!(session_context(&snapshot).transcript.is_none());
        snapshot.segments.push(crate::enhancer::SegmentPayload {
            speaker_label: "Speaker 1".into(),
            start_ms: 0,
            end_ms: 1,
            text: "hello".into(),
        });
        let transcript = session_context(&snapshot).transcript.unwrap();
        assert_eq!(transcript.segments.len(), 1);
        assert_eq!(transcript.ended_at, Some(5_400_000));
    }

    #[test]
    fn message_rows_round_trip_with_the_frontend_shapes() {
        let message = Message {
            id: "m1".into(),
            role: Role::User,
            parts: vec![Part::Text {
                text: "Hi".into(),
                provider_metadata: None,
                state: None,
            }],
            metadata: Metadata {
                chat_scope: Some(Scope::General),
                created_at: Some(1_700_000_000_000),
                context_refs: Some(vec![ContextRef::auto_session("s1")]),
            },
            status: Status::Ready,
        };
        let row = MessageRow::from_message(&message, "g1", "u1", None);
        assert_eq!(row.content, "Hi");
        assert_eq!(row.created_at, "2023-11-14T22:13:20.000Z");
        assert_eq!(
            row.metadata_json,
            r#"{"chatScope":"general","createdAt":1700000000000,"contextRefs":[{"kind":"session","key":"session:auto:s1","source":"auto-current","sessionId":"s1"}]}"#
        );
        assert_eq!(row.parts_json, r#"[{"type":"text","text":"Hi"}]"#);
        assert_eq!(row.clone().into_message(), Some(message));
        // Every ref kind the frontend writes, plus one it does not, survive.
        let refs: Vec<ContextRef> = serde_json::from_str(
            r#"[{"kind":"human","key":"human:manual:h1","source":"manual","humanId":"h1"},{"kind":"organization","key":"organization:manual:o1","source":"manual","organizationId":"o1"},{"kind":"folder","key":"folder:auto:","source":"auto-current","folderId":""},{"kind":"calendar_event","key":"calendar_event:search:e1","eventId":"e1"},{"kind":"session","key":"session:manual:s2","sessionId":"s2"}]"#,
        )
        .unwrap();
        assert_eq!(refs[0], ContextRef::manual("human", "h1").unwrap());
        assert_eq!(refs[1], ContextRef::manual("organization", "o1").unwrap());
        assert_eq!(
            refs[2],
            ContextRef::Folder {
                key: "folder:auto:".into(),
                source: "auto-current".into(),
                folder_id: String::new(),
            }
        );
        assert!(matches!(&refs[3], ContextRef::Other(value) if value["kind"] == "calendar_event"));
        assert_eq!(refs[3].key(), "calendar_event:search:e1");
        assert_eq!(refs[4].key(), "session:manual:s2");
        assert_eq!(
            serde_json::to_string(&refs).unwrap(),
            r#"[{"kind":"human","key":"human:manual:h1","source":"manual","humanId":"h1"},{"kind":"organization","key":"organization:manual:o1","source":"manual","organizationId":"o1"},{"kind":"folder","key":"folder:auto:","source":"auto-current","folderId":""},{"kind":"calendar_event","key":"calendar_event:search:e1","eventId":"e1"},{"kind":"session","key":"session:manual:s2","sessionId":"s2"}]"#
        );
        assert_eq!(ContextRef::manual("calendar_event", "x"), None);
        let deduped = dedupe_refs([
            ContextRef::auto_session("s1"),
            ContextRef::manual("session", "s1").unwrap(),
            ContextRef::auto_session("s1"),
        ]);
        assert_eq!(deduped.len(), 2);
        // Unknown parts survive untouched.
        let tool = MessageRow {
            parts_json: r#"[{"type":"step-start"},{"type":"tool-search_meetings","state":"output-available","input":{}}]"#.into(),
            role: "assistant".into(),
            status: "ready".into(),
            ..row
        };
        let parsed = tool.into_message().unwrap();
        assert_eq!(parsed.parts.len(), 2);
        assert_eq!(parsed.parts[0], Part::StepStart);
        assert!(matches!(parsed.parts[1], Part::Other(_)));
        assert_eq!(
            serde_json::to_string(&parsed.parts[1]).unwrap(),
            r#"{"type":"tool-search_meetings","state":"output-available","input":{}}"#
        );
    }
}
