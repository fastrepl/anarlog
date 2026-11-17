//! Shipping's audio player `Timeline` (`audio-player/timeline.tsx`): play/pause, elapsed and
//! total time, the Pro playback-rate menu, the split-channel WaveSurfer waveform, and the
//! right-click menu (Play / Pause / Resume / Stop, Delete recording).

use std::path::PathBuf;
use std::time::Duration;

use desktop_runtime::{RuntimeHandle, SessionId};
use gpui::{
    Bounds, Context, FontWeight, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point,
    Render, Task, Window, canvas, div, fill, point, prelude::*, px, rgb, size, svg,
};

use super::playback::{Command, Playback, RATES, Snapshot, State};
use super::upload::Uploads;
use crate::{
    product::account,
    ui::theme::{RADIUS, Theme, monospace_font, theme},
    workspace::resource_list,
};

const BAR_WIDTH: f32 = 3.;
const BAR_GAP: f32 = 2.;
const HEIGHT: f32 = 24.;
/// WaveSurfer `splitChannels` colors: (wave, progress) per channel, drawn overlaid.
const CHANNELS: [(u32, u32); 2] = [(0xe8d5d5, 0xc9a3a3), (0xd5dde8, 0xa3b3c9)];

pub struct PlayerView {
    playback: Playback,
    snapshot: Snapshot,
    path: Option<PathBuf>,
    bounds: Option<Bounds<Pixels>>,
    dragging: bool,
    rates_open: bool,
    /// The right-click menu, at its window position.
    menu: Option<Point<Pixels>>,
    /// The note whose recording this is, for Delete recording.
    recording: Option<(RuntimeHandle, SessionId)>,
    _poll: Task<()>,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Play,
    Pause,
    Stop,
    Delete,
}

/// Shipping's timeline menu: the transport action for the current state, Stop unless already
/// stopped, then Delete recording.
fn menu_items(state: State) -> Vec<Option<(MenuAction, &'static str)>> {
    let mut items = vec![Some(match state {
        State::Paused => (MenuAction::Play, "Resume"),
        State::Stopped => (MenuAction::Play, "Play"),
        State::Playing => (MenuAction::Pause, "Pause"),
    })];
    if state != State::Stopped {
        items.push(Some((MenuAction::Stop, "Stop")));
    }
    items.push(None);
    items.push(Some((MenuAction::Delete, "Delete recording")));
    items
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
            menu: None,
            recording: None,
            _poll: poll,
        }
    }

    pub fn set_recording(&mut self, runtime: RuntimeHandle, session: SessionId) {
        self.recording = Some((runtime, session));
    }

    fn menu_action(&mut self, action: MenuAction, cx: &mut Context<Self>) {
        self.menu = None;
        self.rates_open = false;
        match action {
            MenuAction::Play => self.command(Command::Play, cx),
            MenuAction::Pause => self.command(Command::Pause, cx),
            MenuAction::Stop => self.command(Command::Stop, cx),
            MenuAction::Delete => {
                if let Some((runtime, session)) = self.recording.clone() {
                    Uploads::global(cx).update(cx, |uploads, cx| {
                        uploads.delete_recording(&runtime, session, cx)
                    });
                }
            }
        }
        cx.notify();
    }

    fn context_menu(
        &self,
        position: Point<Pixels>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let deleting = self
            .recording
            .as_ref()
            .is_none_or(|(_, session)| Uploads::global(cx).read(cx).deleting(session));
        gpui::deferred(
            gpui::anchored().position(position).snap_to_window().child(
                resource_list::context_menu(colors)
                    .id("timeline-menu")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        cx.notify();
                    }))
                    .children(
                        menu_items(self.snapshot.state())
                            .into_iter()
                            .enumerate()
                            .map(|(index, item)| {
                                let Some((action, label)) = item else {
                                    return div()
                                        .id(("timeline-menu-separator", index))
                                        .mx(px(-4.))
                                        .my_1()
                                        .h(px(1.))
                                        .bg(colors.muted);
                                };
                                let disabled = matches!(action, MenuAction::Delete) && deleting;
                                div()
                                    .id(("timeline-menu", index))
                                    .h(px(30.))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .text_sm()
                                    .rounded(px(4.))
                                    .text_color(colors.foreground)
                                    .when_else(
                                        disabled,
                                        |item| item.opacity(0.5),
                                        |item| {
                                            item.cursor_pointer()
                                                .hover(move |style| style.bg(colors.accent))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.menu_action(action, cx);
                                                }))
                                        },
                                    )
                                    .child(label)
                            }),
                    ),
            ),
        )
        .with_priority(2)
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

/// Shipping's `formatTime`: zero-padded minutes and seconds.
fn clock(seconds: f32) -> String {
    let seconds = seconds.max(0.) as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// One WaveSurfer bar per `BAR_WIDTH + BAR_GAP` pixels, each the loudest peak it covers.
fn bars(peaks: &[[f32; 2]], width: f32) -> Vec<[f32; 2]> {
    let count = (width / (BAR_WIDTH + BAR_GAP)).floor().max(0.) as usize;
    if peaks.is_empty() || count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|index| {
            let start = index * peaks.len() / count;
            let end = ((index + 1) * peaks.len() / count)
                .max(start + 1)
                .min(peaks.len());
            peaks[start..end]
                .iter()
                .fold([0_f32; 2], |a, b| [a[0].max(b[0]), a[1].max(b[1])])
        })
        .collect()
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Shipping offers playback speeds only on paid plans.
        let pro = account::is_pro(cx);
        let colors = theme(window);
        let row = div()
            .relative()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .py(px(6.))
            .pl_1()
            .pr_3()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.menu = Some(event.position);
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        let menu = self
            .menu
            .map(|position| self.context_menu(position, colors, cx));
        let row = row.children(menu);
        if let Some(error) = &self.snapshot.error {
            return row
                .h(px(40.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(error.to_string());
        }
        let peaks = if self.path.is_some() && self.path == self.snapshot.path {
            self.snapshot.waveform.clone()
        } else {
            Default::default()
        };
        let position = self.snapshot.position.as_secs_f32();
        let duration = self.snapshot.duration.as_secs_f32();
        let progress = if duration > 0. {
            (position / duration).clamp(0., 1.)
        } else {
            0.
        };
        let view = cx.entity().downgrade();
        let waveform = canvas(
            move |bounds, _, cx| {
                let _ = view.update(cx, |view, _| view.bounds = Some(bounds));
            },
            move |bounds, _, window, _| {
                let bars = bars(&peaks, f32::from(bounds.size.width));
                let cursor = bounds.left() + bounds.size.width * progress;
                let middle = bounds.center().y;
                for (index, bar) in bars.iter().enumerate() {
                    let left = bounds.left() + px(index as f32 * (BAR_WIDTH + BAR_GAP));
                    for (channel, (wave, played)) in CHANNELS.iter().enumerate() {
                        let height = (bar[channel] * HEIGHT).max(1.);
                        let bar_bounds = Bounds::new(
                            point(left, middle - px(height / 2.)),
                            size(px(BAR_WIDTH), px(height)),
                        );
                        let color = if left < cursor { *played } else { *wave };
                        window.paint_quad(
                            fill(bar_bounds, rgb(color)).corner_radii(px(2_f32.min(height / 2.))),
                        );
                    }
                }
                if duration > 0. {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(cursor - px(1.), bounds.top()),
                            size(px(2.), px(HEIGHT)),
                        ),
                        rgb(0x737373),
                    ));
                }
            },
        )
        .size_full();
        let mono = monospace_font(cx);
        let state = self.snapshot.state();
        let playing = state == State::Playing;
        row.child(
            div()
                .id("play-pause")
                .size_7()
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS))
                .border_1()
                .border_color(colors.border)
                .bg(colors.card)
                .shadow_xs()
                .cursor_pointer()
                .hover(|style| style.bg(colors.accent))
                .child(
                    svg()
                        .path(if playing {
                            "meeting/PauseIcon.svg"
                        } else {
                            "meeting/PlayIcon.svg"
                        })
                        .size(px(14.))
                        .text_color(colors.foreground),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.command(
                        if playing {
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
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_1()
                .font_family(mono.clone())
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(clock(position))
                .child("/")
                .child(clock(duration)),
        )
        .when(pro, |row| {
            row.child(
                div()
                    .id("playback-rate")
                    .h_6()
                    .px(px(6.))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS - 2.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .shadow_xs()
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent))
                    .font_family(mono.clone())
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(format!("{}x", self.snapshot.rate))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.rates_open = !this.rates_open;
                        cx.notify();
                    })),
            )
        })
        .child(
            div()
                .id("waveform")
                .flex_1()
                .min_w_0()
                .h(px(HEIGHT))
                .cursor_pointer()
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
        .when(pro && self.rates_open, |row| {
            row.child(
                gpui::deferred(
                    div()
                        .absolute()
                        .right(px(12.))
                        .bottom(px(36.))
                        .py_1()
                        .rounded_lg()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.card)
                        .shadow_md()
                        .children(RATES.into_iter().enumerate().map(|(index, rate)| {
                            let current = rate == self.snapshot.rate;
                            div()
                                .id(("rate", index))
                                .px_3()
                                .py_1()
                                .font_family(mono.clone())
                                .text_xs()
                                .cursor_pointer()
                                .text_color(if current {
                                    colors.foreground
                                } else {
                                    colors.muted_foreground
                                })
                                .when(current, |item| item.font_weight(FontWeight::SEMIBOLD))
                                .hover(|style| style.bg(colors.accent))
                                .child(format!("{rate}x"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.rates_open = false;
                                    this.command(Command::Rate(rate), cx);
                                }))
                        })),
                )
                .with_priority(1),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timeline_menu_offers_the_transport_action_for_the_player_state() {
        let labels = |state| {
            menu_items(state)
                .into_iter()
                .map(|item| item.map_or("-", |(_, label)| label))
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(State::Stopped), ["Play", "-", "Delete recording"]);
        assert_eq!(
            labels(State::Paused),
            ["Resume", "Stop", "-", "Delete recording"]
        );
        assert_eq!(
            labels(State::Playing),
            ["Pause", "Stop", "-", "Delete recording"]
        );
    }

    #[test]
    fn waveform_bars_keep_each_ranges_loudest_peak() {
        let peaks = [[0.1, 0.2], [0.9, 0.1], [0.3, 0.8], [0.2, 0.2]];
        assert_eq!(bars(&peaks, 10.), [[0.9, 0.2], [0.3, 0.8]]);
        assert_eq!(clock(75.9), "01:15");
    }
}
