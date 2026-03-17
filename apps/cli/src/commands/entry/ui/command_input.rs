use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

use crate::theme::Theme;

pub struct CursorState {
    pub position: Option<Position>,
}

impl Default for CursorState {
    fn default() -> Self {
        Self { position: None }
    }
}

pub struct CommandInput<'a> {
    value: &'a str,
    cursor_col: usize,
    stt_provider: Option<&'a str>,
    llm_provider: Option<&'a str>,
    theme: &'a Theme,
}

impl<'a> CommandInput<'a> {
    pub fn new(
        value: &'a str,
        cursor_col: usize,
        stt_provider: Option<&'a str>,
        llm_provider: Option<&'a str>,
        theme: &'a Theme,
    ) -> Self {
        Self {
            value,
            cursor_col,
            stt_provider,
            llm_provider,
            theme,
        }
    }
}

impl StatefulWidget for CommandInput<'_> {
    type State = CursorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let accent = self.theme.accent;
        let bg = self.theme.input_bg;
        let margin_h: u16 = 2;
        let inner_pad: u16 = 3;

        // Inset area: horizontal margin on both sides
        let box_x = area.x + margin_h;
        let box_w = area.width.saturating_sub(margin_h * 2);

        // Fill background for the inset box
        let bg_style = Style::new().bg(bg);
        for y in area.y..area.y + area.height {
            for x in box_x..box_x + box_w {
                buf[(x, y)].set_style(bg_style);
            }
        }

        // Draw left accent bar (full height of box)
        for y in area.y..area.y + area.height {
            buf[(box_x, y)].set_char('▎').set_style(accent.bg(bg));
        }

        // Content area (after accent bar + inner padding)
        let content_x = box_x + inner_pad;
        let content_width = box_w.saturating_sub(inner_pad + 1);
        let input_y = area.y + 1;

        let input_line = if self.value.is_empty() {
            let placeholder = if self.stt_provider.is_none() || self.llm_provider.is_none() {
                "/connect"
            } else {
                "/listen"
            };
            Line::from(Span::styled(placeholder, self.theme.placeholder))
        } else {
            Line::from(Span::styled(self.value, bg_style))
        };
        let input_area = Rect {
            x: content_x,
            y: input_y,
            width: content_width,
            height: 1,
        };
        input_line.render(input_area, buf);

        // Status line: fixed below input
        if area.height >= 4 {
            let status_y = input_y + 2;
            let stt_label = self.stt_provider.unwrap_or("none");
            let llm_label = self.llm_provider.unwrap_or("none");
            let status_line = Line::from(vec![
                Span::styled("stt", accent.bg(bg)),
                Span::styled(format!(" {}  ", stt_label), self.theme.muted.bg(bg)),
                Span::styled("llm", accent.bg(bg)),
                Span::styled(format!(" {}", llm_label), self.theme.muted.bg(bg)),
            ]);
            let status_area = Rect {
                x: content_x,
                y: status_y,
                width: content_width,
                height: 1,
            };
            status_line.render(status_area, buf);
        }

        // Cursor position
        let cursor_x = content_x
            .saturating_add(self.cursor_col as u16)
            .min(box_x + box_w.saturating_sub(2));
        state.position = Some(Position {
            x: cursor_x,
            y: input_y,
        });
    }
}
