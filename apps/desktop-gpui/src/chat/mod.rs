//! The "Ask anything" chat: the floating button on the main surface, the floating and
//! right-panel chat, and its history. Shipping reference: `apps/desktop/src/chat/*`
//! and `apps/desktop/src/shared/chat-cta.tsx`. Sending reuses the meeting AI services.

pub mod assets;
pub mod markdown;
pub mod state;
pub mod store;
mod view;

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    AppContext, ClipboardItem, Context, Entity, FocusHandle, Focusable, ScrollHandle, Subscription,
    Task, WeakEntity, Window,
};

use crate::{
    meeting::{
        ai::{ChatSession, Message, Part, Role, StreamObserver},
        ai_view::{AiViewServices, ContextPurpose},
        model::{MAX_TEXT, failure},
    },
    ui::input::{InputEvent, TextInput},
    workspace::{WorkspaceView, navigation::Route},
};
use state::{ChatEvent, ChatMode, SentHistory};
use store::{ChatGroup, ChatMessage, Display};
pub use view::{overlay, reserved_width, right_panel};

struct Entry {
    message: ChatMessage,
    blocks: Vec<markdown::Block>,
}

impl Entry {
    fn new(message: ChatMessage) -> Self {
        let blocks = markdown::parse(&message.text());
        Self { message, blocks }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Ready,
    Busy,
    Failed(String),
}

pub struct ChatView {
    runtime: RuntimeHandle,
    workspace: WeakEntity<WorkspaceView>,
    mode: ChatMode,
    route: Route,
    context: Option<SessionId>,
    group: Option<Arc<str>>,
    entries: Vec<Entry>,
    groups: Vec<ChatGroup>,
    history_open: bool,
    model_configured: Option<bool>,
    /// The chosen model is Anarlog's hosted one, which needs a paid plan.
    hosted_model: bool,
    input: Entity<TextInput>,
    sent: SentHistory,
    phase: Phase,
    run: u64,
    session: Option<Arc<ChatSession>>,
    queue: Vec<(u64, String)>,
    next_queued: u64,
    pending: Arc<Mutex<(String, String)>>,
    stream_text: String,
    stream_reasoning: String,
    stream_blocks: Vec<markdown::Block>,
    expanded: HashSet<String>,
    copied: Option<String>,
    right_fraction: f32,
    restore_sidebar: bool,
    focus: FocusHandle,
    scroll: ScrollHandle,
    load: CancellationToken,
    _poll: Option<Task<()>>,
    _subscription: Subscription,
}

impl ChatView {
    pub fn new(
        runtime: RuntimeHandle,
        workspace: WeakEntity<WorkspaceView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new("Ask anything", cx).inline());
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::Submitted => {
                let text = this.input.read(cx).buffer.text.clone();
                this.submit(text, cx);
            }
            InputEvent::Changed => {
                this.sent.edited();
                cx.notify();
            }
            InputEvent::Rejected => {}
        });
        Self {
            runtime,
            workspace,
            mode: ChatMode::FloatingClosed,
            route: Route::Empty,
            context: None,
            group: None,
            entries: Vec::new(),
            groups: Vec::new(),
            history_open: false,
            model_configured: None,
            hosted_model: false,
            input,
            sent: SentHistory::default(),
            phase: Phase::Ready,
            run: 0,
            session: None,
            queue: Vec::new(),
            next_queued: 0,
            pending: Arc::default(),
            stream_text: String::new(),
            stream_reasoning: String::new(),
            stream_blocks: Vec::new(),
            expanded: HashSet::new(),
            copied: None,
            right_fraction: 0.3,
            restore_sidebar: false,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            load: CancellationToken::new(),
            _poll: None,
            _subscription: subscription,
        }
    }

    pub fn mode(&self) -> ChatMode {
        self.mode
    }

    pub fn is_open(&self) -> bool {
        self.mode.is_open()
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
            self.input.read(cx).focus_handle(cx).focus(window, cx);
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
            // The thread follows its newest message across floating and right-panel layouts.
            self.scroll.scroll_to_bottom();
        }
        self.mode = mode;
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

    pub fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.detach_run();
        self.group = None;
        self.entries.clear();
        self.history_open = false;
        cx.notify();
    }

    pub fn select_group(&mut self, group: Arc<str>, cx: &mut Context<Self>) {
        self.detach_run();
        self.history_open = false;
        self.group = Some(group.clone());
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
        let reply = store::model_configured(&self.runtime);
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

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_owned();
        if text.is_empty() || self.model_ready(cx) != Some(true) {
            return;
        }
        self.sent.push(&text);
        self.input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        if self.busy() {
            self.next_queued += 1;
            self.queue.push((self.next_queued, text));
            cx.notify();
            return;
        }
        self.send(text, cx);
    }

    fn send(&mut self, text: String, cx: &mut Context<Self>) {
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
        };
        self.entries.push(Entry::new(user.clone()));
        self.start(user.to_request(), None, cx);
    }

    /// Re-sends the last user message, replacing the last reply like shipping's regenerate.
    fn regenerate(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(user) = self.entries.iter().rev().find(|entry| entry.message.user) else {
            return;
        };
        let user = user.message.to_request();
        let previous = self
            .entries
            .last()
            .filter(|entry| !entry.message.user)
            .map(|entry| entry.message.id.clone());
        if previous.is_some() {
            self.entries.pop();
        }
        self.start(user, previous, cx);
    }

    fn start(&mut self, user: Message, previous: Option<String>, cx: &mut Context<Self>) {
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
        let history = self
            .entries
            .iter()
            .filter(|entry| entry.message.id != user.id)
            .map(|entry| entry.message.to_request())
            .collect::<Vec<_>>();
        let title = state::fallback_title(&message_text(&user));
        let runtime = self.runtime.clone();
        let context = self.context.clone();
        let observer = self.observer();
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
                    .enqueue(system_prompt(services.clone(), context))?
                    .await
                    .map_err(failure)??;
                let mut messages = vec![system];
                messages.extend(history);
                session
                    .queue_send(user, messages, previous, observer)?
                    .await
                    .map_err(failure)?
            }
            .await;
            let _ = this.update(cx, |this, cx| this.finish(run, created, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish(&mut self, run: u64, created: bool, result: Result<Message>, cx: &mut Context<Self>) {
        if run != self.run {
            return;
        }
        self._poll = None;
        self.session = None;
        self.clear_stream();
        match result {
            Ok(reply) => {
                self.phase = Phase::Ready;
                self.entries
                    .push(Entry::new(ChatMessage::from_reply(&reply)));
                self.scroll.scroll_to_bottom();
            }
            Err(ServiceError::Cancelled) => self.phase = Phase::Ready,
            Err(error) => {
                if created && self.entries.len() <= 1 {
                    // The group was never written, like shipping's failed-group rollback.
                    self.group = None;
                }
                self.phase = Phase::Failed(error.to_string());
            }
        }
        if self.phase == Phase::Ready && !self.queue.is_empty() {
            let (_, text) = self.queue.remove(0);
            self.send(text, cx);
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
        self.clear_stream();
    }

    fn clear_stream(&mut self) {
        *self
            .pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Default::default();
        self.stream_text.clear();
        self.stream_reasoning.clear();
        self.stream_blocks.clear();
    }

    fn observer(&self) -> StreamObserver {
        let pending = self.pending.clone();
        Arc::new(move |update| {
            let mut pending = pending.lock().unwrap_or_else(|poison| poison.into_inner());
            pending.0.push_str(&update.text);
            pending.1.push_str(&update.reasoning);
        })
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
                    if !text.is_empty() || !reasoning.is_empty() {
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
        self.queue.retain(|(queued, _)| *queued != id);
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

    fn recall(&mut self, previous: bool, cx: &mut Context<Self>) -> bool {
        let input = self.input.read(cx);
        let buffer = &input.buffer;
        let edge = if previous { 0 } else { buffer.text.len() };
        if !buffer.selection().is_empty() || buffer.cursor != edge {
            return false;
        }
        let current = buffer.text.clone();
        let Some(text) = self.sent.navigate(previous, &current) else {
            return false;
        };
        let cursor = if previous { 0 } else { text.len() };
        self.input.update(cx, |input, cx| {
            input.set_text(text, cx);
            input.buffer.move_to(cursor, false);
        });
        cx.notify();
        true
    }

    fn draft_empty(&self, cx: &gpui::App) -> bool {
        self.input.read(cx).buffer.text.trim().is_empty()
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

/// The chat system prompt, with the current note as context when there is one.
fn system_prompt(
    services: AiViewServices,
    context: Option<SessionId>,
) -> futures::future::BoxFuture<'static, Result<Message>> {
    Box::pin(async move {
        if let Some(session) = context {
            let context = (services.context)(session, ContextPurpose::Preview).await?;
            if let Some(system) = context
                .history
                .into_iter()
                .find(|message| message.role == Role::System)
            {
                return Ok(system);
            }
        }
        let settings = crate::meeting::config::preferences(&services.ai.runtime).await?;
        let language = Some(settings.text("ai_language")).filter(|language| !language.is_empty());
        let text = anlg_template_app::render(anlg_template_app::Template::ChatSystem(
            anlg_template_app::ChatSystem { language },
        ))
        .map_err(failure)?;
        Ok(Message {
            id: "system".into(),
            role: Role::System,
            parts: vec![Part::Text { text }],
        })
    })
}
