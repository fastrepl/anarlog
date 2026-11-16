use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Render, Task, Window,
    canvas, div, fill, point, prelude::*, px, size,
};

use super::playback::{Command, Playback, RATES, Snapshot};
use crate::ui::theme::theme;

pub struct PlayerView {
    playback: Playback,
    snapshot: Snapshot,
    path: Option<PathBuf>,
    bounds: Option<Bounds<Pixels>>,
    dragging: bool,
    rates_open: bool,
    _poll: Task<()>,
}

impl PlayerView {
    pub fn new(playback: Playback, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let snapshot = this.playback.snapshot();
                        if snapshot.revision != this.snapshot.revision {
                            this.snapshot = snapshot;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            playback,
            snapshot: Snapshot::default(),
            path: None,
            bounds: None,
            dragging: false,
            rates_open: false,
            _poll: poll,
        }
    }

    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.path = Some(path.clone());
        self.command(Command::Open(path), cx);
    }

    fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Err(error) = self.playback.send(command) {
            self.snapshot.error = Some(error);
        }
        cx.notify();
    }

    fn seek(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if let Some(bounds) = self.bounds {
            let fraction = ((x - bounds.left()) / bounds.size.width).clamp(0., 1.);
            self.command(Command::Seek(self.snapshot.duration.mul_f32(fraction)), cx);
        }
    }
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let mut panel = div()
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_xl();
        if let Some(error) = &self.snapshot.error {
            return panel.child(error.to_string());
        }
        if self.path.is_none() {
            return panel.child("No audio loaded.");
        }
        if self.path != self.snapshot.path {
            return panel.child("Loading audio…");
        }
        let peaks = self.snapshot.waveform.clone();
        let position = self.snapshot.position.as_secs_f32();
        let duration = self.snapshot.duration.as_secs_f32();
        let view = cx.entity().downgrade();
        let waveform = canvas(
            move |bounds, _, cx| {
                let _ = view.update(cx, |view, _| view.bounds = Some(bounds));
            },
            move |bounds, _, window, _| {
                if peaks.is_empty() {
                    return;
                }
                let count = peaks.len().min(400);
                let width = bounds.size.width / count as f32;
                for index in 0..count {
                    let range = index * peaks.len() / count..(index + 1) * peaks.len() / count;
                    let peak = peaks[range]
                        .iter()
                        .fold([0_f32; 2], |a, b| [a[0].max(b[0]), a[1].max(b[1])]);
                    for (channel, value) in peak.into_iter().enumerate() {
                        let height = px((value * 11.).max(1.));
                        let top = if channel == 0 {
                            bounds.top() + px(12.) - height
                        } else {
                            bounds.top() + px(12.)
                        };
                        let played =
                            duration > 0. && index as f32 / count as f32 <= position / duration;
                        let color = match (channel, played) {
                            (0, false) => gpui::rgb(0xe8d5d5),
                            (0, true) => gpui::rgb(0xc9a3a3),
                            (_, false) => gpui::rgb(0xd5dde8),
                            (_, true) => gpui::rgb(0xa3b3c9),
                        };
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left() + width * index as f32, top),
                                size(width.max(px(1.)), height),
                            ),
                            color,
                        ));
                    }
                }
                if duration > 0. {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(
                                bounds.left()
                                    + bounds.size.width * (position / duration).clamp(0., 1.),
                                bounds.top(),
                            ),
                            size(px(1.), px(24.)),
                        ),
                        colors.foreground,
                    ));
                }
            },
        )
        .w_full()
        .h(px(24.));
        panel = panel
            .child(
                div()
                    .id("play-pause")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded_full()
                    .border_1()
                    .border_color(colors.border)
                    .hover(|style| style.bg(colors.accent))
                    .child(if self.snapshot.playing { "Ⅱ" } else { "▷" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.command(
                            if this.snapshot.playing {
                                Command::Pause
                            } else {
                                Command::Play
                            },
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(format!(
                        "{:02}:{:02} / {:02}:{:02}",
                        position as u64 / 60,
                        position as u64 % 60,
                        duration as u64 / 60,
                        duration as u64 % 60
                    )),
            )
            .child(
                div()
                    .id("waveform")
                    .flex_1()
                    .h(px(24.))
                    .child(waveform)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.dragging = true;
                            this.seek(event.position.x, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if this.dragging && event.pressed_button == Some(MouseButton::Left) {
                            this.seek(event.position.x, cx);
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.dragging = false),
                    ),
            )
            .child(
                div()
                    .id("playback-rate")
                    .cursor_pointer()
                    .text_xs()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .child(format!("{}×", self.snapshot.rate))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.rates_open = !this.rates_open;
                        cx.notify();
                    })),
            )
            .when(self.rates_open, |view| {
                view.child(
                    div()
                        .absolute()
                        .right_0()
                        .bottom(px(36.))
                        .bg(colors.background)
                        .border_1()
                        .border_color(colors.border)
                        .rounded_lg()
                        .py_1()
                        .children(RATES.into_iter().enumerate().map(|(index, rate)| {
                            div()
                                .id(("rate", index))
                                .px_3()
                                .py_1()
                                .text_xs()
                                .cursor_pointer()
                                .when(rate == self.snapshot.rate, |view| view.bg(colors.accent))
                                .hover(|style| style.bg(colors.accent))
                                .child(format!("{rate}×"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.rates_open = false;
                                    this.command(Command::Rate(rate), cx);
                                }))
                        })),
                )
            });
        panel
    }
}
