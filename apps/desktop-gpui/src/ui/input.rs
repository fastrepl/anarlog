use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, ContentMask, Context, DispatchPhase, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, GlobalElementId, InspectorElementId, KeyDownEvent, LayoutId, MouseButton,
    MouseMoveEvent, Pixels, Point, ScrollEvent, ShapedLine, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, canvas, div, fill, point, prelude::*, px,
    relative, rgb, size,
};

use super::{
    caret::Caret,
    text::TextBuffer,
    theme::{RADIUS, is_dark, theme},
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
    breadcrumb: bool,
    muted: bool,
    /// A single-line field's text size; 14px (`text-sm`) unless set.
    text_size: Pixels,
    /// Multi-line layout from the last paint: each source line's byte range, origin and
    /// wrapped shape.
    rows: Vec<(Range<usize>, Point<Pixels>, WrappedLine)>,
    line_height: Pixels,
    /// Shipping's textarea `rows`: the fewest lines a multi-line input shows.
    min_rows: usize,
    /// Lines shown before the input scrolls instead of growing.
    max_rows: Option<usize>,
    /// Takes its parent's height (a fixed-height textarea) and scrolls inside it.
    fill: bool,
    /// Space after each source line, for editors that set lines as paragraphs.
    paragraph_gap: Pixels,
    scroll_y: Pixels,
    viewport_height: Pixels,
    reveal: bool,
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
            breadcrumb: false,
            muted: false,
            text_size: px(14.),
            rows: Vec::new(),
            line_height: px(20.),
            min_rows: 1,
            max_rows: None,
            fill: false,
            paragraph_gap: px(0.),
            scroll_y: px(0.),
            viewport_height: px(0.),
            reveal: false,
            dragging: false,
            caret: None,
        }
    }

    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// Shows at least `min` lines, growing up to `max` (when set) before scrolling.
    pub fn rows(mut self, min: usize, max: Option<usize>) -> Self {
        self.min_rows = min.max(1);
        self.max_rows = max.map(|max| max.max(self.min_rows));
        self
    }

    /// Fills the parent's height, like a fixed-height textarea, scrolling past it.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Space after each source line, like a prompt editor's `p { margin-bottom }`.
    pub fn paragraph_gap(mut self, gap: Pixels) -> Self {
        self.paragraph_gap = gap;
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

    /// Shipping's breadcrumb title: neutral-700 text in light mode, underlined while focused.
    pub fn breadcrumb(mut self) -> Self {
        self.breadcrumb = true;
        self
    }

    /// Muted text, as for shipping's secondary fields (`text-muted-foreground`).
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    /// A single-line field's text size, as for shipping's `text-xs` inputs.
    pub fn text_size(mut self, size: Pixels) -> Self {
        self.text_size = size;
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
        self.scroll_y = px(0.);
        self.layout = None;
        self.rows.clear();
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
        self.reveal = true;
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
                    self.reveal = true;
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let up = event.keystroke.key == "up";
                let target = self
                    .vertical_target(up)
                    .unwrap_or_else(|| self.logical_vertical_target(up));
                self.buffer.move_to(target, modifiers.shift);
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
        self.reveal = true;
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
            self.reveal = true;
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
            let start = self.position_for(self.buffer.utf8(range.start))?;
            let end = self.position_for(self.buffer.utf8(range.end))?;
            let end = if end.y == start.y { end.x } else { start.x };
            return Some(Bounds::from_corners(
                start,
                point(end, start.y + self.line_height),
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
            return self
                .index_for_point(position)
                .map(|index| self.buffer.utf16(index));
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
            .when(self.fill, |view| view.h_full())
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
            .when(self.multiline, |view| {
                view.on_scroll(cx.listener(Self::scroll_wheel))
                    .child(MultilineText {
                        input: input.clone(),
                    })
            })
            .when(!self.multiline, |view| {
                view.child(
                    canvas(
                        move |_, _, _| (),
                        move |bounds, (), window, cx| {
                            let input_for_drag = input.clone();
                            window.on_mouse_event(
                                move |event: &MouseMoveEvent, phase, window, cx| {
                                    if phase == DispatchPhase::Bubble {
                                        input_for_drag.update(cx, |this, cx| {
                                            this.mouse_move(event, window, cx)
                                        });
                                    }
                                },
                            );
                            input.update(cx, |this, cx| {
                                let style = window.text_style();
                                let text = if this.buffer.text.is_empty() {
                                    this.placeholder.clone()
                                } else if this.secret {
                                    "*".repeat(this.buffer.text.len())
                                } else {
                                    this.buffer.text.clone()
                                };
                                let text_color = if this.breadcrumb && !is_dark(window) {
                                    rgb(0x404040).into()
                                } else if this.muted {
                                    colors.muted_foreground
                                } else {
                                    colors.foreground
                                };
                                let underline = (this.breadcrumb
                                    && !this.buffer.text.is_empty()
                                    && this.focus.is_focused(window))
                                .then_some(UnderlineStyle {
                                    color: Some(text_color),
                                    thickness: px(1.),
                                    wavy: false,
                                });
                                let run = TextRun {
                                    len: text.len(),
                                    font: style.font(),
                                    color: if this.buffer.text.is_empty() {
                                        colors.muted_foreground
                                    } else {
                                        text_color
                                    },
                                    background_color: None,
                                    underline,
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
                                let line = window.text_system().shape_line(
                                    text.into(),
                                    this.text_size,
                                    &runs,
                                    None,
                                );
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
                    .h(px(24.)),
                )
            })
    }
}

impl TextInput {
    /// Shipping's textarea height: `rows` lines at least, growing to `max_rows`.
    fn visible_height(&self, content: Pixels, line_height: Pixels) -> Pixels {
        visible_height(content, line_height, self.min_rows, self.max_rows)
    }

    fn display_text(&self) -> String {
        if self.buffer.text.is_empty() {
            self.placeholder.clone()
        } else {
            self.buffer.text.clone()
        }
    }

    /// The caret's top-left corner for a byte offset in the last multi-line layout.
    fn position_for(&self, index: usize) -> Option<Point<Pixels>> {
        let (range, origin, line) = self
            .rows
            .iter()
            .find(|(range, _, _)| index <= range.end)
            .or(self.rows.last())?;
        let local = index.saturating_sub(range.start).min(line.len());
        line.position_for_index(local, self.line_height)
            .map(|position| *origin + position)
    }

    fn index_for_point(&self, position: Point<Pixels>) -> Option<usize> {
        let (range, origin, line) = self
            .rows
            .iter()
            .rev()
            .find(|(_, origin, _)| position.y >= origin.y)
            .or(self.rows.first())?;
        let local = position - *origin;
        let height = line.size(self.line_height).height;
        let local = point(
            local.x,
            local.y.clamp(px(0.), (height - px(1.)).max(px(0.))),
        );
        let index = match line.closest_index_for_position(local, self.line_height) {
            Ok(index) | Err(index) => index,
        };
        Some((range.start + index.min(range.len())).min(self.buffer.text.len()))
    }

    /// Up/down through wrapped rows, as a textarea moves.
    fn vertical_target(&self, up: bool) -> Option<usize> {
        let caret = self.position_for(self.buffer.cursor)?;
        let first = self.rows.first()?.1.y;
        let (range, origin, line) = self.rows.last()?;
        let bottom = origin.y + line.size(self.line_height).height;
        let y = if up {
            caret.y - self.line_height / 2.
        } else {
            caret.y + self.line_height * 1.5
        };
        if y < first {
            Some(0)
        } else if y >= bottom {
            Some(range.end.min(self.buffer.text.len()))
        } else {
            self.index_for_point(point(caret.x, y))
        }
    }

    /// Up/down by source line and character column, before anything was painted.
    fn logical_vertical_target(&self, up: bool) -> usize {
        let starts = std::iter::once(0)
            .chain(self.buffer.text.match_indices('\n').map(|(i, _)| i + 1))
            .collect::<Vec<_>>();
        let row = starts
            .partition_point(|start| *start <= self.buffer.cursor)
            .saturating_sub(1);
        let column = self.buffer.text[starts[row]..self.buffer.cursor]
            .chars()
            .count();
        let target = if up {
            row.saturating_sub(1)
        } else {
            (row + 1).min(starts.len() - 1)
        };
        let line = self.buffer.text[starts[target]..]
            .split('\n')
            .next()
            .unwrap_or("");
        starts[target]
            + line
                .char_indices()
                .nth(column)
                .map_or(line.len(), |(i, _)| i)
    }

    fn scroll_wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((_, origin, line)) = self.rows.last() else {
            return;
        };
        let Some(top) = self
            .rows
            .first()
            .map(|(_, origin, _)| origin.y + self.scroll_y)
        else {
            return;
        };
        let content = origin.y + self.scroll_y + line.size(self.line_height).height - top;
        let viewport = self.viewport_height;
        if content <= viewport {
            return;
        }
        let delta = event.delta.pixel_delta(self.line_height).y;
        let scroll = (self.scroll_y - delta).clamp(px(0.), content - viewport);
        if scroll != self.scroll_y {
            self.scroll_y = scroll;
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn paint_multiline(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let colors = theme(window);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style.line_height_in_pixels(window.rem_size());
        let text = self.display_text();
        let base = TextRun {
            len: text.len(),
            font: style.font(),
            color: if self.buffer.text.is_empty() || self.muted {
                colors.muted_foreground
            } else {
                colors.foreground
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match &self.buffer.marked {
            Some(marked) if !self.buffer.text.is_empty() => vec![
                TextRun {
                    len: marked.start,
                    ..base.clone()
                },
                TextRun {
                    len: marked.len(),
                    underline: Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(colors.foreground),
                        wavy: false,
                    }),
                    ..base.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..base
                },
            ],
            _ => vec![base],
        };
        let lines = window
            .text_system()
            .shape_text(
                text.clone().into(),
                font_size,
                &runs,
                Some(bounds.size.width),
                None,
            )
            .unwrap_or_default();
        self.line_height = line_height;
        self.viewport_height = bounds.size.height;
        let mut start = 0;
        let mut top = px(0.);
        let mut rows = Vec::with_capacity(lines.len());
        for line in lines {
            let range = start..start + line.len();
            start = range.end + 1;
            let height = line.size(line_height).height;
            rows.push((range, point(px(0.), top), line));
            top += height + self.paragraph_gap;
        }
        let content = top - self.paragraph_gap;
        self.rows = rows;
        if std::mem::take(&mut self.reveal)
            && self.focus.is_focused(window)
            && let Some(caret) = self.position_for(self.buffer.cursor)
        {
            self.scroll_y = reveal_scroll(self.scroll_y, caret.y, line_height, bounds.size.height);
        }
        self.scroll_y = self
            .scroll_y
            .clamp(px(0.), (content - bounds.size.height).max(px(0.)));
        let offset = point(bounds.left(), bounds.top() - self.scroll_y);
        for (_, origin, _) in &mut self.rows {
            *origin += offset;
        }
        let focused = self.focus.is_focused(window);
        let caret_visible = self
            .caret
            .as_ref()
            .is_some_and(|caret| caret.read(cx).visible(window));
        let selected = self.buffer.selection();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if focused && !self.buffer.text.is_empty() && !selected.is_empty() {
                for (range, origin, line) in &self.rows {
                    if selected.start > range.end || selected.end < range.start {
                        continue;
                    }
                    for row in visual_rows(line) {
                        let row = row.start + range.start..row.end + range.start;
                        let from = selected.start.max(row.start);
                        let to = selected.end.min(row.end);
                        if from > to || (from == to && to != row.end) {
                            continue;
                        }
                        let x = |index: usize| {
                            line.unwrapped_layout.x_for_index(index - range.start)
                                - line.unwrapped_layout.x_for_index(row.start - range.start)
                        };
                        let row_top = line
                            .position_for_index(row.start - range.start, line_height)
                            .map_or(px(0.), |position| position.y);
                        let left = x(from);
                        // A selected line break shows as a sliver, like a browser textarea.
                        let width = (x(to) - left).max(if to == row.end && selected.end > to {
                            px(4.)
                        } else {
                            px(0.)
                        });
                        window.paint_quad(fill(
                            Bounds::new(
                                point(origin.x + left, origin.y + row_top),
                                size(width, line_height),
                            ),
                            colors.sidebar_accent,
                        ));
                    }
                }
            } else if focused && caret_visible && selected.is_empty() {
                let caret = if self.buffer.text.is_empty() {
                    self.rows.first().map(|(_, origin, _)| *origin)
                } else {
                    self.position_for(self.buffer.cursor)
                };
                if let Some(caret) = caret {
                    window.paint_quad(fill(
                        Bounds::new(caret, size(px(1.), line_height)),
                        colors.foreground,
                    ));
                }
            }
            for (_, origin, line) in &self.rows {
                if origin.y > bounds.bottom() {
                    break;
                }
                if let Err(error) =
                    line.paint(*origin, line_height, TextAlign::Left, None, window, cx)
                {
                    tracing::error!(%error, "input text paint failed");
                }
            }
        });
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, cx.entity()),
            cx,
        );
    }
}

fn visible_height(content: Pixels, line_height: Pixels, min: usize, max: Option<usize>) -> Pixels {
    let height = content.max(line_height * min as f32);
    max.map_or(height, |max| height.min(line_height * max as f32))
}

/// The scroll that keeps a caret row inside the viewport, moving as little as possible.
fn reveal_scroll(
    scroll: Pixels,
    caret_top: Pixels,
    line_height: Pixels,
    viewport: Pixels,
) -> Pixels {
    scroll
        .min(caret_top)
        .max(caret_top + line_height - viewport)
        .max(px(0.))
}

/// Byte ranges of a wrapped line's visual rows.
fn visual_rows(line: &WrappedLine) -> Vec<Range<usize>> {
    let mut starts = vec![0];
    for boundary in line.wrap_boundaries() {
        starts.push(line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index);
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| *start..starts.get(index + 1).copied().unwrap_or(line.len()))
        .collect()
}

/// The multi-line input's text area. Its height comes from wrapping at the width layout
/// gives it, so it is measured rather than sized from the previous frame.
struct MultilineText {
    input: Entity<TextInput>,
}

impl Element for MultilineText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let input = self.input.clone();
        let (fill, gap) = (input.read(cx).fill, input.read(cx).paragraph_gap);
        let style = window.text_style();
        let font = style.font();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = style.line_height_in_pixels(window.rem_size());
        let mut layout = Style::default();
        layout.size.width = relative(1.).into();
        if fill {
            layout.size.height = relative(1.).into();
        }
        let layout_id =
            window.request_measured_layout(layout, move |known, available, window, cx| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let height = known.height.unwrap_or_else(|| {
                    let input = input.read(cx);
                    let text = input.display_text();
                    let run = TextRun {
                        len: text.len(),
                        font: font.clone(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let content = window
                        .text_system()
                        .shape_text(text.into(), font_size, &[run], width, None)
                        .map(|lines| {
                            lines.iter().fold(-gap, |height, line| {
                                height + line.size(line_height).height + gap
                            })
                        })
                        .unwrap_or(line_height);
                    input.visible_height(content, line_height)
                });
                size(width.unwrap_or_default(), height)
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let input = self.input.clone();
        window.on_mouse_event({
            let input = input.clone();
            move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    input.update(cx, |this, cx| this.mouse_move(event, window, cx));
                }
            }
        });
        input.update(cx, |this, cx| this.paint_multiline(bounds, window, cx));
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
    fn multiline_rows_grow_from_the_minimum_to_the_cap() {
        let line = px(20.);
        assert_eq!(visible_height(px(20.), line, 3, Some(3)), px(60.));
        assert_eq!(visible_height(px(180.), line, 3, Some(3)), px(60.));
        assert_eq!(visible_height(px(180.), line, 1, None), px(180.));
        assert_eq!(visible_height(px(80.), line, 2, Some(5)), px(80.));
    }

    #[test]
    fn caret_reveal_scrolls_the_least_to_show_its_row() {
        let line = px(20.);
        // Below the viewport: scroll so the caret row's bottom meets the viewport's.
        assert_eq!(reveal_scroll(px(0.), px(100.), line, px(60.)), px(60.));
        // Above it: scroll up to the caret row.
        assert_eq!(reveal_scroll(px(80.), px(20.), line, px(60.)), px(20.));
        // Already visible: unchanged.
        assert_eq!(reveal_scroll(px(20.), px(40.), line, px(60.)), px(20.));
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
