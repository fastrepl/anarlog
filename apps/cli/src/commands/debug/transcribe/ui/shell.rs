use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

use super::TracingCapture;
use crate::widgets::{ScrollViewState, render_scrollable};

pub(crate) struct TranscribeShell {
    tracing: Arc<TracingCapture>,
    log_lines: Vec<Line<'static>>,
    log_scroll: ScrollViewState,
    log_autoscroll: bool,
    transcript_scroll: ScrollViewState,
    transcript_autoscroll: bool,
    pub(crate) stream_ended: bool,
}

impl TranscribeShell {
    pub(crate) fn new(tracing: Arc<TracingCapture>) -> Self {
        Self {
            tracing,
            log_lines: Vec::new(),
            log_scroll: ScrollViewState::new(),
            log_autoscroll: true,
            transcript_scroll: ScrollViewState::new(),
            transcript_autoscroll: true,
            stream_ended: false,
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.transcript_scroll.scroll_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.transcript_scroll.scroll_up();
                self.transcript_autoscroll = false;
            }
            KeyCode::Char('G') => {
                self.transcript_scroll.scroll_to_bottom();
                self.transcript_autoscroll = true;
            }
            KeyCode::Char('g') => {
                self.transcript_scroll.scroll_to_top();
                self.transcript_autoscroll = false;
            }
            _ => {}
        }

        false
    }

    pub(crate) fn draw(
        &mut self,
        frame: &mut Frame,
        transcript_title: &str,
        transcript_lines: Vec<Line<'static>>,
        placeholder: &str,
        border_style: Style,
    ) {
        self.log_lines.extend(self.tracing.drain_lines());

        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.draw_log_panel(frame, chunks[0]);
        self.draw_transcript_panel(
            frame,
            chunks[1],
            transcript_title,
            transcript_lines,
            placeholder,
            border_style,
        );
    }

    fn draw_log_panel(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::DarkGray))
            .title(" Log ")
            .padding(Padding::new(1, 1, 0, 0));

        let lines = self.log_lines.clone();
        if self.log_autoscroll {
            self.log_scroll.scroll_to_bottom();
        }
        render_scrollable(frame, lines, Some(block), area, &mut self.log_scroll);
    }

    fn draw_transcript_panel(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        lines: Vec<Line<'static>>,
        placeholder: &str,
        border_style: Style,
    ) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" {title} "))
            .padding(Padding::new(1, 1, 0, 0));

        if lines.is_empty() {
            let message = if self.stream_ended {
                placeholder
            } else {
                "Waiting for speech..."
            };
            let paragraph = Paragraph::new(vec![Line::from(Span::styled(
                message.to_string(),
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ))])
            .block(block);
            frame.render_widget(paragraph, area);
        } else {
            if self.transcript_autoscroll {
                self.transcript_scroll.scroll_to_bottom();
            }
            render_scrollable(frame, lines, Some(block), area, &mut self.transcript_scroll);
        }
    }
}
