use std::io::{self, Stderr};

use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::{Terminal, TerminalOptions, Viewport};

pub(crate) struct RecordViewport {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl RecordViewport {
    pub(crate) fn stderr() -> io::Result<Self> {
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(3),
            },
        )?;
        Ok(Self { terminal })
    }

    pub(crate) fn render(&mut self, lines: &[String; 3]) -> io::Result<()> {
        self.terminal.draw(|frame| {
            render_lines(frame, lines);
        })?;
        Ok(())
    }

    pub(crate) fn clear(&mut self) -> io::Result<()> {
        self.terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new(""), area);
        })?;
        Ok(())
    }
}

fn render_lines(frame: &mut ratatui::Frame, lines: &[String; 3]) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame.area());
    for (i, line) in lines.iter().enumerate() {
        frame.render_widget(Paragraph::new(Line::from(line.as_str())), chunks[i]);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn render_draws_three_lines() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(3),
            },
        )
        .unwrap();

        let lines = [
            "recording mic  00:05".to_string(),
            "16000 Hz  1 ch  5.0s audio".to_string(),
            "out.wav  lvl ||||....".to_string(),
        ];
        terminal
            .draw(|frame| {
                render_lines(frame, &lines);
            })
            .unwrap();

        let buf = terminal.backend().buffer().clone();
        let first_line: String = (0..buf.area.width)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(first_line.contains("recording mic"));
    }
}
