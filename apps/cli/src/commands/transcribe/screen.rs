use std::time::Instant;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};
use owhisper_interface::{batch, stream::StreamResponse};

use crate::output::format_hhmmss;
use crate::widgets::InlineBox;
use hypr_listener2_core::BatchErrorCode;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub enum BatchScreenEvent {
    Started,
    Progress(f64),
    Completed(BatchScreenResult),
    Failed(String),
}

pub struct BatchScreenResult {
    pub batch_response: Option<batch::Response>,
    pub streamed_segments: Vec<StreamResponse>,
    pub failure: Option<(BatchErrorCode, String)>,
}

pub enum BatchScreenOutput {
    Done(BatchScreenResult),
    Aborted,
}

enum Phase {
    Waiting,
    InProgress(f64),
    Done,
    Failed(String),
}

pub struct BatchScreen {
    file_name: String,
    started_at: Instant,
    spinner_tick: usize,
    phase: Phase,
    result: Option<BatchScreenResult>,
}

impl BatchScreen {
    pub fn new(file_name: String, started_at: Instant) -> Self {
        Self {
            file_name,
            started_at,
            spinner_tick: 0,
            phase: Phase::Waiting,
            result: None,
        }
    }

    pub fn viewport_height(&self) -> u16 {
        InlineBox::viewport_height(4)
    }
}

impl Screen for BatchScreen {
    type ExternalEvent = BatchScreenEvent;
    type Output = BatchScreenOutput;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return if self.result.is_some() {
                        ScreenControl::Exit(BatchScreenOutput::Done(self.result.take().unwrap()))
                    } else {
                        ScreenControl::Exit(BatchScreenOutput::Aborted)
                    };
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if self.result.is_some() {
                            ScreenControl::Exit(BatchScreenOutput::Done(
                                self.result.take().unwrap(),
                            ))
                        } else {
                            ScreenControl::Exit(BatchScreenOutput::Aborted)
                        }
                    }
                    _ => ScreenControl::Continue,
                }
            }
            _ => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            BatchScreenEvent::Started => {
                self.phase = Phase::InProgress(0.0);
            }
            BatchScreenEvent::Progress(pct) => {
                self.phase = Phase::InProgress(pct);
            }
            BatchScreenEvent::Completed(result) => {
                self.phase = Phase::Done;
                return ScreenControl::Exit(BatchScreenOutput::Done(result));
            }
            BatchScreenEvent::Failed(msg) => {
                self.phase = Phase::Failed(msg);
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        use crate::theme::Theme;

        let theme = Theme::DEFAULT;
        let dim = Style::default().add_modifier(Modifier::DIM);
        let elapsed = format_hhmmss(self.started_at.elapsed());

        let mut lines = vec![
            Line::from(vec![
                Span::styled("File      ", dim),
                Span::raw(&self.file_name),
            ]),
            Line::from(vec![Span::styled("Elapsed   ", dim), Span::raw(elapsed)]),
            Line::raw(""),
        ];

        self.spinner_tick = self.spinner_tick.wrapping_add(1);
        let spinner = SPINNER_FRAMES[self.spinner_tick % SPINNER_FRAMES.len()];

        let status_line = match &self.phase {
            Phase::Waiting => Line::from(vec![
                Span::styled(format!("{spinner}  "), theme.status_degraded),
                Span::raw("Waiting..."),
            ]),
            Phase::InProgress(pct) => {
                let percent = (*pct * 100.0).round().clamp(0.0, 100.0) as u16;
                let filled = (percent as usize) / 5;
                let empty = 20 - filled;
                let bar = format!("{}{} {}%", "█".repeat(filled), "░".repeat(empty), percent);
                Line::from(vec![
                    Span::styled(format!("{spinner}  "), theme.status_degraded),
                    Span::raw("Transcribing ["),
                    Span::raw(bar),
                    Span::raw("]"),
                ])
            }
            Phase::Done => Line::from(vec![
                Span::styled("[✓] ", theme.status_active),
                Span::styled("Transcription complete", theme.status_active),
            ]),
            Phase::Failed(msg) => Line::from(vec![
                Span::styled("[!] ", theme.error),
                Span::styled(format!("Failed: {msg}"), theme.error),
            ]),
        };
        lines.push(status_line);

        let inner = InlineBox::render(frame);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(80)
    }
}
