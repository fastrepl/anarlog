//! The memo editor: caret editing of the note body on top of the TipTap JSON
//! model, with the same persistence cadence as `packages/editor` (changes
//! flushed 500ms after the last keystroke, at most 10s apart).

pub mod model;

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, EntityInputHandler, EventEmitter, FocusHandle, Focusable, KeyBinding,
    Pixels, Point, TextLayout, UTF16Selection, Window, actions,
};

use model::{Caret, Doc};

actions!(
    body_editor,
    [Left, Right, Up, Down, Home, End, Backspace, Delete, Enter,]
);

pub const KEY_CONTEXT: &str = "BodyEditor";
const FLUSH_DEBOUNCE: Duration = Duration::from_millis(500);
const FLUSH_MAX_WAIT: Duration = Duration::from_secs(10);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("enter", Enter, ctx),
    ]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    /// The document changed; the payload is `doc.toJSON()`.
    Flush(String),
}

pub struct BodyEditor {
    focus_handle: FocusHandle,
    pub session_id: String,
    doc: Doc,
    caret: Option<Caret>,
    marked_range: Option<Range<usize>>,
    /// Text layouts captured while painting, one per textblock.
    layouts: Vec<Option<(TextLayout, Bounds<Pixels>)>>,
    dirty_since: Option<Instant>,
    last_input: Option<Instant>,
    flush_scheduled: bool,
}

impl EventEmitter<EditorEvent> for BodyEditor {}

impl BodyEditor {
    pub fn new(session_id: String, body: &str, cx: &mut Context<Self>) -> Self {
        let doc = Doc::parse(body);
        Self {
            focus_handle: cx.focus_handle(),
            session_id,
            layouts: vec![None; doc.textblock_count()],
            doc,
            caret: None,
            marked_range: None,
            dirty_since: None,
            last_input: None,
            flush_scheduled: false,
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn caret(&self) -> Option<Caret> {
        self.caret
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    /// Replace the document from the store unless local edits are pending.
    pub fn replace_body(&mut self, body: &str, cx: &mut Context<Self>) {
        if self.is_dirty() {
            return;
        }
        let doc = Doc::parse(body);
        if doc.to_json() == self.doc.to_json() {
            return;
        }
        self.doc = doc;
        self.layouts = vec![None; self.doc.textblock_count()];
        self.clamp_caret();
        cx.notify();
    }

    pub fn record_layout(&mut self, block: usize, layout: TextLayout, bounds: Bounds<Pixels>) {
        if self.layouts.len() != self.doc.textblock_count() {
            self.layouts = vec![None; self.doc.textblock_count()];
        }
        if let Some(slot) = self.layouts.get_mut(block) {
            *slot = Some((layout, bounds));
        }
    }

    /// Caret position in window coordinates plus the line height, when the
    /// block has been laid out.
    pub fn caret_position(&self) -> Option<(Point<Pixels>, Pixels)> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let position = layout.position_for_index(caret.offset)?;
        Some((position, layout.line_height()))
    }

    pub fn place_caret_at(
        &mut self,
        block: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self
            .layouts
            .get(block)
            .and_then(Option::as_ref)
            .map(|(layout, _)| match layout.index_for_position(position) {
                Ok(index) | Err(index) => index,
            })
            .unwrap_or(0);
        self.doc.ensure_textblock();
        let text = self.doc.text(block);
        self.caret = Some(Caret {
            block: block.min(self.doc.textblock_count().saturating_sub(1)),
            offset: snap(&text, offset.min(text.len())),
        });
        self.focus_handle.focus(window);
        cx.notify();
    }

    /// Clicking below the last block puts the caret at the end, like
    /// `trailing-empty-line-click.ts`.
    pub fn place_caret_at_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        let block = self.doc.textblock_count() - 1;
        self.caret = Some(Caret {
            block,
            offset: self.doc.text(block).len(),
        });
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn clamp_caret(&mut self) {
        if let Some(caret) = &mut self.caret {
            if self.doc.textblock_count() == 0 {
                self.caret = None;
                return;
            }
            caret.block = caret.block.min(self.doc.textblock_count() - 1);
            let text = self.doc.text(caret.block);
            caret.offset = snap(&text, caret.offset.min(text.len()));
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.dirty_since.get_or_insert(now);
        self.last_input = Some(now);
        self.layouts.resize(self.doc.textblock_count(), None);
        self.schedule_flush(cx);
        cx.notify();
    }

    /// `useDebounceCallback(flush, 500, { maxWait: 10_000, trailing: true })`.
    fn schedule_flush(&mut self, cx: &mut Context<Self>) {
        if self.flush_scheduled {
            return;
        }
        self.flush_scheduled = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FLUSH_DEBOUNCE).await;
                let done = this
                    .update(cx, |this, cx| {
                        let now = Instant::now();
                        let quiet = this
                            .last_input
                            .is_none_or(|t| now.duration_since(t) >= FLUSH_DEBOUNCE);
                        let overdue = this
                            .dirty_since
                            .is_some_and(|t| now.duration_since(t) >= FLUSH_MAX_WAIT);
                        if quiet || overdue {
                            this.flush_scheduled = false;
                            this.flush(cx);
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// `flushPendingChanges`: emit the current JSON if anything changed.
    pub fn flush(&mut self, cx: &mut Context<Self>) {
        if let Some(json) = self.take_pending() {
            cx.emit(EditorEvent::Flush(json));
        }
    }

    /// The unsaved JSON, clearing the dirty state, for callers that write it
    /// themselves.
    pub fn take_pending(&mut self) -> Option<String> {
        self.dirty_since.take().map(|_| self.doc.to_json())
    }

    fn move_horizontally(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        let text = self.doc.text(caret.block);
        let next = if delta < 0 {
            if caret.offset == 0 {
                if caret.block == 0 {
                    return;
                }
                let block = caret.block - 1;
                Caret {
                    block,
                    offset: self.doc.text(block).len(),
                }
            } else {
                Caret {
                    block: caret.block,
                    offset: previous_boundary(&text, caret.offset),
                }
            }
        } else if caret.offset >= text.len() {
            if caret.block + 1 >= self.doc.textblock_count() {
                return;
            }
            Caret {
                block: caret.block + 1,
                offset: 0,
            }
        } else {
            Caret {
                block: caret.block,
                offset: next_boundary(&text, caret.offset),
            }
        };
        self.caret = Some(next);
        cx.notify();
    }

    /// Up/down keep the x position, moving a line within the block when the
    /// layout wrapped, otherwise into the neighbouring block.
    fn move_vertically(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        let Some((position, line_height)) = self.caret_position() else {
            return;
        };
        let target_y = position.y + line_height * delta as f32 + line_height / 2.0;
        let within = self
            .layouts
            .get(caret.block)
            .and_then(Option::as_ref)
            .filter(|(_, bounds)| target_y >= bounds.top() && target_y < bounds.bottom())
            .map(|(layout, _)| layout.index_for_position(Point::new(position.x, target_y)));
        let next = match within {
            Some(Ok(index) | Err(index)) => Caret {
                block: caret.block,
                offset: index,
            },
            None => {
                let block = if delta < 0 {
                    if caret.block == 0 {
                        return;
                    }
                    caret.block - 1
                } else {
                    if caret.block + 1 >= self.doc.textblock_count() {
                        return;
                    }
                    caret.block + 1
                };
                let Some((layout, bounds)) = self.layouts.get(block).and_then(Option::as_ref)
                else {
                    return;
                };
                let y = if delta < 0 {
                    bounds.bottom() - line_height / 2.0
                } else {
                    bounds.top() + line_height / 2.0
                };
                let index = match layout.index_for_position(Point::new(position.x, y)) {
                    Ok(index) | Err(index) => index,
                };
                Caret {
                    block,
                    offset: index,
                }
            }
        };
        let text = self.doc.text(next.block);
        self.caret = Some(Caret {
            block: next.block,
            offset: snap(&text, next.offset.min(text.len())),
        });
        cx.notify();
    }

    fn on_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(-1, cx);
    }

    fn on_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(1, cx);
    }

    fn on_up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, cx);
    }

    fn on_down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, cx);
    }

    fn on_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(caret) = &mut self.caret {
            caret.offset = 0;
            cx.notify();
        }
    }

    fn on_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(caret) = self.caret {
            self.caret = Some(Caret {
                block: caret.block,
                offset: self.doc.text(caret.block).len(),
            });
            cx.notify();
        }
    }

    fn on_backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        if caret.offset == 0 {
            if let Some(next) = self.doc.join_backward(caret.block) {
                self.caret = Some(next);
                self.changed(cx);
            }
            return;
        }
        let text = self.doc.text(caret.block);
        let start = previous_boundary(&text, caret.offset);
        self.doc.delete_range(caret.block, start..caret.offset);
        self.caret = Some(Caret {
            block: caret.block,
            offset: start,
        });
        self.changed(cx);
    }

    fn on_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        let text = self.doc.text(caret.block);
        if caret.offset >= text.len() {
            if caret.block + 1 < self.doc.textblock_count() {
                self.doc.join_backward(caret.block + 1);
                self.changed(cx);
            }
            return;
        }
        let end = next_boundary(&text, caret.offset);
        self.doc.delete_range(caret.block, caret.offset..end);
        self.changed(cx);
    }

    fn on_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        self.caret = Some(self.doc.split_block(caret));
        self.changed(cx);
    }

    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        self.caret = Some(self.doc.insert_text(caret, text));
        self.changed(cx);
    }

    fn utf16_to_offset(text: &str, utf16: usize) -> usize {
        let mut count = 0;
        for (index, ch) in text.char_indices() {
            if count >= utf16 {
                return index;
            }
            count += ch.len_utf16();
        }
        text.len()
    }

    fn offset_to_utf16(text: &str, offset: usize) -> usize {
        text[..offset.min(text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    pub fn render_root(&self, cx: &mut Context<Self>) -> gpui::Div {
        use gpui::prelude::*;
        gpui::div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_left))
            .on_action(cx.listener(Self::on_right))
            .on_action(cx.listener(Self::on_up))
            .on_action(cx.listener(Self::on_down))
            .on_action(cx.listener(Self::on_home))
            .on_action(cx.listener(Self::on_end))
            .on_action(cx.listener(Self::on_backspace))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_enter))
    }
}

impl Focusable for BodyEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for BodyEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        let start = Self::utf16_to_offset(&text, range_utf16.start);
        let end = Self::utf16_to_offset(&text, range_utf16.end);
        actual_range
            .replace(Self::offset_to_utf16(&text, start)..Self::offset_to_utf16(&text, end));
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        let offset = Self::offset_to_utf16(&text, caret.offset);
        Some(UTF16Selection {
            range: offset..offset,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        self.marked_range.as_ref().map(|range| {
            Self::offset_to_utf16(&text, range.start)..Self::offset_to_utf16(&text, range.end)
        })
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
        self.doc.ensure_textblock();
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        let text = self.doc.text(caret.block);
        let range = range_utf16
            .map(|r| Self::utf16_to_offset(&text, r.start)..Self::utf16_to_offset(&text, r.end))
            .or_else(|| self.marked_range.clone())
            .unwrap_or(caret.offset..caret.offset);
        if !range.is_empty() {
            self.doc.delete_range(caret.block, range.clone());
        }
        self.caret = Some(Caret {
            block: caret.block,
            offset: range.start,
        });
        self.marked_range = None;
        // Newlines pasted into a textblock become new blocks, as in ProseMirror.
        let mut first = true;
        for line in new_text.split('\n') {
            if !first && let Some(caret) = self.caret {
                self.caret = Some(self.doc.split_block(caret));
            }
            first = false;
            if !line.is_empty() {
                self.insert(line, cx);
            }
        }
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        let text = self.doc.text(caret.block);
        let range = range_utf16
            .map(|r| Self::utf16_to_offset(&text, r.start)..Self::utf16_to_offset(&text, r.end))
            .or_else(|| self.marked_range.clone())
            .unwrap_or(caret.offset..caret.offset);
        if !range.is_empty() {
            self.doc.delete_range(caret.block, range.clone());
        }
        let start = Caret {
            block: caret.block,
            offset: range.start,
        };
        let after = self.doc.insert_text(start, new_text);
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let updated = self.doc.text(caret.block);
        self.caret = Some(match new_selected_range_utf16 {
            Some(selected) => Caret {
                block: caret.block,
                offset: (range.start + Self::utf16_to_offset(new_text, selected.start))
                    .min(updated.len()),
            },
            None => after,
        });
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let text = self.doc.text(caret.block);
        let start = layout.position_for_index(Self::utf16_to_offset(&text, range_utf16.start))?;
        let end = layout.position_for_index(Self::utf16_to_offset(&text, range_utf16.end))?;
        Some(Bounds::from_corners(
            start,
            Point::new(end.x, end.y + layout.line_height()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let text = self.doc.text(caret.block);
        let index = layout.index_for_position(point).ok()?;
        Some(Self::offset_to_utf16(&text, index))
    }
}

fn snap(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn previous_boundary(text: &str, offset: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

fn next_boundary(text: &str, offset: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(text.len())
}
