use std::io::{self, Stderr};

use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::capture::TraceBuffer;
use super::input::{BackgroundInput, InputAction, View};
use super::logo::logo_lines;

const MAX_BOX_WIDTH: u16 = 60;

pub struct InlineViewport {
    terminal: Terminal<CrosstermBackend<Stderr>>,
    traces: Option<TraceBuffer>,
    view: View,
    raw_mode: bool,
    input: Option<BackgroundInput>,
}

impl InlineViewport {
    pub fn stderr(height: u16, traces: Option<TraceBuffer>) -> io::Result<Self> {
        Self::stderr_interactive(height, traces, false)
    }

    pub fn stderr_interactive(
        height: u16,
        traces: Option<TraceBuffer>,
        interactive: bool,
    ) -> io::Result<Self> {
        let raw_mode = interactive || traces.is_some();
        let mut input = None;
        if raw_mode {
            crossterm::terminal::enable_raw_mode()?;
            input = Some(BackgroundInput::spawn());
        }
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = match Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        ) {
            Ok(terminal) => terminal,
            Err(error) => {
                if let Some(mut input) = input {
                    input.shutdown();
                }
                if raw_mode {
                    let _ = crossterm::terminal::disable_raw_mode();
                }
                return Err(error);
            }
        };
        Ok(Self {
            terminal,
            traces,
            view: View::Progress,
            raw_mode,
            input,
        })
    }

    /// Poll keyboard input. Handles view-toggle internally and returns
    /// any actions the caller should handle (seek, pause, etc.).
    pub fn poll_input(&mut self) -> Vec<InputAction> {
        if !self.raw_mode {
            return Vec::new();
        }
        let Some(input) = self.input.as_ref() else {
            return Vec::new();
        };
        let mut passthrough = Vec::new();
        while let Ok(action) = input.try_recv() {
            if action == InputAction::ToggleView {
                self.view = match self.view {
                    View::Progress => View::Traces,
                    View::Traces => View::Progress,
                };
            } else {
                passthrough.push(action);
            }
        }
        passthrough
    }

    fn shutdown_input(&mut self) {
        if let Some(mut input) = self.input.take() {
            input.shutdown();
        }
    }

    fn teardown_raw_mode(&mut self) -> io::Result<()> {
        if !self.raw_mode {
            return Ok(());
        }
        self.shutdown_input();
        crossterm::terminal::disable_raw_mode()?;
        self.raw_mode = false;
        Ok(())
    }

    pub fn draw(&mut self, lines: &[Line]) {
        match self.view {
            View::Progress => self.draw_lines(lines),
            View::Traces => self.draw_traces(),
        }
    }

    pub fn draw_strings(&mut self, lines: &[String]) {
        let styled: Vec<Line> = lines.iter().map(|s| Line::from(s.clone())).collect();
        self.draw(&styled);
    }

    fn draw_lines(&mut self, lines: &[Line]) {
        let logo = logo_lines();
        let logo_width = 10;

        let row_count = lines.len().max(logo.len());
        let mut content: Vec<Line> = Vec::with_capacity(row_count + 1);
        for i in 0..row_count {
            let mut spans: Vec<Span> = Vec::new();
            if i < logo.len() {
                spans.extend(logo[i].spans.iter().cloned());
                spans.push(Span::raw("  "));
            } else {
                spans.push(Span::raw(" ".repeat(logo_width + 2)));
            }
            if i < lines.len() {
                spans.extend(lines[i].spans.iter().cloned());
            }
            content.push(Line::from(spans));
        }

        if self.traces.is_some() {
            content.push(Line::from(vec![
                Span::raw(" ".repeat(logo_width + 2)),
                Span::styled(
                    "press 'd' to toggle traces",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        let _ = self.terminal.draw(|frame| {
            let full = frame.area();
            let capped_width = full.width.min(MAX_BOX_WIDTH);
            let area = Layout::horizontal([Constraint::Length(capped_width)])
                .flex(Flex::Start)
                .split(full)[0];
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray));
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
            let full = frame.area();
            let capped_width = full.width.min(MAX_BOX_WIDTH);
            let area = Layout::horizontal([Constraint::Length(capped_width)])
                .flex(Flex::Start)
                .split(full)[0];
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray));
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

    /// Tear down raw mode but leave the last-drawn content visible.
    /// The caller must have called `draw()` with the final content
    /// immediately before calling this.
    pub fn finish(&mut self) -> io::Result<()> {
        self.teardown_raw_mode()?;
        // After an inline draw, ratatui leaves the cursor on the last
        // viewport row (the bottom border).  We need to move past it
        // so the shell prompt doesn't overwrite the border.
        let mut stderr = io::stderr();
        crossterm::execute!(stderr, crossterm::cursor::MoveDown(1))?;
        eprintln!();
        Ok(())
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

        self.teardown_raw_mode()?;
        Ok(())
    }
}

impl Drop for InlineViewport {
    fn drop(&mut self) {
        let _ = self.teardown_raw_mode();
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
        let content_line: String = (0..buf.area.width)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content_line.contains("recording mic"));
    }
}
