//! The Transcript tab body: `TranscriptViewer` (`renderer/index.tsx`) with
//! its scroll controls and playback follow, `SegmentRenderer` /
//! `WordSpan` (`renderer/segment.tsx`, `renderer/word-span.tsx`).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Pixels, ScrollHandle, SharedString, Stateful, Window,
    div, prelude::*, px, relative,
};

use super::Workspace;
use crate::audio_player::PlayerState;
use crate::db::NotePreview;
use crate::prose_text::{Highlight, HighlightedProseText, ProseLayout, ProseText};
use crate::theme::alpha;
use crate::transcript::{Segment, segment_color, timeline_offset_ms};
use crate::ui::{TailwindText as _, icon};

/// `useScrollDetection` + `usePlaybackAutoScroll` + the word hover state of
/// the open transcript tab.
#[derive(Default)]
pub(super) struct TranscriptView {
    pub scroll: ScrollHandle,
    /// The transcript word under the pointer: `(segment id, word index)`.
    pub hover: Option<(String, usize)>,
    /// Each rendered segment's text layout, for hit-testing words.
    pub layouts: HashMap<String, ProseLayout>,
    /// `lastScrolledWordIdRef`: the current line already scrolled into view.
    followed_line: Option<(String, usize)>,
    /// `userScrolledRef`: a wheel during playback stops the follow until the
    /// playback stops.
    user_scrolled: bool,
    /// The scroll controls pill is hovered (drives the icon colours).
    controls_hovered: bool,
    /// The open speaker picker, and whether its Confirm is hovered.
    pub speaker_assign: Option<super::speaker_assign::SpeakerAssign>,
    pub speaker_confirm_hovered: bool,
    /// Bumped per picker so late loads and confirms find their own picker.
    pub picker_generation: u64,
    /// Edit mode and the section selection.
    pub edit: super::transcript_edit::TranscriptEdit,
    /// The in-flight smooth scroll, so a newer one supersedes it.
    animation: u64,
}

/// What every segment of a transcript renders against.
#[derive(Clone, Copy)]
struct SegmentContext<'a> {
    session_id: &'a str,
    transcript_id: &'a str,
    offset_ms: i64,
    current_ms: i64,
    audio_exists: bool,
    /// Edit (and select) mode: editors instead of word spans.
    edit_mode: bool,
}

/// `behavior: "smooth"`: WebKit eases a programmatic scroll over ~0.3s.
const SMOOTH_SCROLL_MS: u64 = 300;
const SMOOTH_SCROLL_STEP_MS: u64 = 16;

impl Workspace {
    /// The transcript viewer's Space hotkey: play from stopped, pause while
    /// playing, resume while paused. Returns `false` when the Transcript tab
    /// with a player is not showing, so the key falls through.
    pub(super) fn toggle_transcript_playback(&mut self, cx: &mut Context<Self>) -> bool {
        let showing = matches!(
            &self.note,
            super::Note::Ready { preview, tab: super::NoteTab::Transcript }
                if preview.audio_exists
                    && self
                        .audio_player
                        .as_ref()
                        .is_some_and(|player| player.session_id == preview.session.id)
        );
        if !showing {
            return false;
        }
        self.toggle_playback(cx);
        true
    }

    /// `element.scrollTo({ top, behavior: "smooth" })` for the transcript
    /// viewer: ease the offset from where it is to `target_top` (a positive
    /// scroll top).
    fn smooth_scroll_transcript(&mut self, target_top: Pixels, cx: &mut Context<Self>) {
        let handle = self.transcript_view.scroll.clone();
        let max = handle.max_offset().height;
        let target = -target_top.max(px(0.0)).min(max);
        let start = handle.offset().y;
        if (f32::from(target - start)).abs() < 0.5 {
            return;
        }
        self.transcript_view.animation += 1;
        let generation = self.transcript_view.animation;
        cx.spawn(async move |this, cx| {
            let started = std::time::Instant::now();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(SMOOTH_SCROLL_STEP_MS))
                    .await;
                let t = (started.elapsed().as_millis() as f32 / SMOOTH_SCROLL_MS as f32).min(1.0);
                // ease-out cubic
                let eased = 1.0 - (1.0 - t).powi(3);
                let keep_going = this
                    .update(cx, |this, cx| {
                        if this.transcript_view.animation != generation {
                            return false;
                        }
                        let mut offset = handle.offset();
                        offset.y = start + (target - start) * eased;
                        handle.set_offset(offset);
                        cx.notify();
                        t < 1.0
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    /// `usePlaybackAutoScroll`: while playing, `scrollIntoView({ block:
    /// "center" })` the current line the first time it becomes current,
    /// unless the user wheeled since playback started.
    fn follow_transcript_playback(
        &mut self,
        preview: &NotePreview,
        timeline: &[(i64, bool)],
        current_ms: i64,
        cx: &mut Context<Self>,
    ) {
        let playing = self.audio_player.as_ref().is_some_and(|player| {
            player.session_id == preview.session.id && player.state == PlayerState::Playing
        });
        if !playing {
            self.transcript_view.followed_line = None;
            self.transcript_view.user_scrolled = false;
            return;
        }
        if self.transcript_view.user_scrolled {
            return;
        }
        let current = preview.transcripts.iter().find_map(|transcript| {
            let offset_ms = timeline_offset_ms(transcript.started_at_ms, timeline);
            transcript.segments.iter().find_map(|segment| {
                segment
                    .active_line(offset_ms, current_ms)
                    .map(|line| (segment, line))
            })
        });
        let Some((segment, line)) = current else {
            return;
        };
        let key = (segment.id.clone(), line);
        if self.transcript_view.followed_line.as_ref() == Some(&key) {
            return;
        }
        let Some(range) = segment.line_range(line) else {
            return;
        };
        // The line's position comes from the previous frame's layout; a
        // segment that has not been laid out yet is retried next frame.
        let spans = self
            .transcript_view
            .layouts
            .get(&segment.id)
            .map(|layout| layout.line_spans(range))
            .unwrap_or_default();
        let (Some(first), Some(last)) = (spans.first(), spans.last()) else {
            return;
        };
        let viewport = self.transcript_view.scroll.bounds();
        if viewport.size.height <= px(0.0) {
            return;
        }
        self.transcript_view.followed_line = Some(key);
        let line_center = (first.top() + last.bottom()) / 2.0;
        let viewport_center = viewport.top() + viewport.size.height / 2.0;
        let scroll_top = -self.transcript_view.scroll.offset().y;
        self.smooth_scroll_transcript(scroll_top + (line_center - viewport_center), cx);
    }

    /// `TranscriptViewer`: a `gap-8` column of transcripts scrolling inside
    /// the tab, `pb-[4rem]`, with `~ ~ ~` separators between transcripts.
    /// Each transcript lists its segments 16px apart (`SEGMENT_GAP`).
    pub(super) fn render_transcript(
        &mut self,
        preview: &NotePreview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let count = preview.transcripts.len();
        // `useTranscriptTimelineMetadata`: offsets against the earliest
        // transcript of the session (every rendered transcript has words);
        // `currentMs` from the session's player.
        let timeline: Vec<(i64, bool)> = preview
            .transcripts
            .iter()
            .map(|t| (t.started_at_ms, true))
            .collect();
        let current_ms = if preview.audio_exists {
            self.audio_position_ms(&preview.session.id)
        } else {
            0
        };
        let live_ids: HashSet<String> = preview
            .transcripts
            .iter()
            .flat_map(|t| t.segments.iter().map(|s| s.id.clone()))
            .collect();
        self.transcript_view
            .layouts
            .retain(|id, _| live_ids.contains(id));
        self.follow_transcript_playback(preview, &timeline, current_ms, cx);
        let audio_exists = preview.audio_exists;
        // `editMode && !screen.currentActive`
        let edit_mode = self.transcript_edit_mode(&preview.session.id)
            && self.session_mode(&preview.session.id) == super::recording::SessionMode::Inactive;
        if !edit_mode && !self.transcript_view.edit.editors.is_empty() {
            self.transcript_view.edit.editors.clear();
        }
        let playing = self
            .audio_player
            .as_ref()
            .is_some_and(|player| player.state == PlayerState::Playing);

        let viewer = div()
            .id("transcript-viewer")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_8()
            .pb(px(64.0))
            .overflow_y_scroll()
            .track_scroll(&self.transcript_view.scroll)
            .on_scroll_wheel(cx.listener(move |this, _, _, _| {
                // `handleUserScroll`: a wheel while playing ends the follow.
                if playing {
                    this.transcript_view.user_scrolled = true;
                }
            }))
            .children(
                preview
                    .transcripts
                    .iter()
                    .enumerate()
                    .map(|(index, transcript)| {
                        let is_last = index + 1 == count;
                        let offset_ms = timeline_offset_ms(transcript.started_at_ms, &timeline);
                        let context = SegmentContext {
                            session_id: &preview.session.id,
                            transcript_id: &transcript.id,
                            offset_ms,
                            current_ms,
                            audio_exists,
                            edit_mode,
                        };
                        let segments: Vec<AnyElement> = transcript
                            .segments
                            .iter()
                            .map(|segment| {
                                self.render_transcript_segment(segment, &context, window, cx)
                            })
                            .collect();
                        div()
                            .flex()
                            .flex_col()
                            .gap_8()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    .children(segments),
                            )
                            .when(!is_last, |column| {
                                // `TranscriptSeparator`
                                let rule = || {
                                    div()
                                        .flex_1()
                                        .border_t_1()
                                        .border_color(alpha(theme.border, 0.4))
                                };
                                column.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .tw_text_xs()
                                        .font_weight(gpui::FontWeight::LIGHT)
                                        .text_color(theme.muted_foreground)
                                        .child(rule())
                                        .child("~ ~ ~ ~ ~ ~ ~ ~ ~")
                                        .child(rule()),
                                )
                            })
                    }),
            );

        // `relative flex h-full flex-col` around the scroller and the controls.
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(viewer)
            .children(self.render_transcript_scroll_controls(cx))
    }

    /// `canScroll && <div data-transcript-scroll-controls>`: the pill on the
    /// right edge (`top-1/2 right-1 -translate-y-1/2`) with the scroll-to-top
    /// and scroll-to-bottom buttons (`size-8`, `size-3.5` arrows) around a
    /// hairline, faint until hovered.
    fn render_transcript_scroll_controls(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let theme = self.theme;
        let handle = &self.transcript_view.scroll;
        let max = handle.max_offset().height;
        // `hasOverflow`: scrollHeight - clientHeight > 1.
        if max <= px(1.0) {
            return None;
        }
        let scroll_top = -handle.offset().y;
        let is_at_top = scroll_top <= px(1.0);
        let is_at_bottom = !is_at_top && max - scroll_top <= px(1.0);
        let hovered = self.transcript_view.controls_hovered;
        let content_bottom = f32::from(max);
        let icon_color = if hovered {
            theme.foreground
        } else {
            alpha(theme.muted_foreground, 0.45)
        };

        let button =
            |id: &'static str,
             glyph: &'static str,
             disabled: bool,
             on_click: fn(&mut Workspace, f32, &mut Context<Workspace>)| {
                div()
                    .id(id)
                    .flex()
                    .size(px(32.0))
                    .items_center()
                    .justify_center()
                    .when(disabled, |button| button.opacity(0.3))
                    .when(!disabled, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(alpha(theme.muted, 0.55)))
                            .active(move |style| style.bg(alpha(theme.muted, 0.7)))
                    })
                    // Always listening keeps the press state balanced when a
                    // click disables the button before its mouse-up.
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !disabled {
                            on_click(this, content_bottom, cx)
                        }
                    }))
                    .child(icon(glyph, px(14.0), icon_color))
            };

        Some(
            div()
                .id("transcript-scroll-controls")
                .absolute()
                .top(relative(0.5))
                .right(px(4.0))
                // `-translate-y-1/2` of the 32 + 1 + 32 (+ 2 border) pill.
                .mt(px(-33.5))
                .flex()
                .flex_col()
                .overflow_hidden()
                // `.rounded-full` is remapped to the 8px control squircle.
                .rounded(px(crate::squircle::CONTROL_RADIUS))
                .border_1()
                .border_color(gpui::transparent_black())
                .when(hovered, |pill| {
                    // `bg-background/65 backdrop-blur-md` over the card: the
                    // blur flattens to this colour, and opaque fill and
                    // border keep the `shadow-sm` from showing through.
                    let fill = crate::theme::over(alpha(theme.background, 0.65), theme.card);
                    pill.shadow(vec![
                        gpui::BoxShadow {
                            color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                            offset: gpui::point(px(0.0), px(1.0)),
                            blur_radius: px(3.0),
                            spread_radius: px(0.0),
                        },
                        gpui::BoxShadow {
                            color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                            offset: gpui::point(px(0.0), px(1.0)),
                            blur_radius: px(2.0),
                            spread_radius: px(-1.0),
                        },
                    ])
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(fill),
                        Some((1.0, crate::theme::over(alpha(theme.border, 0.5), fill))),
                    ))
                })
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    if this.transcript_view.controls_hovered != *hovered {
                        this.transcript_view.controls_hovered = *hovered;
                        cx.notify();
                    }
                }))
                .child(button(
                    "transcript-scroll-top",
                    "arrow-up",
                    is_at_top,
                    |this, _, cx| this.smooth_scroll_transcript(px(0.0), cx),
                ))
                .child(
                    div()
                        .h(px(1.0))
                        .w_full()
                        .bg(alpha(theme.border, if hovered { 0.6 } else { 0.2 })),
                )
                .child(button(
                    "transcript-scroll-bottom",
                    "arrow-down",
                    is_at_bottom,
                    |this, bottom, cx| {
                        // `scrollToBottom` re-enables the playback follow.
                        this.transcript_view.user_scrolled = false;
                        this.smooth_scroll_transcript(px(bottom), cx)
                    },
                )),
        )
    }

    /// `SegmentRenderer`: `section rounded-lg px-2` with the `py-1 text-xs
    /// font-light` speaker header (the assign trigger `-my-0.5 py-0.5 pr-2`
    /// in the segment colour) above `mt-1.5 text-sm leading-relaxed` words.
    /// Words are `WordSpan`s: seekable ones take `hover:bg-accent/60` and a
    /// click seeks the player; non-final ones are `opacity-60 italic`; the
    /// sentence line under the playhead gets `bg-yellow-100/50` (`-mx-0.5
    /// px-0.5 rounded-xs`).
    fn render_transcript_segment(
        &mut self,
        segment: &Segment,
        context: &SegmentContext<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SegmentContext {
            session_id,
            transcript_id,
            offset_ms,
            current_ms,
            audio_exists,
            edit_mode,
        } = *context;
        let theme = self.theme;
        let color = segment_color(&segment.key, theme.dark);
        let section_key = (transcript_id.to_string(), segment.id.to_string());
        let selected = self.transcript_section_selected(&section_key);
        let editor = edit_mode.then(|| self.segment_editor(transcript_id, segment, window, cx));
        let assign_open = self
            .transcript_view
            .speaker_assign
            .as_ref()
            .is_some_and(|open| open.segment_id() == Some(segment.id.as_str()));
        let layout = self
            .transcript_view
            .layouts
            .entry(segment.id.clone())
            .or_default()
            .clone();

        // `TextRun`s: the body style, non-final words at 60% and italic.
        let mut style = window.text_style();
        style.font_size = px(14.0).into();
        style.color = theme.foreground.into();
        if let Some(family) = &self.font_family {
            style.font_family = family.clone();
        }
        let mut runs: Vec<gpui::TextRun> = Vec::new();
        let mut cursor = 0;
        for word in &segment.words {
            if word.range.start > cursor {
                runs.push(style.to_run(word.range.start - cursor));
            }
            let mut word_style = style.clone();
            if !word.is_final {
                word_style.color = alpha(theme.foreground, 0.6).into();
                word_style.font_style = gpui::FontStyle::Italic;
            }
            runs.push(word_style.to_run(word.range.len()));
            cursor = word.range.end;
        }
        if segment.text.len() > cursor {
            runs.push(style.to_run(segment.text.len() - cursor));
        }

        let mut highlights = Vec::new();
        if audio_exists
            && let Some(line) = segment.active_line(offset_ms, current_ms)
            && let Some(range) = segment.line_range(line)
        {
            highlights.push(Highlight {
                range,
                color: if theme.dark {
                    alpha(gpui::rgb(0x733e0a), 0.3)
                } else {
                    alpha(gpui::rgb(0xfef9c2), 0.5)
                },
                inset_x: px(2.0),
                radius: px(2.0),
            });
        }
        let hovered_word = self
            .transcript_view
            .hover
            .as_ref()
            .filter(|(id, _)| *id == segment.id)
            .map(|(_, index)| *index)
            .and_then(|index| segment.words.get(index));
        if audio_exists && let Some(word) = hovered_word.filter(|word| word.seekable) {
            highlights.push(Highlight {
                range: word.range.clone(),
                color: alpha(theme.accent, 0.6),
                inset_x: px(0.0),
                radius: px(0.0),
            });
        }
        let hover_seekable = audio_exists && hovered_word.is_some_and(|word| word.seekable);

        let text = ProseText::with_layout(
            segment.text.clone(),
            runs,
            px(14.0),
            // `leading-relaxed` = 1.625 * 14 = 22.75, laid out as 22px by WebKit.
            px(22.0),
            layout.clone(),
        );
        let segment_id = segment.id.clone();
        let words_for_hover = segment.clone();
        let words_for_click = segment.clone();
        let hover_layout = layout.clone();
        let click_layout = layout;
        div()
            .id(SharedString::from(format!("segment-{}", segment.id)))
            .relative()
            .rounded_lg()
            .px_2()
            .when(edit_mode, |section| section.cursor_pointer())
            // `data-[transcript-selected=true]:bg-primary/10 ring-1 ring-inset
            // ring-primary/30`
            .when(selected, |section| {
                section.child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(alpha(theme.primary, 0.1)),
                    Some((1.0, alpha(theme.primary, 0.3))),
                ))
            })
            .on_click(cx.listener({
                let key = section_key.clone();
                move |this, event: &ClickEvent, _, cx| {
                    this.click_transcript_section(key.clone(), event.modifiers(), cx);
                }
            }))
            .child(
                div()
                    .relative()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(edit_mode, |header| {
                        header.child(self.render_selection_circle(selected))
                    })
                    .child(
                        // `SpeakerAssignPopover`'s trigger: `-my-0.5 rounded-full
                        // py-0.5 pr-2 hover:underline`, underlined while open. The
                        // popover anchors to the trigger's top-right through an
                        // unstyled wrapper so it inherits none of the label's text
                        // styling.
                        div()
                            .relative()
                            .my(px(-2.0))
                            .child(
                                div()
                                    .id(SharedString::from(format!("speaker-{}", segment.id)))
                                    .py(px(2.0))
                                    .pr_2()
                                    .rounded(px(8.0))
                                    .tw_text_xs()
                                    .font_weight(gpui::FontWeight::LIGHT)
                                    .text_color(color)
                                    .cursor_pointer()
                                    .when(assign_open, |label| {
                                        label.text_decoration_1().text_decoration_solid()
                                    })
                                    .hover(|style| {
                                        style.text_decoration_1().text_decoration_solid()
                                    })
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener({
                                        let segment = segment.clone();
                                        let session_id = session_id.to_string();
                                        let transcript_id = transcript_id.to_string();
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.toggle_speaker_assign(
                                                session_id.clone(),
                                                transcript_id.clone(),
                                                &segment,
                                                window,
                                                cx,
                                            );
                                        }
                                    }))
                                    .child(SharedString::from(segment.speaker_label.clone())),
                            )
                            .children(self.render_speaker_assign_popover(&segment.id, cx)),
                    ),
            )
            .when_some(editor, |section, editor| {
                // `EditableSegmentText`: `mt-1.5 rounded-md text-sm
                // leading-relaxed outline-hidden`; presses stay in the editor
                // rather than toggling the section.
                section.child(
                    div()
                        .mt(px(6.0))
                        .rounded(px(6.0))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(editor),
                )
            })
            .when(!edit_mode, |section| {
                section.child(
                    div()
                        .id(SharedString::from(format!("segment-words-{}", segment.id)))
                        .mt(px(6.0))
                        .text_size(px(14.0))
                        .line_height(px(22.0))
                        .text_color(theme.foreground)
                        .when(hover_seekable, |words| words.cursor_pointer())
                        .on_mouse_move(cx.listener(
                            move |this, event: &gpui::MouseMoveEvent, _, cx| {
                                let next = hover_layout
                                    .index_for_position(event.position)
                                    .ok()
                                    .and_then(|offset| words_for_hover.word_at(offset))
                                    .map(|index| (segment_id.clone(), index));
                                if this.transcript_view.hover != next {
                                    this.transcript_view.hover = next;
                                    cx.notify();
                                }
                            },
                        ))
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if !*hovered && this.transcript_view.hover.take().is_some() {
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                            // A modifier click selects the section instead.
                            let modifiers = event.modifiers();
                            if !audio_exists
                                || modifiers.platform
                                || modifiers.control
                                || modifiers.shift
                            {
                                return;
                            }
                            let Ok(offset) = click_layout.index_for_position(event.position())
                            else {
                                return;
                            };
                            let Some(word) = words_for_click
                                .word_at(offset)
                                .and_then(|index| words_for_click.words.get(index))
                            else {
                                return;
                            };
                            if word.seekable {
                                this.seek_and_play(offset_ms + word.start_ms, cx);
                            }
                        }))
                        .child(HighlightedProseText::new(text, highlights)),
                )
            })
            .into_any_element()
    }
}
