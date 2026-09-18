//! `PersistentChat` in its `FloatingOpen` mode (`chat/components/*`): the
//! floating panel over the main surface with the toolbar (history, new chat),
//! the message list, the composer, and the `ChatSessionProvider` flow —
//! persisted `chat_groups` / `chat_messages`, the context block over the open
//! note, the streamed assistant reply, and the generated chat title. `mod+j`
//! toggles it, Escape and a press on the frame close it.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    AnyElement, ClickEvent, Context, Entity, MouseButton, MouseDownEvent, SharedString, Window,
    div, prelude::*, px,
};

use super::Workspace;
use crate::chat::{self, ContextRef, Message, Metadata, Part, Role, Scope, Status};
use crate::llm_stream::{self, Chunk, Connection, Request, Turn};
use crate::text_area::{Draft, MentionStyle, TextArea, TextAreaEvent, TextAreaStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `FLOATING_PANEL_MIN_WIDTH`
const PANEL_MIN_WIDTH: f32 = 476.0;
/// `FLOATING_CHAT_INPUT_MAX_WIDTH + FLOATING_CHAT_SHELL_INSET * 2`
const PANEL_MAX_WIDTH: f32 = 648.0;
/// `FLOATING_PANEL_TOP_CLEARANCE`
const TOP_CLEARANCE: f32 = 46.0;
/// `max-h-[min(36rem,70vh)]`
const LIST_MAX_HEIGHT: f32 = 576.0;

/// The local reply while the open note's transcript is still being batch
/// transcribed (`ChatSession.sendMessage`'s `isTranscriptUnavailable` branch).
const TRANSCRIPT_UNAVAILABLE_REPLY: &str = "This recording is using batch transcription, so the transcript isn't available to chat yet. Ask again after transcription finishes, or switch to a Pro model for live transcription.";

/// `ChatMode` (`store/zustand/tabs/chat-mode.ts`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ChatMode {
    #[default]
    FloatingClosed,
    FloatingOpen,
    RightPanelOpen,
}

/// `ChatStatus`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatStatus {
    Ready,
    Submitted,
    Streaming,
    Error,
}

/// `ChatSessionProvider` + `chat-context.ts` state for one `ChatScope`; the
/// workspace keeps one per scope and shows the automations one on its tab.
#[derive(Default)]
pub(crate) struct ChatState {
    pub scope: Scope,
    /// `useChatSelection`: the persisted group, once the first send created it.
    pub group_id: Option<String>,
    pub messages: Vec<Message>,
    pub status: Option<ChatStatus>,
    pub error: Option<String>,
    pub composer: Option<Entity<TextArea>>,
    /// `useLanguageModel("chat")`: the resolved connection, `Some(None)`
    /// once resolved without a model.
    pub connection: Option<Option<Connection>>,
    /// Bumped per stream so a stale run cannot touch the list.
    pub run: u64,
    pub abort: Option<Arc<AtomicBool>>,
    /// `queuedMessages`: sends made while a reply streams, with the refs
    /// merged when they were queued.
    pub queued: VecDeque<(String, Vec<ContextRef>)>,
    /// `pendingManualRefs`: notes dropped onto the panel, attached to the
    /// next send and cleared with it or when the chat moves on.
    pub pending_manual_refs: Vec<ContextRef>,
    /// `useMessageHistory`: the sent draft being browsed (newest first) and
    /// the draft set aside while browsing.
    pub history_index: Option<usize>,
    pub draft_before_history: Option<Draft>,
    pub history_open: bool,
    /// `useRecentChatGroups(scope, 5)`
    pub history: Vec<chat::GroupRow>,
    /// `regenerate()`: the assistant row the next finished reply replaces.
    pub replace_previous: Option<String>,
    /// The tool cards whose `<details>` is open, by tool call id.
    pub open_tools: std::collections::HashSet<String>,
    /// The result carousels' first visible card, by tool call id.
    pub tool_pages: std::collections::HashMap<String, usize>,
    /// `useAutoFocusEditor`: focus the composer on the next frame.
    pub focus_pending: bool,
    /// `useChatAutoScroll`'s `shouldAutoScroll`: keep the list pinned to the
    /// bottom until the user scrolls up.
    pub auto_scroll: bool,
    /// `showGoToRecent`: the user scrolled down while unpinned.
    pub show_go_to_recent: bool,
    /// `useDictation`: the composer's voice input.
    pub dictation: super::dictation::DictationState,
}

impl ChatState {
    pub(crate) fn new(scope: Scope) -> Self {
        Self {
            scope,
            auto_scroll: true,
            ..Default::default()
        }
    }

    fn status(&self) -> ChatStatus {
        self.status.unwrap_or(ChatStatus::Ready)
    }

    pub(super) fn busy(&self) -> bool {
        matches!(self.status(), ChatStatus::Submitted | ChatStatus::Streaming)
    }
}

impl Workspace {
    /// `chat.mode !== "FloatingClosed"`
    pub(crate) fn chat_open(&self) -> bool {
        self.chat_mode != ChatMode::FloatingClosed
    }

    /// The chat renders docked on the right: the automations tab always,
    /// the general scope in `RightPanelOpen`.
    pub(crate) fn chat_in_right_panel(&self) -> bool {
        self.automations_open() || self.chat_mode == ChatMode::RightPanelOpen
    }

    /// `chat.sendEvent({ type: "TOGGLE" })`: closed → floating, anything
    /// open → closed.
    pub(crate) fn toggle_chat(&mut self, cx: &mut Context<Self>) {
        if self.chat_open() {
            self.close_chat(cx);
        } else {
            self.open_chat(ChatMode::FloatingOpen, cx);
        }
    }

    /// `OPEN` / `OPEN_RIGHT_PANEL`
    fn open_chat(&mut self, mode: ChatMode, cx: &mut Context<Self>) {
        self.chat_mode = mode;
        self.chat.focus_pending = true;
        self.chat.history_open = false;
        self.resolve_chat_connection(cx);
        self.load_chat_history(cx);
        cx.notify();
    }

    /// `CLOSE`
    pub(crate) fn close_chat(&mut self, cx: &mut Context<Self>) {
        if self.chat_open() {
            self.chat_mode = ChatMode::FloatingClosed;
            self.chat.history_open = false;
            // The composer unmounts with the panel.
            self.cancel_dictation(cx);
            cx.notify();
        }
    }

    /// The automations tab shows its own `automations` chat scope; entering
    /// or leaving it swaps which scope's state is active and resolves the
    /// model for the panel.
    pub(crate) fn set_chat_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.chat.scope != scope {
            self.cancel_dictation(cx);
            std::mem::swap(&mut self.chat, &mut self.parked_chat);
        }
        if scope == Scope::Automations {
            self.resolve_chat_connection(cx);
            self.load_chat_history(cx);
        }
    }

    /// `useLanguageModel("chat")`: the selected provider and model, re-read
    /// whenever the panel opens or the settings change.
    pub(crate) fn resolve_chat_connection(&mut self, cx: &mut Context<Self>) {
        let task = self.store.llm_connection(&self.provider_settings);
        cx.spawn(async move |this, cx| {
            let connection = task.await.ok().flatten();
            this.update(cx, |this, cx| {
                this.chat.connection = Some(connection);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn chat_model_configured(&self) -> bool {
        matches!(self.chat.connection, Some(Some(_)))
    }

    /// `useRecentChatGroups(chatScope, 5)`; the automations nav lists the
    /// same groups, so it reloads too.
    fn load_chat_history(&mut self, cx: &mut Context<Self>) {
        let scope = self.chat.scope;
        if scope == Scope::Automations && self.automations_open() {
            self.reload_automation_chats(cx);
        }
        let task = self.store.chat_groups(scope, Some(5));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(groups)) = task.await {
                this.update(cx, |this, cx| {
                    if this.chat.scope == scope {
                        this.chat.history = groups;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// `startNewChat`
    pub(super) fn start_new_chat(&mut self, cx: &mut Context<Self>) {
        self.stop_chat(cx);
        self.chat.group_id = None;
        self.chat.messages.clear();
        self.chat.status = None;
        self.chat.error = None;
        self.chat.queued.clear();
        self.chat.pending_manual_refs.clear();
        self.chat.history_open = false;
        cx.notify();
    }

    /// `selectChat(groupId)`: load the group's persisted messages.
    pub(super) fn select_chat(&mut self, group_id: String, cx: &mut Context<Self>) {
        self.stop_chat(cx);
        self.chat.history_open = false;
        self.chat.pending_manual_refs.clear();
        self.chat.group_id = Some(group_id.clone());
        self.chat.messages.clear();
        self.chat.status = None;
        self.chat.error = None;
        let task = self.store.chat_messages(group_id.clone());
        cx.spawn(async move |this, cx| {
            let rows = match task.await {
                Ok(Ok(rows)) => rows,
                _ => return,
            };
            this.update(cx, |this, cx| {
                if this.chat.group_id.as_deref() == Some(group_id.as_str()) {
                    this.chat.messages = rows
                        .into_iter()
                        .filter_map(chat::MessageRow::into_message)
                        .collect();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `ChatMessageInput`'s editor: `submitShortcut="enter"`, `Ask anything`.
    fn ensure_chat_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.chat.composer.is_some() {
            return;
        }
        let theme = self.theme;
        let style = TextAreaStyle {
            text: theme.foreground,
            placeholder: theme.muted_foreground,
            selection: theme.selection,
            font_size: px(14.0),
            line_height: px(20.0),
            rows: 1,
            paragraph_gap: px(0.0),
        };
        // `useMentionConfig`: the same `@` candidates the note editor offers.
        let candidates = self.mention_candidates.clone();
        let search = match cx.try_global::<crate::search::Search>() {
            Some(search) => super::mention_popup::search_over(
                candidates,
                search.0.clone(),
                self.store.runtime().clone(),
            ),
            None => std::rc::Rc::new(move |query: &str| {
                crate::editor::mention_picker::search_candidates(&candidates.borrow(), query)
            }),
        };
        let composer = cx.new(|cx| {
            TextArea::new("Ask anything", style, window, cx)
                .enter_submits()
                .with_mentions(
                    search,
                    MentionStyle {
                        icon: theme.muted_foreground,
                        dark: theme.dark,
                    },
                )
                .with_history_navigation()
        });
        cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &TextAreaEvent, window, cx| match event {
                TextAreaEvent::Submit => this.submit_chat_draft(cx),
                TextAreaEvent::Escape => {
                    if this.chat_mode == ChatMode::FloatingOpen {
                        this.close_chat(cx);
                        // The shortcuts keep working the moment the panel goes.
                        window.focus(&this.focus_handle);
                    }
                }
                TextAreaEvent::Changed => {
                    // `handleUserEdit`: typing leaves the history browse.
                    this.chat.history_index = None;
                    cx.notify()
                }
                TextAreaEvent::HistoryPrev => this.navigate_chat_history(true, cx),
                TextAreaEvent::HistoryNext => this.navigate_chat_history(false, cx),
                TextAreaEvent::Blurred => {}
            },
        )
        .detach();
        self.chat.composer = Some(composer);
    }

    /// `useMessageHistory().navigate`: Up walks back through the sent drafts
    /// (the current one set aside first), Down walks forward and finally
    /// restores it.
    fn navigate_chat_history(&mut self, prev: bool, cx: &mut Context<Self>) {
        let Some(composer) = self.chat.composer.clone() else {
            return;
        };
        let total = self.chat_sent_history.len();
        let (next_index, restore, at_end) = if prev {
            if total == 0 {
                return;
            }
            let next = self.chat.history_index.map_or(0, |index| index + 1);
            if next >= total {
                return;
            }
            if self.chat.history_index.is_none() {
                self.chat.draft_before_history = Some(composer.read(cx).draft());
            }
            (Some(next), self.chat_sent_history[next].clone(), false)
        } else {
            let Some(index) = self.chat.history_index else {
                return;
            };
            if index == 0 {
                (
                    None,
                    self.chat.draft_before_history.take().unwrap_or_default(),
                    true,
                )
            } else {
                (
                    Some(index - 1),
                    self.chat_sent_history[index - 1].clone(),
                    true,
                )
            }
        };
        self.chat.history_index = next_index;
        composer.update(cx, |composer, cx| composer.restore(restore, at_end, cx));
        cx.notify();
    }

    /// `pushSentMessage`: newest first, no immediate repeats, fifty kept.
    fn push_sent_history(&mut self, draft: Draft) {
        const MAX_ENTRIES: usize = 50;
        if self.chat_sent_history.first() == Some(&draft) {
            return;
        }
        self.chat_sent_history.insert(0, draft);
        self.chat_sent_history.truncate(MAX_ENTRIES);
    }

    /// `useSubmit`: send the trimmed draft with the refs its mentions carry,
    /// remember it for the history, and clear the editor.
    fn submit_chat_draft(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.chat.composer.clone() else {
            return;
        };
        let (text, mention_refs, draft) = {
            let composer = composer.read(cx);
            // `proseMirrorJsonToText` joins the blocks with nothing between
            // them, so a Shift+Enter break is not in the sent text.
            let text = composer.message_text().replace('\n', "").trim().to_string();
            let refs: Vec<ContextRef> = composer
                .atoms()
                .iter()
                .filter_map(|atom| ContextRef::manual(&atom.kind, &atom.id))
                .collect();
            (text, refs, composer.draft())
        };
        if text.is_empty() {
            return;
        }
        composer.update(cx, |composer, cx| composer.set_text("", cx));
        self.push_sent_history(draft);
        self.chat.history_index = None;
        self.chat.draft_before_history = None;
        self.submit_or_queue_chat_message(text, mention_refs, cx);
    }

    /// `useChatContextPipeline().pendingRefs` + `mergeContextRefs`: the
    /// current note, the dropped notes, then the mentions, one per key.
    fn merged_context_refs(&self, mention_refs: Vec<ContextRef>) -> Vec<ContextRef> {
        let scope = self.chat.scope;
        // The automations scope clears the note context (`chat-panel.tsx`).
        let auto = self
            .selected
            .as_deref()
            .filter(|_| scope == Scope::General)
            .map(ContextRef::auto_session);
        chat::dedupe_refs(
            auto.into_iter()
                .chain(self.chat.pending_manual_refs.iter().cloned())
                .chain(mention_refs),
        )
    }

    /// `submitOrQueueMessage`: a send while a reply streams waits its turn,
    /// with the refs it had when queued.
    fn submit_or_queue_chat_message(
        &mut self,
        text: String,
        mention_refs: Vec<ContextRef>,
        cx: &mut Context<Self>,
    ) {
        if !self.chat_model_configured() {
            return;
        }
        let refs = self.merged_context_refs(mention_refs);
        if self.chat.busy() {
            self.chat.queued.push_back((text, refs));
            cx.notify();
            return;
        }
        self.send_chat_message(text, refs, cx);
    }

    /// `onAddContextEntity`: a note dropped on the panel joins the next send.
    pub(super) fn add_chat_context_session(&mut self, session_id: &str, cx: &mut Context<Self>) {
        let Some(reference) = ContextRef::manual("session", session_id) else {
            return;
        };
        if self
            .chat
            .pending_manual_refs
            .iter()
            .any(|existing| existing.key() == reference.key())
        {
            return;
        }
        self.chat.pending_manual_refs.push(reference);
        cx.notify();
    }

    /// `handleSendMessage` + `sendMessage`: persist the user message (creating
    /// the group with its fallback title and kicking off the generated title
    /// on the first send), then stream the reply.
    fn send_chat_message(
        &mut self,
        text: String,
        context_refs: Vec<ContextRef>,
        cx: &mut Context<Self>,
    ) {
        let Some(Some(connection)) = self.chat.connection.clone() else {
            return;
        };
        let scope = self.chat.scope;
        let transcript_unavailable = scope == Scope::General && self.chat_transcript_unavailable();
        // A user message clears the pending refs (`prevUserMsgCountRef`).
        self.chat.pending_manual_refs.clear();
        let context_refs = Some(context_refs).filter(|refs| !refs.is_empty());
        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::User,
            parts: vec![Part::Text {
                text: text.clone(),
                provider_metadata: None,
                state: None,
            }],
            metadata: Metadata {
                chat_scope: Some(scope),
                created_at: Some(chrono::Utc::now().timestamp_millis()),
                context_refs,
            },
            status: Status::Ready,
        };
        let (group_id, created) = match self.chat.group_id.clone() {
            Some(group_id) => (group_id, false),
            None => (uuid::Uuid::new_v4().to_string(), true),
        };
        self.chat.group_id = Some(group_id.clone());
        let fallback_title = created.then(|| chat::create_fallback_chat_title(&text));
        if let Some(fallback_title) = &fallback_title {
            self.generate_chat_title(group_id.clone(), fallback_title.clone(), text.clone(), cx);
        }
        self.chat.messages.push(message.clone());
        self.chat.error = None;
        self.chat.status = Some(ChatStatus::Submitted);
        self.chat.auto_scroll = true;
        self.chat.show_go_to_recent = false;
        cx.notify();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let owner_user_id = store.owner_user_id().await.unwrap_or_default();
            let row =
                chat::MessageRow::from_message(&message, &group_id, &owner_user_id, Some(text));
            let persist = match fallback_title {
                Some(title) => store.create_chat_group_with_message(
                    group_id.clone(),
                    owner_user_id.clone(),
                    title,
                    row,
                ),
                None => store.upsert_chat_message(row),
            };
            // `beforeSend` must land before the transport runs.
            if !matches!(persist.await, Ok(Ok(()))) {
                this.update(cx, |this, cx| {
                    this.chat.messages.pop();
                    if created {
                        this.chat.group_id = None;
                    }
                    this.chat.status = None;
                    this.flash(
                        super::toast::FlashVariant::Error,
                        "Could not save this chat message.",
                        cx,
                    );
                })
                .ok();
                return;
            }
            if transcript_unavailable {
                // `isTranscriptUnavailable`: a local canned reply, persisted
                // like any assistant row; no generation runs.
                let reply = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: Role::Assistant,
                    parts: vec![Part::Text {
                        text: TRANSCRIPT_UNAVAILABLE_REPLY.to_string(),
                        provider_metadata: None,
                        state: None,
                    }],
                    metadata: Metadata {
                        created_at: Some(
                            (chrono::Utc::now().timestamp_millis() + 1)
                                .max(message.metadata.created_at.unwrap_or(0) + 1),
                        ),
                        ..Metadata::default()
                    },
                    status: Status::Ready,
                };
                let row = chat::MessageRow::from_message(&reply, &group_id, &owner_user_id, None);
                let saved = matches!(store.upsert_chat_message(row).await, Ok(Ok(())));
                this.update(cx, |this, cx| {
                    if saved {
                        this.chat.messages.push(reply);
                    } else {
                        tracing::error!("Failed to save batch transcription chat response");
                        this.chat.messages.retain(|m| m.id != message.id);
                    }
                    this.chat.status = Some(ChatStatus::Ready);
                    this.load_chat_history(cx);
                    cx.notify();
                    if let Some((next, refs)) = this.chat.queued.pop_front() {
                        this.send_chat_message(next, refs, cx);
                    }
                })
                .ok();
                return;
            }
            this.update(cx, |this, cx| {
                this.load_chat_history(cx);
                this.stream_chat_reply(connection, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `isBatchTranscriptionPending && !hasAvailableTranscript` for the
    /// note the chat is open on: a running batch, or a capture (active or
    /// finalizing) that is not transcribing live, while the session has no
    /// transcript yet.
    fn chat_transcript_unavailable(&self) -> bool {
        let Some(session_id) = self.selected.as_deref() else {
            return false;
        };
        let has_transcript = match &self.note {
            super::Note::Ready { preview, .. } if preview.session.id == session_id => {
                preview.has_transcript
            }
            _ => false,
        };
        if has_transcript {
            return false;
        }
        match self.session_mode(session_id) {
            super::recording::SessionMode::RunningBatch => true,
            super::recording::SessionMode::Active => self
                .recording
                .live
                .as_ref()
                .is_some_and(|live| !live.live_active),
            super::recording::SessionMode::Finalizing => self
                .recording
                .pending_post_capture
                .get(session_id)
                .and_then(|pending| pending.snapshot.as_ref())
                .is_some_and(|(live_active, _, _)| !live_active),
            super::recording::SessionMode::Inactive => false,
        }
    }

    /// `generateChatTitle` → `setChatGroupTitleIfCurrent`.
    fn generate_chat_title(
        &mut self,
        group_id: String,
        fallback_title: String,
        initial_request: String,
        cx: &mut Context<Self>,
    ) {
        let Some(Some(connection)) = self.chat.connection.clone() else {
            return;
        };
        let Some(prompt) = chat::title_request(&initial_request) else {
            return;
        };
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            // `generateText({ maxRetries: 2, maxOutputTokens: 32 })`: one whole reply.
            let request = Request::new(chat::TITLE_SYSTEM_PROMPT, prompt, 32);
            let generated = store
                .runtime()
                .spawn(async move { llm_stream::generate(&connection, &request, 2).await })
                .await;
            let text = match generated {
                Ok(Ok(generated)) => generated.text,
                Ok(Err(error)) => {
                    tracing::error!(%error, "Failed to generate chat title");
                    return;
                }
                Err(error) => {
                    tracing::error!(%error, "Failed to generate chat title");
                    return;
                }
            };
            let Some(title) = chat::normalize_generated_chat_title(&text) else {
                return;
            };
            let _ = store
                .set_chat_group_title_if_current(group_id, fallback_title, title)
                .await;
            this.update(cx, |this, cx| this.load_chat_history(cx)).ok();
        })
        .detach();
    }

    /// `CustomChatTransport.sendMessages` → `ToolLoopAgent.stream`: the
    /// system prompt with the tool guidance, the context block prepended to
    /// the last user message, the message window, the `chat-general` tools,
    /// and up to `MAX_TOOL_STEPS` generations — each tool call runs locally
    /// and its result feeds the next step — streamed into the assistant
    /// message; `onFinish` persists it.
    fn stream_chat_reply(&mut self, connection: Connection, cx: &mut Context<Self>) {
        // `providerOptionsName`: the key the SDK files a Responses item's
        // metadata under.
        let provider_name: &'static str = if connection.provider_id == "azure_openai" {
            "azure"
        } else {
            "openai"
        };
        let Some(group_id) = self.chat.group_id.clone() else {
            return;
        };
        self.chat.run += 1;
        let run = self.chat.run;
        let abort = Arc::new(AtomicBool::new(false));
        self.chat.abort = Some(abort.clone());
        self.chat.status = Some(ChatStatus::Submitted);
        let language = self
            .provider_settings
            .string_setting("ai_language", &["language", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        // `extractContextRefsFromMessages`: every ref across the conversation,
        // deduped by key, hydrated once — sessions into the context block,
        // contacts / organizations / folders into text blocks after it.
        let refs = chat::dedupe_refs(
            self.chat
                .messages
                .iter()
                .flat_map(|message| message.metadata.context_refs.iter().flatten())
                .cloned(),
        );
        let context_tasks: Vec<ContextTask> = refs
            .into_iter()
            .filter_map(|reference| match reference {
                ContextRef::Session { session_id, .. } => Some(ContextTask::Session(
                    self.store.chat_session_context(session_id),
                )),
                ContextRef::Other(_) => None,
                reference => Some(ContextTask::Text(self.store.chat_context_text(reference))),
            })
            .collect();
        let history: Vec<Message> = self.chat.messages.clone();
        let replace_previous = self.chat.replace_previous.take();
        // `openEditTab`: proposals the edit tools hand over for review.
        let (edit_requests, mut edit_reviews) =
            tokio::sync::mpsc::unbounded_channel::<crate::chat_tools::PendingEdit>();
        let tool_context = crate::chat_tools::Context {
            session_id: self
                .active_session_tab_id()
                .filter(|_| self.chat.scope == Scope::General),
            folder_filter: self.folder_filter_for_chat(),
            busy_sessions: self.busy_sessions(),
            enhanced_note_id: self.open_enhanced_note_id(),
            edit_requests: Some(edit_requests),
        };
        let runner = Arc::new(crate::chat_tools::Runner {
            store: self.store.clone(),
            search: cx
                .try_global::<crate::search::Search>()
                .map(|search| search.0.clone()),
        });
        let runtime = self.store.runtime().clone();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let owner_user_id = store.owner_user_id().await.unwrap_or_default();
            let mut contexts = Vec::new();
            let mut texts = Vec::new();
            for task in context_tasks {
                match task {
                    ContextTask::Session(task) => {
                        if let Ok(Ok(Some(context))) = task.await {
                            contexts.push(context);
                        }
                    }
                    ContextTask::Text(task) => {
                        if let Ok(Ok(Some(text))) = task.await
                            && !text.trim().is_empty()
                        {
                            texts.push(text.trim().to_string());
                        }
                    }
                }
            }
            let context_block = render_context_blocks(contexts, texts);
            let system = anlg_template_app::render(anlg_template_app::Template::ChatSystem(
                anlg_template_app::ChatSystem {
                    language: Some(language),
                },
            ))
            .unwrap_or_default();
            let system = chat::append_meeting_context_tool_guidance(&system);
            // `convertToModelMessages` over the window, the context block on
            // the last user message only; earlier `search_meetings` outputs
            // carry their hydrated context like `expandSearchMeetingsOutput`.
            let last_user = history
                .iter()
                .rposition(|message| message.role == Role::User);
            let mut base_turns: Vec<Turn> = Vec::new();
            for (index, message) in history.iter().enumerate() {
                match message.role {
                    Role::User => {
                        let text = chat::extract_text_content(&message.parts);
                        // The block is its own text part ahead of the message's.
                        base_turns.push(match &context_block {
                            Some(block) if Some(index) == last_user => {
                                Turn::UserParts(vec![format!("{block}\n\n"), text])
                            }
                            _ => Turn::User(text),
                        });
                    }
                    Role::Assistant => {
                        let parts = expand_search_outputs(&store, &message.parts).await;
                        base_turns.extend(chat::assistant_turns(&parts));
                    }
                }
            }

            let assistant_id = uuid::Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now().timestamp_millis();
            if this
                .update(cx, |this, cx| {
                    if this.chat.run != run {
                        return;
                    }
                    this.chat.messages.push(Message {
                        id: assistant_id.clone(),
                        role: Role::Assistant,
                        parts: vec![Part::StepStart],
                        metadata: Metadata {
                            created_at: Some(created_at),
                            ..Metadata::default()
                        },
                        status: Status::Streaming,
                    });
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            // The parts of the streaming assistant message; `apply` pushes
            // them to the list.
            let mut parts: Vec<Part> = vec![Part::StepStart];
            let apply = |this: &gpui::WeakEntity<Self>,
                         cx: &mut gpui::AsyncApp,
                         parts: &[Part],
                         status: ChatStatus|
             -> bool {
                let parts = parts.to_vec();
                this.update(cx, |this, cx| {
                    if this.chat.run != run {
                        return false;
                    }
                    this.chat.status = Some(status);
                    if let Some(message) = this
                        .chat
                        .messages
                        .iter_mut()
                        .find(|message| message.id == assistant_id)
                    {
                        message.parts = parts;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false)
            };
            let mut outcome: Result<(), String> = Ok(());
            let mut aborted = false;
            'steps: for step in 0..crate::chat_tools::MAX_TOOL_STEPS {
                let mut turns = base_turns.clone();
                if step > 0 {
                    turns.extend(chat::assistant_turns(&parts));
                }
                let turns = chat::window_messages(turns);
                let mut request = Request::new(system.clone(), "", 0);
                request.messages = turns;
                request.tools = crate::chat_tools::specs().to_vec();
                let mut receiver = llm_stream::stream(&runtime, connection.clone(), request);
                let mut text = String::new();
                let mut reasoning = String::new();
                let mut calls: Vec<llm_stream::ToolCall> = Vec::new();
                let mut items: Vec<llm_stream::StoredItem> = Vec::new();
                let step_start = parts.len();
                while let Some(chunk) = receiver.recv().await {
                    if abort.load(Ordering::Relaxed) {
                        aborted = true;
                        break 'steps;
                    }
                    match chunk {
                        Chunk::TextDelta(delta) => text.push_str(&delta),
                        Chunk::ReasoningDelta(delta) => reasoning.push_str(&delta),
                        Chunk::ToolCall(call) => calls.push(call),
                        Chunk::Item(item) => items.push(item),
                        Chunk::Done => break,
                        Chunk::Error(error) => {
                            outcome = Err(error);
                            break 'steps;
                        }
                    }
                    parts.truncate(step_start);
                    parts.extend(step_parts(&reasoning, &text, &items, provider_name, false));
                    if !apply(&this, cx, &parts, ChatStatus::Streaming) {
                        return;
                    }
                }
                if abort.load(Ordering::Relaxed) {
                    aborted = true;
                    break 'steps;
                }
                parts.truncate(step_start);
                parts.extend(step_parts(&reasoning, &text, &items, provider_name, true));
                if calls.is_empty() {
                    break 'steps;
                }
                // `input-available` while the tools run, then their outputs.
                let first_tool = parts.len();
                let call_metadata = |call: &llm_stream::ToolCall| {
                    call.item_id
                        .as_ref()
                        .map(|id| serde_json::json!({ provider_name: { "itemId": id } }))
                };
                for call in &calls {
                    parts.push(Part::tool(
                        &call.name,
                        &call.id,
                        "input-available",
                        call.arguments.clone(),
                        None,
                        None,
                        call_metadata(call),
                    ));
                }
                if !apply(&this, cx, &parts, ChatStatus::Streaming) {
                    return;
                }
                for (offset, call) in calls.iter().enumerate() {
                    let mut run = runner.run(
                        tool_context.clone(),
                        call.id.clone(),
                        call.name.clone(),
                        call.arguments.clone(),
                    );
                    // A tool may open a review and wait for it: keep the
                    // panel responsive and show the review while it runs.
                    let result = loop {
                        tokio::select! {
                            result = &mut run => {
                                break result.unwrap_or_else(|error| Err(error.to_string()));
                            }
                            Some(edit) = edit_reviews.recv() => {
                                if this
                                    .update(cx, |this, cx| this.open_edit_review(edit, cx))
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                    };
                    parts[first_tool + offset] = match result {
                        Ok(output) => Part::tool(
                            &call.name,
                            &call.id,
                            "output-available",
                            call.arguments.clone(),
                            Some(output),
                            None,
                            call_metadata(call),
                        ),
                        Err(error) => Part::tool(
                            &call.name,
                            &call.id,
                            "output-error",
                            call.arguments.clone(),
                            None,
                            Some(error),
                            call_metadata(call),
                        ),
                    };
                    if !apply(&this, cx, &parts, ChatStatus::Streaming) {
                        return;
                    }
                    if abort.load(Ordering::Relaxed) {
                        aborted = true;
                        break 'steps;
                    }
                }
                if step + 1 < crate::chat_tools::MAX_TOOL_STEPS {
                    parts.push(Part::StepStart);
                }
            }
            let has_content = parts.iter().any(|part| match part {
                Part::Text { text, .. } | Part::Reasoning { text, .. } => !text.trim().is_empty(),
                Part::StepStart => false,
                Part::Other(_) => true,
            });
            this.update(cx, |this, cx| {
                if this.chat.run != run {
                    return;
                }
                this.chat.abort = None;
                let position = this
                    .chat
                    .messages
                    .iter()
                    .position(|message| message.id == assistant_id);
                match (&outcome, aborted) {
                    // `isAbort`: the partial reply stays on screen, unpersisted.
                    (_, true) => {
                        if let Some(index) = position {
                            if !has_content {
                                this.chat.messages.remove(index);
                            } else {
                                this.chat.messages[index].parts = parts;
                                this.chat.messages[index].status = Status::Aborted;
                            }
                        }
                        this.chat.status = Some(ChatStatus::Ready);
                    }
                    (Err(error), false) => {
                        if let Some(index) = position {
                            this.chat.messages.remove(index);
                        }
                        this.chat.error = Some(error.clone());
                        this.chat.status = Some(ChatStatus::Error);
                    }
                    (Ok(()), false) => {
                        if let Some(index) = position {
                            let message = &mut this.chat.messages[index];
                            message.parts = parts;
                            message.status = Status::Ready;
                            // `onFinish` persists the assistant message.
                            let row = chat::MessageRow::from_message(
                                message,
                                &group_id,
                                &owner_user_id,
                                None,
                            );
                            let write = match replace_previous.clone() {
                                Some(previous) => store.replace_chat_message(row, previous),
                                None => store.upsert_chat_message(row),
                            };
                            store.runtime().spawn(async move {
                                if let Ok(Err(error)) = write.await {
                                    tracing::error!(%error, "Failed to persist chat message");
                                }
                            });
                        }
                        this.chat.status = Some(ChatStatus::Ready);
                    }
                }
                cx.notify();
                // `ChatQueue`: the next queued send goes out once ready.
                if this.chat.status() == ChatStatus::Ready
                    && let Some((next, refs)) = this.chat.queued.pop_front()
                {
                    this.send_chat_message(next, refs, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `getFolderFilter()`: the folder the sidebar's note filter is scoped to.
    fn folder_filter_for_chat(&self) -> Option<String> {
        None
    }

    /// `getEnhancedNoteId()`: the summary the open note's tab shows, in the
    /// general scope.
    /// `useSessionTab().getSessionId()`: the selected session only while a
    /// sessions tab is what the main surface shows (not settings, folders,
    /// templates, calendar, contacts, automations, or an edit review).
    fn active_session_tab_id(&self) -> Option<String> {
        if self.custom_sidebar_open() || self.edit_review_open() {
            return None;
        }
        self.selected.clone()
    }

    fn open_enhanced_note_id(&self) -> Option<String> {
        if self.chat.scope != Scope::General || self.active_session_tab_id().is_none() {
            return None;
        }
        match &self.note {
            super::Note::Ready {
                tab: super::NoteTab::Enhanced(id),
                ..
            } => Some(id.clone()),
            _ => None,
        }
    }

    /// `isSessionBusy`: the live capture (active or finalizing), the running
    /// batches, and the captures still finishing after their stop.
    fn busy_sessions(&self) -> Vec<String> {
        let mut busy: Vec<String> = Vec::new();
        if let Some(live) = &self.recording.live {
            busy.push(live.session_id.clone());
        }
        busy.extend(self.recording.finalizing.iter().cloned());
        busy.extend(
            self.recording
                .batch
                .iter()
                .filter(|(_, batch)| batch.error.is_none())
                .map(|(id, _)| id.clone()),
        );
        busy.extend(self.recording.pending_post_capture.keys().cloned());
        busy
    }

    /// `stop()`: abort the stream.
    pub(super) fn stop_chat(&mut self, cx: &mut Context<Self>) {
        if let Some(abort) = self.chat.abort.take() {
            abort.store(true, Ordering::Relaxed);
        }
        if self.chat.busy() {
            self.chat.status = Some(ChatStatus::Ready);
            self.chat.run += 1;
            if let Some(last) = self.chat.messages.last_mut()
                && last.role == Role::Assistant
                && last.status == Status::Streaming
            {
                if last
                    .parts
                    .iter()
                    .all(|part| matches!(part, Part::StepStart))
                {
                    self.chat.messages.pop();
                } else {
                    last.status = Status::Aborted;
                }
            }
            cx.notify();
        }
    }

    /// `regenerate()`: drop the last assistant reply (tombstoning its row)
    /// and stream again.
    fn regenerate_chat(&mut self, cx: &mut Context<Self>) {
        if self.chat.busy() {
            return;
        }
        let Some(Some(connection)) = self.chat.connection.clone() else {
            return;
        };
        let Some(group_id) = self.chat.group_id.clone() else {
            return;
        };
        if let Some(last) = self.chat.messages.last()
            && last.role == Role::Assistant
        {
            let previous = self.chat.messages.pop().expect("checked");
            self.chat.replace_previous = Some(previous.id);
        }
        let _ = group_id;
        self.chat.error = None;
        self.stream_chat_reply(connection, cx);
    }

    /// `ChatBody`: the message list, or the empty state; the floating body
    /// scrolls inside `max-h-[min(36rem,70vh)]` with `px-5 py-3`.
    pub(super) fn render_chat_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        if !self.chat_model_configured() {
            return self.render_chat_setup_prompt(cx);
        }
        let has_context = self.chat.scope == Scope::General && self.selected.is_some();
        let right_panel = self.chat_in_right_panel();
        // `useChatAutoScroll`: pinned to the bottom while generating unless the
        // user scrolled up; `Go to recent` once they scroll down again.
        let handle = self.chat_scroll.clone();
        let max = handle.max_offset().height;
        let distance_from_bottom = max + handle.offset().y;
        let is_at_bottom = distance_from_bottom <= px(24.0);
        if is_at_bottom {
            self.chat.auto_scroll = true;
            self.chat.show_go_to_recent = false;
        }
        if self.chat.auto_scroll && !self.chat.messages.is_empty() {
            handle.scroll_to_bottom();
        }
        // Floating: `flex-auto max-h-[min(36rem,70vh)]`, `px-5 py-3`; right
        // panel: `flex-1` filling the column with `px-3 py-5` and the spacer
        // above the messages.
        let list = div()
            .id("chat-body")
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .when(right_panel, |body| body.flex_1())
            .child(
                div()
                    .id("chat-scroll")
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .map(|scroll| {
                        if right_panel {
                            scroll.flex_1()
                        } else {
                            scroll.max_h(px(LIST_MAX_HEIGHT))
                        }
                    })
                    .overflow_y_scroll()
                    .track_scroll(&self.chat_scroll)
                    .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                        let delta_y = match event.delta {
                            gpui::ScrollDelta::Pixels(delta) => f32::from(delta.y),
                            gpui::ScrollDelta::Lines(delta) => delta.y,
                        };
                        // Scrolling up unpins; scrolling down while unpinned offers `Go to recent`.
                        if delta_y > 0.0 {
                            this.chat.auto_scroll = false;
                            this.chat.show_go_to_recent = false;
                        } else if delta_y < 0.0 && !this.chat.auto_scroll {
                            this.chat.show_go_to_recent = true;
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .map(|content| {
                                if right_panel {
                                    content
                                        .min_h_full()
                                        .flex_1()
                                        .px_3()
                                        .py_5()
                                        .child(div().flex_1())
                                } else {
                                    content.px_5().py_3()
                                }
                            })
                            .child(if self.chat.messages.is_empty() {
                                self.render_chat_suggestions(has_context, cx)
                            } else {
                                self.render_chat_messages(window, cx)
                            }),
                    ),
            )
            .when(
                !self.chat.messages.is_empty() && self.chat.show_go_to_recent && !is_at_bottom,
                |body| {
                    // `absolute bottom-3 left-1/2 -translate-x-1/2`: `Go to recent`.
                    body.child(
                        div()
                            .absolute()
                            .bottom(px(12.0))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .id("chat-go-to-recent")
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .h(px(32.0))
                                    .px_3()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .shadow_xs()
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(theme.accent))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.chat.auto_scroll = true;
                                        this.chat.show_go_to_recent = false;
                                        this.chat_scroll.scroll_to_bottom();
                                        cx.notify();
                                    }))
                                    .child(icon("caret-down", px(12.0), theme.foreground))
                                    .child(div().tw_text_xs().child("Go to recent")),
                            ),
                    )
                },
            );
        list.into_any_element()
    }

    /// `ChatBodyEmpty` with a model: the suggestion rows while the open note
    /// gives the chat its context.
    fn render_chat_suggestions(&self, has_context: bool, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let icons = ["list-checks", "envelope", "magnifying-glass"];
        div()
            .flex()
            .justify_start()
            .pb_1()
            .child(
                div()
                    .flex()
                    .w_full()
                    .flex_col()
                    .when(has_context, |column| {
                        column.child(div().flex().flex_col().gap(px(2.0)).children(
                            chat::SUGGESTIONS.iter().zip(icons).enumerate().map(
                                |(index, ((label, prompt), glyph))| {
                                    let prompt = prompt.to_string();
                                    // `grid-cols-[1.5rem_minmax(0,1fr)] gap-x-1.5 rounded-lg
                                    // py-2 pr-3 text-sm text-muted-foreground hover:bg-muted/55`
                                    div()
                                        .id(("chat-suggestion", index))
                                        .flex()
                                        .w_full()
                                        .items_center()
                                        .gap(px(6.0))
                                        .rounded(px(8.0))
                                        .py_2()
                                        .pr_3()
                                        .tw_text_sm()
                                        .text_color(theme.muted_foreground)
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(alpha(theme.muted, 0.55)))
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                this.submit_or_queue_chat_message(
                                                    prompt.clone(),
                                                    Vec::new(),
                                                    cx,
                                                );
                                            },
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .size(px(24.0))
                                                .flex_shrink_0()
                                                .items_center()
                                                .justify_center()
                                                .child(icon(
                                                    glyph,
                                                    px(16.0),
                                                    alpha(theme.muted_foreground, 0.75),
                                                )),
                                        )
                                        .child(div().min_w_0().truncate().child(*label))
                                },
                            ),
                        ))
                    }),
            )
            .into_any_element()
    }

    /// `ChatBodyNonEmpty`: the bubbles, then `Thinking...` while nothing is
    /// renderable yet, then the error bubble.
    fn render_chat_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let status = self.chat.status();
        let last_assistant = self
            .chat
            .messages
            .iter()
            .rposition(|message| message.role == Role::Assistant);
        let last = self.chat.messages.last();
        let waiting = matches!(status, ChatStatus::Submitted | ChatStatus::Streaming)
            && last.is_none_or(|message| {
                message.role != Role::Assistant
                    || !has_renderable_content(message)
                    || matches!(message.parts.last(), Some(Part::StepStart))
            });
        let messages = self.chat.messages.clone();
        let renderer = self.document_renderer(window);
        let mut column = div().flex().flex_col();
        for (index, message) in messages.iter().enumerate() {
            if !has_renderable_content(message) {
                continue;
            }
            let is_user = message.role == Role::User;
            let bubble = if is_user {
                // `w-fit rounded-2xl bg-blue-100 px-3 py-1 text-neutral-800`
                div()
                    .w_auto()
                    .rounded(px(16.0))
                    .bg(gpui::rgb(0xdbeafe))
                    .px_3()
                    .py_1()
                    .tw_text_sm()
                    .text_color(gpui::rgb(0x262626))
                    .children(message.parts.iter().filter_map(|part| match part {
                        Part::Text { text, .. } => Some(div().px(px(2.0)).py_1().children(
                            renderer.chat_blocks(&crate::document::from_body("markdown", text)),
                        )),
                        _ => None,
                    }))
            } else {
                let mut bubble = div()
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .when(theme.dark, |b| {
                        b.rounded(px(16.0)).bg(theme.accent).px_3().py_1()
                    });
                // `isActivityPart`: reasoning with text and every tool but the
                // edit tools fold into one `Activity` disclosure ahead of the
                // visible parts (#7461).
                let activity: Vec<(usize, &Part)> = message
                    .parts
                    .iter()
                    .enumerate()
                    .filter(|(_, part)| is_activity_part(part))
                    .collect();
                if !activity.is_empty() {
                    let running = activity.iter().any(|(_, part)| match part {
                        Part::Reasoning { state, .. } => state.as_deref() == Some("streaming"),
                        other => other.tool_view().is_some_and(|tool| {
                            matches!(tool.state, "input-streaming" | "input-available")
                        }),
                    });
                    let key = format!("activity:{index}");
                    let open = self.chat.open_tools.contains(&key);
                    let body = open.then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .children(activity.iter().map(|(part_index, part)| {
                                self.render_chat_part(part, (index, *part_index), &renderer, cx)
                            }))
                            .into_any_element()
                    });
                    // One past the last part index: unique per message.
                    bubble = bubble.child(self.render_disclosure(
                        (index, message.parts.len()),
                        if running { "spinner" } else { "brain" },
                        "Activity".to_string(),
                        false,
                        open,
                        key,
                        body,
                        cx,
                    ));
                }
                for (part_index, part) in message.parts.iter().enumerate() {
                    if is_activity_part(part) {
                        continue;
                    }
                    bubble = bubble.child(self.render_chat_part(
                        part,
                        (index, part_index),
                        &renderer,
                        cx,
                    ));
                }
                bubble
            };
            let mut row = div()
                .flex()
                .py_2()
                .when(is_user, |row| row.justify_end())
                .when(!is_user, |row| row.justify_start());
            if is_user {
                row = row.child(
                    div()
                        .flex()
                        .min_w_0()
                        .max_w(gpui::relative(0.85))
                        .flex_col()
                        .items_end()
                        .child(bubble),
                );
            } else {
                let is_last_assistant = last_assistant == Some(index);
                let can_regenerate = is_last_assistant && status == ChatStatus::Ready;
                let text = chat::extract_text_content(&message.parts);
                row = row.child(
                    div()
                        .id(("chat-assistant", index))
                        .group("chat-assistant")
                        .flex()
                        .w_full()
                        .min_w_0()
                        .flex_col()
                        .child(bubble)
                        .child(
                            // `mt-1 flex items-center gap-1 opacity-0 group-hover:opacity-100`
                            div()
                                .mt_1()
                                .flex()
                                .items_center()
                                .gap_1()
                                .opacity(0.0)
                                .group_hover("chat-assistant", |row| row.opacity(1.0))
                                .child(
                                    div()
                                        .id(("chat-copy", index))
                                        .p_1()
                                        .cursor_pointer()
                                        .text_color(theme.muted_foreground)
                                        .hover(move |style| style.text_color(theme.foreground))
                                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                text.clone(),
                                            ));
                                        }))
                                        .child(icon("copy", px(14.0), theme.muted_foreground)),
                                )
                                .when(can_regenerate, |actions| {
                                    actions.child(
                                        div()
                                            .id(("chat-regenerate", index))
                                            .p_1()
                                            .cursor_pointer()
                                            .text_color(theme.muted_foreground)
                                            .hover(move |style| style.text_color(theme.foreground))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.regenerate_chat(cx)
                                            }))
                                            .child(icon(
                                                "arrow-counter-clockwise",
                                                px(14.0),
                                                theme.muted_foreground,
                                            )),
                                    )
                                }),
                        ),
                );
            }
            column = column.child(row);
        }
        if waiting {
            // `LoadingMessage`: the spinner and `Thinking...`.
            column = column.child(
                div().flex().py_2().justify_start().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .tw_text_sm()
                        .text_color(theme.foreground)
                        .child(crate::ui::spinner(
                            "chat-thinking",
                            px(16.0),
                            theme.foreground,
                        ))
                        .child("Thinking..."),
                ),
            );
        }
        if status == ChatStatus::Error
            && let Some(error) = &self.chat.error
        {
            // `ErrorMessage`: `rounded-2xl border border-red-200 bg-red-50 px-3
            // py-1 text-red-600` with the hover retry.
            column = column.child(
                div().flex().py_2().justify_start().child(
                    div()
                        .id("chat-error")
                        .group("chat-error")
                        .relative()
                        .rounded(px(16.0))
                        .border_1()
                        .border_color(gpui::rgb(0xffc9c9))
                        .bg(gpui::rgb(0xfef2f2))
                        .px_3()
                        .py_1()
                        .tw_text_sm()
                        .text_color(gpui::rgb(0xe7000b))
                        .child(SharedString::from(error.clone()))
                        .child(
                            div()
                                .id("chat-error-retry")
                                .absolute()
                                .top(px(-4.0))
                                .right(px(-4.0))
                                .flex()
                                .size(px(20.0))
                                .items_center()
                                .justify_center()
                                .rounded(px(10.0))
                                .border_1()
                                .border_color(gpui::rgb(0xffc9c9))
                                .bg(gpui::rgb(0xffffff))
                                .opacity(0.0)
                                .group_hover("chat-error", |style| style.opacity(1.0))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.regenerate_chat(cx)
                                }))
                                .child(icon(
                                    "arrow-counter-clockwise",
                                    px(12.0),
                                    gpui::rgb(0xe7000b),
                                )),
                        ),
                ),
            );
        }
        column.into_any_element()
    }

    /// `ChatBodyEmpty` without a model: the greeting, the Beta chip, and the
    /// `Open AI Settings` button. Shared by the floating frame and the
    /// automations tab's right panel.
    pub(super) fn render_chat_setup_prompt(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .px_5()
            .py_3()
            .child(
                div()
                    .flex()
                    .py_2()
                    .pb_1()
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .flex_col()
                            .child(
                                div()
                                    .mb_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child("Anarlog AI"),
                                    )
                                    .child(
                                        // `BetaChip`: `rounded-full border px-1.5 py-0.5
                                        // text-[10px] font-medium border-sky-200 bg-sky-100
                                        // text-sky-900`.
                                        div()
                                            .rounded(px(8.0))
                                            .border_1()
                                            .border_color(gpui::rgb(0xbae6fd))
                                            .bg(gpui::rgb(0xe0f2fe))
                                            .text_color(gpui::rgb(0x0c4a6e))
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .text_size(px(10.0))
                                            .line_height(px(15.0))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child("Beta"),
                                    ),
                            )
                            .child(
                                div()
                                    .mb_2()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Hi, I'm Anarlog AI. Set up a language model and I'll be ready to help."),
                            )
                            .child(
                                div().flex().child(
                                div()
                                    .id("chat-open-ai-settings")
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .rounded(px(8.0))
                                    .border_1()
                                    .border_color(theme.primary)
                                    .bg(theme.primary)
                                    .text_color(theme.primary_foreground)
                                    .px_3()
                                    .py(px(6.0))
                                    .tw_text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .shadow(vec![gpui::BoxShadow {
                                        color: gpui::Rgba {
                                            r: 87.0 / 255.0,
                                            g: 83.0 / 255.0,
                                            b: 78.0 / 255.0,
                                            a: 0.18,
                                        }
                                        .into(),
                                        offset: gpui::point(px(0.0), px(4.0)),
                                        blur_radius: px(14.0),
                                        spread_radius: px(0.0),
                                    }])
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(alpha(theme.primary, 0.9)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.close_chat(cx);
                                        this.open_settings(
                                            super::settings::SettingsTab::Intelligence,
                                            window,
                                            cx,
                                        );
                                    }))
                                    .child(icon("sparkle", px(12.0), theme.primary_foreground))
                                    .child("Open AI Settings"),
                            )),
                    ),
            )
        .into_any_element()
    }

    /// The main surface's width: the viewport minus the expanded sidebar,
    /// its 4px gutter and 1px border (`FloatingActionButton` measures it the
    /// same way).
    pub(super) fn main_surface_width(&self, window: &Window) -> f32 {
        f32::from(window.viewport_size().width)
            - if self.sidebar_expanded && !self.is_standalone() {
                self.custom_sidebar_width() + 4.0 + 1.0
            } else {
                0.0
            }
    }

    /// `SendButton`: `size-7 rounded-full border`, filled when enabled.
    pub(super) fn render_chat_send_button(&self, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        div()
            .id("chat-send")
            .flex()
            .size(px(28.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(14.0))
            .border_1()
            .map(|button| {
                if enabled {
                    button
                        .border_color(gpui::rgb(0x57534e))
                        .bg(theme.primary)
                        .cursor_pointer()
                        .hover(move |style| style.bg(alpha(theme.primary, 0.9)))
                } else {
                    button.border_color(theme.border)
                }
            })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit_chat_draft(cx)))
            .child(icon(
                "arrow-up",
                px(15.0),
                if enabled {
                    theme.primary_foreground
                } else {
                    alpha(theme.muted_foreground, 0.6)
                },
            ))
            .into_any_element()
    }

    /// `ChatMessageInput`: floating, `px-1 pb-1` around the `rounded-[19px]
    /// bg-white border pl-4 pr-[6px] min-h-[38px]` row with the controls at
    /// its right edge; right panel, `px-2 pb-3` around the elevated
    /// `rounded-xl` column (`px-2 pt-3 pb-2`) with the controls under the
    /// editor, the send button always shown.
    /// `panel_width` is the panel's outer width: the editor takes an explicit
    /// width from it because a percent-wide text child is measured before
    /// its parent's width resolves, and that narrow, tall measurement sticks.
    fn render_chat_composer(
        &mut self,
        panel_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.ensure_chat_composer(window, cx);
        let theme = self.theme;
        let right_panel = self.chat_in_right_panel();
        let composer = self.chat.composer.clone().expect("ensured");
        if std::mem::take(&mut self.chat.focus_pending) {
            composer.update(cx, |composer, cx| composer.focus_end(window, cx));
        }
        let has_content = !composer.read(cx).text().trim().is_empty();
        let streaming = self.chat.busy();
        let show_send = right_panel || streaming || has_content;
        let queued = self.chat.queued.clone();
        // `History N/M` above the surface while browsing sent messages.
        let history_indicator = self.chat.history_index.map(|index| {
            // `text-[11px] leading-none pb-1`: an 11px line over 4px.
            div()
                .flex()
                .items_start()
                .h(px(15.0))
                .flex_shrink_0()
                .map(|row| if right_panel { row.px_2() } else { row.px_4() })
                .text_size(px(11.0))
                .line_height(px(11.0))
                .text_color(alpha(theme.muted_foreground, 0.8))
                .child(SharedString::from(format!(
                    "History {}/{}",
                    index + 1,
                    self.chat_sent_history.len()
                )))
        });
        let mut controls = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .map(|controls| {
                if right_panel {
                    controls.justify_end()
                } else {
                    controls.absolute().right_0().bottom(px(2.0))
                }
            });
        if !streaming {
            // `Start voice input`: `size-7 rounded-full text-muted-foreground`.
            controls = controls.child(
                div()
                    .id("chat-mic")
                    .flex()
                    .size(px(28.0))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(14.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.start_dictation(cx)))
                    .child(icon("microphone", px(17.0), theme.muted_foreground)),
            );
        }
        if streaming {
            controls = controls.child(
                div()
                    .id("chat-stop")
                    .flex()
                    .size(px(28.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(14.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop_chat(cx)))
                    .child(icon("square", px(14.0), theme.foreground)),
            );
        } else if show_send {
            controls = controls.child(self.render_chat_send_button(has_content, cx));
        }
        // `hasVoiceStatus`: the `VoiceStatus` row replaces the controls and
        // the floating row becomes a column (`flex-col items-stretch`, no
        // editor padding, `items-stretch py-2` on the surface).
        let voice_active = self.dictation_active();
        let controls: AnyElement = if voice_active {
            self.render_voice_status(
                show_send && !streaming,
                has_content && !streaming,
                streaming,
                cx,
            )
        } else {
            controls.into_any_element()
        };
        // Leave room for the controls at the editor's right edge.
        let editor_padding = if voice_active {
            0.0
        } else if streaming || show_send {
            64.0
        } else {
            32.0
        };
        if right_panel {
            return div()
                .relative()
                .flex()
                .flex_col()
                .min_w_0()
                .flex_shrink_0()
                .px_2()
                .pb_3()
                .children(history_indicator)
                .when(!queued.is_empty(), |column| {
                    column.child(
                        div()
                            .px_1()
                            .pb(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .children(queued.into_iter().map(|(text, _)| {
                                div()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(SharedString::from(text))
                            })),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .rounded(px(12.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.card)
                        .tw_text_sm()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .px_2()
                                .pt_3()
                                .pb_2()
                                // `border-x`, `px-2` outside and inside.
                                .child(
                                    div()
                                        .w(px(panel_width - 36.0))
                                        .mb_1()
                                        .min_h_0()
                                        .child(composer),
                                )
                                .child(controls),
                        ),
                )
                .into_any_element();
        }
        div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .flex_shrink_0()
            .px_1()
            .pb_1()
            .children(history_indicator)
            .when(!queued.is_empty(), |column| {
                // `ChatQueue`: the waiting sends above the composer.
                column.child(
                    div()
                        .px_3()
                        .pb(px(6.0))
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .children(queued.into_iter().map(|(text, _)| {
                            div()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(SharedString::from(text))
                        })),
                )
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .max_h(px(160.0))
                    .min_h(px(38.0))
                    .rounded(px(19.0))
                    .border_1()
                    .border_color(alpha(theme.border, 0.7))
                    .bg(if theme.dark {
                        theme.card
                    } else {
                        gpui::rgb(0xffffff)
                    })
                    .pl_4()
                    .pr(px(6.0))
                    .map(|surface| {
                        if voice_active {
                            surface.py_2()
                        } else {
                            surface.items_center().py(px(3.0))
                        }
                    })
                    .tw_text_sm()
                    .child(
                        // A column, not a row: a `flex-1` text item is first
                        // measured at its zero flex basis, and that wrapped
                        // height sticks, so the editor takes the full width.
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .w_full()
                            .min_w_0()
                            .min_h(px(30.0))
                            .child(
                                // The panel border, `px-1`, the surface border,
                                // `pl-4` and `pr-[6px]` around the editor.
                                div()
                                    .w(px(panel_width - 34.0))
                                    .max_h(px(144.0))
                                    .pr(px(editor_padding))
                                    .child(composer),
                            )
                            .child(controls),
                    ),
            )
            .into_any_element()
    }

    /// `ChatPanelFrame layout="right-panel"` inside `[data-chat-right-panel]`
    /// (`border-x bg-card rounded-tr-xl`): the `h-9 pt-[9px]` toolbar (none
    /// for the automations scope), the body filling the column, and the
    /// elevated composer.
    pub(super) fn render_chat_right_panel(
        &mut self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let toolbar = (self.chat.scope == Scope::General).then(|| {
            // `ChatToolbarControls` at `size-7`, `pr-1 pl-3`.
            let ghost = |id: &'static str, glyph: &'static str| {
                div()
                    .id(id)
                    .flex()
                    .size(px(28.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
                    .child(icon(glyph, px(16.0), theme.muted_foreground))
            };
            let history = div()
                .id("chat-history")
                .flex()
                .h(px(28.0))
                .items_center()
                .gap(px(6.0))
                .ml(px(-8.0))
                .px(px(10.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .when(self.chat.history_open, |trigger| {
                    trigger.bg(alpha(theme.muted, 0.8))
                })
                .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.chat.history_open = !this.chat.history_open;
                    if this.chat.history_open {
                        this.load_chat_history(cx);
                    }
                    cx.notify();
                }))
                .child(icon(
                    "clock-counter-clockwise",
                    px(16.0),
                    theme.muted_foreground,
                ))
                .child(icon(
                    if self.chat.history_open {
                        "caret-up"
                    } else {
                        "caret-down"
                    },
                    px(14.0),
                    theme.muted_foreground,
                ));
            div()
                .flex()
                .h(px(36.0))
                .flex_shrink_0()
                .items_start()
                .pt(px(9.0))
                .pl_3()
                .pr_1()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .flex_1()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .relative()
                                .child(history)
                                .children(self.render_chat_history_menu(cx)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .child(ghost("chat-new", "plus").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.start_new_chat(cx)),
                        ))
                        .child(
                            ghost("chat-float", "picture-in-picture").on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| {
                                    this.open_chat(ChatMode::FloatingOpen, cx)
                                },
                            )),
                        )
                        .child(ghost("chat-close", "x").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.close_chat(cx)),
                        )),
                )
        });
        let body = self.render_chat_body(window, cx);
        let composer = self
            .chat_model_configured()
            .then(|| self.render_chat_composer(width, window, cx));
        div()
            .id("chat-right-panel")
            // `[data-chat-content]`'s drop: a dragged note becomes a manual
            // context ref for the next send.
            .on_drop(
                cx.listener(|this, drag: &super::session_drag::SessionDrag, _, cx| {
                    this.add_chat_context_session(&drag.session_id, cx);
                }),
            )
            .flex()
            .flex_col()
            .w(px(width))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .border_l_1()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.card)
            .rounded_tr(px(12.0))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(toolbar)
            .child(body)
            .children(composer)
            .into_any_element()
    }

    /// The `[data-chat-floating-frame]` over the main surface: `items-end
    /// justify-center px-3 pb-2` with the top clearance, closing on a press
    /// outside the panel.
    pub(super) fn render_chat_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.chat_mode != ChatMode::FloatingOpen || self.automations_open() {
            return None;
        }
        let theme = self.theme;
        let panel_bg = if theme.dark {
            gpui::rgb(0x202020)
        } else {
            gpui::rgb(0xf4f4f5)
        };
        let ghost = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .flex()
                .size(px(32.0))
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .cursor_pointer()
                .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
                .child(icon(glyph, px(16.0), theme.muted_foreground))
        };
        // `ChatGroups` trigger: `-ml-2 h-8 gap-1.5 rounded-full px-2.5`.
        let history = div()
            .id("chat-history")
            .flex()
            .h(px(32.0))
            .items_center()
            .gap(px(6.0))
            .ml(px(-8.0))
            .px(px(10.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .when(self.chat.history_open, |trigger| {
                trigger.bg(alpha(theme.muted, 0.8))
            })
            .hover(move |style| style.bg(alpha(theme.muted, 0.8)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.chat.history_open = !this.chat.history_open;
                if this.chat.history_open {
                    this.load_chat_history(cx);
                }
                cx.notify();
            }))
            .child(icon(
                "clock-counter-clockwise",
                px(16.0),
                theme.muted_foreground,
            ))
            .child(icon(
                if self.chat.history_open {
                    "caret-up"
                } else {
                    "caret-down"
                },
                px(14.0),
                theme.muted_foreground,
            ));
        let toolbar = div()
            .flex()
            .h(px(44.0))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .relative()
                            .child(history)
                            .children(self.render_chat_history_menu(cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .child(ghost("chat-new", "plus").on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.start_new_chat(cx)),
                    ))
                    .child(
                        ghost("chat-right-panel", "sidebar-left").on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.open_chat(ChatMode::RightPanelOpen, cx)
                            },
                        )),
                    ),
            );

        let body = self.render_chat_body(window, cx);
        // `w-full` between the frame's `px-3`, within the min / max widths.
        let panel_width =
            (self.main_surface_width(window) - 24.0).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH);
        let composer = self
            .chat_model_configured()
            .then(|| self.render_chat_composer(panel_width, window, cx));

        let panel = div()
            .id("chat-panel")
            .on_drop(
                cx.listener(|this, drag: &super::session_drag::SessionDrag, _, cx| {
                    this.add_chat_context_session(&drag.session_id, cx);
                }),
            )
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(PANEL_MIN_WIDTH))
            .max_w(px(PANEL_MAX_WIDTH))
            .rounded(px(24.0))
            .border_1()
            .border_color(alpha(theme.border, 0.7))
            .bg(panel_bg)
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.32),
                offset: gpui::point(px(0.0), px(32.0)),
                blur_radius: px(84.0),
                spread_radius: px(0.0),
            }])
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // `border-t-app-floating-border`: the lighter top edge.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(1.0))
                    .bg(theme.floating_border),
            )
            .child(toolbar)
            .child(body)
            .children(composer);

        let draft_empty = self
            .chat
            .composer
            .as_ref()
            .is_none_or(|composer| composer.read(cx).text().trim().is_empty());
        Some(
            div()
                .id("chat-frame")
                .absolute()
                .inset_0()
                .flex()
                .items_end()
                .justify_center()
                .px_3()
                .pb_2()
                .pt(px(TOP_CLEARANCE))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        // A press outside closes only while the draft is empty.
                        if draft_empty {
                            this.close_chat(cx);
                        }
                    }),
                )
                .child(panel)
                .into_any_element(),
        )
    }

    /// `ChatGroups`' dropdown, opened to the right of the trigger (`side="right"
    /// align="start" sideOffset={4}`): the `RECENT CHATS` header, the last five
    /// groups with their relative times, or `No recent chats`.
    fn render_chat_history_menu(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.chat.history_open {
            return None;
        }
        let theme = self.theme;
        let now = chrono::Utc::now();
        let mut panel = div()
            .relative()
            .flex()
            .flex_col()
            .p(px(6.0))
            .child(crate::squircle::squircle(
                crate::squircle::PANEL_RADIUS,
                Some(theme.floating_panel),
                Some((1.0, theme.floating_border)),
            ))
            .child(
                // `px-2 py-1.5` + `text-[10px] font-semibold tracking-wider uppercase`
                div()
                    .px_2()
                    .py(px(6.0))
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child("RECENT CHATS"),
            );
        if self.chat.history.is_empty() {
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .px_3()
                    .py_6()
                    .child(icon(
                        "chat-circle",
                        px(24.0),
                        alpha(theme.muted_foreground, 0.7),
                    ))
                    .child(
                        div()
                            .mt(px(6.0))
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child("No recent chats"),
                    ),
            );
        } else {
            panel = panel.child(div().flex().flex_col().gap(px(2.0)).children(
                self.chat.history.iter().enumerate().map(|(index, group)| {
                    let group_id = group.id.clone();
                    let active = self.chat.group_id.as_deref() == Some(group.id.as_str());
                    let relative = chrono::DateTime::parse_from_rfc3339(&group.created_at)
                        .map(|created| {
                            crate::automations::format_distance_to_now(
                                created.with_timezone(&chrono::Utc),
                                now,
                            )
                        })
                        .unwrap_or_default();
                    // `ChatGroupItem`: `px-2.5 py-1.5 rounded-[14px]`, active `bg-muted shadow-xs`.
                    div()
                        .id(("chat-group", index))
                        .flex()
                        .w_full()
                        .items_center()
                        .gap(px(10.0))
                        .rounded(px(14.0))
                        .px(px(10.0))
                        .py(px(6.0))
                        .cursor_pointer()
                        .when(active, |row| row.bg(theme.muted))
                        .hover(move |style| style.bg(theme.accent))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_chat(group_id.clone(), cx)
                        }))
                        .child(icon("chat-circle", px(14.0), theme.muted_foreground))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .truncate()
                                        .tw_text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(if active {
                                            theme.foreground
                                        } else {
                                            theme.muted_foreground
                                        })
                                        .child(SharedString::from(group.title.clone())),
                                )
                                .child(
                                    // `text-[11px]`: a 15.71px line box WebKit lays out at 15px.
                                    div()
                                        .mt(px(2.0))
                                        .text_size(px(11.0))
                                        .line_height(px(15.0))
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(relative)),
                                ),
                        )
                }),
            ));
        }
        let menu = super::menu::menu_chrome(theme, "chat-history-menu", 288.0)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                if this.chat.history_open {
                    this.chat.history_open = false;
                    cx.notify();
                }
            }))
            .child(panel);
        // The trigger is 56px wide (`px-2.5` around the two icons); the menu
        // opens 4px past its right edge, and Radix's `shift` slides it up to
        // stay 8px inside the window rather than flipping it above the trigger.
        Some(
            div()
                .absolute()
                .top_0()
                .left(px(56.0 - 8.0 + 4.0))
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopLeft)
                            .snap_to_window_with_margin(px(8.0))
                            .child(menu),
                    )
                    .with_priority(3),
                )
                .into_any_element(),
        )
    }
}

/// `isActivityPart`: reasoning with text, and every tool part except the
/// edit tools, which stay visible with their review controls.
fn is_activity_part(part: &Part) -> bool {
    match part {
        Part::Reasoning { text, .. } => !text.trim().is_empty(),
        Part::Other(_) => part.tool_view().is_some_and(|tool| {
            !matches!(
                tool.name,
                "edit_memo" | "edit_summary" | "update_prompt_template"
            )
        }),
        Part::Text { .. } | Part::StepStart => false,
    }
}

impl Workspace {
    /// `Part`: the reasoning disclosure, a text block, or a tool card.
    fn render_chat_part(
        &self,
        part: &Part,
        key: (usize, usize),
        renderer: &super::document_view::DocumentRenderer,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        match part {
            Part::Reasoning { text, state, .. } => {
                let raw = text.trim();
                if raw.is_empty() {
                    return div().into_any_element();
                }
                let cleaned = raw
                    .replace(['\n', '`', '*', '#', '"'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let streaming = state.as_deref() != Some("done");
                let title = if streaming {
                    cleaned
                        .chars()
                        .rev()
                        .take(150)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<String>()
                } else {
                    cleaned
                };
                // `Reasoning`: a `Disclosure` disabled while streaming, whose
                // body is the raw text at `text-muted-foreground text-sm
                // whitespace-pre-wrap`.
                let id = format!("reasoning:{}:{}", key.0, key.1);
                let open = self.chat.open_tools.contains(&id);
                let body = open.then(|| {
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .whitespace_normal()
                        .child(SharedString::from(text.clone()))
                        .into_any_element()
                });
                self.render_disclosure(key, "brain", title, streaming, open, id, body, cx)
            }
            Part::Text { text, .. } => div()
                .min_w_0()
                .px(px(2.0))
                .py_1()
                .children(renderer.chat_blocks(&crate::document::from_body("markdown", text)))
                .into_any_element(),
            Part::StepStart => div().into_any_element(),
            Part::Other(_) => match part.tool_view() {
                Some(tool) => self.render_tool_part(&tool, key, renderer, cx),
                None => div().into_any_element(),
            },
        }
    }
}

/// One step's reasoning and text parts; `state` is `streaming` until done.
/// One step's reasoning and text parts. `items` are the Responses items the
/// step produced: each `reasoning` item is a reasoning part of its own
/// (empty when no summary streamed, as the SDK's `reasoning-start` /
/// `reasoning-end` make it) and the `message` item labels the text part,
/// both under `provider` (`openai` / `azure`) like the SDK's metadata.
fn step_parts(
    reasoning: &str,
    text: &str,
    items: &[llm_stream::StoredItem],
    provider: &str,
    done: bool,
) -> Vec<Part> {
    let state = Some(if done { "done" } else { "streaming" }.to_string());
    let mut parts = Vec::new();
    let reasoning_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, llm_stream::StoredItem::Reasoning { .. }))
        .collect();
    if reasoning_items.is_empty() {
        if !reasoning.is_empty() {
            parts.push(Part::Reasoning {
                text: reasoning.to_string(),
                provider_metadata: None,
                state: state.clone(),
            });
        }
    } else {
        for (index, item) in reasoning_items.iter().enumerate() {
            let llm_stream::StoredItem::Reasoning {
                id,
                encrypted_content,
            } = item
            else {
                continue;
            };
            parts.push(Part::Reasoning {
                // The summary text, when any streamed, belongs to the first item.
                text: if index == 0 {
                    reasoning.to_string()
                } else {
                    String::new()
                },
                provider_metadata: Some(serde_json::json!({
                    provider: {
                        "itemId": id,
                        "reasoningEncryptedContent": encrypted_content
                    }
                })),
                state: state.clone(),
            });
        }
    }
    if !text.is_empty() {
        let message_item = items.iter().find_map(|item| match item {
            llm_stream::StoredItem::Message { id } => Some(id.clone()),
            _ => None,
        });
        parts.push(Part::Text {
            text: text.to_string(),
            provider_metadata: message_item
                .map(|id| serde_json::json!({ provider: { "itemId": id } })),
            state,
        });
    }
    parts
}

/// `renderContextBlock`: the `ContextBlock` template over the hydrated
/// sessions, trimmed; `None` without any.
fn render_context_block(contexts: Vec<anlg_template_app::SessionContext>) -> Option<String> {
    if contexts.is_empty() {
        return None;
    }
    anlg_template_app::render(anlg_template_app::Template::ContextBlock(
        anlg_template_app::ContextBlock { contexts },
    ))
    .ok()
    .map(|block| block.trim().to_string())
    .filter(|block| !block.is_empty())
}

/// A ref's hydration in flight: a session's `SessionContext`, or the text of
/// a contact / organization / folder ref.
enum ContextTask {
    Session(tokio::task::JoinHandle<anyhow::Result<Option<anlg_template_app::SessionContext>>>),
    Text(tokio::task::JoinHandle<anyhow::Result<Option<String>>>),
}

/// `renderContextBlock`: the sessions' `<context>` block first, then the
/// text contexts joined by blank lines.
fn render_context_blocks(
    contexts: Vec<anlg_template_app::SessionContext>,
    texts: Vec<String>,
) -> Option<String> {
    let mut blocks = Vec::new();
    if let Some(block) = render_context_block(contexts) {
        blocks.push(block);
    }
    if !texts.is_empty() {
        blocks.push(texts.join("\n\n"));
    }
    (!blocks.is_empty()).then(|| blocks.join("\n\n"))
}

/// `expandSearchMeetingsOutput`: a `search_meetings` output going back to
/// the model carries the found sessions' context under `contextText`.
async fn expand_search_outputs(store: &Arc<crate::db::Store>, parts: &[Part]) -> Vec<Part> {
    let mut expanded = Vec::with_capacity(parts.len());
    for part in parts {
        let Some(view) = part.tool_view() else {
            expanded.push(part.clone());
            continue;
        };
        if !matches!(view.name, "search_meetings" | "search_sessions")
            || view.state != "output-available"
        {
            expanded.push(part.clone());
            continue;
        }
        let Some(output) = view.output else {
            expanded.push(part.clone());
            continue;
        };
        let mut contexts = Vec::new();
        for session_id in chat::meeting_ids_from_search_output(output) {
            if let Ok(Ok(Some(context))) = store.chat_session_context(session_id).await {
                contexts.push(context);
            }
        }
        match render_context_block(contexts) {
            Some(block) => {
                let mut output = output.clone();
                if let Some(object) = output.as_object_mut() {
                    object.insert(crate::chat_tools::CONTEXT_TEXT_FIELD.into(), block.into());
                }
                let Part::Other(value) = part else {
                    continue;
                };
                expanded.push(Part::tool(
                    view.name,
                    view.call_id,
                    view.state,
                    view.input.cloned().unwrap_or_else(|| serde_json::json!({})),
                    Some(output),
                    None,
                    value.get("callProviderMetadata").cloned(),
                ));
            }
            None => expanded.push(part.clone()),
        }
    }
    expanded
}

/// `hasRenderableContent`: a text or reasoning part with content, or a tool
/// part.
fn has_renderable_content(message: &Message) -> bool {
    message.parts.iter().any(|part| match part {
        Part::Text { text, .. } | Part::Reasoning { text, .. } => !text.trim().is_empty(),
        Part::StepStart => false,
        Part::Other(_) => true,
    })
}
