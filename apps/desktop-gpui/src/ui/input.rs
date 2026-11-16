use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, DispatchPhase, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseMoveEvent, Pixels, Point, ShapedLine, TextAlign, TextRun, UTF16Selection, UnderlineStyle,
    Window, canvas, div, fill, point, prelude::*, px, size,
};

use super::{
    caret::Caret,
    text::TextBuffer,
    theme::{RADIUS, theme},
};

#[derive(Clone, Debug)]
pub enum InputEvent {
    Changed,
    Submitted,
    Rejected,
}

fn horizontal_scroll(scroll: Pixels, cursor: Pixels, text_width: Pixels, width: Pixels) -> Pixels {
    scroll
        .min(cursor)
        .max(cursor - width)
        .clamp(px(0.), (text_width - width).max(px(0.)))
}

pub struct TextInput {
    pub buffer: TextBuffer,
    focus: FocusHandle,
    layout: Option<(Bounds<Pixels>, ShapedLine)>,
    placeholder: String,
    scroll: Pixels,
    multiline: bool,
    secret: bool,
    inline: bool,
    lines: Vec<(Range<usize>, Bounds<Pixels>, ShapedLine)>,
    dragging: bool,
    caret: Option<Entity<Caret>>,
}

impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            buffer: TextBuffer::default(),
            focus: cx.focus_handle(),
            layout: None,
            placeholder: placeholder.into(),
            scroll: px(0.),
            multiline: false,
            secret: false,
            inline: false,
            lines: Vec::new(),
            dragging: false,
            caret: None,
        }
    }

    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    pub fn secret(mut self) -> Self {
        self.secret = true;
        self.buffer.disable_history();
        self
    }

    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        if self.secret && !text.is_ascii() {
            self.changed(false, cx);
            return;
        }
        self.buffer = TextBuffer::new(text);
        if self.secret {
            self.buffer.disable_history();
        }
        self.scroll = px(0.);
        self.layout = None;
        self.lines.clear();
        self.dragging = false;
        self.reset_caret(cx);
        cx.notify();
    }

    fn reset_caret(&self, cx: &mut App) {
        if let Some(caret) = &self.caret {
            Caret::reset_entity(caret, cx);
        }
    }

    fn changed(&mut self, accepted: bool, cx: &mut Context<Self>) {
        self.reset_caret(cx);
        cx.emit(if accepted {
            InputEvent::Changed
        } else {
            InputEvent::Rejected
        });
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.marked.is_some() {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        let word = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.control
        };
        let line = cfg!(target_os = "macos") && modifiers.platform;
        match event.keystroke.key.as_str() {
            "enter" if self.multiline && !modifiers.secondary(cx) => {
                self.replace_text_in_range(None, "\n", window, cx)
            }
            "enter" => cx.emit(InputEvent::Submitted),
            "z" if modifiers.secondary(cx) => {
                let changed = if modifiers.shift {
                    self.buffer.redo()
                } else {
                    self.buffer.undo()
                };
                if changed {
                    self.changed(true, cx);
                }
            }
            "y" if modifiers.secondary(cx) && !cfg!(target_os = "macos") => {
                if self.buffer.redo() {
                    self.changed(true, cx);
                }
            }
            "a" if modifiers.secondary(cx) => {
                self.buffer.anchor = 0;
                self.buffer.cursor = self.buffer.text.len();
            }
            "c" | "x" if modifiers.secondary(cx) => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.buffer.text[self.buffer.selection()].into(),
                ));
                if event.keystroke.key == "x" {
                    self.replace_text_in_range(None, "", window, cx);
                }
            }
            "v" if modifiers.secondary(cx) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.replace_text_in_range(None, &text, window, cx);
                }
            }
            "left" => self.buffer.move_to(
                if line {
                    self.buffer.line_start()
                } else if word {
                    self.buffer.previous_word()
                } else if !modifiers.shift && !self.buffer.selection().is_empty() {
                    self.buffer.selection().start
                } else {
                    self.buffer.previous()
                },
                modifiers.shift,
            ),
            "right" => self.buffer.move_to(
                if line {
                    self.buffer.line_end()
                } else if word {
                    self.buffer.next_word()
                } else if !modifiers.shift && !self.buffer.selection().is_empty() {
                    self.buffer.selection().end
                } else {
                    self.buffer.next()
                },
                modifiers.shift,
            ),
            "home" => self.buffer.move_to(
                if self.multiline && !modifiers.secondary(cx) {
                    self.buffer.line_start()
                } else {
                    0
                },
                modifiers.shift,
            ),
            "end" => self.buffer.move_to(
                if self.multiline && !modifiers.secondary(cx) {
                    self.buffer.line_end()
                } else {
                    self.buffer.text.len()
                },
                modifiers.shift,
            ),
            "up" | "down" if self.multiline => {
                if line {
                    self.buffer.move_to(
                        if event.keystroke.key == "up" {
                            0
                        } else {
                            self.buffer.text.len()
                        },
                        modifiers.shift,
                    );
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let starts = std::iter::once(0)
                    .chain(self.buffer.text.match_indices('\n').map(|(i, _)| i + 1))
                    .collect::<Vec<_>>();
                let row = starts
                    .partition_point(|start| *start <= self.buffer.cursor)
                    .saturating_sub(1);
                let column = self.buffer.text[starts[row]..self.buffer.cursor]
                    .chars()
                    .count();
                let target = if event.keystroke.key == "up" {
                    row.saturating_sub(1)
                } else {
                    (row + 1).min(starts.len() - 1)
                };
                let line = self.buffer.text[starts[target]..]
                    .split('\n')
                    .next()
                    .unwrap_or("");
                let offset = line
                    .char_indices()
                    .nth(column)
                    .map_or(line.len(), |(i, _)| i);
                self.buffer
                    .move_to(starts[target] + offset, modifiers.shift);
            }
            "backspace" | "delete" => {
                let mut range = self.buffer.selection();
                if range.is_empty() {
                    if event.keystroke.key == "backspace" {
                        range.start = if line {
                            self.buffer.line_start()
                        } else if word {
                            self.buffer.previous_word()
                        } else {
                            self.buffer.previous()
                        };
                    } else {
                        range.end = if line {
                            self.buffer.line_end()
                        } else if word {
                            self.buffer.next_word()
                        } else {
                            self.buffer.next()
                        };
                    }
                }
                self.replace_text_in_range(
                    Some(self.buffer.utf16(range.start)..self.buffer.utf16(range.end)),
                    "",
                    window,
                    cx,
                );
            }
            _ => return,
        }
        self.reset_caret(cx);
        cx.stop_propagation();
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) || !self.focus.is_focused(window) {
            self.dragging = false;
        }
        if self.dragging
            && let Some(index) = self.character_index_for_point(event.position, window, cx)
        {
            self.buffer.move_to(self.buffer.utf8(index), true);
            self.reset_caret(cx);
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.utf8(range.start)..self.buffer.utf8(range.end);
        *actual = Some(self.buffer.utf16(range.start)..self.buffer.utf16(range.end));
        Some(self.buffer.text[range].into())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.buffer.selection();
        Some(UTF16Selection {
            range: self.buffer.utf16(range.start)..self.buffer.utf16(range.end),
            reversed: self.buffer.cursor < self.buffer.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked
            .as_ref()
            .map(|range| self.buffer.utf16(range.start)..self.buffer.utf16(range.end))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        self.reset_caret(cx);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.secret && !text.is_ascii() {
            self.changed(false, cx);
            return;
        }
        let accepted = if self.multiline {
            self.buffer.replace_mode(range, text, None, true)
        } else {
            self.buffer.replace(range, text, None)
        };
        self.changed(accepted, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.secret && !text.is_ascii() {
            self.changed(false, cx);
            return;
        }
        let end = text.encode_utf16().count();
        let accepted = self.buffer.replace_mode(
            range,
            text,
            Some(selected.unwrap_or(end..end)),
            self.multiline,
        );
        self.changed(accepted, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.multiline {
            let start = self.buffer.utf8(range.start);
            let end = self.buffer.utf8(range.end);
            let (range, bounds, line) = self
                .lines
                .iter()
                .find(|(range, _, _)| range.contains(&start) || range.end == start)?;
            return Some(Bounds::from_corners(
                point(
                    bounds.left() + line.x_for_index(start - range.start),
                    bounds.top(),
                ),
                point(
                    bounds.left() + line.x_for_index(end.min(range.end) - range.start),
                    bounds.bottom(),
                ),
            ));
        }
        let (bounds, line) = self.layout.as_ref()?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(self.buffer.utf8(range.start)),
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(self.buffer.utf8(range.end)),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        if self.multiline {
            let (range, bounds, line) = self.lines.iter().min_by_key(|(_, bounds, _)| {
                f32::from(position.y - bounds.center().y).abs() as u32
            })?;
            return Some(
                self.buffer.utf16(
                    (range.start
                        + line
                            .closest_index_for_x(position.x - bounds.left())
                            .min(range.len()))
                    .min(self.buffer.text.len()),
                ),
            );
        }
        let (bounds, line) = self.layout.as_ref()?;
        Some(
            self.buffer.utf16(
                line.closest_index_for_x(position.x - bounds.left())
                    .min(self.buffer.text.len()),
            ),
        )
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let caret = self
            .caret
            .get_or_insert_with(|| Caret::new(&self.focus, window, cx));
        caret.update(cx, |caret, cx| {
            caret.set_enabled(self.buffer.selection().is_empty(), window, cx);
        });
        let colors = theme(window);
        let input = cx.entity();
        div()
            .id("text-input")
            .w_full()
            .overflow_hidden()
            .when(!self.inline, |view| {
                view.px_2()
                    .py_1()
                    .border_1()
                    .border_color(if self.focus.is_focused(window) {
                        colors.ring
                    } else {
                        colors.input
                    })
                    .rounded(px(RADIUS))
                    .bg(colors.card)
            })
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focus.focus(window, cx);
                    cx.stop_propagation();
                    if this.buffer.marked.is_some() {
                        return;
                    }
                    if let Some(index) = this.character_index_for_point(event.position, window, cx)
                    {
                        let index = this.buffer.utf8(index);
                        this.buffer.move_to(index, event.modifiers.shift);
                        if event.click_count == 2 {
                            let word = this.buffer.word_range(index);
                            this.buffer.anchor = word.start;
                            this.buffer.cursor = word.end;
                        } else if event.click_count >= 3 {
                            this.buffer.anchor = this.buffer.line_start();
                            this.buffer.cursor =
                                (this.buffer.line_end() + 1).min(this.buffer.text.len());
                        }
                        this.dragging = true;
                    }
                    this.reset_caret(cx);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(
                canvas(
                    move |_, _, _| (),
                    move |bounds, (), window, cx| {
                        let input_for_drag = input.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                input_for_drag
                                    .update(cx, |this, cx| this.mouse_move(event, window, cx));
                            }
                        });
                        input.update(cx, |this, cx| {
                            if this.multiline {
                                this.paint_multiline(bounds, window, cx);
                                return;
                            }
                            let style = window.text_style();
                            let text = if this.buffer.text.is_empty() {
                                this.placeholder.clone()
                            } else if this.secret {
                                "*".repeat(this.buffer.text.len())
                            } else {
                                this.buffer.text.clone()
                            };
                            let run = TextRun {
                                len: text.len(),
                                font: style.font(),
                                color: if this.buffer.text.is_empty() {
                                    colors.muted_foreground
                                } else {
                                    colors.foreground
                                },
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };
                            let runs = if let Some(marked) = &this.buffer.marked {
                                vec![
                                    TextRun {
                                        len: marked.start,
                                        ..run.clone()
                                    },
                                    TextRun {
                                        len: marked.len(),
                                        underline: Some(UnderlineStyle {
                                            color: Some(colors.foreground),
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
                                .collect()
                            } else {
                                vec![run]
                            };
                            let line =
                                window
                                    .text_system()
                                    .shape_line(text.into(), px(14.), &runs, None);
                            let selected = this.buffer.selection();
                            let cursor_x = line.x_for_index(this.buffer.cursor);
                            let available = (bounds.size.width - px(2.)).max(px(1.));
                            this.scroll = if this.focus.is_focused(window) {
                                horizontal_scroll(this.scroll, cursor_x, line.width, available)
                            } else {
                                px(0.)
                            };
                            let origin = point(bounds.left() - this.scroll, bounds.top());
                            if this.focus.is_focused(window)
                                && (!selected.is_empty()
                                    || this
                                        .caret
                                        .as_ref()
                                        .is_some_and(|caret| caret.read(cx).visible(window)))
                            {
                                let left = line.x_for_index(selected.start);
                                let right = line.x_for_index(selected.end);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(origin.x + left, bounds.top()),
                                        size(
                                            if selected.is_empty() {
                                                px(1.)
                                            } else {
                                                right - left
                                            },
                                            bounds.size.height,
                                        ),
                                    ),
                                    if selected.is_empty() {
                                        colors.foreground
                                    } else {
                                        colors.sidebar_accent
                                    },
                                ));
                            }
                            if let Err(error) = line.paint(
                                origin,
                                bounds.size.height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            ) {
                                tracing::error!(%error, "input text paint failed");
                            }
                            window.handle_input(
                                &this.focus,
                                ElementInputHandler::new(bounds, cx.entity()),
                                cx,
                            );
                            this.layout = Some((Bounds::new(origin, bounds.size), line));
                        });
                    },
                )
                .w_full()
                .h(px(if self.multiline { 144. } else { 24. })),
            )
    }
}

impl TextInput {
    fn paint_multiline(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let colors = theme(window);
        let font = window.text_style().font();
        let text = if self.buffer.text.is_empty() {
            self.placeholder.clone()
        } else {
            self.buffer.text.clone()
        };
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect::<Vec<_>>();
        let row = starts
            .partition_point(|start| *start <= self.buffer.cursor)
            .saturating_sub(1);
        let first = row.saturating_sub(5);
        self.lines.clear();
        for (visible, start) in starts.iter().skip(first).take(6).enumerate() {
            let content = text[*start..].split('\n').next().unwrap_or("");
            let range = *start..*start + content.len();
            let base = TextRun {
                len: content.len(),
                font: font.clone(),
                color: if self.buffer.text.is_empty() {
                    colors.muted_foreground
                } else {
                    colors.foreground
                },
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = if let Some(marked) = self
                .buffer
                .marked
                .as_ref()
                .filter(|marked| marked.start < range.end && marked.end > range.start)
            {
                let start = marked.start.max(range.start) - range.start;
                let end = marked.end.min(range.end) - range.start;
                vec![
                    TextRun {
                        len: start,
                        ..base.clone()
                    },
                    TextRun {
                        len: end - start,
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(colors.foreground),
                            wavy: false,
                        }),
                        ..base.clone()
                    },
                    TextRun {
                        len: range.len() - end,
                        ..base
                    },
                ]
            } else {
                vec![base]
            };
            let line = window.text_system().shape_line(
                content.to_owned().into(),
                px(14.),
                &runs
                    .into_iter()
                    .filter(|run| run.len > 0)
                    .collect::<Vec<_>>(),
                None,
            );
            let local = self
                .buffer
                .cursor
                .saturating_sub(range.start)
                .min(range.len());
            let x = line.x_for_index(local);
            let scroll = (x - bounds.size.width + px(2.)).max(px(0.));
            let origin = point(
                bounds.left() - scroll,
                bounds.top() + px(visible as f32 * 24.),
            );
            let selected = self.buffer.selection();
            if self.focus.is_focused(window)
                && (!selected.is_empty()
                    || self
                        .caret
                        .as_ref()
                        .is_some_and(|caret| caret.read(cx).visible(window)))
                && selected.start <= range.end
                && selected.end >= range.start
            {
                let left =
                    line.x_for_index(selected.start.saturating_sub(range.start).min(range.len()));
                let right =
                    line.x_for_index(selected.end.saturating_sub(range.start).min(range.len()));
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x + left, origin.y),
                        size((right - left).max(px(1.)), px(24.)),
                    ),
                    if selected.is_empty() {
                        colors.foreground
                    } else {
                        colors.sidebar_accent
                    },
                ));
            }
            if let Err(error) = line.paint(origin, px(24.), TextAlign::Left, None, window, cx) {
                tracing::error!(%error, "input text paint failed");
            }
            self.lines.push((
                range,
                Bounds::new(origin, size(bounds.size.width, px(24.))),
                line,
            ));
        }
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, cx.entity()),
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_scroll_resets_after_text_shrinks_or_input_widens() {
        assert_eq!(
            horizontal_scroll(px(800.), px(57.), px(57.), px(226.)),
            px(0.)
        );
        assert_eq!(
            horizontal_scroll(px(800.), px(1024.), px(1024.), px(1200.)),
            px(0.)
        );
        assert_eq!(
            horizontal_scroll(px(800.), px(0.), px(0.), px(226.)),
            px(0.)
        );
    }

    #[test]
    fn horizontal_scroll_keeps_caret_visible_within_text_extent() {
        assert_eq!(
            horizontal_scroll(px(0.), px(1024.), px(1024.), px(226.)),
            px(798.)
        );
        assert_eq!(
            horizontal_scroll(px(798.), px(0.), px(1024.), px(226.)),
            px(0.)
        );
        assert_eq!(
            horizontal_scroll(px(100.), px(200.), px(1024.), px(226.)),
            px(100.)
        );
    }
}
