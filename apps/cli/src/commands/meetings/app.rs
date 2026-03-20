use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_db_app::{EventRow, MeetingRow};
use ratatui::widgets::ListState;

use super::action::Action;
use super::effect::Effect;

pub(crate) struct App {
    events: Vec<EventRow>,
    meetings: Vec<MeetingRow>,
    calendar_configured: Option<bool>,
    list_state: ListState,
    meetings_loaded: bool,
    events_loaded: bool,
    error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            meetings: Vec::new(),
            calendar_configured: None,
            list_state: ListState::default(),
            meetings_loaded: false,
            events_loaded: false,
            error: None,
        }
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::MeetingsLoaded(meetings) => {
                self.meetings_loaded = true;
                self.meetings = meetings;
                if !self.meetings.is_empty() {
                    self.list_state.select(Some(0));
                }
                Vec::new()
            }
            Action::EventsLoaded(events) => {
                self.events_loaded = true;
                self.calendar_configured = Some(true);
                self.events = events;
                Vec::new()
            }
            Action::CalendarNotConfigured => {
                self.events_loaded = true;
                self.calendar_configured = Some(false);
                Vec::new()
            }
            Action::LoadError(msg) => {
                self.meetings_loaded = true;
                self.events_loaded = true;
                self.error = Some(msg);
                Vec::new()
            }
        }
    }

    pub fn events(&self) -> &[EventRow] {
        &self.events
    }

    pub fn meetings(&self) -> &[MeetingRow] {
        &self.meetings
    }

    pub fn calendar_configured(&self) -> Option<bool> {
        self.calendar_configured
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn loading(&self) -> bool {
        !self.meetings_loaded || !self.events_loaded
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
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(meeting) = self.meetings.get(idx) {
                        return vec![Effect::Select(meeting.id.clone())];
                    }
                }
                Vec::new()
            }
            KeyCode::Char('q') => vec![Effect::Exit],
            _ => Vec::new(),
        }
    }
}
