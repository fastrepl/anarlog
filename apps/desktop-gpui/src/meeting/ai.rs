use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use desktop_runtime::{
    CancellationToken, DocumentSnapshot, Result, RuntimeHandle, SaveDocument, ServiceError,
    SessionId,
};
use futures::{StreamExt, future::BoxFuture, stream::BoxStream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::{mpsc, oneshot};

use super::model::{MAX_TEXT, failure};
use super::store::{statement, string};

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Part {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
    },
    #[serde(rename = "dynamic-tool")]
    Tool {
        #[serde(rename = "toolCallId")]
        call_id: String,
        #[serde(rename = "toolName")]
        name: String,
        state: String,
        input: Value,
        output: Value,
        #[serde(rename = "errorText", default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub parts: Vec<Part>,
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Shipping's chat tools (`chat/tools/index.ts`), by tool name with shipping's input fields.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum MeetingTool {
    ListFolders {
        query: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    CreateFolder {
        folder_path: String,
    },
    MoveMeetingsToFolder {
        meeting_ids: Vec<String>,
        folder_path: String,
    },
    ListMeetings(anlg_agent_access::ListMeetingsInput),
    GetMeeting(anlg_agent_access::GetMeetingInput),
    GetMeetingTranscript(anlg_agent_access::GetMeetingTranscriptInput),
    GetRecurringMeetingHistory(anlg_agent_access::GetRecurringMeetingHistoryInput),
    SearchMeetingContent {
        query: String,
        meeting_ids: Option<Vec<String>>,
        limit: Option<u32>,
    },
    #[serde(rename_all = "camelCase")]
    ReadFolderMaterial {
        material_id: String,
        max_chars: Option<u32>,
    },
    FindRelatedMeetings {
        meeting_id: Option<String>,
        limit: Option<u32>,
    },
    SearchMeetings {
        query: Option<String>,
        filters: Option<Value>,
        limit: Option<u32>,
    },
    SearchContacts {
        query: String,
        limit: Option<u32>,
    },
    SearchCalendarEvents {
        query: String,
        limit: Option<u32>,
    },
    #[serde(rename_all = "camelCase")]
    WebSearch {
        query: String,
        include_domains: Option<Vec<String>>,
        exclude_domains: Option<Vec<String>>,
        limit: Option<u32>,
    },
    #[serde(rename_all = "camelCase")]
    EditMemo {
        session_id: Option<String>,
        content: String,
    },
    #[serde(rename_all = "camelCase")]
    EditSummary {
        session_id: Option<String>,
        enhanced_note_id: Option<String>,
        content: String,
    },
    #[serde(rename_all = "camelCase")]
    ApplySessionCorrection {
        session_id: Option<String>,
        target: Option<String>,
        enhanced_note_id: Option<String>,
        old_text: String,
        new_text: String,
        #[serde(default)]
        dictionary_terms: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    MoveMeetingContents {
        source_meeting_id: Option<String>,
        target_meeting_id: String,
    },
}

/// Opens the review tab for a pending edit, like shipping's `openEditTab`.
pub type ReviewOpener = Arc<dyn Fn(Arc<str>) + Send + Sync>;

/// What shipping's tools read from the chat at call time: the tool call (a proposal's id),
/// the note attached to the message, the shown summary, mentioned folders and Pro access.
#[derive(Clone, Default)]
pub struct ToolContext {
    pub call_id: String,
    pub session: Option<String>,
    pub summary: Option<String>,
    pub folders: Vec<String>,
    pub pro: bool,
    pub review: Option<ReviewOpener>,
}

pub enum ProviderEvent {
    Text(String),
    Reasoning(String),
    Tool {
        call_id: String,
        name: String,
        input: Value,
    },
}

pub struct Request {
    pub session: SessionId,
    pub messages: Vec<Message>,
    pub tools_allowed: bool,
    /// Asks for one JSON object instead of prose (shipping's `Output.object`).
    pub structured: Option<Structured>,
    pub cancellation: CancellationToken,
}

/// A structured reply's JSON schema and output token cap.
#[derive(Clone)]
pub struct Structured {
    pub schema: Value,
    pub max_tokens: u32,
}

pub trait ProviderAdapter: Send + Sync {
    fn stream(
        &self,
        request: Request,
    ) -> BoxFuture<'static, Result<BoxStream<'static, Result<ProviderEvent>>>>;
}

pub type ProviderResolver =
    Arc<dyn Fn(SessionId) -> BoxFuture<'static, Result<Arc<dyn ProviderAdapter>>> + Send + Sync>;
pub type ToolExecutor = Arc<
    dyn Fn(MeetingTool, ToolContext, CancellationToken) -> BoxFuture<'static, Result<Value>>
        + Send
        + Sync,
>;
pub type StreamObserver = Arc<dyn Fn(StreamUpdate) + Send + Sync>;
type Job = BoxFuture<'static, ()>;
pub type CaptureActivity = Arc<dyn Fn(&SessionId) -> bool + Send + Sync>;

pub struct StreamUpdate {
    pub task: u64,
    pub text: String,
    pub reasoning: String,
    /// Tool parts as they start and finish, so chat can show cards while a tool waits.
    pub tools: Vec<Part>,
}

pub struct AiServices {
    pub runtime: RuntimeHandle,
    pub resolve: ProviderResolver,
    pub execute: ToolExecutor,
    pub capture_active: CaptureActivity,
    sessions: Mutex<HashMap<SessionId, Weak<ChatSession>>>,
    jobs: mpsc::Sender<Job>,
}

impl AiServices {
    pub async fn flush(&self) -> Result<()> {
        let (send, receive) = oneshot::channel();
        self.jobs
            .send(Box::pin(async move {
                let _ = send.send(());
            }))
            .await
            .map_err(|_| ServiceError::Closed)?;
        receive.await.map_err(|_| ServiceError::Closed)
    }

    pub fn enqueue<T: Send + 'static>(
        &self,
        operation: BoxFuture<'static, Result<T>>,
    ) -> Result<oneshot::Receiver<Result<T>>> {
        let (send, receive) = oneshot::channel();
        self.jobs
            .try_send(Box::pin(async move {
                let _ = send.send(operation.await);
            }))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn new(
        runtime: RuntimeHandle,
        resolve: ProviderResolver,
        execute: ToolExecutor,
        capture_active: CaptureActivity,
    ) -> Result<Self> {
        let (jobs, mut queue) = mpsc::channel::<Job>(8);
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(failure)?;
        std::thread::Builder::new()
            .name("meeting-ai".into())
            .spawn(move || {
                executor.block_on(async move {
                    while let Some(job) = queue.recv().await {
                        job.await;
                    }
                })
            })
            .map_err(failure)?;
        Ok(Self {
            runtime,
            resolve,
            execute,
            capture_active,
            sessions: Mutex::new(HashMap::new()),
            jobs,
        })
    }

    pub fn queue_summary(
        self: &Arc<Self>,
        base: DocumentSnapshot,
        context: SummaryContext,
        cancellation: CancellationToken,
        observe: StreamObserver,
    ) -> Result<oneshot::Receiver<Result<DocumentSnapshot>>> {
        let services = self.clone();
        let (send, receive) = oneshot::channel();
        self.jobs
            .try_send(Box::pin(async move {
                let _ = send.send(
                    services
                        .summarize(base, context, cancellation, observe)
                        .await,
                );
            }))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn chat(self: &Arc<Self>, session: SessionId, group: Arc<str>) -> Result<Arc<ChatSession>> {
        let mut sessions = self.sessions.lock().map_err(failure)?;
        if let Some(chat) = sessions.get(&session).and_then(Weak::upgrade) {
            return if chat.group == group {
                Ok(chat)
            } else {
                Err(ServiceError::Conflict)
            };
        }
        sessions.retain(|_, session| session.strong_count() > 0);
        let chat = Arc::new(ChatSession {
            services: self.clone(),
            session: session.clone(),
            group,
            serial: AsyncMutex::new(()),
            generation: AtomicU64::new(0),
            cancel_epoch: AtomicU64::new(0),
            cancellation: Mutex::new(CancellationToken::new()),
        });
        sessions.insert(session, Arc::downgrade(&chat));
        Ok(chat)
    }

    pub async fn summarize(
        &self,
        base: DocumentSnapshot,
        context: SummaryContext,
        cancellation: CancellationToken,
        observe: StreamObserver,
    ) -> Result<DocumentSnapshot> {
        if (self.capture_active)(&base.session_id) {
            return Err(ServiceError::Busy);
        }
        let target = base.id.clone();
        let session = base.session_id.clone();
        self.runtime.read(cancellation.clone(), move |services| async move {
            let rows = services.executor.execute("SELECT id FROM session_documents WHERE id = ? AND session_id = ? AND kind IN ('summary', 'template_output') AND deleted_at IS NULL".into(), vec![json!(target), json!(session)]).await.map_err(failure)?;
            if rows.is_empty() { return Err(ServiceError::Conflict); }
            Ok(())
        })?.receive().await?;
        let system =
            anlg_template_app::render(anlg_template_app::Template::EnhanceSystem(context.system))
                .map_err(failure)?;
        let user = anlg_template_app::render(anlg_template_app::Template::EnhanceUser(Box::new(
            context.user,
        )))
        .map_err(failure)?;
        if system.len() + user.len() > MAX_TEXT {
            return Err(failure(
                "Summary context exceeds the bounded provider input.",
            ));
        }
        let request = Request {
            session: base.session_id.clone(),
            messages: vec![
                Message {
                    id: "system".into(),
                    role: Role::System,
                    parts: vec![Part::Text { text: system }],
                },
                Message {
                    id: "context".into(),
                    role: Role::User,
                    parts: vec![Part::Text { text: user }],
                },
            ],
            tools_allowed: false,
            structured: None,
            cancellation: cancellation.clone(),
        };
        let provider = tokio::select! {
            _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = (self.resolve)(base.session_id.clone()) => result?,
        };
        let stream = tokio::select! {
            _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = provider.stream(request) => result?,
        };
        let output = consume(stream, &cancellation, 0, &observe).await?;
        if !output.tools.is_empty() {
            return Err(failure(
                "Summary provider returned an unexpected tool call.",
            ));
        }
        if output.text.trim().is_empty() {
            return Err(failure(
                "Summary provider returned no content; existing summary preserved.",
            ));
        }
        if cancellation.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        if (self.capture_active)(&base.session_id) {
            return Err(ServiceError::Busy);
        }
        let body: Arc<str> = anlg_tiptap::md_to_tiptap_json(&output.text)
            .map_err(failure)?
            .to_string()
            .into();
        self.runtime
            .save_document(SaveDocument { base, body })?
            .receive()
            .await
    }
}

impl AiServices {
    /// One system and user prompt without tools (shipping's `streamText`), streaming text to
    /// `observe`. A structured reply returns its JSON text.
    pub async fn generate(
        &self,
        session: SessionId,
        system: String,
        prompt: String,
        structured: Option<Structured>,
        cancellation: CancellationToken,
        observe: StreamObserver,
    ) -> Result<String> {
        if system.len() + prompt.len() > MAX_TEXT {
            return Err(failure("Prompt exceeds the bounded provider input."));
        }
        let request = Request {
            session: session.clone(),
            messages: vec![
                Message {
                    id: "system".into(),
                    role: Role::System,
                    parts: vec![Part::Text { text: system }],
                },
                Message {
                    id: "prompt".into(),
                    role: Role::User,
                    parts: vec![Part::Text { text: prompt }],
                },
            ],
            tools_allowed: false,
            structured,
            cancellation: cancellation.clone(),
        };
        let provider = tokio::select! {
            _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = (self.resolve)(session) => result?,
        };
        let stream = tokio::select! {
            _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = provider.stream(request) => result?,
        };
        let output = consume(stream, &cancellation, 0, &observe).await?;
        // Anthropic answers a structured request through its forced `json` tool.
        match output.tools.into_iter().next() {
            Some((_, name, input)) if name == "json" => Ok(input.to_string()),
            Some(_) => Err(failure("Provider returned an unexpected tool call.")),
            None => Ok(output.text),
        }
    }
}

pub fn record_enhancement_result(
    is_auto: bool,
    provider: &str,
    model: &str,
    used_template: bool,
    result: &Result<DocumentSnapshot>,
) {
    match result {
        Ok(_) => {
            crate::telemetry::track(
                "note_enhanced",
                &[
                    ("is_auto", json!(is_auto)),
                    ("llm_provider", json!(provider)),
                    ("llm_model", json!(model)),
                    ("used_template", json!(used_template)),
                ],
            );
            if used_template {
                crate::telemetry::track(
                    "template_applied",
                    &[(
                        "entry_point",
                        json!(if is_auto { "auto_enhance" } else { "enhance" }),
                    )],
                );
            }
        }
        Err(ServiceError::Cancelled | ServiceError::Busy) => {}
        Err(error) => {
            crate::telemetry::track(
                "enhancement_failed",
                &[
                    ("is_auto", json!(is_auto)),
                    ("llm_provider", json!(provider)),
                    ("failure_stage", json!("generation")),
                ],
            );
            crate::telemetry::capture_operational_error(
                "note_enhancement",
                Some("generation"),
                &error.to_string(),
                Some(provider),
                None,
            );
        }
    }
}

pub struct SummaryContext {
    pub system: anlg_template_app::EnhanceSystem,
    pub user: anlg_template_app::EnhanceUser,
}

pub struct ChatSession {
    services: Arc<AiServices>,
    session: SessionId,
    group: Arc<str>,
    serial: AsyncMutex<()>,
    generation: AtomicU64,
    cancel_epoch: AtomicU64,
    cancellation: Mutex<CancellationToken>,
}

impl ChatSession {
    pub fn queue_send(
        self: &Arc<Self>,
        user: Message,
        history: Vec<Message>,
        previous_assistant: Option<String>,
        observe: StreamObserver,
    ) -> Result<oneshot::Receiver<Result<Message>>> {
        self.queue_send_with(
            user,
            history,
            previous_assistant,
            observe,
            ToolContext::default(),
        )
    }

    pub fn queue_send_with(
        self: &Arc<Self>,
        user: Message,
        history: Vec<Message>,
        previous_assistant: Option<String>,
        observe: StreamObserver,
        context: ToolContext,
    ) -> Result<oneshot::Receiver<Result<Message>>> {
        let session = self.clone();
        let epoch = self.cancel_epoch.load(Ordering::Acquire);
        let (send, receive) = oneshot::channel();
        self.services
            .jobs
            .try_send(Box::pin(async move {
                let _ = send.send(
                    session
                        .send_inner(user, history, previous_assistant, observe, context, epoch)
                        .await,
                );
            }))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn cancel(&self) -> Result<()> {
        self.cancel_epoch.fetch_add(1, Ordering::AcqRel);
        self.cancellation.lock().map_err(failure)?.cancel();
        Ok(())
    }

    pub async fn send(
        &self,
        user: Message,
        history: Vec<Message>,
        previous_assistant: Option<String>,
        observe: StreamObserver,
    ) -> Result<Message> {
        self.send_inner(
            user,
            history,
            previous_assistant,
            observe,
            ToolContext::default(),
            self.cancel_epoch.load(Ordering::Acquire),
        )
        .await
    }

    async fn send_inner(
        &self,
        user: Message,
        mut history: Vec<Message>,
        previous_assistant: Option<String>,
        observe: StreamObserver,
        context: ToolContext,
        epoch: u64,
    ) -> Result<Message> {
        if user.role != Role::User || user.id.is_empty() {
            return Err(ServiceError::Conflict);
        }
        if history.len() > 1000 || serde_json::to_vec(&history).map_err(failure)?.len() > MAX_TEXT {
            return Err(failure("Chat history exceeds the bounded request context."));
        }
        let _serial = self.serial.lock().await;
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancellation = CancellationToken::new();
        *self.cancellation.lock().map_err(failure)? = cancellation.clone();
        if self.cancel_epoch.load(Ordering::Acquire) != epoch {
            return Err(ServiceError::Cancelled);
        }
        self.persist(&user, None, None).await?;
        history.retain(|message| message.id != user.id);
        let mut history = window_history(history);
        history.push(user.clone());
        let mut assistant = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            parts: Vec::new(),
        };
        for step in 0..=5 {
            if cancellation.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            if serde_json::to_vec(&history).map_err(failure)?.len() > MAX_TEXT {
                return Err(failure("Tool history exceeds the bounded provider input."));
            }
            let provider = tokio::select! {
                _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
                result = (self.services.resolve)(self.session.clone()) => result?,
            };
            let request = Request {
                session: self.session.clone(),
                messages: history.clone(),
                tools_allowed: step < 5,
                structured: None,
                cancellation: cancellation.clone(),
            };
            let stream = tokio::select! {
                _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
                result = provider.stream(request) => result?,
            };
            let output = consume(stream, &cancellation, generation, &observe).await?;
            let mut parts = Vec::new();
            if !output.text.is_empty() {
                parts.push(Part::Text { text: output.text });
            }
            if !output.reasoning.is_empty() {
                parts.push(Part::Reasoning {
                    text: output.reasoning,
                });
            }
            if step == 5 && !output.tools.is_empty() {
                return Err(failure("Provider exceeded the five-step tool limit."));
            }
            let finished = output.tools.is_empty();
            for (call_id, name, input) in output.tools {
                let mut part = Part::Tool {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    output: Value::Null,
                    state: "input-available".into(),
                    error: None,
                };
                observe(StreamUpdate {
                    task: generation,
                    text: String::new(),
                    reasoning: String::new(),
                    tools: vec![part.clone()],
                });
                let mut typed = input.clone();
                typed
                    .as_object_mut()
                    .ok_or_else(|| failure("Malformed tool input."))?
                    .insert("tool".into(), json!(name));
                let result = match serde_json::from_value::<MeetingTool>(typed) {
                    Ok(tool) => {
                        let context = ToolContext {
                            call_id: call_id.clone(),
                            ..context.clone()
                        };
                        (self.services.execute)(tool, context, cancellation.clone()).await
                    }
                    Err(error) => Err(failure(format!("Invalid input for tool {name}: {error}"))),
                };
                if let Err(ServiceError::Cancelled) = result {
                    return Err(ServiceError::Cancelled);
                }
                if let Part::Tool {
                    output,
                    state,
                    error,
                    ..
                } = &mut part
                {
                    match result {
                        Ok(value) => {
                            *output = value;
                            *state = "output-available".into();
                        }
                        // The model still sees the failure; the card shows it like shipping's
                        // `output-error`.
                        Err(failed) => {
                            *output = json!({"error": failed.to_string()});
                            *state = "output-error".into();
                            *error = Some(failed.to_string());
                        }
                    }
                    if output.to_string().len() > MAX_TEXT {
                        return Err(failure("Tool output exceeds context limit."));
                    }
                }
                observe(StreamUpdate {
                    task: generation,
                    text: String::new(),
                    reasoning: String::new(),
                    tools: vec![part.clone()],
                });
                parts.push(part);
            }
            history.push(Message {
                id: assistant.id.clone(),
                role: Role::Assistant,
                parts: parts.clone(),
            });
            assistant.parts.extend(parts);
            if finished {
                break;
            }
        }
        if cancellation.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        self.persist(&assistant, Some(&user), previous_assistant)
            .await?;
        Ok(assistant)
    }

    pub async fn flush(&self) {
        let _guard = self.serial.lock().await;
    }

    async fn persist(
        &self,
        message: &Message,
        user: Option<&Message>,
        previous: Option<String>,
    ) -> Result<()> {
        let group = self.group.clone();
        let session = self.session.clone();
        let message = message.clone();
        let user = user.cloned();
        self.services.runtime.submit(move |services| async move {
            let mut rows = services.executor.execute("SELECT owner_user_id, workspace_id FROM chat_groups WHERE id = ? AND deleted_at IS NULL".into(), vec![json!(group)]).await.map_err(failure)?;
            let mut statements = Vec::new();
            if rows.is_empty() && group == session.0 {
                rows = services.executor.execute("SELECT owner_user_id, workspace_id FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))".into(), vec![json!(session), super::access::param()]).await.map_err(failure)?;
                statements.push(statement(
                    "INSERT OR IGNORE INTO chat_groups (id, owner_user_id, workspace_id, title) SELECT id, owner_user_id, workspace_id, title FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))",
                    vec![json!(session), super::access::param()], Some(1)));
            }
            let row = rows.first().ok_or(ServiceError::Conflict)?;
            let owner = string(row, "owner_user_id")?;
            let workspace = string(row, "workspace_id")?;
            for message in user.iter().chain(std::iter::once(&message)) {
                let content = message.parts.iter().filter_map(|part| match part { Part::Text { text } => Some(text.as_str()), _ => None }).collect::<String>();
                let role = match message.role { Role::User => "user", Role::Assistant => "assistant", Role::System => return Err(ServiceError::Conflict) };
                let parts = stored_parts(&message.parts)?;
                if parts.len() > MAX_TEXT { return Err(failure("Chat message exceeds storage limit.")); }
                statements.push(statement(
                    "INSERT INTO chat_messages (id, chat_group_id, owner_user_id, workspace_id, role, content, parts_json) SELECT ?, ?, ?, ?, ?, ?, ? WHERE EXISTS (SELECT 1 FROM chat_groups WHERE id = ? AND deleted_at IS NULL) ON CONFLICT(id) DO UPDATE SET updated_at = chat_messages.updated_at WHERE chat_messages.chat_group_id = excluded.chat_group_id AND chat_messages.deleted_at IS NULL AND chat_messages.role = excluded.role AND chat_messages.content = excluded.content AND chat_messages.parts_json = excluded.parts_json",
                    vec![json!(message.id), json!(group), json!(owner), json!(workspace), json!(role), json!(content), json!(parts), json!(group)], Some(1)));
            }
            if let Some(previous) = previous.filter(|previous| previous != &message.id) {
                statements.push(statement("UPDATE chat_messages SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND chat_group_id = ? AND role = 'assistant' AND deleted_at IS NULL", vec![json!(previous), json!(group)], Some(1)));
            }
            services.executor.execute_transaction(statements).await.map_err(failure)?;
            Ok(())
        })?.receive().await
    }
}

/// Tool parts are stored as shipping's typed `tool-<name>` parts, so both apps render them.
fn stored_parts(parts: &[Part]) -> Result<String> {
    let mut value = serde_json::to_value(parts).map_err(failure)?;
    for part in value.as_array_mut().into_iter().flatten() {
        if part["type"] == "dynamic-tool"
            && let Some(object) = part.as_object_mut()
            && let Some(Value::String(name)) = object.remove("toolName")
        {
            object.insert("type".into(), json!(format!("tool-{name}")));
        }
    }
    serde_json::to_string(&value).map_err(failure)
}

pub fn window_history(mut history: Vec<Message>) -> Vec<Message> {
    if history.len() <= 20 {
        return history;
    }
    let mut start = history.len() - 20;
    while start > 0 && history[start].role != Role::User {
        start -= 1;
    }
    let tail = history.split_off(start);
    history.retain(|message| message.role == Role::System);
    history.extend(tail);
    history
}

struct Output {
    text: String,
    reasoning: String,
    tools: Vec<(String, String, Value)>,
}

async fn consume(
    mut stream: BoxStream<'static, Result<ProviderEvent>>,
    cancellation: &CancellationToken,
    task: u64,
    observe: &StreamObserver,
) -> Result<Output> {
    let mut output = Output {
        text: String::new(),
        reasoning: String::new(),
        tools: Vec::new(),
    };
    let mut projected_text = String::new();
    let mut projected_reasoning = String::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(33));
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
            _ = interval.tick() => {
                if !projected_text.is_empty() || !projected_reasoning.is_empty() {
                    observe(StreamUpdate { task, text: std::mem::take(&mut projected_text), reasoning: std::mem::take(&mut projected_reasoning), tools: Vec::new() });
                }
            }
            event = stream.next() => match event {
                Some(Ok(ProviderEvent::Text(text))) => { projected_text.push_str(&text); output.text.push_str(&text); }
                Some(Ok(ProviderEvent::Reasoning(text))) => { projected_reasoning.push_str(&text); output.reasoning.push_str(&text); }
                Some(Ok(ProviderEvent::Tool { call_id, name, input })) => output.tools.push((call_id, name, input)),
                Some(Err(error)) => return Err(error),
                None => break,
            },
        }
        if output.text.len() + output.reasoning.len() > MAX_TEXT || output.tools.len() > 32 {
            return Err(failure("Provider output exceeds safe stream bounds."));
        }
    }
    if !projected_text.is_empty() || !projected_reasoning.is_empty() {
        observe(StreamUpdate {
            task,
            text: projected_text,
            reasoning: projected_reasoning,
            tools: Vec::new(),
        });
    }
    Ok(output)
}
