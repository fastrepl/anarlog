//! The floating meeting bar: shipping's macOS panel (`plugins/windows/swift-lib`
//! `FloatingBarView.swift`) with the live transcript bubbles from `meeting-float/route-state.ts`.
//! It renders inside its own transparent window sized by [`container_size`].

use std::sync::Arc;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{ChannelProfile, SegmentKey, SpeakerLabeler};
use desktop_runtime::SessionId;
use gpui::{
    Context, Entity, EventEmitter, FontWeight, Hsla, MouseButton, Render, ScrollEvent,
    SharedString, Size, Subscription, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
    svg,
};

use super::live::{CaptureMode, LiveMeeting, LiveState};
use super::widgets::dancing_bars;
use crate::ui::{a11y::Accessible, scroll::BottomAnchor, theme::theme};

pub const INSET: f32 = 4.;
pub const COMPACT_HEIGHT: f32 = 38.;
pub(crate) const STOP_WIDTH: f32 = 62.;
const SOLO_STOP_WIDTH: f32 = 68.;
pub(crate) const ICON_SIZE: f32 = 30.;
const HORIZONTAL_PADDING: f32 = 4.;
const EXPANDED_WIDTH: f32 = 360.;
const EXPANDED_HEIGHT: f32 = 430.;
pub(crate) const EXPANDED_PADDING: f32 = 12.;
const HANDLE_TOP_PADDING: f32 = 7.;
const HANDLE_HEIGHT: f32 = 12.;
pub const HANDLE_RESERVED: f32 = HANDLE_TOP_PADDING + HANDLE_HEIGHT + 2.;
/// Shipping's `LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT`.
const BUBBLE_LIMIT: usize = 200;
const OVERLAP_MS: i64 = 300;
/// The macOS bar's default `floating_bar_opacity`.
const OPACITY: f32 = 0.78;

pub enum FloatingEvent {
    Stop,
}

pub(crate) fn controls_width(shows_expand: bool) -> f32 {
    if shows_expand {
        STOP_WIDTH + ICON_SIZE
    } else {
        SOLO_STOP_WIDTH
    }
}

/// The panel's window size, the bar plus its inset and hover-handle reserve.
pub fn container_size(expanded: bool, shows_expand: bool) -> Size<gpui::Pixels> {
    let (width, height) = if expanded {
        (EXPANDED_WIDTH, EXPANDED_HEIGHT)
    } else {
        (
            controls_width(shows_expand) + HORIZONTAL_PADDING * 2.,
            COMPACT_HEIGHT,
        )
    };
    size(
        px(width + INSET * 2.),
        px(height + HANDLE_RESERVED + INSET * 2.),
    )
}

/// Shipping's `getFloatingLiveCaptionToggleVisible`: captions exist only for live transcription.
pub fn shows_expand(live: &LiveMeeting) -> bool {
    live.capture == CaptureMode::Live || live.paused.is_some()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bubble {
    pub id: String,
    pub speaker: SharedString,
    pub text: SharedString,
    pub is_self: bool,
    start_ms: i64,
    end_ms: i64,
    pub overlaps_previous: bool,
    pub overlaps_next: bool,
}

/// Shipping's `getFloatingSegmentText`.
fn segment_text(segment: &LiveTranscriptSegment) -> String {
    let words: Vec<&str> = segment
        .words
        .iter()
        .map(|word| word.text.trim())
        .filter(|word| !word.is_empty())
        .collect();
    let mut joined = String::new();
    for word in words {
        let punctuation = word.starts_with([',', '.', '?', '!', ';', ':']);
        if !joined.is_empty() && !punctuation {
            joined.push(' ');
        }
        joined.push_str(word);
    }
    let text = if joined.is_empty() {
        segment.text.as_str()
    } else {
        joined.as_str()
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Shipping's `getFloatingTranscriptBubbles`: the newest segments as chat bubbles, the
/// microphone as "You" and other voices numbered in the order they were first heard.
pub fn bubbles(
    segments: &[Arc<LiveTranscriptSegment>],
    labeler: &mut SpeakerLabeler,
) -> Vec<Bubble> {
    let start = segments.len().saturating_sub(BUBBLE_LIMIT);
    let mut bubbles: Vec<Bubble> = segments[start..]
        .iter()
        .filter_map(|segment| {
            let text = segment_text(segment);
            if text.is_empty() {
                return None;
            }
            // Every voice takes a number in first-heard order, the microphone included, so
            // remote voices keep the transcript tab's numbers.
            let number = labeler.unknown_speaker_number(&anonymous(&segment.key));
            let is_self = segment.key.channel == ChannelProfile::DirectMic;
            let speaker = if is_self {
                "You".to_owned()
            } else {
                format!("Speaker {number}")
            };
            Some(Bubble {
                id: segment.id.clone(),
                speaker: speaker.into(),
                text: text.into(),
                is_self,
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                overlaps_previous: false,
                overlaps_next: false,
            })
        })
        .collect();
    mark_overlaps(&mut bubbles);
    bubbles
}

fn anonymous(key: &SegmentKey) -> SegmentKey {
    SegmentKey {
        speaker_human_id: None,
        ..key.clone()
    }
}

/// Shipping's `markFloatingTranscriptOverlaps`: speech that runs 300ms into another speaker's.
fn mark_overlaps(bubbles: &mut [Bubble]) {
    fn other(boundaries: &[(bool, SharedString, i64)], bubble: &Bubble) -> Option<i64> {
        boundaries
            .iter()
            .find(|(is_self, speaker, _)| *is_self != bubble.is_self || *speaker != bubble.speaker)
            .map(|boundary| boundary.2)
    }
    fn rank(
        boundaries: &mut Vec<(bool, SharedString, i64)>,
        bubble: &Bubble,
        value: i64,
        largest: bool,
    ) {
        let better = |candidate: i64, current: i64| {
            if largest {
                candidate > current
            } else {
                candidate < current
            }
        };
        match boundaries
            .iter_mut()
            .find(|(is_self, speaker, _)| *is_self == bubble.is_self && *speaker == bubble.speaker)
        {
            Some(existing) if better(value, existing.2) => existing.2 = value,
            Some(_) => {}
            None => boundaries.push((bubble.is_self, bubble.speaker.clone(), value)),
        }
        boundaries.sort_by_key(|boundary| if largest { -boundary.2 } else { boundary.2 });
        boundaries.truncate(2);
    }
    let mut latest_ends = Vec::new();
    for bubble in bubbles.iter_mut() {
        let long = bubble.end_ms - bubble.start_ms >= OVERLAP_MS;
        bubble.overlaps_previous = long
            && other(&latest_ends, bubble).is_some_and(|end| end >= bubble.start_ms + OVERLAP_MS);
        rank(&mut latest_ends, bubble, bubble.end_ms, true);
    }
    let mut earliest_starts = Vec::new();
    for bubble in bubbles.iter_mut().rev() {
        bubble.overlaps_next = other(&earliest_starts, bubble)
            .is_some_and(|start| start <= bubble.end_ms - OVERLAP_MS);
        if bubble.end_ms - bubble.start_ms >= OVERLAP_MS {
            rank(&mut earliest_starts, bubble, bubble.start_ms, false);
        }
    }
}

pub struct FloatingBar {
    live: Entity<LiveState>,
    session: Option<SessionId>,
    expanded: bool,
    hovered: bool,
    stop_hovered: bool,
    labeler: SpeakerLabeler,
    bubbles: Vec<Bubble>,
    segments: Option<Arc<[Arc<LiveTranscriptSegment>]>>,
    scroll: BottomAnchor,
    _observe: Subscription,
}

impl EventEmitter<FloatingEvent> for FloatingBar {}

impl FloatingBar {
    pub fn new(live: Entity<LiveState>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&live, |this, _, cx| this.refresh(cx));
        let mut bar = Self {
            live,
            session: None,
            expanded: false,
            hovered: false,
            stop_hovered: false,
            labeler: SpeakerLabeler::new(),
            bubbles: Vec::new(),
            segments: None,
            scroll: BottomAnchor::new(true),
            _observe: observe,
        };
        bar.refresh(cx);
        bar
    }

    /// Opens or collapses the live transcript, as the expand control does.
    pub fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        let shows_expand = self.live.read(cx).current().is_some_and(shows_expand);
        let expanded = expanded && shows_expand;
        if self.expanded != expanded {
            self.expanded = expanded;
            self.scroll.scroll_to_bottom();
            cx.notify();
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.read(cx).current().cloned() else {
            return;
        };
        if self.session.as_ref() != Some(&live.session) {
            // Each meeting starts with captions minimized (`LiveCaptionDefaultVisibilitySync`).
            self.session = Some(live.session.clone());
            self.labeler = SpeakerLabeler::new();
            self.expanded = false;
            self.scroll.pin();
        }
        if self.expanded && !shows_expand(&live) {
            self.set_expanded(false, cx);
        }
        let changed = self
            .segments
            .as_ref()
            .is_none_or(|segments| !Arc::ptr_eq(segments, &live.segments));
        if changed {
            self.bubbles = bubbles(&live.segments, &mut self.labeler);
            self.segments = Some(live.segments.clone());
        }
        cx.notify();
    }

    fn expanded_panel(
        &self,
        palette: &Palette,
        notice: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut previous: Option<&Bubble> = None;
        let mut rows = Vec::new();
        for bubble in &self.bubbles {
            let label = previous.is_none_or(|previous| {
                previous.speaker != bubble.speaker || previous.is_self != bubble.is_self
            });
            rows.push(bubble_row(bubble, label, palette).into_any_element());
            previous = Some(bubble);
        }
        self.scroll.sync_render(true);
        let has_content = !self.bubbles.is_empty();
        div()
            .size_full()
            .relative()
            .p(px(EXPANDED_PADDING))
            .child(
                div()
                    .id("floating-transcript")
                    .size_full()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .overflow_y_scroll()
                    .track_scroll(self.scroll.handle())
                    .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                        if this.scroll.on_scroll(event, window) {
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .children(rows)
                            .when_some(notice, |view, notice| {
                                view.child(
                                    div()
                                        .w_full()
                                        .py(px(6.))
                                        .text_size(px(12.))
                                        .text_center()
                                        .text_color(palette.content.opacity(0.55))
                                        .child(notice),
                                )
                            })
                            .child(div().h(px(EXPANDED_PADDING)))
                            .child(
                                div()
                                    .id("floating-transcript-anchor")
                                    .h(px(1.))
                                    .flex_shrink_0(),
                            ),
                    ),
            )
            .when(
                !self.scroll.is_pinned() && !self.scroll.is_at_bottom() && has_content,
                |view| {
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
                                    .id("floating-bottom")
                                    .a11y_button("Go to bottom")
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(12.))
                                    .py(px(7.))
                                    .bg(palette.chip)
                                    .border(px(0.5))
                                    .border_color(palette.chip_stroke)
                                    .text_color(palette.content)
                                    .text_size(px(11.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .cursor_pointer()
                                    .child(icon(
                                        "meeting/ArrowDown02Icon.svg",
                                        10.,
                                        palette.content,
                                    ))
                                    .child("Go to bottom")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll.scroll_to_bottom();
                                        cx.notify();
                                    })),
                            ),
                    )
                },
            )
    }
}

pub(crate) struct Palette {
    pub content: Hsla,
    pub surface: Hsla,
    pub envelope: Hsla,
    pub control: Hsla,
    pub outer: Hsla,
    pub inner: Hsla,
    pub dots: Hsla,
    pub chip: Hsla,
    pub chip_stroke: Hsla,
    bubble_self: Hsla,
    bubble_other: Hsla,
    overlap: Hsla,
}

fn palette(dark: bool) -> Palette {
    palette_with(dark, OPACITY)
}

/// The bar's colors at a `floating_bar_opacity`; dictation draws at its 0.95 maximum.
pub(crate) fn palette_with(dark: bool, opacity: f32) -> Palette {
    let content: Hsla = if dark { rgb(0xffffff) } else { rgb(0x1f1c1a) }.into();
    let surface: Hsla = if dark { rgb(0x6e7066) } else { rgb(0xdbd9d1) }.into();
    let pick = |dark_value: f32, light_value: f32| if dark { dark_value } else { light_value };
    Palette {
        content,
        surface: surface.opacity(opacity * 0.82),
        envelope: surface.opacity((opacity * 1.08).min(0.95)),
        control: content.opacity(pick(0.08, 0.07)),
        outer: content.opacity(pick(0.14, 0.12)),
        inner: content.opacity(pick(0.28, 0.18)),
        dots: content.opacity(pick(0.48, 0.36)),
        chip: if dark { rgb(0x2e2e2b) } else { rgb(0xf2f2ed) }.into(),
        chip_stroke: if dark { rgb(0x5c5c57) } else { rgb(0xc2bfb8) }.into(),
        bubble_self: gpui::black().opacity(pick(0.34, 0.24)),
        bubble_other: gpui::black().opacity(pick(0.28, 0.2)),
        overlap: gpui::white().opacity(pick(0.26, 0.34)),
    }
}

pub(crate) const ACCENT: u32 = 0xff334d;
const ERROR_ACCENT: u32 = 0xff403d;

pub(crate) fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

fn bubble_row(bubble: &Bubble, label: bool, palette: &Palette) -> impl IntoElement {
    let overlapping = bubble.overlaps_previous || bubble.overlaps_next;
    let content = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .when(label || overlapping, |view| {
            view.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .when(label, |row| {
                        row.child(
                            div()
                                .text_size(px(10.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(gpui::white())
                                .truncate()
                                .child(bubble.speaker.clone()),
                        )
                    })
                    .when(overlapping, |row| {
                        row.child(
                            div()
                                .size(px(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(8.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui::white().opacity(0.72))
                                .child("↔"),
                        )
                    }),
            )
        })
        .child(
            div()
                .w_full()
                .px(px(11.))
                .py(px(8.))
                .bg(if bubble.is_self {
                    palette.bubble_self
                } else {
                    palette.bubble_other
                })
                .when(overlapping, |view| {
                    view.border_1().border_color(palette.overlap)
                })
                .text_size(px(13.))
                .line_height(px(17.))
                .text_color(gpui::white())
                .child(bubble.text.clone()),
        );
    div()
        .flex()
        .w_full()
        .when(bubble.is_self, |row| {
            row.child(div().w(px(40.)).flex_shrink_0())
        })
        .child(content)
        .when(!bubble.is_self, |row| {
            row.child(div().w(px(40.)).flex_shrink_0())
        })
}

pub(crate) fn hover_handle(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            const DOT: f32 = 1.6;
            let mut y = DOT / 2.;
            while y <= HANDLE_HEIGHT {
                let mut x = DOT / 2.;
                while px(x) <= bounds.size.width {
                    window.paint_quad(fill(
                        gpui::Bounds::new(
                            point(
                                bounds.left() + px(x - DOT / 2.),
                                bounds.top() + px(y - DOT / 2.),
                            ),
                            size(px(DOT), px(DOT)),
                        ),
                        color,
                    ));
                    x += 5.;
                }
                y += 7.;
            }
        },
    )
    .absolute()
    .top(px(HANDLE_TOP_PADDING))
    .left(px(8.))
    .right(px(8.))
    .h(px(HANDLE_HEIGHT))
}

impl Render for FloatingBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(live) = self.live.read(cx).current().cloned() else {
            return div().id("floating-bar").size_full();
        };
        let palette = palette(theme(window).background.l < 0.5);
        let shows_expand = shows_expand(&live);
        let expanded = self.expanded && shows_expand;
        let container = container_size(expanded, shows_expand);
        // The bar owns its window's size: compact, or tall enough for the live transcript.
        if window.viewport_size() != container {
            window.defer(cx, move |window, _| window.resize(container));
        }
        let width = container.width - px(INSET * 2.);
        let body = container.height - px(INSET * 2. + HANDLE_RESERVED);
        let handle = self.hovered && !expanded;
        let error = live.warning.is_some();
        let accent: Hsla = rgb(if error { ERROR_ACCENT } else { ACCENT }).into();
        let notice = live.warning.clone().or_else(|| live.paused.clone());
        let stop_width = if shows_expand {
            STOP_WIDTH
        } else {
            SOLO_STOP_WIDTH
        };
        let stop = div()
            .id("floating-stop")
            .a11y_button("Stop listening")
            .w(px(stop_width))
            .h(px(ICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .bg(if self.stop_hovered {
                Hsla::from(rgb(ACCENT)).opacity(0.18)
            } else {
                palette.control
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.stop_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(FloatingEvent::Stop)))
            .map(|stop| {
                if self.stop_hovered {
                    stop.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(rgb(ACCENT))
                            .child(div().size(px(9.)).bg(rgb(ACCENT)))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Stop"),
                            ),
                    )
                } else if error {
                    stop.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(1.5))
                            .child(div().w(px(3.2)).h(px(8.)).bg(accent))
                            .child(div().size(px(3.2)).bg(accent)),
                    )
                } else {
                    stop.child(dancing_bars("floating-bars", live.amplitude, accent))
                }
            });
        let controls = div()
            .absolute()
            .left((width - px(controls_width(shows_expand))) / 2.)
            .top(body - px(COMPACT_HEIGHT))
            .w(px(controls_width(shows_expand)))
            .h(px(COMPACT_HEIGHT))
            .flex()
            .items_center()
            .child(stop)
            .when(shows_expand, |controls| {
                controls.child(
                    div()
                        .id("floating-expand")
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
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.set_expanded(!expanded, cx)),
                        ),
                )
            });
        let bar_height = body + if handle { px(HANDLE_RESERVED) } else { px(0.) };
        div()
            .id("floating-bar")
            .a11y_group(gpui::Role::Toolbar, "Meeting")
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
                                        .child(self.expanded_panel(&palette, notice, cx)),
                                )
                            })
                            .child(controls),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_transcript::SegmentWord;

    fn segment(
        id: &str,
        channel: ChannelProfile,
        speaker: Option<i32>,
        range: (i64, i64),
        words: &[&str],
    ) -> Arc<LiveTranscriptSegment> {
        Arc::new(LiveTranscriptSegment {
            id: id.into(),
            key: SegmentKey {
                channel,
                speaker_index: speaker,
                speaker_human_id: None,
            },
            start_ms: range.0,
            end_ms: range.1,
            text: String::new(),
            words: words
                .iter()
                .map(|text| SegmentWord {
                    text: (*text).into(),
                    start_ms: range.0,
                    end_ms: range.1,
                    channel,
                    is_final: true,
                    id: None,
                })
                .collect(),
        })
    }

    #[test]
    fn bubbles_label_speakers_join_words_and_mark_overlapping_speech() {
        let segments = [
            segment(
                "a",
                ChannelProfile::DirectMic,
                None,
                (0, 2000),
                &[" Hello", " there", " ."],
            ),
            segment(
                "b",
                ChannelProfile::RemoteParty,
                Some(1),
                (1500, 4000),
                &[" Hi"],
            ),
            segment(
                "c",
                ChannelProfile::RemoteParty,
                Some(0),
                (4200, 5000),
                &[" Yes", ","],
            ),
            segment(
                "d",
                ChannelProfile::RemoteParty,
                Some(1),
                (5000, 5100),
                &["  "],
            ),
        ];
        let mut labeler = SpeakerLabeler::new();
        let bubbles = bubbles(&segments, &mut labeler);
        let summary: Vec<_> = bubbles
            .iter()
            .map(|bubble| {
                (
                    bubble.speaker.as_ref(),
                    bubble.text.as_ref(),
                    bubble.is_self,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("You", "Hello there.", true),
                ("Speaker 2", "Hi", false),
                ("Speaker 3", "Yes,", false)
            ]
        );
        assert!(bubbles[0].overlaps_next && bubbles[1].overlaps_previous);
        assert!(!bubbles[2].overlaps_previous && !bubbles[2].overlaps_next);
        // Numbering is first-heard for the meeting, so later windows keep each voice's number.
        let later = super::bubbles(&segments[2..3], &mut labeler);
        assert_eq!(later[0].speaker.as_ref(), "Speaker 3");
    }

    #[test]
    fn bar_sizes_follow_shipping_layout() {
        assert_eq!(container_size(false, true), size(px(108.), px(67.)));
        assert_eq!(container_size(false, false), size(px(84.), px(67.)));
        assert_eq!(container_size(true, true), size(px(368.), px(459.)));
    }
}
