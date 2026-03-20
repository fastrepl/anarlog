use crossterm::event::{KeyCode, KeyModifiers};

use super::action::Action;
use super::effect::Effect;

pub(crate) struct App {
    pub(crate) current: String,
    pub(crate) latest: String,
    pub(crate) update_command: String,
    pub(crate) selected: usize,
}

const ITEM_COUNT: usize = 3;

impl App {
    pub(crate) fn new(current: String, latest: String, update_command: String) -> Self {
        Self {
            current,
            latest,
            update_command,
            selected: 0,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Vec<Effect> {
        match (key.code, key.modifiers) {
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                self.selected = self.selected.saturating_sub(1);
                Vec::new()
            }
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                if self.selected + 1 < ITEM_COUNT {
                    self.selected += 1;
                }
                Vec::new()
            }
            (KeyCode::Enter, _) => match self.selected {
                0 => vec![Effect::AcceptUpdate],
                1 => vec![Effect::Skip],
                2 => vec![Effect::SkipVersion],
                _ => Vec::new(),
            },
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                vec![Effect::Skip]
            }
            _ => Vec::new(),
        }
    }
}
