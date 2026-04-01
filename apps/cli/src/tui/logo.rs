use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub const LOGO_COLOR: Color = Color::Rgb(0xFD, 0xE6, 0xAE);

const LOGO: [&str; 3] = [" █▌  ▐█ ", "▐▌    ▐▌", " █▌  ▐█ "];

pub fn logo_lines<'a>() -> Vec<Line<'a>> {
    LOGO.iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(LOGO_COLOR))))
        .collect()
}
