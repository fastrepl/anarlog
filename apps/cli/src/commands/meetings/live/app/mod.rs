mod state;
mod ui_state;

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use hypr_transcript::SpeakerLabelContext;

use crate::theme::Theme;
use hypr_listener_core::State;
use ratatui::layout::Rect;

use self::state::LiveState;
use self::ui_state::LiveUiState;
use super::action::Action;
use super::effect::Effect;
pub(crate) use ui_state::Mode;

pub(crate) struct App {
    state: LiveState,
    ui: LiveUiState,
    participant_names: HashMap<String, String>,
}

impl App {
    pub(crate) fn new(participant_names: HashMap<String, String>) -> Self {
        Self {
            state: LiveState::new(),
            ui: LiveUiState::new(),
            participant_names,
        }
    }

    pub(crate) fn speaker_label_context(&self) -> Option<SpeakerLabelContext> {
        if self.participant_names.is_empty() {
            return None;
        }
        Some(SpeakerLabelContext {
            self_human_id: None,
            human_name_by_id: self.participant_names.clone(),
        })
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.state.elapsed()
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::RuntimeEvent(event) => {
                self.state.handle_runtime_event(event);
                Vec::new()
            }
        }
    }

    pub(crate) fn apply_runtime_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = super::runtime::RuntimeEvent>,
    {
        self.state.apply_runtime_events(events);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit { force: false }];
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(self.ui.mode(), Mode::Normal | Mode::Insert)
        {
            match key.code {
                KeyCode::Left => {
                    self.ui.adjust_notepad_width(-2);
                    return Vec::new();
                }
                KeyCode::Right => {
                    self.ui.adjust_notepad_width(2);
                    return Vec::new();
                }
                _ => {}
            }
        }

        match self.ui.mode() {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Insert => self.handle_insert_key(key),
            Mode::Command => self.handle_command_key(key),
        }
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.ui.mode() != Mode::Insert {
            return Vec::new();
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.ui.memo_mut().insert_str(&pasted);
        Vec::new()
    }

    pub(crate) fn mode(&self) -> Mode {
        self.ui.mode()
    }

    pub(crate) fn memo_focused(&self) -> bool {
        self.ui.mode() == Mode::Insert
    }

    pub(crate) fn transcript_focused(&self) -> bool {
        self.ui.mode() == Mode::Normal
    }

    pub(crate) fn memo_mut(&mut self) -> &mut Editor<Theme> {
        self.ui.memo_mut()
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut crate::widgets::ScrollViewState {
        self.ui.scroll_state_mut()
    }

    pub(crate) fn notepad_width_percent(&self) -> u16 {
        self.ui.notepad_width_percent()
    }

    pub(crate) fn listener_state(&self) -> State {
        self.state.listener_state()
    }

    pub(crate) fn status(&self) -> &str {
        self.state.status()
    }

    pub(crate) fn degraded(&self) -> Option<&hypr_listener_core::DegradedError> {
        self.state.degraded()
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.state.last_error()
    }

    pub(crate) fn mic_muted(&self) -> bool {
        self.state.mic_muted()
    }

    pub(crate) fn mic_history(&self) -> &std::collections::VecDeque<u64> {
        self.state.mic_history()
    }

    pub(crate) fn speaker_history(&self) -> &std::collections::VecDeque<u64> {
        self.state.speaker_history()
    }

    pub(crate) fn word_count(&self) -> usize {
        self.state.word_count()
    }

    pub(crate) fn words(&self) -> Vec<hypr_transcript::FinalizedWord> {
        self.state.words().to_vec()
    }

    pub(crate) fn hints(&self) -> Vec<hypr_transcript::RuntimeSpeakerHint> {
        self.state.hints().to_vec()
    }

    pub(crate) fn memo_text(&self) -> String {
        self.ui.memo().text()
    }

    pub(crate) fn command_buffer(&self) -> &str {
        self.ui.command_buffer()
    }

    pub(crate) fn segments(&self) -> Vec<hypr_transcript::Segment> {
        self.state.segments()
    }

    pub(crate) fn word_age_secs(&self, id: &str) -> f64 {
        self.state.word_age_secs(id)
    }

    pub(crate) fn frame_elapsed(&mut self) -> std::time::Duration {
        self.ui.frame_elapsed()
    }

    pub(crate) fn check_new_segments(&mut self, current_count: usize, transcript_area: Rect) {
        self.ui.check_new_segments(current_count, transcript_area);
    }

    pub(crate) fn process_effects(
        &mut self,
        elapsed: std::time::Duration,
        buf: &mut ratatui::buffer::Buffer,
        area: Rect,
    ) {
        self.ui.process_effects(elapsed, buf, area);
    }

    pub(crate) fn has_active_animations(&self) -> bool {
        self.ui.has_active_effects() || self.state.has_recent_words()
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char(':') => {
                self.ui.enter_command_mode();
            }
            KeyCode::Char('i') | KeyCode::Char('m') | KeyCode::Char('a') | KeyCode::Tab => {
                self.ui.enter_insert_mode();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui.scroll_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui.scroll_up();
            }
            KeyCode::Char('G') => {
                self.ui.scroll_bottom();
            }
            KeyCode::Char('g') => {
                self.ui.scroll_top();
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_insert_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc || key.code == KeyCode::Tab {
            self.ui.enter_normal_mode();
            return Vec::new();
        }

        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.ui.reset_memo();
            return Vec::new();
        }

        self.ui.memo_mut().handle_key(key);

        Vec::new()
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.ui.enter_normal_mode();
                self.ui.clear_command_buffer();
            }
            KeyCode::Enter => {
                return self.execute_command();
            }
            KeyCode::Backspace => {
                if self.ui.command_buffer().is_empty() {
                    self.ui.enter_normal_mode();
                } else {
                    self.ui.pop_command_char();
                }
            }
            KeyCode::Char(c) => {
                self.ui.push_command_char(c);
            }
            _ => {}
        }
        Vec::new()
    }

    fn execute_command(&mut self) -> Vec<Effect> {
        let cmd = self.ui.command_buffer().trim().to_string();
        self.ui.clear_command_buffer();
        self.ui.enter_normal_mode();

        match cmd.as_str() {
            "q" | "quit" => {
                vec![Effect::Exit { force: false }]
            }
            "q!" | "quit!" => {
                vec![Effect::Exit { force: true }]
            }
            _ => {
                self.state.push_error(format!("Unknown command: :{cmd}"));
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_exits_without_force() {
        let mut app = App::new(HashMap::new());

        let effects = app.dispatch(Action::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));

        assert!(matches!(
            effects.as_slice(),
            [Effect::Exit { force: false }]
        ));
    }
}
