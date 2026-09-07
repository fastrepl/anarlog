//! Transcript text selection (`renderer/selection.ts`,
//! `renderer/selection-menu.tsx`): dragging across the words selects a
//! range that is highlighted with `--selection-overlay` and opens the
//! floating `Change speaker / Play from here / Copy` menu below it; a
//! right-click opens the same menu at the pointer for the selection under
//! it, or for the whole segment.

use gpui::{
    AnyElement, ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, Window, div,
    prelude::*, px,
};

use super::Workspace;
use super::speaker_assign::{AssignTarget, SelectionGroup};
use crate::db::NotePreview;
use crate::theme::alpha;
use crate::transcript::{Segment, timeline_offset_ms};
use crate::ui::{TailwindText as _, icon};

/// A caret in the viewer: the segment and the byte offset in its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Caret {
    pub segment_id: String,
    pub offset: usize,
}

/// Where the floating menu anchors.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MenuAnchor {
    /// `placement: "bottom"` under the selection's bounding box.
    Below(gpui::Bounds<Pixels>),
    /// `placement: "bottom-start"` at the right-click point.
    Point(Point<Pixels>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuView {
    Actions,
    Speaker,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TextSelection {
    pub anchor: Caret,
    pub head: Caret,
    /// The mouse is still down.
    pub dragging: bool,
    pub menu: Option<(MenuAnchor, MenuView)>,
}

/// `TranscriptWordSelection` resolved from the carets.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResolvedSelection {
    pub text: String,
    pub start_ms: i64,
    pub groups: Vec<SelectionGroup>,
}

/// `--selection-overlay`
fn selection_overlay() -> gpui::Rgba {
    alpha(gpui::rgb(0x3b82f6), 0.3)
}

/// The viewer's segments in order with their transcript id and offset.
fn ordered_segments(preview: &NotePreview) -> Vec<(&str, i64, &Segment)> {
    let timeline: Vec<(i64, bool)> = preview
        .transcripts
        .iter()
        .map(|t| (t.started_at_ms, true))
        .collect();
    preview
        .transcripts
        .iter()
        .flat_map(|transcript| {
            let offset_ms = timeline_offset_ms(transcript.started_at_ms, &timeline);
            transcript
                .segments
                .iter()
                .map(move |segment| (transcript.id.as_str(), offset_ms, segment))
        })
        .collect()
}

/// The byte range of `segment` (at `index` in the order) covered by the
/// selection from `start` to `end` (already ordered), if any.
pub(super) fn segment_range(
    order: &[&str],
    index: usize,
    segment_len: usize,
    start: &Caret,
    end: &Caret,
) -> Option<std::ops::Range<usize>> {
    let start_index = order.iter().position(|id| *id == start.segment_id)?;
    let end_index = order.iter().position(|id| *id == end.segment_id)?;
    if index < start_index || index > end_index {
        return None;
    }
    let from = if index == start_index {
        start.offset
    } else {
        0
    };
    let to = if index == end_index {
        end.offset
    } else {
        segment_len
    };
    (from < to).then_some(from.min(segment_len)..to.min(segment_len))
}

impl TextSelection {
    /// The carets in document order.
    fn ordered<'a>(&'a self, order: &[&str]) -> (&'a Caret, &'a Caret) {
        let position = |caret: &Caret| {
            order
                .iter()
                .position(|id| *id == caret.segment_id)
                .map(|index| (index, caret.offset))
        };
        match (position(&self.anchor), position(&self.head)) {
            (Some(a), Some(h)) if h < a => (&self.head, &self.anchor),
            _ => (&self.anchor, &self.head),
        }
    }

    fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
}

impl Workspace {
    /// The selection's range inside `segment`, for the highlight.
    pub(super) fn text_selection_range(
        &self,
        preview: &NotePreview,
        segment_id: &str,
    ) -> Option<std::ops::Range<usize>> {
        let selection = self.transcript_view.text_selection.as_ref()?;
        if selection.is_collapsed() {
            return None;
        }
        let segments = ordered_segments(preview);
        let order: Vec<&str> = segments.iter().map(|(_, _, s)| s.id.as_str()).collect();
        let index = order.iter().position(|id| *id == segment_id)?;
        let (start, end) = selection.ordered(&order);
        segment_range(&order, index, segments[index].2.text.len(), start, end)
    }

    /// `getTranscriptSelectionFromRange`: the words the range touches grouped
    /// per transcript, the range's text (a DOM range's `toString()` runs
    /// through the speaker labels between segments), and the first word's
    /// timeline start.
    pub(super) fn resolve_text_selection(
        &self,
        preview: &NotePreview,
    ) -> Option<ResolvedSelection> {
        let selection = self.transcript_view.text_selection.as_ref()?;
        if selection.is_collapsed() {
            return None;
        }
        let segments = ordered_segments(preview);
        let order: Vec<&str> = segments.iter().map(|(_, _, s)| s.id.as_str()).collect();
        let (start, end) = selection.ordered(&order);
        let mut groups: Vec<SelectionGroup> = Vec::new();
        let mut text = String::new();
        let mut start_ms: Option<i64> = None;
        let mut first = true;
        for (index, (transcript_id, offset_ms, segment)) in segments.iter().enumerate() {
            let Some(range) = segment_range(&order, index, segment.text.len(), start, end) else {
                continue;
            };
            if !first {
                text.push_str(&segment.speaker_label);
            }
            first = false;
            text.push_str(&segment.text[range.clone()]);
            for word in &segment.words {
                if word.range.start >= range.end || word.range.end <= range.start {
                    continue;
                }
                let Some(id) = word.id.clone().filter(|id| !id.is_empty()) else {
                    continue;
                };
                start_ms.get_or_insert(offset_ms + word.start_ms);
                match groups
                    .iter_mut()
                    .find(|group| group.transcript_id == *transcript_id)
                {
                    Some(group) => {
                        if !group.word_ids.contains(&id) {
                            group.word_ids.push(id);
                        }
                    }
                    None => groups.push(SelectionGroup {
                        transcript_id: transcript_id.to_string(),
                        segment_key: segment.key.clone(),
                        word_ids: vec![id],
                    }),
                }
            }
        }
        if groups.is_empty() {
            return None;
        }
        Some(ResolvedSelection {
            text: text.trim().to_string(),
            start_ms: start_ms.unwrap_or(0),
            groups,
        })
    }

    /// The selection's bounding box over the laid-out lines.
    fn text_selection_bounds(&self, preview: &NotePreview) -> Option<gpui::Bounds<Pixels>> {
        let selection = self.transcript_view.text_selection.as_ref()?;
        let segments = ordered_segments(preview);
        let order: Vec<&str> = segments.iter().map(|(_, _, s)| s.id.as_str()).collect();
        let (start, end) = selection.ordered(&order);
        let mut union: Option<gpui::Bounds<Pixels>> = None;
        for (index, (_, _, segment)) in segments.iter().enumerate() {
            let Some(range) = segment_range(&order, index, segment.text.len(), start, end) else {
                continue;
            };
            let Some(layout) = self.transcript_view.layouts.get(&segment.id) else {
                continue;
            };
            for span in layout.line_spans(range) {
                union = Some(match union {
                    Some(bounds) => bounds.union(&span),
                    None => span,
                });
            }
        }
        union
    }

    /// The caret nearest to a window position over the viewer, by the
    /// segment layout whose vertical band the point falls in (or the nearest
    /// one), like a browser drag across block boundaries.
    fn caret_at(&self, preview: &NotePreview, position: Point<Pixels>) -> Option<Caret> {
        let mut best: Option<(Pixels, Caret)> = None;
        for (_, _, segment) in ordered_segments(preview) {
            let Some(layout) = self.transcript_view.layouts.get(&segment.id) else {
                continue;
            };
            let Some(bounds) = layout.bounds() else {
                continue;
            };
            let distance = if position.y < bounds.top() {
                bounds.top() - position.y
            } else if position.y > bounds.bottom() {
                position.y - bounds.bottom()
            } else {
                px(0.0)
            };
            if best.as_ref().is_none_or(|(d, _)| distance < *d)
                && let Some(offset) = layout.nearest_index(position)
            {
                best = Some((
                    distance,
                    Caret {
                        segment_id: segment.id.clone(),
                        offset,
                    },
                ));
                if distance == px(0.0) {
                    break;
                }
            }
        }
        best.map(|(_, caret)| caret)
    }

    pub(super) fn clear_text_selection(&mut self, cx: &mut Context<Self>) -> bool {
        if self.transcript_view.text_selection.take().is_some() {
            cx.notify();
            return true;
        }
        false
    }

    /// A left press on a segment's words starts a drag selection there.
    pub(super) fn begin_text_selection(&mut self, caret: Caret, cx: &mut Context<Self>) {
        let for_text_menu = self
            .transcript_view
            .speaker_assign
            .as_ref()
            .is_some_and(|open| open.for_selection())
            && self
                .transcript_view
                .text_selection
                .as_ref()
                .is_some_and(|selection| selection.menu.is_some());
        if for_text_menu {
            self.close_speaker_assign(cx);
        }
        self.transcript_view.text_selection = Some(TextSelection {
            anchor: caret.clone(),
            head: caret,
            dragging: true,
            menu: None,
        });
        cx.notify();
    }

    /// The viewer's mouse move while dragging extends the head.
    pub(super) fn drag_text_selection(
        &mut self,
        preview: &NotePreview,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .transcript_view
            .text_selection
            .as_ref()
            .is_some_and(|selection| selection.dragging)
        {
            return;
        }
        let Some(caret) = self.caret_at(preview, position) else {
            return;
        };
        let selection = self.transcript_view.text_selection.as_mut().unwrap();
        if selection.head == caret {
            return;
        }
        selection.head = caret;
        // `selectionchange` shows the menu under the growing range at once.
        let bounds = self
            .resolve_text_selection(preview)
            .and_then(|_| self.text_selection_bounds(preview));
        if let Some(selection) = self.transcript_view.text_selection.as_mut() {
            selection.menu = bounds.map(|bounds| (MenuAnchor::Below(bounds), MenuView::Actions));
        }
        cx.notify();
    }

    /// Mouse up ends the drag: a collapsed range is no selection, a real one
    /// shows the menu under it (`useSelectionMenuState`).
    pub(super) fn end_text_selection(&mut self, preview: &NotePreview, cx: &mut Context<Self>) {
        let Some(selection) = self.transcript_view.text_selection.as_mut() else {
            return;
        };
        if !selection.dragging {
            return;
        }
        selection.dragging = false;
        if selection.is_collapsed() || self.resolve_text_selection(preview).is_none() {
            self.transcript_view.text_selection = None;
            cx.notify();
            return;
        }
        let bounds = self.text_selection_bounds(preview);
        if let Some(selection) = self.transcript_view.text_selection.as_mut() {
            selection.menu = bounds.map(|bounds| (MenuAnchor::Below(bounds), MenuView::Actions));
        }
        cx.notify();
    }

    /// `handleContextMenu` → `getTranscriptContextSelection`: the selection
    /// under the pointer, else the whole segment, with the menu at the point.
    pub(super) fn open_text_context_menu(
        &mut self,
        preview: &NotePreview,
        segment: &Segment,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let covers = self
            .text_selection_range(preview, &segment.id)
            .zip(self.caret_at(preview, position))
            .is_some_and(|(range, caret)| {
                caret.segment_id == segment.id
                    && range.start <= caret.offset
                    && caret.offset <= range.end
            });
        if !covers {
            self.transcript_view.text_selection = Some(TextSelection {
                anchor: Caret {
                    segment_id: segment.id.clone(),
                    offset: 0,
                },
                head: Caret {
                    segment_id: segment.id.clone(),
                    offset: segment.text.len(),
                },
                dragging: false,
                menu: None,
            });
        }
        if let Some(selection) = self.transcript_view.text_selection.as_mut() {
            selection.dragging = false;
            selection.menu = Some((MenuAnchor::Point(position), MenuView::Actions));
        }
        cx.notify();
    }

    fn close_text_selection_menu(&mut self, cx: &mut Context<Self>) {
        // `handleClose`: hide and drop the native selection too.
        self.transcript_view.text_selection = None;
        if self
            .transcript_view
            .speaker_assign
            .as_ref()
            .is_some_and(|open| open.for_selection())
        {
            self.transcript_view.speaker_assign = None;
        }
        cx.notify();
    }

    /// `SelectionFloatingMenu`: `bg-card shadow-lg rounded-md border p-1`,
    /// `min-w-40` for the actions or `w-80` for the speaker picker.
    pub(super) fn render_text_selection_menu(
        &self,
        preview: &NotePreview,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let selection = self.transcript_view.text_selection.as_ref()?;
        let (anchor, view) = selection.menu.clone()?;
        let resolved = self.resolve_text_selection(preview)?;
        let theme = self.theme;
        let audio_exists = preview.audio_exists;

        let button = |id: &'static str, glyph: AnyElement, label: &'static str| {
            div()
                .id(id)
                .flex()
                .w_full()
                .items_center()
                .gap_2()
                .px_2()
                .py(px(6.0))
                .rounded(px(2.0))
                .tw_text_xs()
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .child(glyph)
                .child(label)
        };

        let content: AnyElement = match view {
            MenuView::Actions => {
                let play_ms = resolved.start_ms;
                let copy_text = resolved.text.clone();
                let groups = resolved.groups.clone();
                let session_id = preview.session.id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        button(
                            "text-selection-change-speaker",
                            icon("user-switch", px(14.0), theme.foreground).into_any_element(),
                            "Change speaker",
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, window, cx| {
                                if let Some(selection) =
                                    this.transcript_view.text_selection.as_mut()
                                    && let Some((_, view)) = selection.menu.as_mut()
                                {
                                    *view = MenuView::Speaker;
                                }
                                this.open_speaker_picker(
                                    session_id.clone(),
                                    AssignTarget::Selection {
                                        groups: groups.clone(),
                                    },
                                    false,
                                    window,
                                    cx,
                                );
                            },
                        )),
                    )
                    .when(audio_exists, |menu| {
                        menu.child(
                            button(
                                "text-selection-play",
                                icon("play", px(14.0), theme.foreground).into_any_element(),
                                "Play from here",
                            )
                            .on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.seek_and_play(play_ms, cx);
                                    this.close_text_selection_menu(cx);
                                },
                            )),
                        )
                    })
                    .child(
                        button(
                            "text-selection-copy",
                            div()
                                .w(px(14.0))
                                .text_center()
                                .child("⌘")
                                .into_any_element(),
                            "Copy",
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    copy_text.clone(),
                                ));
                                this.close_text_selection_menu(cx);
                            },
                        )),
                    )
                    .into_any_element()
            }
            MenuView::Speaker => match self.transcript_view.speaker_assign.as_ref() {
                Some(open) if open.for_selection() => {
                    self.render_speaker_picker_body(open, cx).into_any_element()
                }
                _ => div().into_any_element(),
            },
        };

        let width = match view {
            MenuView::Actions => 160.0,
            MenuView::Speaker => 320.0,
        };
        let menu = div()
            .id("text-selection-menu")
            .occlude()
            .min_w(px(width))
            .when(view == MenuView::Speaker, |menu| {
                menu.w(px(width)).max_h(px(448.0))
            })
            .p_1()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .shadow(vec![
                gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(10.0)),
                    blur_radius: px(15.0),
                    spread_radius: px(-3.0),
                },
                gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(4.0)),
                    blur_radius: px(6.0),
                    spread_radius: px(-4.0),
                },
            ])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.close_text_selection_menu(cx);
            }))
            .child(content);

        // `offset(6)` / `offset(4)` with `flip()` and `shift({ padding: 8 })`.
        let viewport = window.viewport_size();
        let (origin, corner) = match anchor {
            MenuAnchor::Below(bounds) => {
                let x = bounds.left() + bounds.size.width / 2.0 - px(width / 2.0);
                let below = bounds.bottom() + px(6.0);
                if f32::from(viewport.height - below) < 160.0 {
                    (
                        Point::new(x, bounds.top() - px(6.0)),
                        gpui::Corner::BottomLeft,
                    )
                } else {
                    (Point::new(x, below), gpui::Corner::TopLeft)
                }
            }
            MenuAnchor::Point(point) => {
                let below = point.y + px(4.0);
                if f32::from(viewport.height - below) < 160.0 {
                    (
                        Point::new(point.x, point.y - px(4.0)),
                        gpui::Corner::BottomLeft,
                    )
                } else {
                    (Point::new(point.x, below), gpui::Corner::TopLeft)
                }
            }
        };
        Some(
            gpui::deferred(
                gpui::anchored()
                    .position(origin)
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.0))
                    .child(menu),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    /// `SelectionHighlight`: every segment's selected range, keyed by segment
    /// id, for the highlights of one frame.
    pub(super) fn text_selection_ranges(
        &self,
        preview: &NotePreview,
    ) -> std::collections::HashMap<String, std::ops::Range<usize>> {
        let mut ranges = std::collections::HashMap::new();
        let Some(selection) = self.transcript_view.text_selection.as_ref() else {
            return ranges;
        };
        if selection.is_collapsed() {
            return ranges;
        }
        let segments = ordered_segments(preview);
        let order: Vec<&str> = segments.iter().map(|(_, _, s)| s.id.as_str()).collect();
        let (start, end) = selection.ordered(&order);
        for (index, (_, _, segment)) in segments.iter().enumerate() {
            if let Some(range) = segment_range(&order, index, segment.text.len(), start, end) {
                ranges.insert(segment.id.clone(), range);
            }
        }
        ranges
    }
}

/// The `--selection-overlay` highlight for `range`.
pub(super) fn selection_highlight(range: std::ops::Range<usize>) -> crate::prose_text::Highlight {
    crate::prose_text::Highlight {
        range,
        color: selection_overlay(),
        inset_x: px(0.0),
        radius: px(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret(segment: &str, offset: usize) -> Caret {
        Caret {
            segment_id: segment.into(),
            offset,
        }
    }

    #[test]
    fn segment_ranges_follow_the_carets_across_segments() {
        let order = ["a", "b", "c", "d"];
        let start = caret("b", 3);
        let end = caret("c", 5);
        assert_eq!(segment_range(&order, 0, 10, &start, &end), None);
        assert_eq!(segment_range(&order, 1, 10, &start, &end), Some(3..10));
        assert_eq!(segment_range(&order, 2, 10, &start, &end), Some(0..5));
        assert_eq!(segment_range(&order, 3, 10, &start, &end), None);
        // Within one segment, and an empty range.
        assert_eq!(
            segment_range(&order, 1, 10, &caret("b", 2), &caret("b", 6)),
            Some(2..6)
        );
        assert_eq!(
            segment_range(&order, 1, 10, &caret("b", 4), &caret("b", 4)),
            None
        );
    }

    #[test]
    fn carets_order_by_segment_then_offset() {
        let order = ["a", "b"];
        let selection = TextSelection {
            anchor: caret("b", 1),
            head: caret("a", 7),
            dragging: false,
            menu: None,
        };
        let (start, end) = selection.ordered(&order);
        assert_eq!((start, end), (&caret("a", 7), &caret("b", 1)));
        let same = TextSelection {
            anchor: caret("a", 9),
            head: caret("a", 2),
            dragging: false,
            menu: None,
        };
        let (start, end) = same.ordered(&order);
        assert_eq!((start.offset, end.offset), (2, 9));
    }
}
