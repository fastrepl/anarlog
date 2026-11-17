use std::path::PathBuf;

use desktop_runtime::{
    DocumentSnapshot, OpenSession, Result, RuntimeHandle, ServiceError, SessionId,
};
use futures::future::BoxFuture;
use gpui::{
    Context, Entity, EventEmitter, FocusHandle, KeyDownEvent, Pixels, Render, Subscription,
    WeakEntity, Window, div, prelude::*, px,
};

use crate::{
    chat::{
        ChatView,
        state::{ChatEvent, ChatMode},
    },
    contracts::{EditorEvent, EditorInit, LaneContext, WorkspaceEvent},
    editor::{EditorPane, menu::EditorRequest},
    ui::theme::{system_font, theme},
    workspace::{NoteEvent, NoteView, WorkspaceView, navigation::Route},
};

/// Shipping's standalone note window (`routes/app/note.$sessionId.tsx`): the whole note with
/// its chat. Navigation, meeting and product requests go to the main window.
pub struct NoteWindow {
    id: SessionId,
    runtime: RuntimeHandle,
    vault: PathBuf,
    note: Entity<NoteView>,
    editor: Option<Entity<EditorPane>>,
    chat: Entity<ChatView>,
    focus: FocusHandle,
    closing: bool,
    /// Closing waits for an unsaved title to save first.
    close_after_title: bool,
    status: String,
    /// Width added so the note keeps its minimum beside the chat's right panel.
    widened: Option<Pixels>,
    editor_subscriptions: Vec<Subscription>,
    _subscriptions: Vec<Subscription>,
}

/// Shipping's `STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX` and the right panel's minimum.
const NOTE_MIN_WIDTH: f32 = 420.;
const RIGHT_PANEL_MIN_WIDTH: f32 = 320.;

impl EventEmitter<EditorEvent> for NoteWindow {}
impl EventEmitter<EditorRequest> for NoteWindow {}
impl EventEmitter<WorkspaceEvent> for NoteWindow {}
impl EventEmitter<NoteEvent> for NoteWindow {}

impl NoteWindow {
    pub fn new(
        runtime: RuntimeHandle,
        session: OpenSession,
        vault: PathBuf,
        workspace: WeakEntity<WorkspaceView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = session.summary.id.clone();
        let note = cx.new(|cx| {
            let mut note = NoteView::new(runtime.clone(), window, cx);
            note.set_standalone();
            note
        });
        let chat = cx.new(|cx| {
            let mut chat = ChatView::new(runtime.clone(), workspace, cx);
            chat.set_note(note.downgrade());
            chat.set_route(Route::Session(id.clone()), cx);
            chat
        });
        window.set_window_title(&session.summary.title);
        let root = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = root.update(cx, |this, cx| this.close(window, cx));
            false
        });
        let subscriptions = vec![
            cx.observe_in(&chat, window, |this, _, window, cx| {
                this.fit_right_panel(window, cx)
            }),
            cx.subscribe_in(
                &note,
                window,
                |this, _, event: &WorkspaceEvent, window, cx| match event {
                    WorkspaceEvent::OpenEditor {
                        session_id,
                        document,
                    } => this.open_editor(session_id, document, window, cx),
                    event => cx.emit(event.clone()),
                },
            ),
            cx.subscribe_in(
                &note,
                window,
                |this, _, event: &NoteEvent, window, cx| match event {
                    NoteEvent::TitleUpdated(session) => {
                        window.set_window_title(&session.summary.title);
                        if std::mem::take(&mut this.close_after_title) {
                            this.close(window, cx);
                        }
                    }
                    NoteEvent::RenameFailed if std::mem::take(&mut this.close_after_title) => {
                        this.status = "Close paused; the title was not saved.".into();
                        cx.notify();
                    }
                    NoteEvent::Request(_) | NoteEvent::OpenTemplates => cx.emit(event.clone()),
                    _ => {}
                },
            ),
        ];
        note.update(cx, |note, cx| note.show(session, cx));
        Self {
            id,
            runtime,
            vault,
            note,
            editor: None,
            chat,
            focus: cx.focus_handle(),
            closing: false,
            close_after_title: false,
            status: String::new(),
            widened: None,
            editor_subscriptions: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// The memo editor, which the note view shows as its content.
    fn open_editor(
        &mut self,
        session_id: &SessionId,
        document: &DocumentSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = match &self.editor {
            Some(editor) if &editor.read(cx).init.session_id == session_id => editor.clone(),
            _ => {
                let init = EditorInit {
                    session_id: session_id.clone(),
                    document: document.clone(),
                    return_focus: self.focus.clone(),
                };
                let (runtime, vault) = (self.runtime.clone(), self.vault.clone());
                let editor = cx.new(|cx| {
                    let mut editor = EditorPane::new(LaneContext { runtime }, init, window, cx);
                    editor.configure_attachments(vault, cx);
                    editor
                });
                self.editor_subscriptions = vec![
                    cx.subscribe(&editor, |_, _, event: &EditorEvent, cx| {
                        cx.emit(event.clone())
                    }),
                    cx.subscribe(&editor, |_, _, event: &EditorRequest, cx| {
                        cx.emit(event.clone())
                    }),
                ];
                self.editor = Some(editor.clone());
                editor
            }
        };
        self.note.update(cx, |note, cx| {
            note.set_content(session_id, editor.into(), cx)
        });
    }

    pub fn flush(
        &mut self,
        read_only: bool,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, Result<()>> {
        if self.note.read(cx).has_unsaved_title(cx) {
            return Box::pin(async { Err(ServiceError::Busy) });
        }
        let Some(editor) = &self.editor else {
            return Box::pin(async { Ok(()) });
        };
        let flush = editor.update(cx, |editor, cx| {
            editor.set_read_only(read_only, cx);
            editor.flush()
        });
        Box::pin(async move { flush.await.map_err(|_| ServiceError::Closed)? })
    }

    pub fn resume(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_read_only(false, cx));
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if self.note.read(cx).has_unsaved_title(cx) {
            self.close_after_title = true;
            self.note.update(cx, |note, cx| note.rename(cx));
            return;
        }
        self.closing = true;
        self.status.clear();
        let flush = self.flush(true, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = flush.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.closing = false;
                match result {
                    Ok(()) => window.remove_window(),
                    Err(error) => {
                        this.resume(cx);
                        this.status = format!("Close paused; draft retained: {error}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// Shipping widens the window when the right panel would squeeze the note, and gives the
    /// width back when the panel closes.
    fn fit_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.chat.read(cx).mode() == ChatMode::RightPanelOpen;
        let mut size = window.viewport_size();
        if open && self.widened.is_none() {
            let deficit = px(NOTE_MIN_WIDTH + RIGHT_PANEL_MIN_WIDTH) - size.width;
            self.widened = Some(deficit.max(px(0.)));
            if deficit > px(0.) {
                size.width += deficit;
                window.resize(size);
            }
        } else if !open
            && let Some(widened) = self.widened.take()
            && widened > px(0.)
        {
            size.width -= widened;
            window.resize(size);
        }
    }

    /// Shipping's Escape closes the chat first, then the window.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.chat.read(cx).mode() != ChatMode::FloatingClosed {
            self.chat
                .update(cx, |chat, cx| chat.event(ChatEvent::Close, window, cx));
        } else {
            self.close(window, cx);
        }
    }
}

impl Render for NoteWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let route = Route::Session(self.id.clone());
        div()
            .id("note-window")
            .size_full()
            .flex()
            .font_family(system_font(cx))
            .text_color(colors.foreground)
            .bg(colors.background)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.escape(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                // Shipping's `MainPanel`: a bordered card, rounded on macOS.
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(colors.card)
                    .border_1()
                    .border_color(colors.border)
                    .when(cfg!(target_os = "macos"), |card| card.rounded(px(12.)))
                    .child(div().flex_1().min_h_0().child(self.note.clone()))
                    .when(!self.status.is_empty(), |card| {
                        card.child(
                            div()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(self.status.clone()),
                        )
                    })
                    .children(crate::chat::overlay(&self.chat, &route, window, cx)),
            )
            .children(crate::chat::right_panel(&self.chat, window, cx))
    }
}
