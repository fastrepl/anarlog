use std::path::PathBuf;

use desktop_runtime::{OpenSession, Result, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;
use gpui::{Context, Entity, EventEmitter, Render, Subscription, Window, div, prelude::*};

use crate::{
    contracts::{EditorEvent, EditorInit, LaneContext, MeetingEvent, MeetingIntent},
    editor::{EditorPane, menu::EditorRequest},
    meeting::MeetingPane,
    ui::theme::theme,
};

pub struct NoteWindow {
    editor: Entity<EditorPane>,
    meeting: Entity<MeetingPane>,
    show_meeting: bool,
    closing: bool,
    status: String,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for NoteWindow {}
impl EventEmitter<EditorRequest> for NoteWindow {}
impl EventEmitter<MeetingEvent> for NoteWindow {}

impl NoteWindow {
    pub fn new(
        runtime: RuntimeHandle,
        note: OpenSession,
        vault: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self> {
        let document = note
            .note
            .ok_or_else(|| ServiceError::Failed("This session has no note document".into()))?;
        let focus = cx.focus_handle();
        let editor = cx.new(|cx| {
            let mut editor = EditorPane::new(
                LaneContext {
                    runtime: runtime.clone(),
                },
                EditorInit {
                    session_id: note.summary.id.clone(),
                    document,
                    return_focus: focus,
                },
                window,
                cx,
            );
            editor.configure_attachments(vault, cx);
            editor
        });
        let meeting = cx.new(|cx| {
            MeetingPane::new(
                LaneContext { runtime },
                MeetingIntent::Open(note.summary.id.clone()),
                window,
                cx,
            )
        });
        window.set_window_title(&note.summary.title);
        let root = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = root.update(cx, |this, cx| this.close(window, cx));
            false
        });
        let subscriptions = vec![
            cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| {
                if let EditorEvent::SaveFailed { error, .. } = event {
                    this.status = format!("Save failed; draft retained: {error}");
                    cx.notify();
                }
                cx.emit(event.clone());
            }),
            cx.subscribe(&editor, |_, _, event: &EditorRequest, cx| {
                cx.emit(event.clone())
            }),
            cx.subscribe(&meeting, |this, _, event: &MeetingEvent, cx| {
                if let MeetingEvent::Failed(error) = event {
                    this.status = error.to_string();
                    cx.notify();
                }
                cx.emit(event.clone());
            }),
        ];
        Ok(Self {
            editor,
            meeting,
            show_meeting: false,
            closing: false,
            status: String::new(),
            _subscriptions: subscriptions,
        })
    }

    pub fn flush(
        &mut self,
        read_only: bool,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, Result<()>> {
        let flush = self.editor.update(cx, |editor, cx| {
            editor.set_read_only(read_only, cx);
            editor.flush()
        });
        Box::pin(async move { flush.await.map_err(|_| ServiceError::Closed)? })
    }

    pub fn resume(&mut self, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.set_read_only(false, cx));
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
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
}

impl Render for NoteWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(
                div()
                    .flex()
                    .child(
                        div()
                            .id("toggle-meeting")
                            .p_2()
                            .cursor_pointer()
                            .child(if self.show_meeting {
                                "Hide transcript"
                            } else {
                                "Transcript and AI"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_meeting = !this.show_meeting;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("attach-file")
                            .p_2()
                            .cursor_pointer()
                            .child("Attach file")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editor
                                    .update(cx, |editor, cx| editor.choose_attachment(cx));
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(div().flex_1().min_w_0().child(self.editor.clone()))
                    .when(self.show_meeting, |view| {
                        view.child(div().w(gpui::px(360.)).child(self.meeting.clone()))
                    }),
            )
            .when(!self.status.is_empty(), |view| {
                view.child(div().p_2().child(self.status.clone()))
            })
    }
}
