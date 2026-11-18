//! The "Ask anything" chat: the floating button on the main surface, the floating and
//! right-panel chat, and its history. Shipping reference: `apps/desktop/src/chat/*`
//! and `apps/desktop/src/shared/chat-cta.tsx`. Sending reuses the meeting AI services.

pub mod assets;
pub mod composer;
pub mod markdown;
pub mod search;
pub mod state;
pub mod store;
mod tools_view;
mod view;
pub mod voice;

use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    AppContext, ClipboardItem, Context, Entity, FocusHandle, ScrollHandle, Subscription, Task,
    WeakEntity, Window,
};
use serde_json::Value;

use crate::{
    meeting::{
        ai::{ChatSession, Message, Part, Role, StreamObserver, ToolContext},
        ai_view::{AiViewServices, ContextPurpose},
        model::{MAX_TEXT, failure},
    },
    workspace::{WorkspaceView, navigation::Route},
};
use composer::{Composer, ComposerEvent, ContextRef};
use search::{MentionKind, SearchIndex};
use state::{ChatEvent, ChatMode, SentHistory};
use store::{ChatGroup, ChatMessage, Display, ToolPart};
pub use view::{overlay, reserved_width, right_panel};

struct Entry {
    message: ChatMessage,
    /// Each text part's Markdown, in order; edit cards render between them.
    blocks: Vec<Vec<markdown::Block>>,
    /// A reply stopped mid-stream stays on screen but was never saved, as in shipping.
    persisted: bool,
}

impl Entry {
    fn new(message: ChatMessage) -> Self {
        let blocks = message
            .parts
            .iter()
            .filter_map(|part| match part {
                Display::Text(text) => Some(markdown::parse(text)),
                _ => None,
            })
            .collect();
        Self {
            message,
            blocks,
            persisted: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Ready,
    Busy,
    Failed(String),
}

struct Queued {
    id: u64,
    text: String,
    refs: Vec<ContextRef>,
}

/// Unsent drafts per chat, like shipping's `DraftCache`: the 50 most recent are kept.
#[derive(Default)]
struct Drafts(VecDeque<(String, Value)>);

impl Drafts {
    const LIMIT: usize = 50;

    fn take(&mut self, key: &str) -> Option<Value> {
        let index = self.0.iter().position(|(draft, _)| draft == key)?;
        self.0.remove(index).map(|(_, doc)| doc)
    }

    fn put(&mut self, key: String, doc: Value) {
        self.take(&key);
        self.0.push_back((key, doc));
        while self.0.len() > Self::LIMIT {
            self.0.pop_front();
        }
    }
}

pub struct ChatView {
    runtime: RuntimeHandle,
    workspace: WeakEntity<WorkspaceView>,
    /// A standalone note window's chat: summary suggestions follow that note, and the main
    /// window's sidebar is left alone.
    note: Option<WeakEntity<crate::workspace::NoteView>>,
    mode: ChatMode,
    route: Route,
    context: Option<SessionId>,
    /// Notes dropped from the timeline, sent as context with the next message (shipping's
    /// `pendingManualRefs`).
    dropped: Vec<ContextRef>,
    group: Option<Arc<str>>,
    /// Shipping's chat `sessionId`: the group's id, or a fresh one for a new chat. Drafts and
    /// queued messages belong to it.
    draft_key: String,
    drafts: Drafts,
    entries: Vec<Entry>,
    /// The chat's saved messages are loading; sending waits for them.
    loading: bool,
    /// A reply left running in the background finished while this chat ran another; its
    /// history reloads once that one ends.
    stale: bool,
    groups: Vec<ChatGroup>,
    history_open: bool,
    model_configured: Option<bool>,
    /// The chosen model is Anarlog's hosted one, which needs a paid plan.
    hosted_model: bool,
    composer: Entity<Composer>,
    sent: SentHistory,
    phase: Phase,
    run: u64,
    /// Stops the current reply, including while its chat is saved and its context loads.
    cancel: CancellationToken,
    session: Option<Arc<ChatSession>>,
    queue: Vec<Queued>,
    next_queued: u64,
    pending: Arc<Mutex<(String, String)>>,
    pending_tools: Arc<Mutex<Vec<Part>>>,
    stream_text: String,
    stream_reasoning: String,
    stream_blocks: Vec<markdown::Block>,
    /// The streaming reply's tool calls, updated as they start and finish.
    stream_tools: Vec<ToolPart>,
    /// Edit tools send their review here so its tab opens, like shipping's `openEditTab`.
    reviews: futures::channel::mpsc::UnboundedSender<Arc<str>>,
    expanded: HashSet<String>,
    /// The first visible result of each search card's carousel.
    carousels: std::collections::HashMap<String, usize>,
    copied: Option<String>,
    pub(crate) markdown: markdown::Interaction,
    right_fraction: f32,
    restore_sidebar: bool,
    /// When the floating panel last opened, for its reveal animation, and how many times.
    revealed_at: Option<std::time::Instant>,
    reveals: u64,
    /// The floating panel's height when last painted; the reveal runs up to it.
    panel_height: std::rc::Rc<std::cell::Cell<f32>>,
    /// Focus moves to the composer in the frame that first draws it; a handle focused while
    /// undrawn loses focus to the workspace.
    focus_composer: bool,
    voice: voice::VoicePhase,
    /// Bumped when chat closes, so a late recording result is dropped like shipping's.
    voice_generation: u64,
    toast: Option<(voice::Tone, &'static str, u64)>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    load: CancellationToken,
    _poll: Option<Task<()>>,
    _reviews: Task<()>,
    _subscription: Subscription,
}

impl ChatView {
    pub fn new(
        runtime: RuntimeHandle,
        workspace: WeakEntity<WorkspaceView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let index = SearchIndex::default();
        let composer = cx.new(|cx| Composer::new(runtime.clone(), index, cx));
        let subscription = cx.subscribe(&composer, |this, _, event, cx| match event {
            ComposerEvent::Submit => this.submit_draft(cx),
            ComposerEvent::Changed => {
                this.sent.edited();
                cx.notify();
            }
            ComposerEvent::Recall(previous) => this.recall(*previous, cx),
        });
        let (reviews, mut opened) = futures::channel::mpsc::unbounded::<Arc<str>>();
        let review_workspace = workspace.clone();
        let review_task = cx.spawn(async move |_, cx| {
            use futures::StreamExt;
            while let Some(id) = opened.next().await {
                let _ = cx.update(|cx| {
                    crate::workspace::edit_review::bump(cx);
                    review_workspace.update(cx, |workspace, cx| {
                        workspace.open_route(Route::Edit(id), true, cx)
                    })
                });
            }
        });
        Self {
            runtime,
            workspace,
            note: None,
            mode: ChatMode::FloatingClosed,
            route: Route::Empty,
            context: None,
            dropped: Vec::new(),
            group: None,
            draft_key: uuid::Uuid::new_v4().to_string(),
            drafts: Drafts::default(),
            entries: Vec::new(),
            loading: false,
            stale: false,
            groups: Vec::new(),
            history_open: false,
            model_configured: None,
            hosted_model: false,
            composer,
            sent: SentHistory::default(),
            phase: Phase::Ready,
            run: 0,
            cancel: CancellationToken::new(),
            session: None,
            queue: Vec::new(),
            next_queued: 0,
            pending: Arc::default(),
            pending_tools: Arc::default(),
            stream_text: String::new(),
            stream_reasoning: String::new(),
            stream_blocks: Vec::new(),
            stream_tools: Vec::new(),
            reviews,
            expanded: HashSet::new(),
            carousels: Default::default(),
            copied: None,
            markdown: markdown::Interaction::default(),
            right_fraction: 0.3,
            restore_sidebar: false,
            revealed_at: None,
            reveals: 0,
            panel_height: Default::default(),
            focus_composer: false,
            voice: voice::VoicePhase::Idle,
            voice_generation: 0,
            toast: None,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            load: CancellationToken::new(),
            _poll: None,
            _reviews: review_task,
            _subscription: subscription,
        }
    }

    pub fn set_note(&mut self, note: WeakEntity<crate::workspace::NoteView>) {
        self.note = Some(note);
    }

    pub fn mode(&self) -> ChatMode {
        self.mode
    }

    pub fn is_open(&self) -> bool {
        self.mode.is_open()
    }

    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    pub fn event(&mut self, event: ChatEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mode = self.mode.next(event);
        if mode == self.mode {
            return;
        }
        let opened = !self.mode.is_open() && mode.is_open();
        self.set_mode(mode, cx);
        if mode.is_open() {
            if opened {
                self.refresh_model(cx);
            }
            self.focus_composer = true;
            window.refresh();
        }
        cx.notify();
    }

    /// The right panel squeezes the note surface, which collapses the left sidebar; it comes
    /// back when the panel closes, like shipping's auto-collapse.
    fn set_mode(&mut self, mode: ChatMode, cx: &mut Context<Self>) {
        let right = ChatMode::RightPanelOpen;
        let (was_right, is_right) = (self.mode == right, mode == right);
        if mode != self.mode {
            self.history_open = false;
            self.markdown.close();
            // The thread follows its newest message across floating and right-panel layouts.
            self.scroll.scroll_to_bottom();
            if mode == ChatMode::FloatingOpen {
                self.revealed_at = Some(std::time::Instant::now());
                self.reveals += 1;
            }
        }
        self.mode = mode;
        if !mode.is_open() {
            self.cancel_voice(cx);
        }
        if was_right == is_right || self.note.is_some() {
            return;
        }
        // Workspace may be mid-update when it forwards navigation or shortcuts here.
        let (workspace, chat) = (self.workspace.clone(), cx.weak_entity());
        cx.defer(move |cx| {
            let Ok(expanded) = workspace.read_with(cx, |workspace, _| workspace.sidebar_expanded())
            else {
                return;
            };
            let restore = chat
                .update(cx, |chat, _| {
                    if is_right {
                        chat.restore_sidebar = expanded;
                        false
                    } else {
                        std::mem::take(&mut chat.restore_sidebar) && !expanded
                    }
                })
                .unwrap_or(false);
            if restore {
                let _ = workspace.update(cx, |workspace, cx| workspace.toggle_sidebar(cx));
            }
        });
    }

    /// Called after each workspace navigation.
    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        let mode = self.mode.after_navigation(&self.route, &route);
        self.context = state::context_session(self.context.take(), &route);
        self.route = route;
        self.set_mode(mode, cx);
        cx.notify();
    }

    /// Keeps the current draft for this chat and shows the draft saved for `key`.
    fn switch_draft(&mut self, key: String, cx: &mut Context<Self>) {
        let composer = self.composer.read(cx);
        let draft = composer.doc();
        if composer.has_text() || !composer.context_refs().is_empty() {
            self.drafts
                .put(std::mem::replace(&mut self.draft_key, key), draft);
        } else {
            self.draft_key = key;
        }
        let restored = self.drafts.take(&self.draft_key);
        self.composer.update(cx, |composer, cx| match restored {
            Some(doc) => composer.set_doc(&doc, false, cx),
            None => composer.clear(cx),
        });
    }

    /// Shipping's `onAddContextEntity`: a note dropped on the chat joins the next message.
    pub fn add_dropped_session(&mut self, session: SessionId, cx: &mut Context<Self>) {
        let reference = ContextRef {
            kind: MentionKind::Session,
            id: session.0.to_string(),
        };
        if !self.dropped.contains(&reference) {
            self.dropped.push(reference);
            cx.notify();
        }
    }

    /// The next message's context: dropped notes, then the draft's mentions.
    fn pending_refs(&mut self, cx: &mut Context<Self>) -> Vec<ContextRef> {
        let mut refs = std::mem::take(&mut self.dropped);
        for reference in self.composer.read(cx).context_refs() {
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
        refs
    }

    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.dropped.clear();
        self.detach_run();
        self.load.cancel();
        self.loading = false;
        self.group = None;
        self.entries.clear();
        self.history_open = false;
        self.switch_draft(uuid::Uuid::new_v4().to_string(), cx);
        cx.notify();
    }

    pub fn select_group(&mut self, group: Arc<str>, cx: &mut Context<Self>) {
        self.dropped.clear();
        self.detach_run();
        self.history_open = false;
        self.group = Some(group.clone());
        self.switch_draft(group.to_string(), cx);
        self.entries.clear();
        self.load_history(group, cx);
        cx.notify();
    }

    /// Shows the chat's saved messages. A newer load or another chat drops this one's result.
    fn load_history(&mut self, group: Arc<str>, cx: &mut Context<Self>) {
        self.load.cancel();
        self.load = CancellationToken::new();
        self.loading = true;
        let load = self.load.clone();
        let reply = store::messages(&self.runtime, group, load.clone());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                if load.is_cancelled() {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(messages) => {
                        this.entries = messages.into_iter().map(Entry::new).collect();
                        this.scroll.scroll_to_bottom();
                    }
                    Err(ServiceError::Cancelled) => {}
                    Err(error) => this.phase = Phase::Failed(error.to_string()),
                }
                this.send_queued(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens or closes a reply's Activity disclosure.
    pub fn toggle_activity(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.to_owned());
        }
        cx.notify();
    }

    pub fn toggle_history(&mut self, cx: &mut Context<Self>) {
        self.history_open = !self.history_open;
        if self.history_open {
            self.refresh_groups(cx);
        }
        cx.notify();
    }

    fn refresh_groups(&mut self, cx: &mut Context<Self>) {
        let reply = store::recent_groups(&self.runtime, 5);
        cx.spawn(async move |this, cx| {
            if let Ok(groups) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.groups = groups;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Shipping's `useLanguageModel` yields no model for Anarlog's hosted one without a paid plan.
    fn model_ready(&self, cx: &gpui::App) -> Option<bool> {
        self.model_configured.map(|configured| {
            configured && (!self.hosted_model || crate::product::account::is_paid(cx))
        })
    }

    fn refresh_model(&mut self, cx: &mut Context<Self>) {
        let secret = cx
            .try_global::<crate::meeting::config::ProviderServices>()
            .map(|providers| providers.secret.clone());
        let reply = store::model_status(&self.runtime, secret);
        cx.spawn(async move |this, cx| {
            let (configured, hosted) = async { reply?.receive().await }
                .await
                .unwrap_or((false, false));
            let _ = this.update(cx, |this, cx| {
                this.model_configured = Some(configured);
                this.hosted_model = hosted;
                cx.notify();
            });
        })
        .detach();
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.open_route(Route::settings("intelligence"), cx);
    }

    /// Workspace navigation updates this chat, so it waits until the chat's own update ends.
    fn open_route(&self, route: Route, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        cx.defer(move |cx| {
            let _ = workspace.update(cx, |workspace, cx| workspace.open_route(route, true, cx));
        });
    }

    fn busy(&self) -> bool {
        self.phase == Phase::Busy
    }

    /// The current note is recording, which swaps in shipping's live-meeting suggestions.
    fn live_meeting(&self, cx: &gpui::App) -> bool {
        self.context.as_ref().is_some_and(|session| {
            crate::meeting::live::LiveState::mode(cx, session)
                == crate::meeting::live::SessionMode::Active
        })
    }

    /// Shipping offers summary rewrites only while the shown note view is a summary.
    fn summary_shown(&self, cx: &gpui::App) -> bool {
        self.context.is_some()
            && matches!(self.route, Route::Session(_))
            && match &self.note {
                Some(note) => note.read_with(cx, |note, _| note.editable_summary_shown()),
                None => self
                    .workspace
                    .read_with(cx, |workspace, cx| workspace.editable_summary_shown(cx)),
            }
            .unwrap_or(false)
    }

    fn submit_draft(&mut self, cx: &mut Context<Self>) {
        let composer = self.composer.read(cx);
        let text = composer.text().trim().to_owned();
        if text.is_empty() || self.model_ready(cx) != Some(true) {
            return;
        }
        let doc = composer.doc();
        let refs = self.pending_refs(cx);
        self.sent.push(doc);
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        self.submit(text, refs, cx);
    }

    fn submit(&mut self, text: String, refs: Vec<ContextRef>, cx: &mut Context<Self>) {
        // Sending waits for the reply in progress and for the chat's history.
        if self.busy() || self.loading {
            self.next_queued += 1;
            self.queue.push(Queued {
                id: self.next_queued,
                text,
                refs,
            });
            cx.notify();
            return;
        }
        self.send(text, refs, cx);
    }

    fn send(&mut self, text: String, refs: Vec<ContextRef>, cx: &mut Context<Self>) {
        if text.len() > MAX_TEXT {
            self.phase = Phase::Failed("Message exceeds the size limit.".into());
            cx.notify();
            return;
        }
        let user = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            user: true,
            parts: vec![Display::Text(text)],
            created_at: Some(Utc::now()),
            refs,
        };
        self.entries.push(Entry::new(user.clone()));
        self.start(user, None, cx);
    }

    /// Re-sends the last user message, replacing the last reply like shipping's regenerate.
    fn regenerate(&mut self, cx: &mut Context<Self>) {
        if self.busy() || self.loading {
            return;
        }
        let Some(user) = self.entries.iter().rev().find(|entry| entry.message.user) else {
            return;
        };
        let user = user.message.clone();
        let previous = self
            .entries
            .last()
            .filter(|entry| !entry.message.user)
            .map(|entry| (entry.message.id.clone(), entry.persisted));
        if previous.is_some() {
            self.entries.pop();
        }
        let previous = previous.and_then(|(id, persisted)| persisted.then_some(id));
        self.start(user, previous, cx);
    }

    /// Mentions from every message in this chat, oldest first, like shipping's committed refs.
    fn chat_refs(&self) -> Vec<ContextRef> {
        let mut refs: Vec<ContextRef> = Vec::new();
        for reference in self
            .entries
            .iter()
            .filter(|entry| entry.message.user)
            .flat_map(|entry| &entry.message.refs)
        {
            if !refs.contains(reference) {
                refs.push(reference.clone());
            }
        }
        refs
    }

    fn start(&mut self, user: ChatMessage, previous: Option<String>, cx: &mut Context<Self>) {
        let Some(services) = cx.try_global::<AiViewServices>().cloned() else {
            self.phase = Phase::Failed("Intelligence is still starting. Try again shortly.".into());
            cx.notify();
            return;
        };
        let (group, created) = match self.group.clone() {
            Some(group) => (group, false),
            // The draft's key names the new chat, so its unsent follow-ups stay with it.
            None => (Arc::from(self.draft_key.as_str()), true),
        };
        let session = match services.ai.chat(SessionId(group.clone()), group.clone()) {
            Ok(session) => session,
            Err(error) => {
                self.phase = Phase::Failed(error.to_string());
                cx.notify();
                return;
            }
        };
        self.group = Some(group.clone());
        self.session = Some(session.clone());
        self.run += 1;
        self.phase = Phase::Busy;
        self.clear_stream();
        // Each reply streams into its own buffers; one left running in the background keeps
        // writing only to its own.
        self.pending = Arc::default();
        self.pending_tools = Arc::default();
        self.cancel = CancellationToken::new();
        let cancel = self.cancel.clone();
        let run = self.run;
        let request = user.to_request();
        let history = self
            .entries
            .iter()
            .filter(|entry| entry.message.id != user.id)
            .map(|entry| entry.message.to_request())
            .collect::<Vec<_>>();
        let title = state::fallback_title(&message_text(&request));
        let runtime = self.runtime.clone();
        let context = self.context.clone();
        let refs = self.chat_refs();
        let observer = self.observer();
        let tool_context = self.tool_context(&user, cx);
        self.scroll.scroll_to_bottom();
        self.poll(run, cx);
        cx.spawn(async move |this, cx| {
            let mut saved = !created;
            let result = async {
                if created {
                    store::create_group(&runtime, group.clone(), title)?
                        .receive()
                        .await?;
                    saved = true;
                }
                let prompt = services.ai.enqueue(system_prompt(
                    services.clone(),
                    runtime.clone(),
                    context,
                    refs,
                ))?;
                let system = cancel
                    .run_until_cancelled(prompt)
                    .await
                    .ok_or(ServiceError::Cancelled)?
                    .map_err(failure)??;
                let mut messages = vec![system];
                messages.extend(history);
                // Stop before the request exists never reaches the provider.
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                let sent =
                    session.queue_send_with(request, messages, previous, observer, tool_context)?;
                // The user row exists once the session starts streaming; its metadata keeps
                // the mentions for later turns and reloads.
                let reply = sent.await.map_err(failure)?;
                if let Ok(metadata) = store::set_metadata(&runtime, user.id.clone(), &user.refs) {
                    let _ = metadata.receive().await;
                }
                reply
            }
            .await;
            let _ = this.update(cx, |this, cx| this.finish(run, group, saved, result, cx));
        })
        .detach();
        cx.notify();
    }

    /// What shipping's tools read at call time: the attached note, its shown summary, the
    /// message's folder mentions and Pro access.
    fn tool_context(&self, user: &ChatMessage, cx: &gpui::App) -> ToolContext {
        let summary = self
            .context
            .as_ref()
            .filter(|session| self.route == Route::Session((*session).clone()))
            .and_then(|_| match &self.note {
                Some(note) => note.read_with(cx, |note, _| note.shown_summary()).ok(),
                None => self
                    .workspace
                    .read_with(cx, |workspace, cx| workspace.shown_summary(cx))
                    .ok(),
            })
            .flatten();
        let reviews = self.reviews.clone();
        ToolContext {
            call_id: String::new(),
            session: self.context.as_ref().map(|session| session.0.to_string()),
            summary: summary.map(|summary| summary.to_string()),
            folders: user
                .refs
                .iter()
                .filter(|reference| reference.kind == MentionKind::Folder)
                .map(|reference| reference.id.clone())
                .collect(),
            pro: crate::product::account::is_pro(cx),
            review: Some(Arc::new(move |id| {
                let _ = reviews.unbounded_send(id);
            })),
        }
    }

    /// `saved` says the reply's chat exists, so a retry reuses it.
    fn finish(
        &mut self,
        run: u64,
        group: Arc<str>,
        saved: bool,
        result: Result<Message>,
        cx: &mut Context<Self>,
    ) {
        if run != self.run {
            // A reply left running in the background was saved to the chat shown again since.
            if result.is_ok() && self.group.as_ref() == Some(&group) {
                if self.busy() {
                    self.stale = true;
                } else {
                    self.load_history(group, cx);
                }
            }
            return;
        }
        if !saved {
            // The chat was never written, like shipping's failed-group rollback.
            self.group = None;
        }
        self._poll = None;
        self.session = None;
        self.drain_pending();
        self.drain_tools(cx);
        match result {
            Ok(reply) => {
                self.phase = Phase::Ready;
                self.entries
                    .push(Entry::new(ChatMessage::from_reply(&reply)));
                self.scroll.scroll_to_bottom();
            }
            Err(ServiceError::Cancelled) => {
                self.phase = Phase::Ready;
                // Shipping keeps the stopped reply on screen without saving it.
                let mut parts = Vec::new();
                if !self.stream_reasoning.trim().is_empty() {
                    parts.push(Display::Reasoning(self.stream_reasoning.clone()));
                }
                parts.extend(
                    self.stream_tools
                        .iter()
                        .filter(|tool| !tool.running())
                        .cloned()
                        .map(Display::Tool),
                );
                if !self.stream_text.trim().is_empty() {
                    parts.push(Display::Text(self.stream_text.clone()));
                }
                if !parts.is_empty() {
                    let mut entry = Entry::new(ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        user: false,
                        parts,
                        created_at: None,
                        refs: Vec::new(),
                    });
                    entry.persisted = false;
                    self.entries.push(entry);
                }
            }
            Err(error) => self.phase = Phase::Failed(error.to_string()),
        }
        self.clear_stream();
        if std::mem::take(&mut self.stale) {
            self.load_history(group, cx);
        }
        self.send_queued(cx);
        cx.notify();
    }

    fn send_queued(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Ready && !self.loading && !self.queue.is_empty() {
            let next = self.queue.remove(0);
            self.send(next.text, next.refs, cx);
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        if let Some(session) = &self.session {
            let _ = session.cancel();
        }
        cx.notify();
    }

    /// Leaves an in-flight reply running in the background; it is still saved to its chat.
    fn detach_run(&mut self) {
        self.run += 1;
        self.stale = false;
        self._poll = None;
        self.session = None;
        self.queue.clear();
        self.phase = Phase::Ready;
        self.markdown.close();
        self.clear_stream();
    }

    fn clear_stream(&mut self) {
        *self
            .pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Default::default();
        self.pending_tools
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
        self.stream_text.clear();
        self.stream_reasoning.clear();
        self.stream_blocks.clear();
        self.stream_tools.clear();
    }

    /// Moves streamed text not yet picked up by the poll into the visible reply.
    fn drain_pending(&mut self) {
        let (text, reasoning) = std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        );
        self.stream_text.push_str(&text);
        self.stream_reasoning.push_str(&reasoning);
    }

    fn observer(&self) -> StreamObserver {
        let (pending, tools) = (self.pending.clone(), self.pending_tools.clone());
        Arc::new(move |update| {
            let mut pending = pending.lock().unwrap_or_else(|poison| poison.into_inner());
            pending.0.push_str(&update.text);
            pending.1.push_str(&update.reasoning);
            if !update.tools.is_empty() {
                tools
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .extend(update.tools);
            }
        })
    }

    /// Moves tool updates from the stream into the visible reply, newest state per call.
    fn drain_tools(&mut self, cx: &mut gpui::App) -> bool {
        let updates = std::mem::take(
            &mut *self
                .pending_tools
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        );
        let changed = !updates.is_empty();
        for part in updates.iter().filter_map(ToolPart::from_part) {
            // Shipping's `withProAccess` also asks the user to upgrade.
            if part.failed() && part.error.as_deref() == Some(crate::meeting::tools::PRO_REQUIRED) {
                crate::product::account::notify_plan_required(cx);
            }
            match self.stream_tools.iter_mut().find(|tool| tool.id == part.id) {
                Some(tool) => *tool = part,
                None => self.stream_tools.push(part),
            }
        }
        changed
    }

    fn poll(&mut self, run: u64, cx: &mut Context<Self>) {
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let updated = this.update(cx, |this, cx| {
                    if this.run != run {
                        return false;
                    }
                    let (text, reasoning) = std::mem::take(
                        &mut *this
                            .pending
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner()),
                    );
                    let tools = this.drain_tools(cx);
                    if !text.is_empty() || !reasoning.is_empty() || tools {
                        let follow = view::at_bottom(&this.scroll);
                        this.stream_text.push_str(&text);
                        this.stream_reasoning.push_str(&reasoning);
                        if !text.is_empty() {
                            this.stream_blocks = markdown::parse(&this.stream_text);
                        }
                        if follow {
                            this.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    }
                    true
                });
                if !matches!(updated, Ok(true)) {
                    break;
                }
            }
        }));
    }

    fn remove_queued(&mut self, id: u64, cx: &mut Context<Self>) {
        self.queue.retain(|queued| queued.id != id);
        cx.notify();
    }

    fn copy(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = Some(id.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                if this.copied.as_deref() == Some(id.as_str()) {
                    this.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Shows a reply state that needs a live model: streaming (or "Thinking..." with no text),
    /// failed, or stopped mid-stream. The screenshot harness has no model to produce them.
    #[doc(hidden)]
    pub fn preview_reply(
        &mut self,
        reasoning: &str,
        text: &str,
        outcome: Option<std::result::Result<(), String>>,
        cx: &mut Context<Self>,
    ) {
        self.run += 1;
        self.phase = Phase::Busy;
        self.stream_reasoning = reasoning.into();
        self.stream_text = text.into();
        self.stream_blocks = markdown::parse(text);
        self.scroll.scroll_to_bottom();
        let group = self.group.clone().unwrap_or_else(|| "".into());
        match outcome {
            Some(Ok(())) => self.finish(self.run, group, true, Err(ServiceError::Cancelled), cx),
            Some(Err(error)) => self.finish(self.run, group, true, Err(failure(error)), cx),
            None => {}
        }
        cx.notify();
    }

    /// Shipping's `toast.warning` / `toast.error`, shown for its usual duration.
    fn show_toast(&mut self, tone: voice::Tone, message: &'static str, cx: &mut Context<Self>) {
        let id = self.toast.map_or(0, |(_, _, id)| id) + 1;
        self.toast = Some((tone, message, id));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(tone.duration()).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.is_some_and(|(_, _, current)| current == id) {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn start_voice(&mut self, cx: &mut Context<Self>) {
        if self.voice != voice::VoicePhase::Idle {
            return;
        }
        let recording = crate::meeting::live::LiveState::global(cx)
            .and_then(|state| state.read(cx).current().map(|live| live.live()))
            .unwrap_or(false);
        if recording {
            self.show_toast(
                voice::Tone::Warning,
                "Voice input is unavailable while Anarlog is recording a meeting.",
                cx,
            );
            return;
        }
        let Some(backend) = cx.try_global::<voice::Voice>().cloned() else {
            self.show_toast(voice::Tone::Error, "Could not start voice input", cx);
            return;
        };
        self.voice = voice::VoicePhase::Starting;
        let generation = self.voice_generation;
        let started = backend.0.start();
        cx.spawn(async move |this, cx| {
            let result = started.await;
            let _ = this.update(cx, |this, cx| {
                if this.voice_generation != generation {
                    backend.0.cancel();
                    return;
                }
                match result {
                    Ok(()) => {
                        this.voice = voice::VoicePhase::Recording(std::time::Instant::now());
                        this.tick_voice(generation, cx);
                    }
                    Err(_) => {
                        this.voice = voice::VoicePhase::Idle;
                        this.show_toast(voice::Tone::Error, "Could not start voice input", cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Redraws the elapsed time, stopping at shipping's five-minute limit.
    fn tick_voice(&mut self, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let running = this.update(cx, |this, cx| {
                    let voice::VoicePhase::Recording(started) = this.voice else {
                        return false;
                    };
                    if this.voice_generation != generation {
                        return false;
                    }
                    if started.elapsed() >= voice::MAX_RECORDING {
                        this.stop_voice(cx);
                        return false;
                    }
                    cx.notify();
                    true
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn stop_voice(&mut self, cx: &mut Context<Self>) {
        let voice::VoicePhase::Recording(_) = self.voice else {
            return;
        };
        let Some(backend) = cx.try_global::<voice::Voice>().cloned() else {
            self.voice = voice::VoicePhase::Idle;
            return;
        };
        self.voice = voice::VoicePhase::Transcribing;
        let generation = self.voice_generation;
        let stopped = backend.0.stop();
        cx.spawn(async move |this, cx| {
            let result = stopped.await;
            let _ = this.update(cx, |this, cx| {
                this.voice = voice::VoicePhase::Idle;
                if this.voice_generation != generation {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(text) if !text.trim().is_empty() => {
                        this.composer
                            .update(cx, |composer, cx| composer.insert_text(&text, cx));
                    }
                    Ok(_) => this.show_toast(voice::Tone::Warning, "No speech detected", cx),
                    Err(_) => {
                        this.show_toast(voice::Tone::Error, "Could not transcribe voice input", cx)
                    }
                }
                this.focus_composer = true;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn cancel_voice(&mut self, cx: &mut Context<Self>) {
        self.voice_generation += 1;
        if matches!(
            self.voice,
            voice::VoicePhase::Starting | voice::VoicePhase::Recording(_)
        ) && let Some(backend) = cx.try_global::<voice::Voice>()
        {
            backend.0.cancel();
        }
        if self.voice != voice::VoicePhase::Transcribing {
            self.voice = voice::VoicePhase::Idle;
        }
    }

    /// A copy control shows its check for two seconds, like Streamdown's.
    pub(crate) fn mark_copied(&mut self, id: String, cx: &mut Context<Self>) {
        self.markdown.menu = None;
        self.markdown.copied = Some(id.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                if this.markdown.copied.as_deref() == Some(id.as_str()) {
                    this.markdown.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Shipping's `useMessageHistory`: recalled prompts replace the draft with the caret at
    /// the start going back and at the end going forward.
    fn recall(&mut self, previous: bool, cx: &mut Context<Self>) {
        let current = self.composer.read(cx).doc();
        let Some(doc) = self.sent.navigate(previous, &current) else {
            return;
        };
        if doc != current {
            self.composer
                .update(cx, |composer, cx| composer.set_doc(&doc, previous, cx));
        }
        cx.notify();
    }

    fn draft_empty(&self, cx: &gpui::App) -> bool {
        !self.composer.read(cx).has_text()
    }
}

impl Drop for ChatView {
    fn drop(&mut self) {
        self.load.cancel();
    }
}

fn message_text(message: &Message) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The chat system prompt with shipping's context: the mentioned notes, then the current
/// note, then the mentioned people, organizations and folders.
fn system_prompt(
    services: AiViewServices,
    runtime: RuntimeHandle,
    current: Option<SessionId>,
    refs: Vec<ContextRef>,
) -> futures::future::BoxFuture<'static, Result<Message>> {
    Box::pin(async move {
        let settings = crate::meeting::config::preferences(&services.ai.runtime).await?;
        let language = Some(settings.text("ai_language")).filter(|language| !language.is_empty());
        let system = anlg_template_app::render(anlg_template_app::Template::ChatSystem(
            anlg_template_app::ChatSystem { language },
        ))
        .map_err(failure)?;
        let mut sessions = refs
            .iter()
            .filter(|reference| reference.kind == MentionKind::Session)
            .map(|reference| SessionId(reference.id.as_str().into()))
            .filter(|session| Some(session) != current.as_ref())
            .collect::<Vec<_>>();
        sessions.extend(current);
        let mut blocks = vec![system.clone()];
        for session in sessions {
            let Ok(context) = (services.context)(session, ContextPurpose::Preview).await else {
                continue;
            };
            if let Some(Part::Text { text }) = context
                .history
                .into_iter()
                .find(|message| message.role == Role::System)
                .and_then(|message| message.parts.into_iter().next())
            {
                // Each note's context opens with the same system prompt; keep only its block.
                let block = text.strip_prefix(&system).unwrap_or(&text).trim();
                if !block.is_empty() {
                    blocks.push(block.to_owned());
                }
            }
        }
        for reference in refs
            .into_iter()
            .filter(|reference| reference.kind != MentionKind::Session)
        {
            if let Ok(Some(text)) = store::mention_context(&runtime, reference)?.receive().await {
                blocks.push(text);
            }
        }
        let text = blocks.join("\n\n");
        if text.len() > MAX_TEXT {
            return Err(failure("Meeting context exceeds the provider input limit."));
        }
        Ok(Message {
            id: "system".into(),
            role: Role::System,
            parts: vec![Part::Text { text }],
        })
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use desktop_runtime::Profile;
    use futures::{StreamExt, future::BoxFuture, stream::BoxStream};
    use gpui::{AppContext, TestAppContext, WeakEntity};

    use super::*;
    use crate::meeting::ai::{AiServices, ProviderAdapter, ProviderEvent, Request};

    fn start_runtime() -> (tempfile::TempDir, RuntimeHandle) {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        (directory, runtime)
    }

    /// Answers "re: <message>". "fail" fails once; a message starting with "slow" streams its
    /// answer, then " more" once released.
    #[derive(Clone, Default)]
    struct Model(Arc<ModelState>);

    #[derive(Default)]
    struct ModelState {
        failed: AtomicBool,
        streaming: Mutex<Vec<String>>,
        released: Mutex<Vec<String>>,
    }

    impl Model {
        fn streaming(&self, text: &str) -> bool {
            self.0
                .streaming
                .lock()
                .unwrap()
                .iter()
                .any(|item| item == text)
        }

        fn release(&self, text: &str) {
            self.0.released.lock().unwrap().push(text.into());
        }
    }

    impl ProviderAdapter for Model {
        fn stream(
            &self,
            request: Request,
        ) -> BoxFuture<'static, Result<BoxStream<'static, Result<ProviderEvent>>>> {
            let state = self.0.clone();
            let text = request
                .messages
                .iter()
                .rev()
                .find(|message| message.role == Role::User)
                .map(message_text)
                .unwrap_or_default();
            Box::pin(async move {
                if text == "fail" && !state.failed.swap(true, Ordering::SeqCst) {
                    return Err(failure("The model is unavailable."));
                }
                let first = futures::stream::iter([Ok(ProviderEvent::Text(format!("re: {text}")))]);
                if !text.starts_with("slow") {
                    return Ok(first.boxed());
                }
                state.streaming.lock().unwrap().push(text.clone());
                let rest = futures::stream::once(async move {
                    while !state.released.lock().unwrap().contains(&text) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    Ok(ProviderEvent::Text(" more".into()))
                });
                Ok(first.chain(rest).boxed())
            })
        }
    }

    fn chat_with_model(
        cx: &mut TestAppContext,
    ) -> (tempfile::TempDir, RuntimeHandle, Model, Entity<ChatView>) {
        cx.executor().allow_parking();
        let (directory, runtime) = start_runtime();
        let model = Model::default();
        let provider = model.clone();
        let approvals = crate::meeting::tools::Approvals::default();
        let services = AiViewServices {
            ai: Arc::new(
                AiServices::new(
                    runtime.clone(),
                    Arc::new(move |_| {
                        let provider = provider.clone();
                        Box::pin(async move { Ok(Arc::new(provider) as Arc<dyn ProviderAdapter>) })
                    }),
                    crate::meeting::tools::executor(runtime.clone(), approvals.clone(), None),
                    Arc::new(|_| false),
                )
                .unwrap(),
            ),
            context: crate::meeting::context::resolver(runtime.clone()),
            approvals,
        };
        cx.update(|cx| cx.set_global(services));
        let chat = cx.new(|cx| ChatView::new(runtime.clone(), WeakEntity::new_invalid(), cx));
        (directory, runtime, model, chat)
    }

    fn wait(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
        for _ in 0..1000 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn idle(cx: &mut TestAppContext, chat: &Entity<ChatView>) {
        wait(cx, |cx| {
            chat.read_with(cx, |chat, _| !chat.busy() && !chat.loading)
        });
    }

    fn texts(cx: &mut TestAppContext, chat: &Entity<ChatView>) -> Vec<String> {
        chat.read_with(cx, |chat, _| {
            chat.entries
                .iter()
                .map(|entry| entry.message.text())
                .collect()
        })
    }

    fn send(cx: &mut TestAppContext, chat: &Entity<ChatView>, text: &str) {
        chat.update(cx, |chat, cx| chat.submit(text.into(), Vec::new(), cx));
    }

    #[gpui::test]
    fn a_background_reply_never_streams_into_the_next_chat(cx: &mut TestAppContext) {
        let (_directory, runtime, model, chat) = chat_with_model(cx);
        send(cx, &chat, "slow a");
        wait(cx, |_| model.streaming("slow a"));
        chat.update(cx, |chat, cx| chat.new_chat(cx));
        send(cx, &chat, "slow b");
        model.release("slow a");
        wait(cx, |_| model.streaming("slow b"));
        chat.update(cx, |chat, cx| chat.stop(cx));
        idle(cx, &chat);
        let shown = texts(cx, &chat);
        assert!(!shown.iter().any(|text| text.contains("more")), "{shown:?}");
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn retrying_a_failed_first_reply_reuses_its_saved_chat(cx: &mut TestAppContext) {
        let (_directory, runtime, _, chat) = chat_with_model(cx);
        send(cx, &chat, "fail");
        idle(cx, &chat);
        chat.update(cx, |chat, cx| {
            assert!(matches!(chat.phase, Phase::Failed(_)));
            chat.phase = Phase::Ready;
            chat.regenerate(cx);
        });
        idle(cx, &chat);
        chat.read_with(cx, |chat, _| assert_eq!(chat.phase, Phase::Ready));
        assert_eq!(texts(cx, &chat), ["fail", "re: fail"]);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn a_new_chats_follow_up_draft_returns_with_it_from_history(cx: &mut TestAppContext) {
        let (_directory, runtime, _, chat) = chat_with_model(cx);
        send(cx, &chat, "hello");
        idle(cx, &chat);
        let group = chat.read_with(cx, |chat, _| chat.group.clone().unwrap());
        let composer = chat.read_with(cx, |chat, _| chat.composer.clone());
        composer.update(cx, |composer, cx| composer.insert_text("and then?", cx));
        chat.update(cx, |chat, cx| chat.select_group("other".into(), cx));
        chat.update(cx, |chat, cx| chat.select_group(group, cx));
        composer.read_with(cx, |composer, _| assert_eq!(composer.text(), "and then?"));
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn stopping_while_a_reply_prepares_never_sends_it(cx: &mut TestAppContext) {
        let (_directory, runtime, _, chat) = chat_with_model(cx);
        chat.update(cx, |chat, cx| {
            chat.submit("hello".into(), Vec::new(), cx);
            chat.stop(cx);
        });
        idle(cx, &chat);
        assert_eq!(texts(cx, &chat), ["hello"]);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn a_message_sent_while_history_loads_follows_the_history(cx: &mut TestAppContext) {
        let (_directory, runtime, _, chat) = chat_with_model(cx);
        send(cx, &chat, "first");
        idle(cx, &chat);
        let group = chat.read_with(cx, |chat, _| chat.group.clone().unwrap());
        chat.update(cx, |chat, cx| {
            chat.new_chat(cx);
            chat.select_group(group, cx);
            chat.submit("second".into(), Vec::new(), cx);
        });
        idle(cx, &chat);
        assert_eq!(
            texts(cx, &chat),
            ["first", "re: first", "second", "re: second"]
        );
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn returning_to_a_chat_shows_its_background_reply_once_done(cx: &mut TestAppContext) {
        let (_directory, runtime, model, chat) = chat_with_model(cx);
        send(cx, &chat, "slow a");
        wait(cx, |_| model.streaming("slow a"));
        let group = chat.read_with(cx, |chat, _| chat.group.clone().unwrap());
        chat.update(cx, |chat, cx| {
            chat.new_chat(cx);
            chat.select_group(group, cx);
        });
        idle(cx, &chat);
        model.release("slow a");
        wait(cx, |cx| texts(cx, &chat).len() == 2);
        assert_eq!(texts(cx, &chat), ["slow a", "re: slow a more"]);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn a_note_windows_chat_edits_the_summary_that_window_shows(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let (_directory, runtime) = start_runtime();
        let open = futures::executor::block_on(
            runtime
                .create_note("Design critique".into())
                .unwrap()
                .receive(),
        )
        .unwrap();
        let session = open.summary.id.clone();
        let id = session.clone();
        let insert = runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        "INSERT INTO session_documents (id, session_id, kind, title, body_format, body, sort_order)
                         VALUES ('critique-summary', ?, 'summary', '', 'prosemirror_json', '{\"type\":\"doc\"}', 0)"
                            .into(),
                        vec![serde_json::json!(id)],
                    )
                    .await
                    .map_err(failure)
            })
            .unwrap();
        futures::executor::block_on(insert.receive()).unwrap();
        cx.update(crate::ui::theme::UiFonts::init);
        let (note, cx) = cx.add_window_view(|window, cx| {
            crate::workspace::NoteView::new(runtime.clone(), window, cx)
        });
        note.update(cx, |note, cx| note.open(session.clone(), cx));
        for _ in 0..400 {
            cx.run_until_parked();
            if note.read_with(cx, |note, _| note.shown_summary().is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let chat = cx.new(|cx| ChatView::new(runtime.clone(), WeakEntity::new_invalid(), cx));
        let summary = chat.update(cx, |chat, cx| {
            chat.set_note(note.downgrade());
            chat.set_route(Route::Session(session), cx);
            let user = ChatMessage {
                id: "u".into(),
                user: true,
                parts: Vec::new(),
                created_at: None,
                refs: Vec::new(),
            };
            chat.tool_context(&user, cx).summary
        });
        assert_eq!(summary.as_deref(), Some("critique-summary"));
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn opening_ai_settings_from_chat_navigates_after_the_click(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        cx.update(crate::ui::theme::UiFonts::init);
        let lane = crate::contracts::LaneContext {
            runtime: runtime.clone(),
        };
        let (workspace, cx) =
            cx.add_window_view(|window, cx| WorkspaceView::new(lane, ready, window, cx));
        cx.run_until_parked();
        let chat = workspace.read_with(cx, |workspace, _| workspace.chat().clone());
        chat.update(cx, |chat, cx| chat.open_settings(cx));
        cx.run_until_parked();
        assert_eq!(
            workspace.read_with(cx, |workspace, _| workspace.current_route()),
            Route::settings("intelligence")
        );
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[gpui::test]
    fn a_note_dropped_on_chat_rides_with_the_next_message_only(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let chat = cx.new(|cx| ChatView::new(runtime, WeakEntity::new_invalid(), cx));
        let session = |id: &str| ContextRef {
            kind: MentionKind::Session,
            id: id.into(),
        };
        chat.update(cx, |chat, cx| {
            chat.add_dropped_session(SessionId("a".into()), cx);
            chat.add_dropped_session(SessionId("a".into()), cx);
            assert_eq!(chat.pending_refs(cx), [session("a")]);
            assert!(chat.pending_refs(cx).is_empty());
            chat.add_dropped_session(SessionId("b".into()), cx);
            chat.new_chat(cx);
            assert!(chat.pending_refs(cx).is_empty());
        });
    }
}
