use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use super::action::Action;
use super::effect::Effect;
use super::list::ModelRow;

pub(crate) struct App {
    models: Vec<ModelRow>,
    list_state: ListState,
    loading: bool,
    error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            list_state: ListState::default(),
            loading: true,
            error: None,
        }
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Loaded(models) => {
                self.loading = false;
                self.models = models;
                if !self.models.is_empty() {
                    self.list_state.select(Some(0));
                }
                Vec::new()
            }
            Action::LoadError(msg) => {
                self.loading = false;
                self.error = Some(msg);
                Vec::new()
            }
        }
    }

    pub fn models(&self) -> &[ModelRow] {
        &self.models
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return vec![Effect::Exit];
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.list_state.select_previous();
                Vec::new()
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list_state.select_next();
                Vec::new()
            }
            KeyCode::Char('q') => vec![Effect::Exit],
            _ => Vec::new(),
        }
    }
}
