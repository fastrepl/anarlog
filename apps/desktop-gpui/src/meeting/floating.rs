use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anlg_listener_core::LiveTranscriptSegment;
use desktop_runtime::{ServiceError, SessionId};
use gpui::{
    Context, EventEmitter, MouseButton, Render, SharedString, Task, Window, div, prelude::*, px,
};

use super::capture::{CaptureService, Phase};
use crate::ui::theme::theme;

pub enum FloatingEvent {
    Expanded(bool),
    OpenSession(SessionId),
    Failed(ServiceError),
}

pub struct RecordingBar {
    capture: CaptureService,
    revision: u64,
    session: Option<SessionId>,
    phase: Phase,
    status: Arc<str>,
    error: Option<ServiceError>,
    amplitude: (u16, u16),
    expanded: bool,
    captions: BTreeMap<String, Arc<LiveTranscriptSegment>>,
    _poll: Task<()>,
}

impl EventEmitter<FloatingEvent> for RecordingBar {}
impl RecordingBar {
    pub fn new(capture: CaptureService, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let update = this.capture.take_update(this.revision);
                        let amplitude = (update.amplitude.0 / 50, update.amplitude.1 / 50);
                        if this.revision == update.revision && this.amplitude == amplitude {
                            return;
                        }
                        if this.session != update.session {
                            this.captions.clear();
                        }
                        this.session = update.session;
                        this.revision = update.revision;
                        this.phase = update.phase;
                        this.status = update.status;
                        this.error = update.error;
                        this.amplitude = amplitude;
                        for (id, segment) in update.changes {
                            match segment {
                                Some(segment) => {
                                    this.captions.insert(id, segment);
                                }
                                None => {
                                    this.captions.remove(&id);
                                }
                            }
                        }
                        while this.captions.len() > 12 {
                            if let Some(oldest) = this
                                .captions
                                .values()
                                .min_by_key(|segment| segment.start_ms)
                                .map(|segment| segment.id.clone())
                            {
                                this.captions.remove(&oldest);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            capture,
            revision: 0,
            session: None,
            phase: Phase::Idle,
            status: "Ready".into(),
            error: None,
            amplitude: (0, 0),
            expanded: false,
            captions: BTreeMap::new(),
            _poll: poll,
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        let reply = self.capture.stop();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.await.unwrap_or(Err(ServiceError::Closed)),
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(error.clone());
                    cx.emit(FloatingEvent::Failed(error));
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

impl Render for RecordingBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let mut captions: Vec<_> = self.captions.values().collect();
        captions.sort_by_key(|segment| segment.start_ms);
        div()
            .flex()
            .flex_col()
            .w(px(if self.expanded { 360. } else { 240. }))
            .h(px(if self.expanded { 430. } else { 38. }))
            .bg(colors.background)
            .text_color(colors.foreground)
            .border_1()
            .border_color(colors.border)
            .rounded_lg()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(38.))
                    .min_h(px(38.))
                    .gap_2()
                    .px_2()
                    .child(
                        div()
                            .id("drag-recording-window")
                            .cursor_move()
                            .child("⋮")
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .child(SharedString::from(self.status.clone())),
                    )
                    .child(
                        div()
                            .w(px(16.))
                            .h(px(18.))
                            .flex()
                            .items_end()
                            .gap(px(2.))
                            .children([self.amplitude.0, self.amplitude.1].into_iter().map(
                                |amplitude| {
                                    div()
                                        .w(px(5.))
                                        .h(px((amplitude.min(100) as f32 / 100. * 18.).max(2.)))
                                        .bg(colors.primary)
                                },
                            )),
                    )
                    .when(
                        matches!(self.phase, Phase::Listening | Phase::Loading),
                        |view| {
                            view.child(
                                div()
                                    .id("stop-recording")
                                    .cursor_pointer()
                                    .child("Stop")
                                    .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                            )
                        },
                    )
                    .child(
                        div()
                            .id("expand-recording")
                            .size(px(30.))
                            .cursor_pointer()
                            .child(if self.expanded { "−" } else { "+" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.expanded = !this.expanded;
                                cx.emit(FloatingEvent::Expanded(this.expanded));
                                cx.notify();
                            })),
                    ),
            )
            .when(self.expanded, |view| {
                view.when_some(self.error.as_ref(), |view, error| {
                    view.child(
                        div()
                            .p_2()
                            .text_sm()
                            .text_color(colors.destructive)
                            .child(error.to_string()),
                    )
                })
                .child(
                    div()
                        .id("captions")
                        .flex_1()
                        .overflow_y_scroll()
                        .px_2()
                        .children(captions.into_iter().map(|segment| {
                            div()
                                .id(SharedString::from(segment.id.clone()))
                                .my_2()
                                .p_2()
                                .rounded_lg()
                                .bg(colors.muted)
                                .child(
                                    segment
                                        .words
                                        .iter()
                                        .map(|word| word.text.as_str())
                                        .collect::<String>(),
                                )
                        })),
                )
                .when_some(self.session.clone(), |view, session| {
                    view.child(
                        div()
                            .id("open-recording-meeting")
                            .cursor_pointer()
                            .p_2()
                            .child("Open meeting")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(FloatingEvent::OpenSession(session.clone()))
                            })),
                    )
                })
            })
    }
}
