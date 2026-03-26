mod capture;

pub use capture::{CaptureLayer, TraceBuffer, new_trace_buffer};

use std::io::{self, Stderr};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};

pub const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn truncate_line(s: &str, max: usize) -> &str {
    let char_count = s.chars().count();
    if char_count <= max {
        return s;
    }
    let skip = char_count - max;
    match s.char_indices().nth(skip) {
        Some((byte_idx, _)) => &s[byte_idx..],
        None => s,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Progress,
    Traces,
}

pub struct InlineViewport {
    terminal: Terminal<CrosstermBackend<Stderr>>,
    traces: Option<TraceBuffer>,
    view: View,
    height: u16,
    raw_mode: bool,
}

impl InlineViewport {
    pub fn stderr(height: u16, traces: Option<TraceBuffer>) -> io::Result<Self> {
        let raw_mode = traces.is_some();
        if raw_mode {
            crossterm::terminal::enable_raw_mode()?;
        }
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;
        Ok(Self {
            terminal,
            traces,
            view: View::Progress,
            height,
            raw_mode,
        })
    }

    pub fn poll_toggle(&mut self) {
        if !self.raw_mode {
            return;
        }
        if event::poll(std::time::Duration::ZERO).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            })) = event::read()
            {
                self.view = match self.view {
                    View::Progress => View::Traces,
                    View::Traces => View::Progress,
                };
            }
        }
    }

    pub fn draw(&mut self, lines: &[String]) {
        match self.view {
            View::Progress => self.draw_lines(lines),
            View::Traces => self.draw_traces(),
        }
    }

    fn draw_lines(&mut self, lines: &[String]) {
        let has_traces = self.traces.is_some();
        let mut content: Vec<Line> = lines.iter().map(|s| Line::from(s.as_str())).collect();
        if has_traces {
            content.push(
                Line::from("  press 'd' to toggle traces")
                    .style(Style::default().fg(Color::DarkGray)),
            );
        }

        let _ = self.terminal.draw(|frame| {
            let area = frame.area();
            let block = Block::bordered().border_type(BorderType::Rounded);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let chunks =
                Layout::vertical(vec![Constraint::Length(1); inner.height as usize]).split(inner);
            for (i, line) in content.iter().enumerate() {
                if i < chunks.len() {
                    frame.render_widget(Paragraph::new(line.clone()), chunks[i]);
                }
            }
        });
    }

    fn draw_traces(&mut self) {
        let traces = match self.traces {
            Some(ref buf) => buf,
            None => return,
        };

        let _ = self.terminal.draw(|frame| {
            let area = frame.area();
            let block = Block::bordered().border_type(BorderType::Rounded);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let inner_height = inner.height as usize;
            let trace_lines: Vec<String> = if let Ok(buf) = traces.lock() {
                buf.iter()
                    .rev()
                    .take(inner_height.saturating_sub(1))
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            } else {
                vec![]
            };

            let chunks = Layout::vertical(vec![Constraint::Length(1); inner_height]).split(inner);

            let header = Paragraph::new(Line::from("[traces] press 'd' to toggle"))
                .style(Style::default().fg(Color::DarkGray));
            if !chunks.is_empty() {
                frame.render_widget(header, chunks[0]);
            }

            for (i, line) in trace_lines.iter().enumerate() {
                if i + 1 < chunks.len() {
                    let p = Paragraph::new(Line::from(line.as_str()))
                        .style(Style::default().fg(Color::DarkGray));
                    frame.render_widget(p, chunks[i + 1]);
                }
            }
        });
    }

    pub fn clear(&mut self) -> io::Result<()> {
        use crossterm::{cursor, execute, terminal};

        let area = self.terminal.get_frame().area();
        let height = area.height;

        self.terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(ratatui::widgets::Clear, area);
            frame.render_widget(Paragraph::new(""), area);
        })?;

        let mut stderr = io::stderr();
        execute!(
            stderr,
            cursor::MoveUp(height),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )?;

        if self.raw_mode {
            crossterm::terminal::disable_raw_mode()?;
        }
        Ok(())
    }
}

impl Drop for InlineViewport {
    fn drop(&mut self) {
        if self.raw_mode {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn draw_card(terminal: &mut Terminal<TestBackend>, lines: &[String]) {
        let content: Vec<Line> = lines.iter().map(|s| Line::from(s.as_str())).collect();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let block = Block::bordered().border_type(BorderType::Rounded);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                let chunks = Layout::vertical(vec![Constraint::Length(1); inner.height as usize])
                    .split(inner);
                for (i, line) in content.iter().enumerate() {
                    if i < chunks.len() {
                        frame.render_widget(Paragraph::new(line.clone()), chunks[i]);
                    }
                }
            })
            .unwrap();
    }

    #[test]
    fn render_draws_card_with_three_lines() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(5),
            },
        )
        .unwrap();

        let lines = [
            "recording mic  00:05".to_string(),
            "16000 Hz  1 ch  5.0s audio".to_string(),
            "out.wav  lvl ||||....".to_string(),
        ];
        draw_card(&mut terminal, &lines);

        let buf = terminal.backend().buffer().clone();
        // Row 1 is inside the top border
        let content_line: String = (0..buf.area.width)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content_line.contains("recording mic"));
    }
}
