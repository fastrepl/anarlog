//! The dictation panel: shipping's floating bar in dictation mode (`FloatingBarView.swift` with
//! `DictationTranscript.swift`). It is always dark, its stop control finishes the dictation
//! ("Done"), and the expand control toggles the live transcript preview.

use gpui::{
    Context, EventEmitter, FontWeight, HighlightStyle, Hsla, MouseButton, Render, ScrollHandle,
    SharedString, Size, StyledText, Window, div, prelude::*, px, rgb,
};

use super::controller::Phase;
use crate::meeting::{
    floating::{
        ACCENT, COMPACT_HEIGHT, EXPANDED_PADDING, HANDLE_RESERVED, ICON_SIZE, INSET, STOP_WIDTH,
        container_size, controls_width, hover_handle, icon, palette_with,
    },
    widgets::{dancing_bars, spinner},
};
use crate::ui::a11y::Accessible;

/// Dictation sends `opacity: 1`, which the panel clamps to its maximum.
const OPACITY: f32 = 0.95;

/// What shipping's `getDictationPanelState` hands the panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panel {
    pub owner: String,
    pub phase: Phase,
    pub amplitude: f32,
    pub microphone: String,
    pub text: String,
    pub partial: String,
    pub preview_enabled: bool,
    pub preview_unavailable: bool,
    pub expanded: bool,
}

impl Panel {
    /// The transcript line's placeholder while no words have arrived.
    pub fn placeholder(&self) -> &'static str {
        if self.preview_unavailable {
            "Live preview is unavailable. Your text will appear when you finish."
        } else if !self.preview_enabled {
            "Enable Live transcript preview in Settings → Dictation to see words as you speak."
        } else {
            match self.phase {
                Phase::Transcribing => "Finishing…",
                Phase::Starting => "Starting…",
                _ => "Listening…",
            }
        }
    }
}

/// The panel window's size; the expand control is always shown while dictating.
pub fn size(expanded: bool) -> Size<gpui::Pixels> {
    container_size(expanded, true)
}

pub enum BarEvent {
    Finish,
    ToggleExpanded,
}

pub struct DictationBar {
    panel: Panel,
    hovered: bool,
    stop_hovered: bool,
    pinned: bool,
    scroll: ScrollHandle,
}

impl EventEmitter<BarEvent> for DictationBar {}

impl DictationBar {
    pub fn new(panel: Panel) -> Self {
        Self {
            panel,
            hovered: false,
            stop_hovered: false,
            pinned: true,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn set(&mut self, panel: Panel, cx: &mut Context<Self>) {
        if panel == self.panel {
            return;
        }
        let words_changed = panel.text != self.panel.text || panel.partial != self.panel.partial;
        if panel.expanded && !self.panel.expanded {
            self.pinned = true;
        }
        self.panel = panel;
        if self.pinned && words_changed {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn transcript(&self, color: Hsla) -> impl IntoElement {
        let panel = &self.panel;
        let (text, highlight): (SharedString, _) =
            if panel.text.is_empty() && panel.partial.is_empty() {
                let placeholder = panel.placeholder();
                (placeholder.into(), 0..placeholder.len())
            } else {
                let separator = if panel.text.is_empty() || panel.partial.is_empty() {
                    ""
                } else {
                    " "
                };
                let line = format!("{}{separator}{}", panel.text, panel.partial);
                let partial = line.len() - panel.partial.len()..line.len();
                (line.into(), partial)
            };
        div()
            .w_full()
            .px(px(4.))
            .text_size(px(15.))
            // SwiftUI's 15pt system line plus its 5pt `lineSpacing`.
            .line_height(px(23.))
            .text_color(color)
            .child(StyledText::new(text).with_highlights([(
                highlight,
                // Highlight colors blend over the line's color, so dim with alpha instead.
                HighlightStyle {
                    fade_out: Some(0.4),
                    ..HighlightStyle::default()
                },
            )]))
    }

    fn expanded_panel(
        &self,
        content: Hsla,
        chip: (Hsla, Hsla),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_words = !self.panel.text.is_empty() || !self.panel.partial.is_empty();
        div()
            .size_full()
            .relative()
            .p(px(EXPANDED_PADDING))
            .child(
                div()
                    .id("dictation-transcript")
                    .size_full()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|this, _, _, cx| {
                        let pinned = this.scroll.max_offset().y + this.scroll.offset().y < px(20.);
                        if this.pinned != pinned {
                            this.pinned = pinned;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.transcript(content))
                            .child(div().h(px(EXPANDED_PADDING))),
                    ),
            )
            .when(!self.pinned && has_words, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom(px(EXPANDED_PADDING))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("dictation-bottom")
                                .a11y_button("Go to bottom")
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .px(px(12.))
                                .py(px(7.))
                                .bg(chip.0)
                                .border(px(0.5))
                                .border_color(chip.1)
                                .text_color(content)
                                .text_size(px(11.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .cursor_pointer()
                                .child(icon("meeting/ArrowDown02Icon.svg", 10., content))
                                .child("Go to bottom")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pinned = true;
                                    this.scroll.scroll_to_bottom();
                                    cx.notify();
                                })),
                        ),
                )
            })
    }
}

impl Render for DictationBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette_with(true, OPACITY);
        let expanded = self.panel.expanded;
        let container = size(expanded);
        if window.viewport_size() != container {
            window.defer(cx, move |window, _| window.resize(container));
        }
        let width = container.width - px(INSET * 2.);
        let body = container.height - px(INSET * 2. + HANDLE_RESERVED);
        let handle = self.hovered && !expanded;
        let accent: Hsla = rgb(ACCENT).into();
        let transcribing = self.panel.phase == Phase::Transcribing;
        let amplitude = if self.panel.phase == Phase::Recording {
            self.panel.amplitude
        } else {
            0.
        };
        let stop = div()
            .id("dictation-done")
            .a11y_button(if transcribing {
                "Finishing dictation"
            } else {
                "Stop dictation"
            })
            .a11y_disabled(transcribing)
            .w(px(STOP_WIDTH))
            .h(px(ICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .bg(if self.stop_hovered {
                accent.opacity(0.18)
            } else {
                palette.control
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.stop_hovered = *hovered;
                cx.notify();
            }))
            .when(!transcribing, |stop| {
                stop.cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(BarEvent::Finish)))
            })
            .map(|stop| {
                if transcribing {
                    stop.child(spinner("dictation-finishing", 16., palette.content))
                } else if self.stop_hovered {
                    stop.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(accent)
                            .child(div().size(px(9.)).bg(accent))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Done"),
                            ),
                    )
                } else {
                    stop.child(dancing_bars("dictation-bars", amplitude, accent))
                }
            });
        let controls = div()
            .absolute()
            .left((width - px(controls_width(true))) / 2.)
            .top(body - px(COMPACT_HEIGHT))
            .w(px(controls_width(true)))
            .h(px(COMPACT_HEIGHT))
            .flex()
            .items_center()
            .child(stop)
            .child(
                div()
                    .id("dictation-expand")
                    .a11y_button(if expanded { "Collapse" } else { "Expand" })
                    .aria_expanded(expanded)
                    .size(px(ICON_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .hover(|style| style.bg(palette.control))
                    .child(icon(
                        if expanded {
                            "meeting/ArrowShrinkIcon.svg"
                        } else {
                            "meeting/ArrowExpandIcon.svg"
                        },
                        14.,
                        palette.content,
                    ))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(BarEvent::ToggleExpanded))),
            );
        let bar_height = body + if handle { px(HANDLE_RESERVED) } else { px(0.) };
        div()
            .id("dictation-bar")
            .a11y_group(gpui::Role::Toolbar, "Dictation")
            .size_full()
            .relative()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .absolute()
                    .left(px(INSET))
                    .bottom(px(INSET))
                    .w(width)
                    .h(bar_height)
                    .overflow_hidden()
                    .bg(if handle {
                        palette.envelope
                    } else {
                        palette.surface
                    })
                    .border(px(0.5))
                    .border_color(palette.outer)
                    .child(
                        div()
                            .absolute()
                            .top(px(1.))
                            .left(px(1.))
                            .right(px(1.))
                            .bottom(px(1.))
                            .border(px(0.5))
                            .border_color(palette.inner),
                    )
                    .when(handle, |bar| bar.child(hover_handle(palette.dots)))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .bottom_0()
                            .w(width)
                            .h(body)
                            .when(expanded, |view| {
                                view.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .w(width)
                                        .h(body - px(COMPACT_HEIGHT))
                                        .child(self.expanded_panel(
                                            palette.content,
                                            (palette.chip, palette.chip_stroke),
                                            cx,
                                        )),
                                )
                            })
                            .child(controls),
                    ),
            )
    }
}
