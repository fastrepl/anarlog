use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_db_app::{HumanRow, OrganizationRow, TimelineRow};
use ratatui::widgets::ListState;

use super::action::Action;
use super::effect::Effect;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pane {
    Orgs,
    Humans,
    Timeline,
}

pub(crate) struct App {
    pane: Pane,
    orgs: Vec<OrganizationRow>,
    all_humans: Vec<HumanRow>,
    entries: Vec<TimelineRow>,
    org_state: ListState,
    human_state: ListState,
    entry_state: ListState,
    loading_contacts: bool,
    loading_entries: bool,
    error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            pane: Pane::Orgs,
            orgs: Vec::new(),
            all_humans: Vec::new(),
            entries: Vec::new(),
            org_state: ListState::default().with_selected(Some(0)),
            human_state: ListState::default(),
            entry_state: ListState::default(),
            loading_contacts: true,
            loading_entries: false,
            error: None,
        }
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::ContactsLoaded { orgs, humans } => {
                self.loading_contacts = false;
                self.orgs = orgs;
                self.all_humans = humans;
                self.recompute_humans();
                Vec::new()
            }
            Action::ContactsLoadError(msg) => {
                self.loading_contacts = false;
                self.error = Some(msg);
                Vec::new()
            }
            Action::EntriesLoaded(entries) => {
                self.loading_entries = false;
                self.entries = entries;
                if !self.entries.is_empty() {
                    self.entry_state.select(Some(0));
                } else {
                    self.entry_state.select(None);
                }
                Vec::new()
            }
            Action::EntriesLoadError(msg) => {
                self.loading_entries = false;
                self.error = Some(msg);
                Vec::new()
            }
        }
    }

    pub fn pane(&self) -> Pane {
        self.pane
    }

    pub fn orgs(&self) -> &[OrganizationRow] {
        &self.orgs
    }

    pub fn filtered_humans(&self) -> Vec<&HumanRow> {
        match self.selected_org_id() {
            Some(id) => self.all_humans.iter().filter(|h| h.org_id == id).collect(),
            None => self.all_humans.iter().collect(),
        }
    }

    pub fn entries(&self) -> &[TimelineRow] {
        &self.entries
    }

    pub fn org_state_mut(&mut self) -> &mut ListState {
        &mut self.org_state
    }

    pub fn human_state_mut(&mut self) -> &mut ListState {
        &mut self.human_state
    }

    pub fn entry_state_mut(&mut self) -> &mut ListState {
        &mut self.entry_state
    }

    pub fn loading_contacts(&self) -> bool {
        self.loading_contacts
    }

    pub fn loading_entries(&self) -> bool {
        self.loading_entries
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn selected_human(&self) -> Option<&HumanRow> {
        let idx = self.human_state.selected()?;
        let humans = self.filtered_humans();
        humans.get(idx).copied()
    }

    fn selected_org_id(&self) -> Option<&str> {
        let idx = self.org_state.selected()?;
        if idx == 0 {
            return None;
        }
        self.orgs.get(idx - 1).map(|o| o.id.as_str())
    }

    fn org_list_len(&self) -> usize {
        self.orgs.len() + 1
    }

    fn recompute_humans(&mut self) {
        let humans = self.filtered_humans();
        if humans.is_empty() {
            self.human_state.select(None);
        } else {
            self.human_state.select(Some(0));
        }
        self.entries.clear();
        self.entry_state.select(None);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return vec![Effect::Exit];
        }

        if key.code == KeyCode::Char('q') {
            return vec![Effect::Exit];
        }

        match key.code {
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.pane = match self.pane {
                    Pane::Orgs => Pane::Humans,
                    Pane::Humans => Pane::Timeline,
                    Pane::Timeline => Pane::Timeline,
                };
                Vec::new()
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.pane = match self.pane {
                    Pane::Orgs => Pane::Orgs,
                    Pane::Humans => Pane::Orgs,
                    Pane::Timeline => Pane::Humans,
                };
                Vec::new()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match self.pane {
                    Pane::Orgs => {
                        self.org_state.select_previous();
                        self.recompute_humans();
                    }
                    Pane::Humans => self.human_state.select_previous(),
                    Pane::Timeline => self.entry_state.select_previous(),
                }
                Vec::new()
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.pane {
                    Pane::Orgs => {
                        let max = self.org_list_len().saturating_sub(1);
                        let cur = self.org_state.selected().unwrap_or(0);
                        if cur < max {
                            self.org_state.select(Some(cur + 1));
                        }
                        self.recompute_humans();
                    }
                    Pane::Humans => self.human_state.select_next(),
                    Pane::Timeline => self.entry_state.select_next(),
                }
                Vec::new()
            }
            KeyCode::Enter => self.handle_enter(),
            _ => Vec::new(),
        }
    }

    fn handle_enter(&mut self) -> Vec<Effect> {
        match self.pane {
            Pane::Orgs => {
                self.recompute_humans();
                self.pane = Pane::Humans;
                Vec::new()
            }
            Pane::Humans => {
                if let Some(human) = self.selected_human() {
                    let human_id = human.id.clone();
                    self.loading_entries = true;
                    self.entries.clear();
                    self.entry_state.select(None);
                    self.pane = Pane::Timeline;
                    vec![Effect::LoadTimeline(human_id)]
                } else {
                    Vec::new()
                }
            }
            Pane::Timeline => {
                if let Some(idx) = self.entry_state.selected() {
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.source_type == "meeting" {
                            return vec![Effect::ViewSession(entry.source_id.clone())];
                        }
                    }
                }
                Vec::new()
            }
        }
    }
}
