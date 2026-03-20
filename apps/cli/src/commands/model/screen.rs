use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use super::runtime::DownloadEvent;
use crate::widgets::InlineBox;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

enum Phase {
    Downloading(u8),
    Done,
    Failed,
}

pub(super) struct DownloadScreen {
    model_name: String,
    spinner_tick: usize,
    phase: Phase,
}

impl DownloadScreen {
    pub fn new(model_name: String) -> Self {
        Self {
            model_name,
            spinner_tick: 0,
            phase: Phase::Downloading(0),
        }
    }

    pub fn viewport_height(&self) -> u16 {
        InlineBox::viewport_height(3)
    }
}

impl Screen for DownloadScreen {
    type ExternalEvent = DownloadEvent;
    type Output = bool;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return match self.phase {
                        Phase::Done => ScreenControl::Exit(true),
                        _ => ScreenControl::Exit(false),
                    };
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => match self.phase {
                        Phase::Done => ScreenControl::Exit(true),
                        _ => ScreenControl::Exit(false),
                    },
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
            DownloadEvent::Progress(pct) => {
                self.phase = Phase::Downloading(pct);
            }
            DownloadEvent::Completed => {
                self.phase = Phase::Done;
                return ScreenControl::Exit(true);
            }
            DownloadEvent::Failed => {
                self.phase = Phase::Failed;
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

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Model     ", dim),
                Span::raw(&self.model_name),
            ]),
            Line::raw(""),
        ];

        self.spinner_tick = self.spinner_tick.wrapping_add(1);
        let spinner = SPINNER_FRAMES[self.spinner_tick % SPINNER_FRAMES.len()];

        let status_line = match &self.phase {
            Phase::Downloading(pct) => {
                let percent = (*pct).min(99) as usize;
                let filled = percent / 5;
                let empty = 20 - filled;
                let bar = format!("{}{} {}%", "█".repeat(filled), "░".repeat(empty), percent);
                Line::from(vec![
                    Span::styled(format!("{spinner}  "), theme.status_degraded),
                    Span::raw("Downloading ["),
                    Span::raw(bar),
                    Span::raw("]"),
                ])
            }
            Phase::Done => Line::from(vec![
                Span::styled("[✓] ", theme.status_active),
                Span::styled("Download complete", theme.status_active),
            ]),
            Phase::Failed => Line::from(vec![
                Span::styled("[!] ", theme.error),
                Span::styled("Download failed", theme.error),
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
