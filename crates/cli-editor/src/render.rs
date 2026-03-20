use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::{Editor, StyleSheet};

impl<S: StyleSheet> Widget for &Editor<S> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let height = inner.height as usize;
        self.last_known_height.set(inner.height);

        let mut scroll = self.scroll_offset.get();
        if self.cursor.row < scroll {
            scroll = self.cursor.row;
        } else if self.cursor.row >= scroll + height {
            scroll = self.cursor.row - height + 1;
        }
        self.scroll_offset.set(scroll);

        if self.buffer.is_empty() {
            if let Some(ref text) = self.placeholder_text {
                let span = Span::styled(text.as_str(), self.placeholder_style);
                buf.set_line(inner.x, inner.y, &Line::from(span), inner.width);
            }
            render_cursor(buf, inner.x, inner.y, inner);
            return;
        }

        let highlights = if self.highlight_enabled {
            let result = self.highlighter.highlight(self.buffer.lines());
            *self.links.borrow_mut() = result.links;
            result.styles
        } else {
            *self.links.borrow_mut() = Vec::new();
            Vec::new()
        };

        let lines = self.buffer.lines();
        for i in 0..height {
            let line_idx = scroll + i;
            if line_idx >= lines.len() {
                break;
            }

            let y = inner.y + i as u16;

            let line_text = &lines[line_idx];
            let is_cursor_line =
                line_idx == self.cursor.row && self.cursor_line_style != Style::default();

            let rline = if self.highlight_enabled
                && line_idx < highlights.len()
                && !highlights[line_idx].is_empty()
            {
                build_highlighted_line(line_text, &highlights[line_idx])
            } else {
                Line::from(line_text.as_str())
            };

            let rline = if is_cursor_line {
                rline.style(self.cursor_line_style)
            } else {
                rline
            };

            buf.set_line(inner.x, y, &rline, inner.width);

            if line_idx == self.cursor.row {
                let cursor_x = inner.x + self.cursor.col as u16;
                render_cursor(buf, cursor_x, y, inner);
            }
        }
    }
}

impl<S: StyleSheet> Editor<S> {
    pub fn cursor_position(&self, area: Rect) -> Position {
        let inner = if let Some(ref block) = self.block {
            block.inner(area)
        } else {
            area
        };
        let scroll = self.scroll_offset.get();
        let visible_row = self.cursor.row.saturating_sub(scroll);
        let max_x = inner.x + inner.width.saturating_sub(1);
        let max_y = inner.y + inner.height.saturating_sub(1);
        let x = (inner.x + self.cursor.col as u16).min(max_x);
        let y = (inner.y + visible_row as u16).min(max_y);
        Position { x, y }
    }
}

fn render_cursor(buf: &mut Buffer, x: u16, y: u16, inner: Rect) {
    let clamped_x = x.min(inner.x + inner.width.saturating_sub(1));
    if clamped_x < inner.x || y < inner.y || y >= inner.y + inner.height {
        return;
    }
    let cell = &mut buf[(clamped_x, y)];
    if cell.symbol() == " " || cell.symbol().is_empty() {
        cell.set_char(' ');
    }
    cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
}

fn build_highlighted_line<'a>(text: &'a str, spans: &[(Range<usize>, Style)]) -> Line<'a> {
    if spans.is_empty() {
        return Line::from(text);
    }

    let mut sorted: Vec<_> = spans.to_vec();
    sorted.sort_by_key(|(r, _)| r.start);

    let mut result = Vec::new();
    let mut pos = 0;

    for (range, style) in &sorted {
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        if end <= pos {
            continue;
        }
        let effective_start = start.max(pos);
        if effective_start > pos {
            result.push(Span::raw(&text[pos..effective_start]));
        }
        if effective_start < end {
            result.push(Span::styled(&text[effective_start..end], *style));
        }
        pos = end;
    }

    if pos < text.len() {
        result.push(Span::raw(&text[pos..]));
    }

    Line::from(result)
}
