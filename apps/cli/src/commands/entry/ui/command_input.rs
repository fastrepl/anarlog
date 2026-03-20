use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

use crate::theme::Theme;

#[derive(Default)]
pub struct CursorState {
    pub position: Option<Position>,
}

pub struct CommandInput<'a> {
    value: &'a str,
    cursor_col: usize,
    stt_provider: Option<&'a str>,
    llm_provider: Option<&'a str>,
    theme: &'a Theme,
}

const MARGIN_H: u16 = 2;
const INNER_PAD: u16 = 3;

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

    fn fill_background(buf: &mut Buffer, area: Rect, box_x: u16, box_w: u16, style: Style) {
        for y in area.y..area.y + area.height {
            for x in box_x..box_x + box_w {
                buf[(x, y)].set_style(style);
            }
        }
    }

    fn draw_accent_bar(buf: &mut Buffer, area: Rect, box_x: u16, style: Style) {
        for y in area.y..area.y + area.height {
            buf[(box_x, y)].set_char('▎').set_style(style);
        }
    }

    fn input_line(&self, bg_style: Style) -> Line<'_> {
        if self.value.is_empty() {
            let placeholder = "Press \"/\" to see list of commands";
            Line::from(Span::styled(placeholder, self.theme.placeholder))
        } else {
            Line::from(Span::styled(self.value, bg_style))
        }
    }

    fn status_line(&self, bg: Color) -> Line<'_> {
        let stt_label = self.stt_provider.unwrap_or("none");
        let llm_label = self.llm_provider.unwrap_or("none");
        let accent = self.theme.accent;
        Line::from(vec![
            Span::styled("stt", accent.bg(bg)),
            Span::styled(format!(" {}  ", stt_label), self.theme.muted.bg(bg)),
            Span::styled("llm", accent.bg(bg)),
            Span::styled(format!(" {}", llm_label), self.theme.muted.bg(bg)),
        ])
    }

    fn cursor_position(&self, content_x: u16, box_x: u16, box_w: u16, input_y: u16) -> Position {
        let cursor_x = content_x
            .saturating_add(self.cursor_col as u16)
            .min(box_x + box_w.saturating_sub(2));
        Position {
            x: cursor_x,
            y: input_y,
        }
    }
}

impl StatefulWidget for CommandInput<'_> {
    type State = CursorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let bg = self.theme.input_bg;
        let bg_style = Style::new().bg(bg);

        let box_x = area.x + MARGIN_H;
        let box_w = area.width.saturating_sub(MARGIN_H * 2);
        let content_x = box_x + INNER_PAD;
        let content_width = box_w.saturating_sub(INNER_PAD + 1);
        let input_y = area.y + 1;

        Self::fill_background(buf, area, box_x, box_w, bg_style);
        Self::draw_accent_bar(buf, area, box_x, self.theme.accent.bg(bg));

        let content_rect = |y| Rect {
            x: content_x,
            y,
            width: content_width,
            height: 1,
        };

        self.input_line(bg_style).render(content_rect(input_y), buf);

        if area.height >= 4 {
            self.status_line(bg).render(content_rect(input_y + 2), buf);
        }

        state.position = Some(self.cursor_position(content_x, box_x, box_w, input_y));
    }
}
