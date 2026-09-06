//! Transcript edit mode (`editingTranscriptSessionId` in `session/index.tsx`):
//! the pencil on the Transcript pill turns every segment into an
//! `EditableSegmentText` and switches the viewer to select mode, whose
//! selected sections drive the `MultiSelectionBar` (`renderer/index.tsx`,
//! `renderer/selection.ts`, `renderer/selection-menu.tsx`).

use std::collections::HashMap;

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, SharedString, Window,
    div, prelude::*, px, relative,
};

use super::Workspace;
use super::speaker_assign::{AssignTarget, SelectionGroup};
use crate::db::NotePreview;
use crate::text_area::{TextArea, TextAreaEvent, TextAreaStyle};
use crate::theme::alpha;
use crate::transcript::{Segment, timeline_offset_ms};
use crate::ui::{TailwindText as _, icon};

/// `getTranscriptSectionKey`: `(transcript id, segment id)`.
pub(super) type SectionKey = (String, String);

/// One segment's `EditableSegmentText`.
pub(super) struct SegmentEditor {
    pub area: Entity<TextArea>,
    /// `originalText`, what Escape restores and what a save compares to.
    original: String,
    transcript_id: String,
    word_ids: Vec<String>,
}

/// `TranscriptWordSelection` for one section.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SectionSelection {
    pub text: String,
    pub start_ms: i64,
    pub group: SelectionGroup,
}

#[derive(Default)]
pub(super) struct TranscriptEdit {
    /// `editingTranscriptSessionId`
    pub session_id: Option<String>,
    /// `selectedEntries`, in selection order.
    pub selected: Vec<SectionKey>,
    /// `selectionAnchor` for Shift-click ranges.
    pub anchor: Option<SectionKey>,
    /// The editors of the segments shown in edit mode, by segment id.
    pub editors: HashMap<String, SegmentEditor>,
    /// The bar's `Change speaker` / `Merge` buttons being hovered.
    pub change_speaker_hovered: bool,
}

/// `normalizeEditableTranscriptText`
pub fn normalize_editable_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `getTranscriptMergeTarget`: at least two selected sections, contiguous in
/// `order`, all in one transcript and channel, the first with a speaker
/// identity. Returns the first section's selection.
pub(super) fn merge_target<'a>(
    selected: &[SectionKey],
    order: &[SectionKey],
    entries: &'a HashMap<SectionKey, SectionSelection>,
) -> Option<&'a SectionSelection> {
    if selected.len() < 2 {
        return None;
    }
    let indexes: Vec<usize> = order
        .iter()
        .enumerate()
        .filter(|(_, key)| selected.contains(key))
        .map(|(index, _)| index)
        .collect();
    if indexes.len() != selected.len() {
        return None;
    }
    if indexes.windows(2).any(|pair| pair[1] != pair[0] + 1) {
        return None;
    }
    let first = entries.get(&order[indexes[0]])?;
    let target = &first.group;
    if target.segment_key.speaker_human_id.is_none() && target.segment_key.speaker_index.is_none() {
        return None;
    }
    for index in &indexes {
        let group = &entries.get(&order[*index])?.group;
        if group.transcript_id != target.transcript_id
            || group.segment_key.channel != target.segment_key.channel
        {
            return None;
        }
    }
    Some(first)
}

/// `mergeTranscriptSelections`: one group per transcript with every selected
/// word id, the texts joined by blank lines, the earliest start.
pub(super) fn merge_selections(
    selections: &[&SectionSelection],
) -> Option<(String, i64, Vec<SelectionGroup>)> {
    if selections.is_empty() {
        return None;
    }
    let mut groups: Vec<SelectionGroup> = Vec::new();
    for selection in selections {
        let next = &selection.group;
        match groups
            .iter_mut()
            .find(|group| group.transcript_id == next.transcript_id)
        {
            Some(group) => {
                for word_id in &next.word_ids {
                    if !group.word_ids.contains(word_id) {
                        group.word_ids.push(word_id.clone());
                    }
                }
            }
            None => groups.push(next.clone()),
        }
    }
    let text = selections
        .iter()
        .map(|selection| selection.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let start_ms = selections
        .iter()
        .map(|selection| selection.start_ms)
        .min()
        .unwrap_or(0);
    Some((text, start_ms, groups))
}

impl Workspace {
    /// `transcriptEditMode`: the open note is the session being edited.
    pub(super) fn transcript_edit_mode(&self, session_id: &str) -> bool {
        self.transcript_view.edit.session_id.as_deref() == Some(session_id)
    }

    /// `canEdit`: an inactive session with a stored transcript whose event,
    /// if any, has ended.
    pub(super) fn can_edit_transcript(&self, preview: &NotePreview) -> bool {
        if !preview.has_transcript
            || self.session_mode(&preview.session.id) != super::recording::SessionMode::Inactive
        {
            return false;
        }
        preview.session_event().is_none_or(|event| {
            event.ended_at.as_deref().is_some_and(|ended| {
                crate::timeline::parse_date(ended, &chrono::Local)
                    .is_some_and(|ended| ended <= chrono::Utc::now())
            })
        })
    }

    /// `handleTranscriptEditModeChange`: leaving edit mode saves the focused
    /// editor and drops the selection.
    pub(super) fn set_transcript_edit_mode(
        &mut self,
        session_id: &str,
        edit_mode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !edit_mode {
            self.blur_transcript_editors(window, cx);
            self.transcript_view.edit.session_id = None;
            self.transcript_view.edit.editors.clear();
        } else {
            self.transcript_view.edit.session_id = Some(session_id.to_string());
        }
        self.clear_transcript_selection(cx);
        cx.notify();
    }

    /// `blurActiveTranscriptEditor`: commit the focused editor by moving
    /// focus back to the shell (its `Blurred` handler saves).
    fn blur_transcript_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self
            .transcript_view
            .edit
            .editors
            .values()
            .any(|editor| editor.area.read(cx).focus_handle(cx).is_focused(window));
        if focused {
            self.focus_handle.focus(window);
        }
    }

    pub(super) fn clear_transcript_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let edit = &mut self.transcript_view.edit;
        if edit.selected.is_empty() && edit.anchor.is_none() {
            return false;
        }
        edit.selected.clear();
        edit.anchor = None;
        cx.notify();
        true
    }

    /// The viewer's sections in order with their selections.
    fn transcript_sections(
        &self,
        preview: &NotePreview,
    ) -> (Vec<SectionKey>, HashMap<SectionKey, SectionSelection>) {
        let timeline: Vec<(i64, bool)> = preview
            .transcripts
            .iter()
            .map(|t| (t.started_at_ms, true))
            .collect();
        let mut order = Vec::new();
        let mut entries = HashMap::new();
        for transcript in &preview.transcripts {
            let offset_ms = timeline_offset_ms(transcript.started_at_ms, &timeline);
            for segment in &transcript.segments {
                let key = (transcript.id.clone(), segment.id.clone());
                if let Some(selection) = section_selection(&transcript.id, offset_ms, segment) {
                    entries.insert(key.clone(), selection);
                }
                order.push(key);
            }
        }
        (order, entries)
    }

    /// `handleSegmentSelection`: in select mode (or with Cmd/Ctrl/Shift held)
    /// a section click toggles it, Shift extending from the anchor; a plain
    /// click outside select mode drops the selection.
    pub(super) fn click_transcript_section(
        &mut self,
        key: SectionKey,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let select_mode = matches!(
            &self.note,
            super::Note::Ready { preview, .. } if self.transcript_edit_mode(&preview.session.id)
        );
        let has_modifier = modifiers.platform || modifiers.control || modifiers.shift;
        if !select_mode && !has_modifier {
            self.clear_transcript_selection(cx);
            return;
        }
        let super::Note::Ready { preview, .. } = &self.note else {
            return;
        };
        let (order, entries) = self.transcript_sections(preview);
        let edit = &mut self.transcript_view.edit;
        if modifiers.shift
            && let Some(anchor) = &edit.anchor
            && let (Some(anchor_index), Some(target_index)) = (
                order.iter().position(|k| k == anchor),
                order.iter().position(|k| k == &key),
            )
        {
            let (start, end) = (
                anchor_index.min(target_index),
                anchor_index.max(target_index),
            );
            for section in &order[start..=end] {
                if entries.contains_key(section) && !edit.selected.contains(section) {
                    edit.selected.push(section.clone());
                }
            }
        } else if let Some(index) = edit.selected.iter().position(|k| *k == key) {
            edit.selected.remove(index);
        } else if entries.contains_key(&key) {
            edit.selected.push(key.clone());
        }
        edit.anchor = Some(key);
        cx.notify();
    }

    pub(super) fn transcript_section_selected(&self, key: &SectionKey) -> bool {
        self.transcript_view.edit.selected.contains(key)
    }

    /// The editor for `segment`, created on first render in edit mode with
    /// the normalised text; its `Blurred` saves edits, Escape restores and
    /// blurs, Cmd/Ctrl+Enter blurs.
    pub(super) fn segment_editor(
        &mut self,
        transcript_id: &str,
        segment: &Segment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextArea> {
        let original = normalize_editable_text(&segment.text);
        if let Some(editor) = self.transcript_view.edit.editors.get_mut(&segment.id) {
            // A save (or another writer) changed the stored words: follow
            // them unless the user is typing.
            if editor.original != original {
                editor.original = original.clone();
                if !editor.area.read(cx).focus_handle(cx).is_focused(window) {
                    editor
                        .area
                        .update(cx, |area, cx| area.set_text(original, cx));
                }
            }
            return editor.area.clone();
        }
        let theme = self.theme;
        let style = TextAreaStyle {
            text: theme.foreground,
            placeholder: theme.muted_foreground,
            selection: theme.selection,
            font_size: px(14.0),
            // `leading-relaxed` = 1.625 * 14 = 22.75, laid out as 22px by WebKit.
            line_height: px(22.0),
            rows: 1,
        };
        let area = cx.new(|cx| {
            let mut area = TextArea::new("", style, window, cx);
            area.set_text(original.clone(), cx);
            area
        });
        let segment_id = segment.id.clone();
        cx.subscribe_in(
            &area,
            window,
            move |this, area, event: &TextAreaEvent, window, cx| match event {
                TextAreaEvent::Blurred => this.save_segment_editor(&segment_id, cx),
                TextAreaEvent::Escape => {
                    if let Some(editor) = this.transcript_view.edit.editors.get(&segment_id) {
                        let original = editor.original.clone();
                        area.update(cx, |area, cx| area.set_text(original, cx));
                    }
                    this.focus_handle.focus(window);
                }
                TextAreaEvent::Submit => this.focus_handle.focus(window),
                TextAreaEvent::Changed => {}
            },
        )
        .detach();
        self.transcript_view.edit.editors.insert(
            segment.id.clone(),
            SegmentEditor {
                area: area.clone(),
                original,
                transcript_id: transcript_id.to_string(),
                word_ids: segment
                    .words
                    .iter()
                    .filter_map(|word| word.id.clone().filter(|id| !id.is_empty()))
                    .collect(),
            },
        );
        area
    }

    /// `handleBlur`: `updateTranscriptSegmentText` when the normalised text
    /// changed and the segment has persisted word ids.
    fn save_segment_editor(&mut self, segment_id: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.transcript_view.edit.editors.get_mut(segment_id) else {
            return;
        };
        let next = normalize_editable_text(editor.area.read(cx).text());
        if next == editor.original || editor.word_ids.is_empty() {
            return;
        }
        editor.original = next.clone();
        let task = self.store.update_transcript_segment_text(
            editor.transcript_id.clone(),
            editor.word_ids.clone(),
            next,
        );
        let session_id = self.selected.clone();
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                tracing::error!(%error, "[transcript] failed to update text");
                return;
            }
            this.update(cx, |this, cx| {
                if let Some(session_id) = session_id
                    && this.selected.as_deref() == Some(session_id.as_str())
                {
                    this.reload_note(session_id, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `handleMergeSegments`: pull every selected word of the target's
    /// transcript into the first selected section's speaker.
    fn merge_transcript_selection(&mut self, cx: &mut Context<Self>) {
        let super::Note::Ready { preview, .. } = &self.note else {
            return;
        };
        let (order, entries) = self.transcript_sections(preview);
        let selected = self.transcript_view.edit.selected.clone();
        let Some(target) = merge_target(&selected, &order, &entries) else {
            return;
        };
        let target_group = target.group.clone();
        let selections: Vec<&SectionSelection> = order
            .iter()
            .filter(|key| selected.contains(key))
            .filter_map(|key| entries.get(key))
            .collect();
        let Some((_, _, groups)) = merge_selections(&selections) else {
            return;
        };
        let session_id = preview.session.id.clone();
        let store = self.store.clone();
        let tasks: Vec<_> = groups
            .into_iter()
            .filter(|group| group.transcript_id == target_group.transcript_id)
            .map(|group| {
                store.merge_transcript_segments(
                    group.transcript_id,
                    target_group.segment_key.clone(),
                    group.word_ids,
                )
            })
            .collect();
        self.clear_transcript_selection(cx);
        cx.spawn(async move |this, cx| {
            for task in tasks {
                if let Err(error) = task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                    tracing::error!(%error, "[transcript] failed to merge segments");
                }
            }
            this.update(cx, |this, cx| {
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.reload_note(session_id, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The bar's `Change speaker`: the picker over the selection's groups.
    fn open_selection_speaker_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .transcript_view
            .speaker_assign
            .as_ref()
            .is_some_and(|open| open.for_selection())
        {
            self.close_speaker_assign(cx);
            return;
        }
        let super::Note::Ready { preview, .. } = &self.note else {
            return;
        };
        let (order, entries) = self.transcript_sections(preview);
        let selected = &self.transcript_view.edit.selected;
        let selections: Vec<&SectionSelection> = order
            .iter()
            .filter(|key| selected.contains(key))
            .filter_map(|key| entries.get(key))
            .collect();
        let Some((_, _, groups)) = merge_selections(&selections) else {
            return;
        };
        let session_id = preview.session.id.clone();
        self.open_speaker_picker(
            session_id,
            AssignTarget::Selection { groups },
            false,
            window,
            cx,
        );
    }

    /// `SegmentHeader`'s select-mode circle: `size-4 rounded-full border`,
    /// filled `bg-primary` with the bold `size-2.5` check when selected.
    pub(super) fn render_selection_circle(&self, selected: bool) -> AnyElement {
        let theme = self.theme;
        div()
            .relative()
            .flex()
            .size(px(16.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                selected.then_some(theme.primary),
                Some((
                    1.0,
                    if selected {
                        theme.primary
                    } else {
                        alpha(theme.muted_foreground, 0.4)
                    },
                )),
            ))
            .when(selected, |circle| {
                circle.child(icon("check", px(10.0), theme.primary_foreground))
            })
            .into_any_element()
    }

    /// `MultiSelectionBar`, portaled into the session FAB's selection slot:
    /// centred above the chat CTA (`mb-2`, sitting `translate-y-8` low until
    /// the CTA is hovered) with the count, `Change speaker`, `Merge`, and the
    /// clear button.
    pub(super) fn render_transcript_selection_bar(
        &self,
        preview: &NotePreview,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let selected = &self.transcript_view.edit.selected;
        if selected.is_empty() {
            return None;
        }
        let theme = self.theme;
        let (order, entries) = self.transcript_sections(preview);
        let can_merge = merge_target(selected, &order, &entries).is_some();
        let count = selected.len();
        let change_hovered = self.transcript_view.edit.change_speaker_hovered;

        let bar = div()
            .id("transcript-selection-bar")
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .p_1()
            .pl_3()
            .tw_text_xs()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(theme.card),
                Some((1.0, theme.border)),
            ))
            .child(
                div()
                    .relative()
                    .text_color(theme.muted_foreground)
                    .whitespace_nowrap()
                    .child(SharedString::from(format!("{count} selected"))),
            )
            .child(
                // `bg-primary text-primary-foreground hover:bg-primary/90 flex
                // h-7 items-center gap-1.5 rounded-full px-3 font-medium`
                div()
                    .id("transcript-change-speaker")
                    .relative()
                    .flex()
                    .h(px(28.0))
                    .items_center()
                    .gap(px(6.0))
                    .px_3()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.primary_foreground)
                    .cursor_pointer()
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if this.transcript_view.edit.change_speaker_hovered != *hovered {
                            this.transcript_view.edit.change_speaker_hovered = *hovered;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.open_selection_speaker_picker(window, cx);
                    }))
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(if change_hovered {
                            alpha(theme.primary, 0.9)
                        } else {
                            theme.primary
                        }),
                        None,
                    ))
                    .child(div().relative().child(icon(
                        "user-switch",
                        px(14.0),
                        theme.primary_foreground,
                    )))
                    .child(div().relative().child("Change speaker"))
                    .children(self.render_selection_speaker_popover(cx)),
            )
            .child(
                // `hover:bg-accent flex h-7 items-center gap-1.5 rounded-full
                // px-2 font-medium disabled:opacity-50`
                div()
                    .id("transcript-merge")
                    .flex()
                    .h(px(28.0))
                    .items_center()
                    .gap(px(6.0))
                    .px_2()
                    .rounded(px(crate::squircle::CONTROL_RADIUS))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .when(!can_merge, |button| button.opacity(0.5))
                    .when(can_merge, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if can_merge {
                            this.merge_transcript_selection(cx);
                        }
                    }))
                    .child(icon("arrows-merge", px(14.0), theme.foreground))
                    .child("Merge"),
            )
            .child(
                // `hover:bg-accent flex size-7 items-center justify-center rounded-full`
                div()
                    .id("transcript-clear-selection")
                    .flex()
                    .size(px(28.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(crate::squircle::CONTROL_RADIUS))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.close_speaker_assign(cx);
                        this.clear_transcript_selection(cx);
                    }))
                    .child(icon("x", px(14.0), theme.foreground)),
            )
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
            ]);

        // Inside the FAB column (`flex-col-reverse items-center` at
        // `bottom-3`): the slot sits above the 40px CTA box (`mb-2`), pulled
        // `translate-y-8` down until the CTA is hovered.
        let lifted = self.chat_cta_hovered();
        Some(
            div()
                .absolute()
                .bottom(px(40.0 + 8.0 - if lifted { 0.0 } else { 32.0 }))
                .left(relative(0.5))
                .w(px(0.0))
                .flex()
                .justify_center()
                .child(div().flex_shrink_0().child(bar))
                .into_any_element(),
        )
    }
}

/// `getTranscriptSelectionFromSegment`
fn section_selection(
    transcript_id: &str,
    offset_ms: i64,
    segment: &Segment,
) -> Option<SectionSelection> {
    let word_ids: Vec<String> = segment
        .words
        .iter()
        .filter_map(|word| word.id.clone().filter(|id| !id.is_empty()))
        .collect();
    if word_ids.is_empty() {
        return None;
    }
    Some(SectionSelection {
        text: segment.text.trim().to_string(),
        start_ms: offset_ms + segment.words.first().map(|word| word.start_ms).unwrap_or(0),
        group: SelectionGroup {
            transcript_id: transcript_id.to_string(),
            segment_key: segment.key.clone(),
            word_ids,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_transcript::{ChannelProfile, SegmentKey};

    fn key(segment: &str) -> SectionKey {
        ("t1".to_string(), segment.to_string())
    }

    fn selection(
        segment: &str,
        speaker_index: Option<i32>,
        human: Option<&str>,
    ) -> SectionSelection {
        SectionSelection {
            text: segment.to_string(),
            start_ms: 0,
            group: SelectionGroup {
                transcript_id: "t1".into(),
                segment_key: SegmentKey {
                    channel: ChannelProfile::RemoteParty,
                    speaker_index,
                    speaker_human_id: human.map(str::to_string),
                },
                word_ids: vec![format!("{segment}-w1"), format!("{segment}-w2")],
            },
        }
    }

    fn entries(
        items: &[(&str, Option<i32>, Option<&str>)],
    ) -> HashMap<SectionKey, SectionSelection> {
        items
            .iter()
            .map(|(segment, index, human)| (key(segment), selection(segment, *index, *human)))
            .collect()
    }

    #[test]
    fn normalizes_editable_text_like_the_frontend() {
        assert_eq!(
            normalize_editable_text("  Good \n  evening  to you "),
            "Good evening to you"
        );
    }

    #[test]
    fn merge_target_needs_a_contiguous_same_channel_selection_with_identity() {
        let order = vec![key("a"), key("b"), key("c")];
        let entries = entries(&[
            ("a", Some(0), None),
            ("b", Some(1), None),
            ("c", Some(0), None),
        ]);
        assert!(merge_target(&[key("a")], &order, &entries).is_none());
        assert!(merge_target(&[key("a"), key("c")], &order, &entries).is_none());
        assert_eq!(
            merge_target(&[key("b"), key("a")], &order, &entries).map(|s| s.text.as_str()),
            Some("a")
        );
        let unlabeled = self::entries(&[("a", None, None), ("b", Some(1), None)]);
        assert!(merge_target(&[key("a"), key("b")], &order, &unlabeled).is_none());
    }

    #[test]
    fn merged_selection_groups_words_per_transcript() {
        let a = selection("a", Some(0), Some("alice"));
        let b = selection("b", Some(1), None);
        let (text, start, groups) = merge_selections(&[&a, &b]).unwrap();
        assert_eq!(text, "a\n\nb");
        assert_eq!(start, 0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].word_ids, ["a-w1", "a-w2", "b-w1", "b-w2"]);
        assert_eq!(
            groups[0].segment_key.speaker_human_id.as_deref(),
            Some("alice")
        );
        assert!(merge_selections(&[]).is_none());
    }
}
