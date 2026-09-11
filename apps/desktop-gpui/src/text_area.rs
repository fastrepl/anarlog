//! Multi-line plain-text field (`<textarea>`): wrapped `StyledText` with a
//! caret and selection painted from its `TextLayout`, `EntityInputHandler`
//! for typing and IME, Enter inserting a newline, and `Blurred` emitted when
//! focus leaves (the app's textareas save `onBlur`).

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, ElementInputHandler,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, HighlightStyle, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Rgba, SharedString,
    StyledText, TextLayout, UTF16Selection, Window, actions, canvas, div, fill, point, prelude::*,
    px, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::mention_picker::{self, MentionState, Search};

actions!(
    text_area,
    [
        Backspace,
        Delete,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        DeleteWordBackward,
        DeleteWordForward,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Paste,
        Cut,
        Copy,
        Newline,
        /// A plain Enter: a newline, or a submit for `submitShortcut="enter"`
        /// fields.
        Enter,
        Escape,
        Submit,
        Undo,
        Redo,
        Tab,
        ShiftTab,
    ]
);

/// ProseMirror history's `newGroupDelay`: edits closer than this share one
/// undo step.
const UNDO_GROUP_DELAY: Duration = Duration::from_millis(500);

const KEY_CONTEXT: &str = "TextArea";

pub fn bind_keys(cx: &mut App) {
    let m = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let ctx = Some(KEY_CONTEXT);
    // The webview's word movement modifier: Alt on macOS, Ctrl elsewhere.
    let w = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        // WebKit deletes with a shifted key as well.
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("shift-delete", Delete, ctx),
        KeyBinding::new(&format!("{w}-left"), WordLeft, ctx),
        KeyBinding::new(&format!("{w}-right"), WordRight, ctx),
        KeyBinding::new(&format!("{w}-shift-left"), SelectWordLeft, ctx),
        KeyBinding::new(&format!("{w}-shift-right"), SelectWordRight, ctx),
        KeyBinding::new(&format!("{w}-backspace"), DeleteWordBackward, ctx),
        KeyBinding::new(&format!("{w}-delete"), DeleteWordForward, ctx),
        KeyBinding::new(&format!("{w}-shift-backspace"), DeleteWordBackward, ctx),
        KeyBinding::new(&format!("{w}-shift-delete"), DeleteWordForward, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new(&format!("{m}-a"), SelectAll, ctx),
        KeyBinding::new(&format!("{m}-v"), Paste, ctx),
        KeyBinding::new(&format!("{m}-c"), Copy, ctx),
        KeyBinding::new(&format!("{m}-x"), Cut, ctx),
        KeyBinding::new("enter", Enter, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("escape", Escape, ctx),
        KeyBinding::new(&format!("{m}-enter"), Submit, ctx),
        KeyBinding::new(&format!("{m}-z"), Undo, ctx),
        KeyBinding::new(&format!("{m}-shift-z"), Redo, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("shift-tab", ShiftTab, ctx),
    ]);
    if !cfg!(target_os = "macos") {
        cx.bind_keys([KeyBinding::new("ctrl-y", Redo, ctx)]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAreaEvent {
    Changed,
    /// Focus left the field (the `onBlur` save point).
    Blurred,
    Escape,
    /// Cmd/Ctrl+Enter, for fields that commit on it.
    Submit,
    /// `historyNavCommand`: Up with the caret at the very start, or Down at
    /// the very end, of a field that recalls sent messages.
    HistoryPrev,
    HistoryNext,
}

/// An inline `mention-@` atom: the chip's display text in the content and
/// the node's attrs (`packages/editor/src/chat/schema.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub range: Range<usize>,
    /// `session`, `human`, or `organization`.
    pub kind: String,
    pub id: String,
    pub label: String,
}

/// The chip colours (`MentionAvatar`'s glyph tint and the facehash palette).
#[derive(Debug, Clone, Copy)]
pub struct MentionStyle {
    pub icon: Rgba,
    pub dark: bool,
}

/// The field's content with its chips, as history and drafts keep it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Draft {
    pub content: String,
    pub atoms: Vec<Atom>,
}

#[derive(Debug, Clone)]
struct Snapshot {
    content: SharedString,
    atoms: Vec<Atom>,
    selected_range: Range<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct TextAreaStyle {
    pub text: Rgba,
    pub placeholder: Rgba,
    pub selection: Rgba,
    pub font_size: Pixels,
    pub line_height: Pixels,
    /// `rows`: the minimum height in lines.
    pub rows: usize,
    /// The `margin-bottom` between paragraphs when the field stands in for
    /// a ProseMirror editor (`.prompt-editor p { margin: 0 0 4px }`); zero
    /// for a plain `<textarea>`.
    pub paragraph_gap: Pixels,
}

/// The laid-out text: one `TextLayout` per paragraph when the paragraphs are
/// spaced (a `\n` ends a paragraph, the gap follows it), or a single one for
/// the whole text. Offsets are global byte offsets into the content.
#[derive(Clone, Default)]
struct AreaLayout {
    parts: Vec<(Range<usize>, TextLayout)>,
    gap: Pixels,
}

impl AreaLayout {
    fn part_for_index(&self, index: usize) -> Option<&(Range<usize>, TextLayout)> {
        self.parts
            .iter()
            .find(|(range, _)| index >= range.start && index <= range.end)
            .or(self.parts.last())
    }

    fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let (range, layout) = self.part_for_index(index)?;
        layout.position_for_index(index.clamp(range.start, range.end) - range.start)
    }

    /// The visual line (a paragraph's wrapped line) holding the caret at
    /// `index`: its start and its end, which on a soft wrap is the index the
    /// next line starts at — after the collapsed trailing space, where the
    /// layout draws a caret at the end of the wrapped line like WebKit's
    /// `endOfLine`. `None` before the layout exists.
    fn visual_line_edges(&self, text: &str, index: usize) -> Option<(usize, usize)> {
        let (range, layout) = self.part_for_index(index)?;
        let local = index.clamp(range.start, range.end) - range.start;
        let wrapped = layout.line_layout_for_index(local)?;
        // The layout lays each `\n`-separated line out on its own; the wrap
        // boundaries index into that line.
        let part = &text[range.clone()];
        let line_start = part[..local].rfind('\n').map_or(0, |i| i + 1);
        let line_end = part[local..].find('\n').map_or(part.len(), |i| local + i);
        let within = local - line_start;
        let unwrapped = &wrapped.unwrapped_layout;
        let mut start = 0;
        for boundary in &wrapped.wrap_boundaries {
            let wrap_end = unwrapped.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index;
            if within <= wrap_end {
                return Some((
                    range.start + line_start + start,
                    range.start + line_start + wrap_end,
                ));
            }
            start = wrap_end;
        }
        Some((range.start + line_start + start, range.start + line_end))
    }

    /// The paragraph under `position.y`, the gap after a paragraph counting
    /// as that paragraph, then the layout's own hit test clamped into it.
    fn index_for_position(&self, position: Point<Pixels>) -> Result<usize, usize> {
        let mut chosen = None;
        for (ix, (_, layout)) in self.parts.iter().enumerate() {
            let bounds = layout.bounds();
            if position.y < bounds.bottom() + self.gap || ix == self.parts.len() - 1 {
                chosen = Some(ix);
                break;
            }
        }
        let Some(ix) = chosen else {
            return Err(0);
        };
        let (range, layout) = &self.parts[ix];
        let bounds = layout.bounds();
        let y = position.y.max(bounds.top()).min(bounds.bottom() - px(1.0));
        match layout.index_for_position(point(position.x, y)) {
            Ok(index) => Ok((range.start + index).min(range.end)),
            Err(index) => Err((range.start + index).min(range.end)),
        }
    }

    fn bounds(&self) -> Bounds<Pixels> {
        let mut iter = self.parts.iter().map(|(_, layout)| layout.bounds());
        let Some(first) = iter.next() else {
            return Bounds::default();
        };
        iter.fold(first, |acc, bounds| acc.union(&bounds))
    }
}

/// The paragraph byte ranges of `text` (without their trailing `\n`), or
/// the whole text as one when `split` is false.
fn paragraph_ranges(text: &str, split: bool) -> Vec<Range<usize>> {
    if !split {
        return std::iter::once(0..text.len()).collect();
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    for (ix, ch) in text.char_indices() {
        if ch == '\n' {
            ranges.push(start..ix);
            start = ix + 1;
        }
    }
    ranges.push(start..text.len());
    ranges
}

pub struct TextArea {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    style: TextAreaStyle,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    layout: AreaLayout,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// `submitShortcut="enter"`: Enter submits and Shift+Enter breaks the line.
    enter_submits: bool,
    /// Up / Down at the edges emit the history events instead of moving.
    history_navigation: bool,
    atoms: Vec<Atom>,
    mention_search: Option<Search>,
    mention_style: Option<MentionStyle>,
    mention: Option<MentionState>,
    /// The trigger offset whose popup was dismissed or used, so it stays
    /// closed until the caret leaves it.
    mention_dismissed: Option<usize>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<Instant>,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl TextArea {
    pub fn new(
        placeholder: impl Into<SharedString>,
        style: TextAreaStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        cx.on_focus_out(&focus_handle, window, |this: &mut Self, _, _, cx| {
            this.is_selecting = false;
            cx.emit(TextAreaEvent::Blurred);
            cx.notify();
        })
        .detach();
        Self {
            focus_handle,
            content: SharedString::default(),
            placeholder: placeholder.into(),
            style,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            layout: AreaLayout::default(),
            last_bounds: None,
            is_selecting: false,
            enter_submits: false,
            history_navigation: false,
            atoms: Vec::new(),
            mention_search: None,
            mention_style: None,
            mention: None,
            mention_dismissed: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
        }
    }

    /// `submitShortcut="enter"`
    pub fn enter_submits(mut self) -> Self {
        self.enter_submits = true;
        self
    }

    /// `mentionConfig`: `@` opens the suggestion popup over `search`, and the
    /// chosen items become chips.
    pub fn with_mentions(mut self, search: Search, style: MentionStyle) -> Self {
        self.mention_search = Some(search);
        self.mention_style = Some(style);
        self
    }

    /// `useMessageHistory`: Up at the start and Down at the end recall sent
    /// messages through `HistoryPrev` / `HistoryNext`.
    pub fn with_history_navigation(mut self) -> Self {
        self.history_navigation = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    /// `proseMirrorJsonToText`: the text with each chip as `@label`.
    pub fn message_text(&self) -> String {
        message_text(&self.content, &self.atoms)
    }

    pub fn draft(&self) -> Draft {
        Draft {
            content: self.content.to_string(),
            atoms: self.atoms.clone(),
        }
    }

    /// `replaceContent(content, selection)`: the field takes a draft back
    /// with the caret at its start or end. Not a user edit (`isApplyingRef`),
    /// so no `Changed` is emitted; the owner redraws itself.
    pub fn restore(&mut self, draft: Draft, at_end: bool, cx: &mut Context<Self>) {
        self.content = draft.content.into();
        self.atoms = draft.atoms;
        let offset = if at_end { self.content.len() } else { 0 };
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.marked_range = None;
        self.mention = None;
        self.mention_dismissed = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_edit = None;
        cx.notify();
    }

    /// The open popup's state, while it has results to show.
    pub fn mention(&self) -> Option<&MentionState> {
        self.mention
            .as_ref()
            .filter(|state| !state.items.is_empty())
    }

    /// Window position under the trigger (`coordsAtPos(from)`) and the line
    /// height, for the popup's `bottom-start` placement.
    pub fn mention_anchor(&self) -> Option<(Point<Pixels>, Pixels)> {
        let state = self.mention()?;
        let position = if self.content.is_empty() {
            self.last_bounds?.origin
        } else {
            self.layout.position_for_index(state.from)?
        };
        Some((position, self.style.line_height))
    }

    pub fn select_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut()
            && index < state.items.len()
        {
            state.selected = index;
            cx.notify();
        }
    }

    /// `insertMention`: the chip plus a space replace `@query`; the popup
    /// stays closed for that trigger.
    pub fn insert_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(state) = self.mention.clone() else {
            return;
        };
        let Some(item) = state.items.get(index).cloned() else {
            return;
        };
        self.record_edit(true);
        let display = crate::mention::display_text(&item.label);
        let range = self.splice(state.from..state.to, &format!("{display} "));
        let atom_range = range.start..range.start + display.len();
        let position = self
            .atoms
            .iter()
            .position(|atom| atom.range.start > atom_range.start)
            .unwrap_or(self.atoms.len());
        self.atoms.insert(
            position,
            Atom {
                range: atom_range.clone(),
                kind: item.kind,
                id: item.id,
                label: item.label,
            },
        );
        let caret = atom_range.end + 1;
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.marked_range = None;
        self.mention = None;
        self.mention_dismissed = Some(state.from);
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    /// The candidate rows changed: re-run the open popup's query.
    pub fn rerun_mention_search(&mut self, cx: &mut Context<Self>) {
        if self.mention.take().is_some() {
            self.refresh_mention();
            cx.notify();
        }
    }

    /// `findMention` on the caret after every change.
    fn refresh_mention(&mut self) {
        let Some(search) = self.mention_search.clone() else {
            return;
        };
        if !self.selected_range.is_empty() {
            self.mention = None;
            return;
        }
        let ranges: Vec<Range<usize>> = self.atoms.iter().map(|atom| atom.range.clone()).collect();
        let caret = mention_picker::Caret {
            block: 0,
            offset: self.cursor_offset(),
        };
        let Some((from, to, query)) = mention_picker::find_mention(&self.content, &ranges, caret)
        else {
            self.mention = None;
            self.mention_dismissed = None;
            return;
        };
        if self.mention_dismissed == Some(from) {
            self.mention = None;
            return;
        }
        if let Some(state) = self.mention.as_mut()
            && state.from == from
            && state.query == query
        {
            state.to = to;
            return;
        }
        let items = search(&query);
        self.mention = Some(MentionState {
            block: 0,
            from,
            to,
            query,
            items,
            selected: 0,
        });
    }

    fn dismiss_mention(&mut self) {
        if let Some(state) = self.mention.take() {
            self.mention_dismissed = Some(state.from);
        }
    }

    fn snap_out_of_atoms(&self, offset: usize, direction: isize) -> usize {
        snap_out_of_atoms(&self.atoms, offset, direction)
    }

    /// Snapshot the field before an edit; typing bursts within
    /// `UNDO_GROUP_DELAY` share one step, `structural` edits never do.
    fn record_edit(&mut self, structural: bool) {
        let now = Instant::now();
        let grouped = !structural
            && self
                .last_edit
                .is_some_and(|last| now.duration_since(last) < UNDO_GROUP_DELAY);
        self.last_edit = Some(now);
        self.redo_stack.clear();
        if grouped && !self.undo_stack.is_empty() {
            return;
        }
        self.undo_stack.push(Snapshot {
            content: self.content.clone(),
            atoms: self.atoms.clone(),
            selected_range: self.selected_range.clone(),
        });
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        self.content = snapshot.content;
        self.atoms = snapshot.atoms;
        let end = self.content.len();
        self.selected_range =
            snapshot.selected_range.start.min(end)..snapshot.selected_range.end.min(end);
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_edit = None;
        self.refresh_mention();
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(Snapshot {
            content: self.content.clone(),
            atoms: self.atoms.clone(),
            selected_range: self.selected_range.clone(),
        });
        self.apply_snapshot(snapshot, cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(Snapshot {
            content: self.content.clone(),
            atoms: self.atoms.clone(),
            selected_range: self.selected_range.clone(),
        });
        self.apply_snapshot(snapshot, cx);
    }

    /// A click on the field's padding: focus and put the caret at the end,
    /// the way a `<textarea>` does for presses below its last line.
    pub fn focus_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window);
        }
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        cx.notify();
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text: SharedString = text.into();
        if text == self.content {
            return;
        }
        self.content = text;
        self.atoms.clear();
        let end = self.content.len();
        self.selected_range = end.min(self.selected_range.start)..end.min(self.selected_range.end);
        self.marked_range = None;
        self.mention = None;
        self.mention_dismissed = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_edit = None;
        cx.notify();
    }

    /// `ChatEditorHandle.insertText`: the trimmed text replaces the selection,
    /// padded with a blank on either side that touches a non-blank character.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let value = text.trim();
        if value.is_empty() {
            return;
        }
        let range = self.selected_range.clone();
        let before = self.content[..range.start].chars().next_back();
        let after = self.content[range.end..].chars().next();
        let mut insertion = String::new();
        if before.is_some_and(|c| !c.is_whitespace()) {
            insertion.push(' ');
        }
        insertion.push_str(value);
        if after.is_some_and(|c| !c.is_whitespace()) {
            insertion.push(' ');
        }
        self.record_edit(true);
        let range = self.splice(range, &insertion);
        let end = range.start + insertion.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.refresh_mention();
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.refresh_mention();
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.refresh_mention();
        cx.notify();
    }

    /// One grapheme back, over a whole chip when the caret sits after one.
    fn previous_boundary(&self, offset: usize) -> usize {
        let boundary = self
            .content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0);
        self.snap_out_of_atoms(boundary, -1)
    }

    /// One grapheme forward, over a whole chip when the caret sits before one.
    fn next_boundary(&self, offset: usize) -> usize {
        let boundary = self
            .content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len());
        self.snap_out_of_atoms(boundary, 1)
    }

    /// Home: WebKit's `startOfLine` — the visual line once laid out, the
    /// paragraph before that.
    fn line_start(&self, offset: usize) -> usize {
        if let Some((start, _)) = self.layout.visual_line_edges(&self.content, offset) {
            return start;
        }
        self.content[..offset].rfind('\n').map_or(0, |i| i + 1)
    }

    /// End: `endOfLine`, the visual line's end (after a soft wrap's trailing
    /// space), the paragraph's end before layout.
    fn line_end(&self, offset: usize) -> usize {
        if let Some((_, end)) = self.layout.visual_line_edges(&self.content, offset) {
            return end;
        }
        self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |i| offset + i)
    }

    /// The offset one wrapped line above/below the caret, at the same x.
    fn vertical_neighbour(&self, offset: usize, delta: f32) -> Option<usize> {
        let position = self.layout.position_for_index(offset)?;
        let target = point(
            position.x,
            position.y + self.style.line_height * delta + self.style.line_height / 2.0,
        );
        let bounds = self.layout.bounds();
        if target.y < bounds.top() {
            return Some(0);
        }
        if target.y > bounds.bottom() {
            return Some(self.content.len());
        }
        Some(match self.layout.index_for_position(target) {
            Ok(index) | Err(index) => self.snap_out_of_atoms(index.min(self.content.len()), 0),
        })
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let bounds = self.layout.bounds();
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        match self.layout.index_for_position(position) {
            Ok(index) | Err(index) => self.snap_out_of_atoms(index.min(self.content.len()), 0),
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// Replaces `range`, widened to swallow any chip it cuts into (an atom
    /// goes as a whole, like ProseMirror's), and returns the range replaced.
    fn splice(&mut self, range: Range<usize>, new_text: &str) -> Range<usize> {
        let range = splice_atoms(&mut self.atoms, range, new_text.len());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        range
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut().filter(|s| !s.items.is_empty()) {
            state.selected = (state.selected + state.items.len() - 1) % state.items.len();
            cx.notify();
            return;
        }
        if self.history_navigation && self.selected_range == (0..0) {
            cx.emit(TextAreaEvent::HistoryPrev);
            return;
        }
        if let Some(offset) = self.vertical_neighbour(self.cursor_offset(), -1.0) {
            self.move_to(offset, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut().filter(|s| !s.items.is_empty()) {
            state.selected = (state.selected + 1) % state.items.len();
            cx.notify();
            return;
        }
        let end = self.content.len();
        if self.history_navigation && self.selected_range == (end..end) {
            cx.emit(TextAreaEvent::HistoryNext);
            return;
        }
        if let Some(offset) = self.vertical_neighbour(self.cursor_offset(), 1.0) {
            self.move_to(offset, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_neighbour(self.cursor_offset(), -1.0) {
            self.select_to(offset, cx);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_neighbour(self.cursor_offset(), 1.0) {
            self.select_to(offset, cx);
        }
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_start(self.cursor_offset()), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_end(self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.line_start(self.cursor_offset());
        self.select_to(offset, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.line_end(self.cursor_offset());
        self.select_to(offset, cx);
    }

    /// Ctrl/Alt-Left/Right: WebKit's word movement.
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            crate::text_input::previous_word_start(&self.content, self.cursor_offset())
        } else {
            self.selected_range.start
        };
        self.move_to(offset, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            crate::text_input::next_word_end(&self.content, self.cursor_offset())
        } else {
            self.selected_range.end
        };
        self.move_to(offset, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = crate::text_input::previous_word_start(&self.content, self.cursor_offset());
        self.select_to(offset, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = crate::text_input::next_word_end(&self.content, self.cursor_offset());
        self.select_to(offset, cx);
    }

    /// Ctrl/Alt-Backspace / -Delete: `deleteWordBackward` / `deleteWordForward`.
    fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let start = crate::text_input::previous_word_start(&self.content, self.cursor_offset());
            self.selected_range = start..self.cursor_offset();
            self.selection_reversed = false;
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let end = crate::text_input::next_word_end(&self.content, self.cursor_offset());
            self.selected_range = self.cursor_offset()..end;
            self.selection_reversed = false;
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let start = self.previous_boundary(self.cursor_offset());
            self.selected_range = start..self.cursor_offset();
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let end = self.next_boundary(self.cursor_offset());
            self.selected_range = self.cursor_offset()..end;
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx)
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention() {
            let index = state.selected;
            self.insert_mention(index, cx);
            return;
        }
        if self.enter_submits {
            cx.emit(TextAreaEvent::Submit);
        } else {
            self.replace_text_in_range(None, "\n", window, cx)
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Submit);
    }

    /// Escape closes an open popup first; otherwise the owner hears it.
    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if self.mention().is_some() {
            self.dismiss_mention();
            cx.notify();
            return;
        }
        cx.emit(TextAreaEvent::Escape);
    }

    /// A `<textarea>` leaves Tab to sequential focus navigation.
    fn tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        crate::text_input::focus_by_keyboard(window, cx, None, Window::focus_next);
    }

    fn shift_tab(&mut self, _: &ShiftTab, window: &mut Window, cx: &mut Context<Self>) {
        crate::text_input::focus_by_keyboard(window, cx, None, Window::focus_prev);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\r\n", "\n"), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        self.is_selecting = true;
        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window);
        }
        let index = self.index_for_mouse_position(event.position);
        // A double-click selects the word, a triple-click the paragraph.
        match event.click_count {
            2 => {
                let range = crate::text_input::word_range_at(&self.content, index);
                self.move_to(range.start, cx);
                self.select_to(range.end, cx);
            }
            count if count >= 3 => {
                let start = self.content[..index].rfind('\n').map_or(0, |at| at + 1);
                let end = self.content[index..]
                    .find('\n')
                    .map_or(self.content.len(), |at| index + at);
                self.move_to(start, cx);
                self.select_to(end, cx);
            }
            _ if event.modifiers.shift => self.select_to(index, cx),
            _ => self.move_to(index, cx),
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let index = self.index_for_mouse_position(event.position);
            self.select_to(index, cx);
        }
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        // Deleting or pasting over a chip is a step of its own.
        let touches_atom = self
            .atoms
            .iter()
            .any(|atom| atom.range.start < range.end && range.start < atom.range.end);
        self.record_edit(touches_atom || new_text.contains('\n') || new_text.len() > 1);
        let range = self.splice(range, new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        self.refresh_mention();
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.record_edit(false);
        let range = self.splice(range, new_text);
        self.marked_range = Some(range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.layout.position_for_index(range.start)?;
        let end = self.layout.position_for_index(range.end)?;
        Some(Bounds::from_corners(
            start,
            point(end.x, end.y + self.style.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.layout.index_for_position(point).ok()?;
        Some(self.offset_to_utf16(index))
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let focused = self.focus_handle.is_focused(window);
        let empty = self.content.is_empty();
        let text: SharedString = if empty {
            self.placeholder.clone()
        } else {
            self.content.clone()
        };
        let mut text_style = window.text_style();
        text_style.color = if empty {
            style.placeholder.into()
        } else {
            style.text.into()
        };
        text_style.font_size = style.font_size.into();
        text_style.line_height = style.line_height.into();
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        if let Some(marked) = &self.marked_range
            && !empty
        {
            highlights.push((
                marked.clone(),
                HighlightStyle {
                    underline: Some(gpui::UnderlineStyle {
                        color: Some(style.text.into()),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..Default::default()
                },
            ));
        }
        if !empty {
            // `.mention { font-weight: 500 }`
            for atom in &self.atoms {
                highlights.push((
                    atom.range.clone(),
                    HighlightStyle {
                        font_weight: Some(gpui::FontWeight::MEDIUM),
                        ..Default::default()
                    },
                ));
            }
            highlights.sort_by_key(|(range, _)| range.start);
        }
        // One `StyledText` per paragraph when they are spaced, each with the
        // highlights that fall inside it, stacked with the gap between.
        let split = style.paragraph_gap > px(0.0) && !empty;
        let ranges = paragraph_ranges(&text, split);
        let mut parts = Vec::with_capacity(ranges.len());
        let mut children: Vec<AnyElement> = Vec::with_capacity(ranges.len());
        for range in ranges {
            let local: Vec<(Range<usize>, HighlightStyle)> = highlights
                .iter()
                .filter(|(highlight, _)| highlight.start < range.end && highlight.end > range.start)
                .map(|(highlight, style)| {
                    (
                        highlight.start.max(range.start) - range.start
                            ..highlight.end.min(range.end) - range.start,
                        *style,
                    )
                })
                .collect();
            let paragraph: SharedString = if split {
                text[range.clone()].to_string().into()
            } else {
                text.clone()
            };
            let styled = StyledText::new(paragraph).with_default_highlights(&text_style, local);
            parts.push((range, styled.layout().clone()));
            if split {
                // An empty paragraph still takes a line, as an empty `<p>` does.
                children.push(
                    div()
                        .min_h(style.line_height)
                        .child(styled)
                        .into_any_element(),
                );
            } else {
                children.push(styled.into_any_element());
            }
        }
        self.layout = AreaLayout {
            parts,
            gap: style.paragraph_gap,
        };
        let layout = self.layout.clone();
        let entity = cx.entity();
        let handler_entity = entity.clone();
        let focus_handle = self.focus_handle.clone();
        let caret = (focused && self.selected_range.is_empty()).then_some(self.cursor_offset());
        let selection =
            (focused && !self.selected_range.is_empty()).then_some(self.selected_range.clone());
        let selection_color = style.selection;
        let caret_color = style.text;
        let line_height = style.line_height;
        let chips: Vec<(usize, String, String)> = if empty {
            Vec::new()
        } else {
            self.atoms
                .iter()
                .map(|atom| (atom.range.start, atom.kind.clone(), atom.label.clone()))
                .collect()
        };
        let mention_style = self.mention_style;
        let font_size = style.font_size;

        div()
            .id("text-area")
            .relative()
            .w_full()
            .min_h(line_height * style.rows as f32)
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::delete_word_backward))
            .on_action(cx.listener(Self::delete_word_forward))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::shift_tab))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            // The title bar's Edit menu (`runEditCommand`) on the focused field.
            .on_action(cx.listener(|this, _: &crate::actions::Undo, window, cx| {
                this.undo(&Undo, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Redo, window, cx| {
                this.redo(&Redo, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::actions::Cut, window, cx| this.cut(&Cut, window, cx)),
            )
            .on_action(cx.listener(|this, _: &crate::actions::Copy, window, cx| {
                this.copy(&Copy, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Paste, window, cx| {
                this.paste(&Paste, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::actions::SelectAll, window, cx| {
                    this.select_all(&SelectAll, window, cx)
                }),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle);
                    crate::edit_menu::request(cx, event.position, this.focus_handle.clone());
                }),
            )
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .map(|area| {
                if split {
                    area.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(style.paragraph_gap)
                            .children(children),
                    )
                } else {
                    area.children(children)
                }
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &focus_handle,
                            ElementInputHandler::new(bounds, handler_entity.clone()),
                            cx,
                        );
                        entity.update(cx, |this, _| this.last_bounds = Some(bounds));
                        if let Some(range) = selection.clone() {
                            paint_selection(&layout, range, selection_color, line_height, window);
                        }
                        // `MentionAvatar` over each chip's placeholder:
                        // `vertical-align: middle` with `top: -2px`.
                        if let Some(mention_style) = mention_style {
                            let font = window.text_style().font();
                            for (start, kind, label) in &chips {
                                let Some(origin) = layout.position_for_index(*start) else {
                                    continue;
                                };
                                let origin = point(
                                    origin.x,
                                    origin.y + (line_height - font_size) / 2.0 - px(2.0),
                                );
                                crate::mention::paint_avatar(
                                    window,
                                    cx,
                                    Bounds::new(origin, size(font_size, font_size)),
                                    kind,
                                    label,
                                    &font,
                                    mention_style.icon,
                                    mention_style.dark,
                                );
                            }
                        }
                        if let Some(caret) = caret {
                            let position = if empty {
                                Some(bounds.origin)
                            } else {
                                layout.position_for_index(caret)
                            };
                            if let Some(position) = position {
                                window.paint_quad(fill(
                                    Bounds::new(position, size(px(1.0), line_height)),
                                    caret_color,
                                ));
                            }
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// `proseMirrorJsonToText` over a field's text: each chip as `@label`
/// (`@id` for a chip without one).
fn message_text(content: &str, atoms: &[Atom]) -> String {
    let mut out = String::new();
    let mut cursor = 0;
    for atom in atoms {
        out.push_str(&content[cursor..atom.range.start]);
        out.push('@');
        out.push_str(if atom.label.is_empty() {
            &atom.id
        } else {
            &atom.label
        });
        cursor = atom.range.end;
    }
    out.push_str(&content[cursor..]);
    out
}

/// `mentionSkipPlugin`: an offset inside a chip moves to its edge in
/// `direction` (`> 0` forward, `< 0` back, `0` the nearer one).
fn snap_out_of_atoms(atoms: &[Atom], offset: usize, direction: isize) -> usize {
    for atom in atoms {
        if atom.range.start < offset && offset < atom.range.end {
            let nearer_start = offset - atom.range.start <= atom.range.end - offset;
            return if direction > 0 || (direction == 0 && !nearer_start) {
                atom.range.end
            } else {
                atom.range.start
            };
        }
    }
    offset
}

/// Widens `range` over every chip it cuts into, drops the chips inside it,
/// and shifts the chips after it by the replacement's length difference.
fn splice_atoms(atoms: &mut Vec<Atom>, mut range: Range<usize>, new_len: usize) -> Range<usize> {
    for atom in atoms.iter() {
        let overlaps = atom.range.start < range.end && range.start < atom.range.end;
        if overlaps {
            range.start = range.start.min(atom.range.start);
            range.end = range.end.max(atom.range.end);
        }
    }
    let delta = new_len as isize - (range.end - range.start) as isize;
    atoms.retain(|atom| atom.range.end <= range.start || atom.range.start >= range.end);
    for atom in atoms.iter_mut() {
        if atom.range.start >= range.end {
            atom.range.start = (atom.range.start as isize + delta) as usize;
            atom.range.end = (atom.range.end as isize + delta) as usize;
        }
    }
    range
}

/// One quad per wrapped line the byte range covers.
fn paint_selection(
    layout: &AreaLayout,
    range: Range<usize>,
    color: Rgba,
    line_height: Pixels,
    window: &mut Window,
) {
    let mut start = range.start;
    while start < range.end {
        let Some(origin) = layout.position_for_index(start) else {
            return;
        };
        let (mut lo, mut hi) = (start, range.end);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            match layout.position_for_index(mid) {
                Some(p) if p.y == origin.y => lo = mid,
                _ => hi = mid - 1,
            }
        }
        let end_x = layout
            .position_for_index(lo)
            .map(|p| p.x)
            .unwrap_or(origin.x);
        let width = (end_x - origin.x).max(px(4.0));
        window.paint_quad(fill(Bounds::new(origin, size(width, line_height)), color));
        if lo >= range.end {
            break;
        }
        start = lo + 1;
        while start < range.end
            && layout
                .position_for_index(start)
                .is_some_and(|p| p.y == origin.y)
        {
            start += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_split_on_newlines_and_keep_empty_ones() {
        assert_eq!(
            paragraph_ranges("a\nbb\n\nc", true),
            [0..1, 2..4, 5..5, 6..7]
        );
        assert_eq!(paragraph_ranges("a\nbb", false), vec![0..4usize]);
        assert_eq!(paragraph_ranges("", true), vec![0..0usize]);
        // A trailing newline ends with an empty paragraph, like a trailing `<p>`.
        assert_eq!(paragraph_ranges("a\n", true), [0..1, 2..2]);
    }

    #[test]
    fn a_paragraph_owns_its_newline_offset() {
        let layout = AreaLayout {
            parts: vec![
                (0..1, TextLayout::default()),
                (2..4, TextLayout::default()),
                (5..5, TextLayout::default()),
            ],
            gap: px(4.0),
        };
        assert_eq!(layout.part_for_index(0).map(|(r, _)| r.clone()), Some(0..1));
        assert_eq!(layout.part_for_index(1).map(|(r, _)| r.clone()), Some(0..1));
        assert_eq!(layout.part_for_index(2).map(|(r, _)| r.clone()), Some(2..4));
        assert_eq!(layout.part_for_index(5).map(|(r, _)| r.clone()), Some(5..5));
        assert_eq!(layout.part_for_index(9).map(|(r, _)| r.clone()), Some(5..5));
    }

    fn chip(text: &mut String, kind: &str, id: &str, label: &str) -> Atom {
        let display = crate::mention::display_text(label);
        let start = text.len();
        text.push_str(&display);
        Atom {
            range: start..text.len(),
            kind: kind.into(),
            id: id.into(),
            label: label.into(),
        }
    }

    #[test]
    fn message_text_flattens_chips_like_the_frontend() {
        let mut text = String::from("Tell me about ");
        let ada = chip(&mut text, "human", "h1", "Ada Lovelace");
        text.push_str(" and ");
        let nameless = chip(&mut text, "session", "s1", "");
        text.push_str(" please");
        assert_eq!(
            message_text(&text, &[ada, nameless]),
            "Tell me about @Ada Lovelace and @s1 please"
        );
    }

    #[test]
    fn carets_skip_chips_and_edits_take_them_whole() {
        let mut text = String::from("a ");
        let ada = chip(&mut text, "human", "h1", "Ada");
        text.push_str(" b");
        let atoms = vec![ada.clone()];
        let inside = ada.range.start + 2;
        assert_eq!(snap_out_of_atoms(&atoms, inside, 1), ada.range.end);
        assert_eq!(snap_out_of_atoms(&atoms, inside, -1), ada.range.start);
        assert_eq!(
            snap_out_of_atoms(&atoms, ada.range.end - 1, 0),
            ada.range.end
        );
        assert_eq!(
            snap_out_of_atoms(&atoms, ada.range.start, 0),
            ada.range.start
        );
        assert_eq!(snap_out_of_atoms(&atoms, 1, 1), 1);

        // Backspace after the chip removes the whole chip.
        let mut atoms = vec![ada.clone()];
        let range = splice_atoms(&mut atoms, ada.range.end - 1..ada.range.end, 0);
        assert_eq!(range, ada.range.clone());
        assert!(atoms.is_empty());

        // An edit before the chip shifts it.
        let mut atoms = vec![ada.clone()];
        let range = splice_atoms(&mut atoms, 0..1, 3);
        assert_eq!(range, 0..1);
        assert_eq!(atoms[0].range, ada.range.start + 2..ada.range.end + 2);

        // An edit after the chip leaves it alone.
        let mut atoms = vec![ada.clone()];
        splice_atoms(&mut atoms, ada.range.end..ada.range.end + 1, 0);
        assert_eq!(atoms[0].range, ada.range);
    }
}
