//! Single-line text field. Composition goes through GPUI's native input handler, and the value is
//! published as AccessKit text runs so assistive technology can read, select, and replace it.

use std::{borrow::Cow, cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    A11ySubtreeBuilder, AccessibleAction, App, Bounds, ClipboardItem, ContentMask, Context,
    CursorStyle, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, InspectorElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, Pixels, Point, Role, ShapedLine, SharedString, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window,
    accesskit::{self, ActionData, NodeId, TextPosition, TextSelection},
    actions, div, fill, hsla, point,
    prelude::*,
    px, relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        Home,
        End,
        SelectAll,
        Copy,
        Cut,
        Paste
    ]
);

pub fn key_bindings() -> Vec<KeyBinding> {
    let context = Some("TextInput");
    vec![
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new(secondary!("a"), SelectAll, context),
        KeyBinding::new(secondary!("c"), Copy, context),
        KeyBinding::new(secondary!("x"), Cut, context),
        KeyBinding::new(secondary!("v"), Paste, context),
    ]
}

/// AccessKit indexes characters and word starts with `u8`, so a long value spans several runs.
const RUN_CHARACTERS: usize = u8::MAX as usize;

/// A slice of the value published as one AccessKit text run.
struct Run {
    range: Range<usize>,
    /// UTF-8 length of each selectable character (a grapheme cluster).
    lengths: Vec<u8>,
    word_starts: Vec<u8>,
}

fn runs(text: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut run = Run {
        range: 0..0,
        lengths: Vec::new(),
        word_starts: Vec::new(),
    };
    let mut after_whitespace = true;
    for grapheme in text.graphemes(true) {
        let whitespace = grapheme.chars().all(char::is_whitespace);
        // A cluster longer than 255 bytes cannot be one AccessKit character; split it by scalar.
        let mut pieces = Vec::new();
        let mut piece = 0;
        for character in grapheme.chars() {
            if piece + character.len_utf8() > u8::MAX as usize {
                pieces.push(piece as u8);
                piece = 0;
            }
            piece += character.len_utf8();
        }
        pieces.push(piece as u8);
        for (index, length) in pieces.into_iter().enumerate() {
            if run.lengths.len() == RUN_CHARACTERS {
                let end = run.range.end;
                runs.push(std::mem::replace(
                    &mut run,
                    Run {
                        range: end..end,
                        lengths: Vec::new(),
                        word_starts: Vec::new(),
                    },
                ));
            }
            if index == 0 && after_whitespace && !whitespace {
                run.word_starts.push(run.lengths.len() as u8);
            }
            run.lengths.push(length);
            run.range.end += length as usize;
        }
        after_whitespace = whitespace;
    }
    runs.push(run);
    runs
}

fn position(runs: &[(NodeId, Run)], offset: usize) -> Option<TextPosition> {
    let (node, run) = runs
        .iter()
        .find(|(_, run)| offset < run.range.end)
        .or(runs.last())?;
    let mut end = run.range.start;
    let character_index = run
        .lengths
        .iter()
        .take_while(|length| {
            end += **length as usize;
            end <= offset
        })
        .count();
    Some(TextPosition {
        node: *node,
        character_index,
    })
}

fn offset(runs: &[(NodeId, Run)], position: &TextPosition) -> Option<usize> {
    let (_, run) = runs.iter().find(|(node, _)| *node == position.node)?;
    let characters = run.lengths.iter().take(position.character_index);
    Some(run.range.start + characters.map(|length| *length as usize).sum::<usize>())
}

fn utf16_to_byte(text: &str, target: usize) -> usize {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units >= target {
            return byte;
        }
        units += character.len_utf16();
    }
    text.len()
}

fn byte_to_utf16(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}

fn single_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\r', '\n']) {
        Cow::Owned(text.replace("\r\n", " ").replace(['\r', '\n'], " "))
    } else {
        Cow::Borrowed(text)
    }
}

/// What the current frame laid out and published, shared with the accessibility callbacks.
#[derive(Default)]
struct Frame {
    /// The shaped value; absent while the placeholder is showing.
    line: Option<ShapedLine>,
    bounds: Bounds<Pixels>,
    scroll: Pixels,
    runs: Vec<(NodeId, Run)>,
}

pub struct Changed;

pub struct TextInput {
    id: ElementId,
    label: SharedString,
    focus: FocusHandle,
    text: String,
    selection: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    frame: Rc<RefCell<Frame>>,
}

impl EventEmitter<Changed> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    /// `label` is both the accessible name and the placeholder.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            focus: cx.focus_handle(),
            text: String::new(),
            selection: 0..0,
            reversed: false,
            marked: None,
            frame: Rc::default(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn clear(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.edit(0..self.text.len(), "", window, cx);
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn previous(&self, offset: usize) -> usize {
        self.text[..offset]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    fn next(&self, offset: usize) -> usize {
        self.text[offset..]
            .graphemes(true)
            .next()
            .map_or(self.text.len(), |grapheme| offset + grapheme.len())
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.selection = offset..offset;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn select(&mut self, anchor: usize, focus: usize, window: &Window, cx: &mut Context<Self>) {
        self.selection = anchor.min(focus)..anchor.max(focus);
        self.reversed = focus < anchor;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    /// Replaces `range` and returns the range of the inserted text.
    fn edit(
        &mut self,
        range: Range<usize>,
        text: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Range<usize> {
        let text = single_line(text);
        let changed = !range.is_empty() || !text.is_empty();
        self.text.replace_range(range.clone(), &text);
        let inserted = range.start..range.start + text.len();
        self.selection = inserted.end..inserted.end;
        self.reversed = false;
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        if changed {
            cx.emit(Changed);
        }
        cx.notify();
        inserted
    }

    /// The range an input-method or key edit applies to when it names none.
    fn target(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .map(|range| self.bytes(range))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone())
    }

    fn bytes(&self, range_utf16: Range<usize>) -> Range<usize> {
        utf16_to_byte(&self.text, range_utf16.start)..utf16_to_byte(&self.text, range_utf16.end)
    }

    fn utf16(&self, range: &Range<usize>) -> Range<usize> {
        byte_to_utf16(&self.text, range.start)..byte_to_utf16(&self.text, range.end)
    }

    /// The caret offset closest to a window x coordinate.
    fn offset_at(&self, x: Pixels) -> usize {
        let frame = self.frame.borrow();
        let Some(line) = &frame.line else {
            return 0;
        };
        let index = line.closest_index_for_x(x - frame.bounds.left() + frame.scroll);
        if index >= self.text.len() {
            return self.text.len();
        }
        self.text
            .grapheme_indices(true)
            .map(|(start, _)| start)
            .take_while(|start| *start <= index)
            .last()
            .unwrap_or(0)
    }

    fn selected_text(&self) -> Option<ClipboardItem> {
        (!self.selection.is_empty())
            .then(|| ClipboardItem::new_string(self.text[self.selection.clone()].to_owned()))
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selection.is_empty() {
            self.previous(self.cursor())..self.cursor()
        } else {
            self.selection.clone()
        };
        self.edit(range, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selection.is_empty() {
            self.cursor()..self.next(self.cursor())
        } else {
            self.selection.clone()
        };
        self.edit(range, "", window, cx);
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            self.previous(self.cursor())
        } else {
            self.selection.start
        };
        self.move_to(offset, window, cx);
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            self.next(self.cursor())
        } else {
            self.selection.end
        };
        self.move_to(offset, window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous(self.cursor()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next(self.cursor()), window, cx);
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.select(0, self.text.len(), window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.selected_text() {
            cx.write_to_clipboard(item);
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.selected_text() {
            cx.write_to_clipboard(item);
            self.edit(self.selection.clone(), "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.edit(self.target(None), &text, window, cx);
        }
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let offset = self.offset_at(event.position.x);
        if event.modifiers.shift {
            self.select_to(offset, window, cx);
        } else {
            self.move_to(offset, window, cx);
        }
    }

    fn set_accessible_selection(
        &mut self,
        selection: &TextSelection,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let resolved = {
            let frame = self.frame.borrow();
            offset(&frame.runs, &selection.anchor).zip(offset(&frame.runs, &selection.focus))
        };
        // The runs describe the last frame; ignore positions the value no longer has.
        if let Some((anchor, focus)) = resolved
            && self.text.is_char_boundary(anchor)
            && self.text.is_char_boundary(focus)
        {
            self.select(anchor, focus, window, cx);
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.bytes(range_utf16);
        adjusted_range.replace(self.utf16(&range));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16(&self.selection),
            reversed: self.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(self.target(range_utf16), text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.edit(self.target(range_utf16), new_text, window, cx);
        if let Some(selected) = new_selected_range_utf16 {
            let composed = &self.text[inserted.clone()];
            self.selection = inserted.start + utf16_to_byte(composed, selected.start)
                ..inserted.start + utf16_to_byte(composed, selected.end);
        }
        self.marked = (!inserted.is_empty()).then_some(inserted);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.bytes(range_utf16);
        let frame = self.frame.borrow();
        let line = frame.line.as_ref()?;
        let left = element_bounds.left() - frame.scroll;
        Some(Bounds::from_corners(
            point(left + line.x_for_index(range.start), element_bounds.top()),
            point(left + line.x_for_index(range.end), element_bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(byte_to_utf16(&self.text, self.offset_at(point.x)))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.bytes(range_utf16);
        self.select(range.start, range.end, window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(byte_to_utf16(&self.text, self.text.len()))
    }
}

/// Publishes the value as text runs under the field's accessibility node.
fn publish(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor: usize,
    focus: usize,
    scale: f32,
    frame: &RefCell<Frame>,
) {
    let mut frame = frame.borrow_mut();
    let published: Vec<(NodeId, Run)> = runs(text)
        .into_iter()
        .enumerate()
        .map(|(index, run)| (builder.synthetic_node_id(index), run))
        .collect();
    for (id, run) in &published {
        let mut node = accesskit::Node::new(Role::TextRun);
        node.set_value(&text[run.range.clone()]);
        node.set_character_lengths(run.lengths.clone());
        node.set_word_starts(run.word_starts.clone());
        node.set_text_direction(accesskit::TextDirection::LeftToRight);
        if let Some(line) = &frame.line {
            // AccessKit geometry is in physical pixels; character positions are run-relative.
            let origin = frame.bounds.left() - frame.scroll;
            let left = line.x_for_index(run.range.start);
            let mut byte = run.range.start;
            let mut positions = Vec::with_capacity(run.lengths.len());
            let mut widths = Vec::with_capacity(run.lengths.len());
            for length in &run.lengths {
                let start = line.x_for_index(byte);
                byte += *length as usize;
                positions.push(f32::from(start - left) * scale);
                widths.push(f32::from(line.x_for_index(byte) - start) * scale);
            }
            node.set_bounds(accesskit::Rect {
                x0: f64::from(f32::from(origin + left) * scale),
                y0: f64::from(f32::from(frame.bounds.top()) * scale),
                x1: f64::from(f32::from(origin + line.x_for_index(run.range.end)) * scale),
                y1: f64::from(f32::from(frame.bounds.bottom()) * scale),
            });
            node.set_character_positions(positions);
            node.set_character_widths(widths);
        }
        builder.push_child(*id, node);
    }
    if let Some((anchor, focus)) = position(&published, anchor).zip(position(&published, focus)) {
        builder
            .parent_node()
            .set_text_selection(TextSelection { anchor, focus });
    }
    frame.runs = published;
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let frame = self.frame.clone();
        let text = self.text.clone();
        let (anchor, focus) = if self.reversed {
            (self.selection.end, self.selection.start)
        } else {
            (self.selection.start, self.selection.end)
        };
        let select = cx.entity();
        let set_value = cx.entity();
        let replace = cx.entity();
        div()
            .id(self.id.clone())
            .role(Role::TextInput)
            .aria_label(self.label.clone())
            .track_focus(&self.focus)
            .key_context("TextInput")
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_a11y_action(
                AccessibleAction::SetTextSelection,
                move |data, window, cx| {
                    if let Some(ActionData::SetTextSelection(selection)) = data {
                        select.update(cx, |this, cx| {
                            this.set_accessible_selection(selection, window, cx);
                        });
                    }
                },
            )
            .on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                if let Some(ActionData::Value(value)) = data {
                    set_value.update(cx, |this, cx| {
                        this.edit(0..this.text.len(), value, window, cx);
                    });
                }
            })
            .on_a11y_action(
                AccessibleAction::ReplaceSelectedText,
                move |data, window, cx| {
                    if let Some(ActionData::Value(value)) = data {
                        replace.update(cx, |this, cx| {
                            this.edit(this.selection.clone(), value, window, cx);
                        });
                    }
                },
            )
            .a11y_synthetic_children(move |builder| {
                publish(builder, &text, anchor, focus, scale, &frame);
            })
            .child(Line(cx.entity()))
    }
}

struct Line(Entity<TextInput>);

struct Layout {
    line: ShapedLine,
    scroll: Pixels,
}

impl Element for Line {
    type RequestLayoutState = ();
    type PrepaintState = Layout;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Layout {
        let input = self.0.read(cx);
        let style = window.text_style();
        let placeholder = input.text.is_empty();
        let (text, color) = if placeholder {
            (input.label.clone(), hsla(0., 0., 0., 0.4))
        } else {
            (SharedString::from(input.text.clone()), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.marked.clone() {
            Some(marked) => vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.len(),
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        // Keep the caret inside the field when the value is wider than it.
        let mut frame = input.frame.borrow_mut();
        let caret = if placeholder {
            px(0.)
        } else {
            line.x_for_index(input.cursor())
        };
        let visible = bounds.size.width - px(1.);
        let scroll = frame.scroll.max(caret - visible).min(caret).max(px(0.));
        frame.line = (!placeholder).then(|| line.clone());
        frame.bounds = bounds;
        frame.scroll = scroll;
        Layout { line, scroll }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        layout: &mut Layout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus, selection, cursor, placeholder) = {
            let input = self.0.read(cx);
            (
                input.focus.clone(),
                input.selection.clone(),
                input.cursor(),
                input.text.is_empty(),
            )
        };
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.0.clone()), cx);
        let origin = point(bounds.left() - layout.scroll, bounds.top());
        let line = &layout.line;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if !selection.is_empty() {
                window.paint_quad(fill(
                    Bounds::from_corners(
                        point(origin.x + line.x_for_index(selection.start), bounds.top()),
                        point(origin.x + line.x_for_index(selection.end), bounds.bottom()),
                    ),
                    rgba(0x2563eb40),
                ));
            }
            line.paint(
                origin,
                bounds.size.height,
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();
            if selection.is_empty() && focus.is_focused(window) {
                let caret = if placeholder {
                    px(0.)
                } else {
                    line.x_for_index(cursor)
                };
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x + caret, bounds.top()),
                        size(px(1.), bounds.size.height),
                    ),
                    rgb(0x2563eb),
                ));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    fn published(text: &str) -> Vec<(NodeId, Run)> {
        runs(text)
            .into_iter()
            .enumerate()
            .map(|(index, run)| (NodeId(index as u64 + 1), run))
            .collect()
    }

    #[test]
    fn text_runs_address_grapheme_clusters_and_words() {
        // "é" is e + combining accent; the family emoji is five scalars joined into one cluster.
        let text = "회의 cafe\u{301} 👨‍👩‍👧";
        let runs = published(text);
        assert_eq!(runs.len(), 1);
        let run = &runs[0].1;
        assert_eq!(run.range, 0..text.len());
        assert_eq!(run.lengths, [3, 3, 1, 1, 1, 1, 3, 1, 18]);
        assert_eq!(run.word_starts, [0, 3, 8]);

        // Every caret stop survives the trip to an AccessKit position and back.
        let mut stops: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .collect();
        stops.push(text.len());
        for (character_index, stop) in stops.into_iter().enumerate() {
            let position = position(&runs, stop).unwrap();
            assert_eq!(position.character_index, character_index);
            assert_eq!(offset(&runs, &position), Some(stop));
        }
    }

    #[test]
    fn long_values_split_into_runs_that_stay_addressable() {
        let text = "가".repeat(300);
        let runs = published(&text);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].1.lengths.len(), 255);
        assert_eq!(runs[1].1.range, 765..900);
        // The second run continues a word, so it starts none.
        assert!(runs[1].1.word_starts.is_empty());

        let boundary = position(&runs, 765).unwrap();
        assert_eq!((boundary.node, boundary.character_index), (runs[1].0, 0));
        let end = position(&runs, text.len()).unwrap();
        assert_eq!((end.node, end.character_index), (runs[1].0, 45));
        assert_eq!(offset(&runs, &end), Some(text.len()));

        // An empty value still needs a run for the caret to sit in.
        let empty = published("");
        assert_eq!(position(&empty, 0).unwrap().character_index, 0);
    }

    #[gpui::test]
    fn composes_and_edits_in_utf16_ranges(cx: &mut TestAppContext) {
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("filter", "Filter", cx));
        let changes = std::rc::Rc::new(std::cell::Cell::new(0));
        let _subscription = cx.update(|_, cx| {
            let changes = changes.clone();
            cx.subscribe(&input, move |_, _: &Changed, _| {
                changes.set(changes.get() + 1)
            })
        });

        input.update_in(cx, |input, window, cx| {
            input.replace_text_in_range(None, "🦀 ", window, cx);

            // A Korean syllable is recomposed in place as each jamo arrives.
            for step in ["ㅎ", "하", "한"] {
                input.replace_and_mark_text_in_range(None, step, Some(1..1), window, cx);
                assert_eq!(input.text(), format!("🦀 {step}"));
                assert_eq!(input.marked_text_range(window, cx), Some(3..4));
            }
            input.replace_text_in_range(None, "한", window, cx);
            assert_eq!(input.marked_text_range(window, cx), None);
            assert_eq!(
                input.selected_text_range(false, window, cx).unwrap().range,
                4..4
            );

            // Cancelling a composition removes only the marked text.
            input.replace_and_mark_text_in_range(None, "ㄱ", None, window, cx);
            input.replace_and_mark_text_in_range(None, "", None, window, cx);
            assert_eq!(input.text(), "🦀 한");
            assert_eq!(input.marked_text_range(window, cx), None);

            // The crab is one caret stop but two UTF-16 units.
            let mut adjusted = None;
            let crab = input.text_for_range(0..2, &mut adjusted, window, cx);
            assert_eq!((crab.as_deref(), adjusted), (Some("🦀"), Some(0..2)));
            input.replace_text_in_range(Some(0..2), "게", window, cx);
            assert_eq!(input.text(), "게 한");

            // The field is single-line whatever the source of the text.
            input.replace_text_in_range(Some(0..3), "a\r\nb\nc", window, cx);
            assert_eq!(input.text(), "a b c");
        });
        assert!(changes.get() > 0);

        let before = changes.get();
        input.update_in(cx, |input, window, cx| {
            input.move_to(0, window, cx);
            input.backspace(&Backspace, window, cx);
        });
        assert_eq!(
            changes.get(),
            before,
            "an edit that changes nothing is not reported"
        );
    }
}
