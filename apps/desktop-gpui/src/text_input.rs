//! Single-line text field. GPUI ships no text input widget; this adapts the
//! `EntityInputHandler` example from the gpui crate (Apache-2.0) into a
//! reusable entity that emits `Changed` while typing and `Committed` on
//! Enter/blur, which is how the app's `<input>` fields persist.

use std::cell::RefCell;
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    Rgba, ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window,
    actions, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input,
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
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Paste,
        Cut,
        Copy,
        Commit,
        ShiftEnter,
        ModEnter,
        Up,
        Down,
        Escape,
        Tab,
        ShiftTab,
    ]
);

const KEY_CONTEXT: &str = "TextInput";

thread_local! {
    /// Every live field, so a keyboard focus change can find the one it
    /// landed on.
    static INPUTS: RefCell<Vec<gpui::WeakEntity<TextInput>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `move_focus` (`Window::focus_next` / `focus_prev`) as a keyboard
/// focus change: WebKit selects an `<input>`'s whole value when the keyboard
/// focuses it (a `<textarea>` keeps its caret at the start). `from` is the
/// field the key was pressed in, which is mid-update and cannot be read.
pub(crate) fn focus_by_keyboard(
    window: &mut Window,
    cx: &mut App,
    from: Option<gpui::EntityId>,
    move_focus: fn(&mut Window),
) {
    move_focus(window);
    let Some(focused) = window.focused(cx) else {
        return;
    };
    let inputs: Vec<_> = INPUTS.with_borrow_mut(|inputs| {
        inputs.retain(|input| input.upgrade().is_some());
        inputs
            .iter()
            .filter_map(gpui::WeakEntity::upgrade)
            .collect()
    });
    for input in inputs {
        if Some(input.entity_id()) == from {
            continue;
        }
        if input.read(cx).focus_handle == focused {
            input.update(cx, |input, cx| {
                input.selected_range = 0..input.content.len();
                input.selection_reversed = false;
                cx.notify();
            });
            return;
        }
    }
}

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
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new(&format!("{m}-a"), SelectAll, ctx),
        KeyBinding::new(&format!("{m}-v"), Paste, ctx),
        KeyBinding::new(&format!("{m}-c"), Copy, ctx),
        KeyBinding::new(&format!("{m}-x"), Cut, ctx),
        KeyBinding::new("enter", Commit, ctx),
        KeyBinding::new("shift-enter", ShiftEnter, ctx),
        KeyBinding::new(&format!("{m}-enter"), ModEnter, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("escape", Escape, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("shift-tab", ShiftTab, ctx),
    ]);
}

/// WebKit's `previousWordPosition` (`startWordBoundary`): the start of the
/// word before `offset`, spaces and punctuation between skipped; 0 when
/// there is none.
pub fn previous_word_start(text: &str, offset: usize) -> usize {
    let mut start = 0;
    for (index, word) in text.split_word_bound_indices() {
        if index >= offset {
            break;
        }
        if word.chars().any(char::is_alphanumeric) {
            start = index;
        }
    }
    start
}

/// WebKit's `nextWordPosition` on macOS and Unix (`endWordBoundary`, no
/// `shouldSkipSpaceWhenMovingRight`): the end of the word after `offset`,
/// `text.len()` when there is none.
pub fn next_word_end(text: &str, offset: usize) -> usize {
    for (index, word) in text.split_word_bound_indices() {
        let end = index + word.len();
        if end > offset && word.chars().any(char::is_alphanumeric) {
            return end;
        }
    }
    text.len()
}

/// WebKit's double-click selection: the UAX #29 word (or the run of spaces
/// or the punctuation mark) under `offset`, the last one past the end.
pub fn word_range_at(text: &str, offset: usize) -> Range<usize> {
    let mut last = 0..0;
    for (start, word) in text.split_word_bound_indices() {
        let end = start + word.len();
        if offset < end {
            return start..end;
        }
        last = start..end;
    }
    last
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputEvent {
    Changed,
    /// Enter, before the field blurs; fires whether or not there were edits.
    Enter,
    /// Shift+Enter and Cmd/Ctrl+Enter, for fields that step or submit on them.
    ShiftEnter,
    ModEnter,
    /// Enter or blur after an edit.
    Committed,
    /// Backspace on an empty field, for chip inputs that pop the last chip.
    BackspaceEmpty,
    /// Arrow up/down; single-line fields have no use for them, so hosts such
    /// as the open-note dialog get to move their selection.
    Navigate(i32),
    Escape,
}

/// Colours the caller resolves from the theme.
#[derive(Debug, Clone, Copy)]
pub struct TextInputStyle {
    pub text: Rgba,
    pub placeholder: Rgba,
    pub selection: Rgba,
    /// `focus:underline`
    pub underline_when_focused: bool,
    /// `type="password"`: draw one bullet per character.
    pub masked: bool,
}

/// The mask glyph and its UTF-8 length.
const MASK: &str = "\u{2022}";

/// Byte offset in the content -> byte offset in the masked display string.
fn masked_offset(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())].chars().count() * MASK.len()
}

/// Byte offset in the masked display string -> byte offset in the content.
fn unmasked_offset(content: &str, offset: usize) -> usize {
    let chars = offset / MASK.len();
    content
        .char_indices()
        .nth(chars)
        .map(|(index, _)| index)
        .unwrap_or(content.len())
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    style: TextInputStyle,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    dirty: bool,
    was_focused: bool,
    scroll_offset: Pixels,
    /// Enter emits `Enter` without blurring (a find field that steps to the
    /// next match), instead of committing like a form field.
    enter_keeps_focus: bool,
    /// A chip input: Tab with text acts like Enter and keeps the focus
    /// (`if (e.key === "Enter" || e.key === "Tab") && inputValue.trim()`);
    /// an empty field lets Tab move the focus on like any other.
    tab_submits_when_filled: bool,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(
        placeholder: impl Into<SharedString>,
        style: TextInputStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A tab stop, so Tab walks the fields in render order like the
        // webview's sequential focus navigation.
        let focus_handle = cx.focus_handle().tab_stop(true);
        cx.on_focus_out(&focus_handle, window, |this: &mut Self, _, window, cx| {
            this.blurred(window, cx);
        })
        .detach();
        INPUTS.with_borrow_mut(|inputs| inputs.push(cx.weak_entity()));
        Self {
            focus_handle,
            content: SharedString::default(),
            placeholder: placeholder.into(),
            style,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            dirty: false,
            was_focused: false,
            scroll_offset: px(0.0),
            enter_keeps_focus: false,
            tab_submits_when_filled: false,
        }
    }

    pub fn enter_keeps_focus(mut self) -> Self {
        self.enter_keeps_focus = true;
        self
    }

    pub fn tab_submits_when_filled(mut self) -> Self {
        self.tab_submits_when_filled = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_style(&mut self, style: TextInputStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    /// Whether the user has edits that have not been committed yet.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Replace the content from the store; keeps the caret in range.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text: SharedString = text.into();
        if text == self.content {
            return;
        }
        self.content = text;
        let end = self.content.len();
        self.selected_range = end.min(self.selected_range.start)..end.min(self.selected_range.end);
        self.marked_range = None;
        self.dirty = false;
        cx.notify();
    }

    fn blurred(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = false;
        self.scroll_offset = px(0.0);
        if self.dirty {
            self.dirty = false;
            cx.emit(TextInputEvent::Committed);
        }
        cx.notify();
    }

    fn commit(&mut self, _: &Commit, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Enter);
        if self.enter_keeps_focus {
            return;
        }
        // `e.currentTarget.blur()`: the focus-out handler emits `Committed`.
        window.blur();
        self.blurred(window, cx);
    }

    fn shift_enter(&mut self, _: &ShiftEnter, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::ShiftEnter);
    }

    /// Sequential focus navigation: the next tab stop, or the chip input's
    /// submit while it holds text.
    fn tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab_submits_when_filled && !self.content.trim().is_empty() {
            cx.emit(TextInputEvent::Enter);
            return;
        }
        let from = cx.entity_id();
        focus_by_keyboard(window, cx, Some(from), Window::focus_next);
    }

    fn shift_tab(&mut self, _: &ShiftTab, window: &mut Window, cx: &mut Context<Self>) {
        let from = cx.entity_id();
        focus_by_keyboard(window, cx, Some(from), Window::focus_prev);
    }

    fn mod_enter(&mut self, _: &ModEnter, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::ModEnter);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Navigate(-1));
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Navigate(1));
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Escape);
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

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }

    /// `input.select()`
    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    /// Ctrl/Alt-Left/Right: WebKit's word movement.
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            previous_word_start(&self.content, self.cursor_offset())
        } else {
            self.selected_range.start
        };
        self.move_to(offset, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            next_word_end(&self.content, self.cursor_offset())
        } else {
            self.selected_range.end
        };
        self.move_to(offset, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = previous_word_start(&self.content, self.cursor_offset());
        self.select_to(offset, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = next_word_end(&self.content, self.cursor_offset());
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
            let start = previous_word_start(&self.content, self.cursor_offset());
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
            let end = next_word_end(&self.content, self.cursor_offset());
            self.selected_range = self.cursor_offset()..end;
            self.selection_reversed = false;
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.content.is_empty() {
            cx.emit(TextInputEvent::BackspaceEmpty);
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
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
        // A double-click selects the word, a triple-click the whole value.
        match event.click_count {
            2 => {
                let range = word_range_at(&self.content, index);
                self.selection_reversed = false;
                self.selected_range = range;
                cx.notify();
            }
            count if count >= 3 => self.select_all_text(cx),
            _ if event.modifiers.shift => self.select_to(index, cx),
            _ => self.move_to(index, cx),
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let index = line.closest_index_for_x(position.x - bounds.left() + self.scroll_offset);
        if self.style.masked {
            unmasked_offset(&self.content, index)
        } else {
            index
        }
    }

    /// Content byte offset -> offset in the shaped (possibly masked) line.
    fn display_offset(&self, offset: usize) -> usize {
        if self.style.masked {
            masked_offset(&self.content, offset)
        } else {
            offset
        }
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
        cx.notify()
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

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn splice(&mut self, range: Range<usize>, new_text: &str) {
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.dirty = true;
    }
}

impl EntityInputHandler for TextInput {
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.splice(range.clone(), new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.emit(TextInputEvent::Changed);
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
        self.splice(range.clone(), new_text);
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let left = bounds.left() - self.scroll_offset;
        Some(Bounds::from_corners(
            point(
                left + last_layout.x_for_index(self.display_offset(range.start)),
                bounds.top(),
            ),
            point(
                left + last_layout.x_for_index(self.display_offset(range.end)),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x + self.scroll_offset)?;
        let utf8_index = if self.style.masked {
            unmasked_offset(&self.content, utf8_index)
        } else {
            utf8_index
        };
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_offset: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.display_offset(input.selected_range.start)
            ..input.display_offset(input.selected_range.end);
        let cursor = input.display_offset(input.cursor_offset());
        let marked_range = input
            .marked_range
            .as_ref()
            .map(|range| input.display_offset(range.start)..input.display_offset(range.end));
        let focused = input.focus_handle.is_focused(window);
        let text_style = window.text_style();
        let colors = input.style;

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), colors.placeholder)
        } else if colors.masked {
            (
                SharedString::from(MASK.repeat(content.chars().count())),
                colors.text,
            )
        } else {
            (content, colors.text)
        };

        let underline = (focused
            && colors.underline_when_focused
            && !display_text.is_empty()
            && !input.content.is_empty())
        .then_some(UnderlineStyle {
            color: Some(text_color.into()),
            thickness: px(1.0),
            wavy: false,
        });
        let run = TextRun {
            len: display_text.len(),
            font: text_style.font(),
            color: text_color.into(),
            background_color: None,
            underline,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        // `overflow-x-auto` while focused: keep the caret in view.
        let mut scroll_offset = if focused {
            input.scroll_offset
        } else {
            px(0.0)
        };
        let cursor_x = line.x_for_index(cursor);
        if focused {
            if cursor_x - scroll_offset > bounds.size.width - px(1.0) {
                scroll_offset = cursor_x - bounds.size.width + px(1.0);
            } else if cursor_x - scroll_offset < px(0.0) {
                scroll_offset = cursor_x;
            }
            let max_scroll = (line.width - bounds.size.width + px(1.0)).max(px(0.0));
            scroll_offset = scroll_offset.clamp(px(0.0), max_scroll);
        }
        let left = bounds.left() - scroll_offset;

        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(left + cursor_x, bounds.top()),
                        size(px(1.), bounds.bottom() - bounds.top()),
                    ),
                    colors.text,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(left + line.x_for_index(selected_range.start), bounds.top()),
                        point(left + line.x_for_index(selected_range.end), bounds.bottom()),
                    ),
                    colors.selection,
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_offset,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line = prepaint.line.take().unwrap();
        let scroll_offset = prepaint.scroll_offset;
        let focused = focus_handle.is_focused(window);
        let selection = prepaint.selection.take();
        let cursor = prepaint.cursor.take();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            // Only the focused control paints its selection (the frame
            // selection follows the focus); an unfocused field keeps its
            // range for when it is focused again.
            if focused && let Some(selection) = selection {
                window.paint_quad(selection)
            }
            line.paint(
                point(bounds.origin.x - scroll_offset, bounds.origin.y),
                window.line_height(),
                window,
                cx,
            )
            .unwrap();
            if focused && let Some(cursor) = cursor {
                window.paint_quad(cursor);
            }
        });

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_offset = scroll_offset;
            input.was_focused = focused;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("text-input")
            .flex()
            .w_full()
            .min_w_0()
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
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::commit))
            .on_action(cx.listener(Self::shift_enter))
            .on_action(cx.listener(Self::mod_enter))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::shift_tab))
            // The title bar's Edit menu (`runEditCommand`) on the focused field.
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
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::word_range_at;

    #[test]
    fn word_movement_skips_spaces_and_marks_like_webkit() {
        use super::{next_word_end, previous_word_start};
        let text = "one, two  three";
        // Backward from the end lands on the start of `three`, then `two`, then `one`.
        assert_eq!(previous_word_start(text, text.len()), 10);
        assert_eq!(previous_word_start(text, 10), 5);
        assert_eq!(previous_word_start(text, 7), 5);
        assert_eq!(previous_word_start(text, 5), 0);
        assert_eq!(previous_word_start(text, 0), 0);
        // Forward lands on word ends: `one|,`, `two|`, `three|`.
        assert_eq!(next_word_end(text, 0), 3);
        assert_eq!(next_word_end(text, 3), 8);
        assert_eq!(next_word_end(text, 8), 15);
        assert_eq!(next_word_end(text, 15), 15);
    }

    #[test]
    fn double_click_selects_the_word_space_run_or_mark_under_the_offset() {
        let text = "Beta Gamma, Delta";
        assert_eq!(word_range_at(text, 0), 0..4);
        assert_eq!(word_range_at(text, 7), 5..10);
        // The run of spaces and the comma are their own segments.
        assert_eq!(word_range_at(text, 4), 4..5);
        assert_eq!(word_range_at(text, 10), 10..11);
        // Past the end the last segment is selected.
        assert_eq!(word_range_at(text, text.len()), 12..17);
        assert_eq!(word_range_at("", 0), 0..0);
    }
}
