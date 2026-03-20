use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};

use crate::output::format_hhmmss;
use crate::widgets::InlineBox;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
pub const AUTO_EXIT_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

pub enum ExitEvent {
    TaskStarted(usize),
    TaskDone(usize),
    TaskFailed(usize, String),
    AllDone,
    AutoExit,
}

enum TaskState {
    Done,
    InProgress,
    NotStarted,
    Failed(String),
}

struct TaskItem {
    label: &'static str,
    state: TaskState,
}

pub struct ExitScreen {
    session_id: String,
    elapsed: std::time::Duration,
    spinner_tick: usize,
    tasks: Vec<TaskItem>,
}

impl ExitScreen {
    pub fn new(
        session_id: String,
        elapsed: std::time::Duration,
        task_labels: Vec<&'static str>,
    ) -> Self {
        let tasks = task_labels
            .into_iter()
            .map(|label| TaskItem {
                label,
                state: TaskState::NotStarted,
            })
            .collect();
        Self {
            session_id,
            elapsed,
            spinner_tick: 0,
            tasks,
        }
    }

    pub fn viewport_height(&self) -> u16 {
        let content = 6 + 1 + self.tasks.len() as u16;
        InlineBox::viewport_height(content)
    }
}

impl Screen for ExitScreen {
    type ExternalEvent = ExitEvent;
    type Output = ();

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return ScreenControl::Exit(());
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => ScreenControl::Exit(()),
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
            ExitEvent::TaskStarted(idx) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::InProgress;
                }
            }
            ExitEvent::TaskDone(idx) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::Done;
                }
            }
            ExitEvent::TaskFailed(idx, msg) => {
                if let Some(task) = self.tasks.get_mut(idx) {
                    task.state = TaskState::Failed(msg);
                }
            }
            ExitEvent::AllDone => {}
            ExitEvent::AutoExit => {
                return ScreenControl::Exit(());
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        use crate::theme::Theme;

        let theme = Theme::DEFAULT;
        let dim = Style::default().add_modifier(Modifier::DIM);
        let chat_cmd = format!(
            "char chat --session {} --api-key <KEY> --model <MODEL>",
            self.session_id
        );

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Session   ", dim),
                Span::raw(&self.session_id),
            ]),
            Line::from(vec![
                Span::styled("Duration  ", dim),
                Span::raw(format_hhmmss(self.elapsed)),
            ]),
            Line::raw(""),
            Line::from(Span::styled("Chat with this session:", dim)),
            Line::raw(""),
            Line::from(Span::styled(
                chat_cmd,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::raw(""),
        ];

        self.spinner_tick = self.spinner_tick.wrapping_add(1);
        let spinner = SPINNER_FRAMES[self.spinner_tick % SPINNER_FRAMES.len()];
        for task in &self.tasks {
            let line = match &task.state {
                TaskState::Done => Line::from(vec![
                    Span::styled("[✓] ", theme.status_active),
                    Span::styled(task.label, dim.add_modifier(Modifier::CROSSED_OUT)),
                ]),
                TaskState::InProgress => Line::from(vec![
                    Span::styled(format!("{spinner}  "), theme.status_degraded),
                    Span::raw(task.label),
                ]),
                TaskState::NotStarted => Line::from(vec![
                    Span::styled("[ ] ", dim),
                    Span::styled(task.label, dim),
                ]),
                TaskState::Failed(msg) => Line::from(vec![
                    Span::styled("[!] ", theme.error),
                    Span::styled(task.label, theme.error),
                    Span::styled(format!(" ({msg})"), dim.fg(Color::Red)),
                ]),
            };
            lines.push(line);
        }

        let inner = InlineBox::render(frame);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(80)
    }
}
