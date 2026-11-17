use std::sync::{Arc, Mutex};
use std::time::Duration;

use desktop_runtime::{CancellationToken, DocumentSnapshot, Result, ServiceError, SessionId};
use futures::future::BoxFuture;
use gpui::{
    Context, Entity, EventEmitter, Global, ListAlignment, ListState, Render, SharedString, Task,
    Window, div, list, prelude::*,
};

use super::ai::{AiServices, ChatSession, Message, Part, Role, StreamObserver, SummaryContext};
use super::model::{MAX_TEXT, failure};
use crate::ui::a11y::Accessible;
use crate::ui::{input::TextInput, theme::theme};

pub struct AiContext {
    pub group: Arc<str>,
    pub history: Vec<Message>,
    pub summary: Option<(DocumentSnapshot, SummaryContext)>,
}

pub enum ContextPurpose {
    Preview,
    Summarize,
}

pub type ContextResolver =
    Arc<dyn Fn(SessionId, ContextPurpose) -> BoxFuture<'static, Result<AiContext>> + Send + Sync>;

#[derive(Clone)]
pub struct AiViewServices {
    pub ai: Arc<AiServices>,
    pub context: ContextResolver,
    pub approvals: super::tools::Approvals,
}
impl Global for AiViewServices {}

pub enum AiEvent {
    SummarySaved(DocumentSnapshot),
}

pub struct AiPane {
    services: AiViewServices,
    session: SessionId,
    chat: Option<Arc<ChatSession>>,
    history: Vec<Message>,
    rows: Vec<Entity<MessageRow>>,
    list: ListState,
    input: Entity<TextInput>,
    pending: Arc<Mutex<(String, String)>>,
    stream: String,
    reasoning: String,
    status: String,
    busy: bool,
    proposal: Option<super::tools::Proposal>,
    cancellation: CancellationToken,
    _poll: Task<()>,
}
impl EventEmitter<AiEvent> for AiPane {}

impl AiPane {
    pub fn new(session: SessionId, services: AiViewServices, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let proposal = this.services.approvals.pending();
                        if proposal.as_ref().map(|p| &p.id) != this.proposal.as_ref().map(|p| &p.id)
                        {
                            this.proposal = proposal;
                            cx.notify();
                        }
                        let (text, reasoning) = std::mem::take(
                            &mut *this
                                .pending
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner()),
                        );
                        if !text.is_empty() || !reasoning.is_empty() {
                            this.stream.push_str(&text);
                            this.reasoning.push_str(&reasoning);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let input = cx.new(|cx| TextInput::new("Ask about this meeting", cx));
        let mut pane = Self {
            services,
            session,
            chat: None,
            history: Vec::new(),
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Bottom, gpui::px(400.)),
            input,
            pending: Arc::new(Mutex::new((String::new(), String::new()))),
            stream: String::new(),
            reasoning: String::new(),
            status: "Loading meeting chat…".into(),
            busy: true,
            proposal: None,
            cancellation: CancellationToken::new(),
            _poll: poll,
        };
        pane.load(cx);
        pane
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let request = self.services.ai.enqueue((self.services.context)(
            self.session.clone(),
            ContextPurpose::Preview,
        ));
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(context) => match this.services.ai.chat(this.session.clone(), context.group)
                    {
                        Ok(chat) => {
                            this.chat = Some(chat);
                            this.history = context.history;
                            this.rows = this
                                .history
                                .iter()
                                .filter(|message| message.role != Role::System)
                                .map(|message| cx.new(|_| MessageRow::from(message)))
                                .collect();
                            this.list.reset(this.rows.len());
                            this.status.clear();
                        }
                        Err(error) => this.status = error.to_string(),
                    },
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn observer(&mut self) -> StreamObserver {
        self.stream.clear();
        self.reasoning.clear();
        self.cancellation = CancellationToken::new();
        *self
            .pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = (String::new(), String::new());
        let pending = self.pending.clone();
        Arc::new(move |update| {
            let mut pending = pending.lock().unwrap_or_else(|poison| poison.into_inner());
            pending.0.push_str(&update.text);
            pending.1.push_str(&update.reasoning);
        })
    }

    fn send(&mut self, regenerate: bool, cx: &mut Context<Self>) {
        let Some(chat) = self.chat.clone() else {
            return;
        };
        if self.busy {
            return;
        }
        let user = if regenerate {
            let Some(user) = self
                .history
                .iter()
                .rev()
                .find(|message| message.role == Role::User)
                .cloned()
            else {
                return;
            };
            user
        } else {
            let text = self.input.read(cx).buffer.text.trim().to_owned();
            if text.is_empty() {
                return;
            }
            if text.len() > MAX_TEXT {
                self.status = "Message exceeds the size limit.".into();
                cx.notify();
                return;
            }
            Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: Role::User,
                parts: vec![Part::Text { text }],
            }
        };
        let previous = regenerate
            .then(|| {
                self.history
                    .iter()
                    .rev()
                    .find(|message| message.role == Role::Assistant)
                    .map(|message| message.id.clone())
            })
            .flatten();
        let history = self
            .history
            .iter()
            .filter(|message| previous.as_ref() != Some(&message.id))
            .cloned()
            .collect();
        let request = chat.queue_send(user.clone(), history, previous.clone(), self.observer());
        if let Err(error) = request {
            self.status = error.to_string();
            cx.notify();
            return;
        }
        self.busy = true;
        self.status = "Waiting for provider…".into();
        if !regenerate {
            self.rows.push(cx.new(|_| MessageRow::from(&user)));
            self.history.push(user.clone());
            self.list
                .splice(self.rows.len() - 1..self.rows.len() - 1, 1);
        }
        self.input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(message) => {
                        if let Some(previous) = previous
                            && let Some(index) = this
                                .history
                                .iter()
                                .position(|message| message.id == previous)
                        {
                            this.history.remove(index);
                            this.rows.remove(index);
                            this.list.splice(index..index + 1, 0);
                        }
                        for message in [user, message] {
                            if !this
                                .history
                                .iter()
                                .any(|existing| existing.id == message.id)
                            {
                                this.rows.push(cx.new(|_| MessageRow::from(&message)));
                                this.history.push(message);
                                this.list
                                    .splice(this.rows.len() - 1..this.rows.len() - 1, 1);
                            }
                        }
                        this.stream.clear();
                        this.reasoning.clear();
                        *this
                            .pending
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner()) =
                            (String::new(), String::new());
                        this.status.clear();
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn summarize(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let services = self.services.clone();
        let session = self.session.clone();
        let observer = self.observer();
        let cancellation = self.cancellation.clone();
        let queue = self.services.ai.enqueue(Box::pin(async move {
            let context = (services.context)(session, ContextPurpose::Summarize).await?;
            let (base, context) = context.summary.ok_or_else(|| {
                ServiceError::Unsupported(
                    "Select a summary document and template in the editor first.".into(),
                )
            })?;
            services
                .ai
                .summarize(base, context, cancellation, observer)
                .await
        }));
        self.busy = true;
        self.status = "Generating summary…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match queue {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(document) => {
                        this.status = "Summary saved.".into();
                        cx.emit(AiEvent::SummarySaved(document));
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Drop for AiPane {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(chat) = &self.chat {
            let _ = chat.cancel();
        }
    }
}

impl Render for AiPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .gap_2()
            .p_3()
            .child(
                div().flex_1().min_h_0().child(
                    list(
                        self.list.clone(),
                        cx.processor(|this, index: usize, _, _| {
                            this.rows[index].clone().into_any_element()
                        }),
                    )
                    .size_full(),
                ),
            )
            .when(!self.reasoning.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(SharedString::from(self.reasoning.clone())),
                )
            })
            .when(!self.stream.is_empty(), |view| {
                view.child(SharedString::from(self.stream.clone()))
            })
            .when_some(self.proposal.as_ref(), |view, proposal| {
                let approve = proposal.id.clone();
                let reject = proposal.id.clone();
                view.child(
                    div()
                        .id("review-meeting-edit")
                        .max_h_64()
                        .overflow_y_scroll()
                        .p_3()
                        .border_1()
                        .border_color(colors.border)
                        .child("Review proposed edit")
                        .child(
                            div()
                                .text_color(colors.muted)
                                .child(SharedString::from(proposal.before.clone())),
                        )
                        .child(div().child(SharedString::from(proposal.after.clone())))
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    div()
                                        .id("apply-meeting-edit")
                                        .a11y_button("Apply")
                                        .cursor_pointer()
                                        .child("Apply")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.services.approvals.decide(&approve, true);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id("reject-meeting-edit")
                                        .a11y_button("Cancel")
                                        .cursor_pointer()
                                        .child("Cancel")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.services.approvals.decide(&reject, false);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            .child(self.status.clone())
            .child(self.input.clone())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .w_full()
                    .gap_3()
                    .when(!self.busy && self.chat.is_some(), |view| {
                        view.child(
                            div()
                                .id("send-chat")
                                .a11y_button("Send")
                                .flex_shrink_0()
                                .cursor_pointer()
                                .child("Send")
                                .on_click(cx.listener(|this, _, _, cx| this.send(false, cx))),
                        )
                        .child(
                            div()
                                .id("regenerate-chat")
                                .a11y_button("Regenerate reply")
                                .flex_shrink_0()
                                .cursor_pointer()
                                .child("Regenerate reply")
                                .on_click(cx.listener(|this, _, _, cx| this.send(true, cx))),
                        )
                        .child(
                            div()
                                .id("summarize")
                                .a11y_button("Generate summary")
                                .flex_shrink_0()
                                .cursor_pointer()
                                .child("Generate summary")
                                .on_click(cx.listener(|this, _, _, cx| this.summarize(cx))),
                        )
                    })
                    .when(self.busy, |view| {
                        view.child(
                            div()
                                .id("stop-ai")
                                .a11y_button("Stop")
                                .cursor_pointer()
                                .child("Stop")
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.cancellation.cancel();
                                    if let Some(chat) = &this.chat {
                                        let _ = chat.cancel();
                                    }
                                })),
                        )
                    }),
            )
    }
}

struct MessageRow {
    role: &'static str,
    text: Arc<str>,
}
impl From<&Message> for MessageRow {
    fn from(message: &Message) -> Self {
        let text = message
            .parts
            .iter()
            .map(|part| match part {
                Part::Text { text } | Part::Reasoning { text } => text.clone(),
                Part::Tool { name, state, .. } => format!("{name}: {state}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
            .into();
        Self {
            role: if message.role == Role::User {
                "You"
            } else {
                "Anarlog"
            },
            text,
        }
    }
}
impl Render for MessageRow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .py_2()
            .child(self.role)
            .child(SharedString::from(self.text.clone()))
    }
}
