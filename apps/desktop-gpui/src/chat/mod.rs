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
    mode: ChatMode,
    route: Route,
    context: Option<SessionId>,
    group: Option<Arc<str>>,
    /// Shipping's chat `sessionId`: the group's id, or a fresh one for a new chat. Drafts and
    /// queued messages belong to it.
    draft_key: String,
    drafts: Drafts,
    entries: Vec<Entry>,
    groups: Vec<ChatGroup>,
    history_open: bool,
    model_configured: Option<bool>,
    /// The chosen model is Anarlog's hosted one, which needs a paid plan.
    hosted_model: bool,
    composer: Entity<Composer>,
    sent: SentHistory,
    phase: Phase,
    run: u64,
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
            mode: ChatMode::FloatingClosed,
            route: Route::Empty,
            context: None,
            group: None,
            draft_key: uuid::Uuid::new_v4().to_string(),
            drafts: Drafts::default(),
            entries: Vec::new(),
            groups: Vec::new(),
            history_open: false,
            model_configured: None,
            hosted_model: false,
            composer,
            sent: SentHistory::default(),
            phase: Phase::Ready,
            run: 0,
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
        if was_right == is_right {
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

    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.detach_run();
        self.group = None;
        self.entries.clear();
        self.history_open = false;
        self.switch_draft(uuid::Uuid::new_v4().to_string(), cx);
        cx.notify();
    }

    pub fn select_group(&mut self, group: Arc<str>, cx: &mut Context<Self>) {
        self.detach_run();
        self.history_open = false;
        self.group = Some(group.clone());
        self.switch_draft(group.to_string(), cx);
        self.entries.clear();
        self.load.cancel();
        self.load = CancellationToken::new();
        let reply = store::messages(&self.runtime, group.clone(), self.load.clone());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                if this.group.as_ref() != Some(&group) {
                    return;
                }
                match result {
                    Ok(messages) => {
                        this.entries = messages.into_iter().map(Entry::new).collect();
                        this.scroll.scroll_to_bottom();
                    }
                    Err(ServiceError::Cancelled) => {}
                    Err(error) => this.phase = Phase::Failed(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
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
        let _ = self.workspace.update(cx, |workspace, cx| {
            workspace.open_route(Route::settings("intelligence"), true, cx)
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
            && self
                .workspace
                .read_with(cx, |workspace, cx| workspace.editable_summary_shown(cx))
                .unwrap_or(false)
    }

    fn submit_draft(&mut self, cx: &mut Context<Self>) {
        let composer = self.composer.read(cx);
        let text = composer.text().trim().to_owned();
        if text.is_empty() || self.model_ready(cx) != Some(true) {
            return;
        }
        let refs = composer.context_refs();
        let doc = composer.doc();
        self.sent.push(doc);
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        self.submit(text, refs, cx);
    }

    fn submit(&mut self, text: String, refs: Vec<ContextRef>, cx: &mut Context<Self>) {
        if self.busy() {
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
        if self.busy() {
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
            None => (Arc::from(uuid::Uuid::new_v4().to_string()), true),
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
            let result = async {
                if created {
                    store::create_group(&runtime, group.clone(), title)?
                        .receive()
                        .await?;
                }
                let system = services
                    .ai
                    .enqueue(system_prompt(
                        services.clone(),
                        runtime.clone(),
                        context,
                        refs,
                    ))?
                    .await
                    .map_err(failure)??;
                let mut messages = vec![system];
                messages.extend(history);
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
            let _ = this.update(cx, |this, cx| this.finish(run, created, result, cx));
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
            .and_then(|_| {
                self.workspace
                    .read_with(cx, |workspace, cx| workspace.shown_summary(cx))
                    .ok()
                    .flatten()
            });
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

    fn finish(&mut self, run: u64, created: bool, result: Result<Message>, cx: &mut Context<Self>) {
        if run != self.run {
            return;
        }
        self._poll = None;
        self.session = None;
        self.drain_pending();
        self.drain_tools();
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
            Err(error) => {
                if created && self.entries.len() <= 1 {
                    // The group was never written, like shipping's failed-group rollback.
                    self.group = None;
                }
                self.phase = Phase::Failed(error.to_string());
            }
        }
        self.clear_stream();
        if self.phase == Phase::Ready && !self.queue.is_empty() {
            let next = self.queue.remove(0);
            self.send(next.text, next.refs, cx);
        }
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            let _ = session.cancel();
        }
        cx.notify();
    }

    /// Leaves an in-flight reply running in the background; it is still saved to its chat.
    fn detach_run(&mut self) {
        self.run += 1;
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
    fn drain_tools(&mut self) -> bool {
        let updates = std::mem::take(
            &mut *self
                .pending_tools
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        );
        let changed = !updates.is_empty();
        for part in updates.iter().filter_map(ToolPart::from_part) {
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
                    let tools = this.drain_tools();
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
        match outcome {
            Some(Ok(())) => self.finish(self.run, false, Err(ServiceError::Cancelled), cx),
            Some(Err(error)) => self.finish(self.run, false, Err(failure(error)), cx),
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
