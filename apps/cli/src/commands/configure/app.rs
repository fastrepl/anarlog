use crossterm::event::{KeyCode, KeyEvent};
use hypr_db_app::CalendarRow;
use ratatui::widgets::ListState;

use super::action::Action;
use super::effect::Effect;
use super::runtime::{CalendarPermissionState, RuntimeEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Stt,
    Llm,
    Calendar,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Stt, Tab::Llm, Tab::Calendar];

    pub fn index(self) -> usize {
        match self {
            Tab::Stt => 0,
            Tab::Llm => 1,
            Tab::Calendar => 2,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Stt => "STT",
            Tab::Llm => "LLM",
            Tab::Calendar => "Calendar",
        }
    }

    pub fn setting_key(self) -> &'static str {
        match self {
            Tab::Stt => "current_stt_provider",
            Tab::Llm => "current_llm_provider",
            Tab::Calendar => unreachable!(),
        }
    }

    pub fn connection_type(self) -> &'static str {
        match self {
            Tab::Stt => "stt",
            Tab::Llm => "llm",
            Tab::Calendar => "cal",
        }
    }
}

pub struct ProviderTab {
    pub current: Option<String>,
    pub providers: Vec<String>,
    pub list_state: ListState,
}

impl ProviderTab {
    fn new() -> Self {
        Self {
            current: None,
            providers: Vec::new(),
            list_state: ListState::default(),
        }
    }

    fn reset_cursor(&mut self) {
        let idx = self
            .current
            .as_ref()
            .and_then(|c| self.providers.iter().position(|p| p == c))
            .unwrap_or(0);
        self.list_state = ListState::default();
        self.list_state.select(Some(idx));
    }
}

pub struct App {
    pub tab: Tab,
    pub stt: ProviderTab,
    pub llm: ProviderTab,
    pub calendars: Vec<CalendarRow>,
    pub cal_cursor: usize,
    pub cal_permission: Option<CalendarPermissionState>,
    pub loading: bool,
    pub error: Option<String>,
}

impl App {
    pub fn new(initial_tab: Option<Tab>) -> (Self, Vec<Effect>) {
        let app = Self {
            tab: initial_tab.unwrap_or(Tab::Stt),
            stt: ProviderTab::new(),
            llm: ProviderTab::new(),
            calendars: Vec::new(),
            cal_cursor: 0,
            cal_permission: None,
            loading: true,
            error: None,
        };

        (
            app,
            vec![
                Effect::LoadSettings,
                Effect::LoadCalendars,
                Effect::CheckCalendarPermission,
            ],
        )
    }

    fn provider_tab(&mut self) -> Option<&mut ProviderTab> {
        match self.tab {
            Tab::Stt => Some(&mut self.stt),
            Tab::Llm => Some(&mut self.llm),
            Tab::Calendar => None,
        }
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Runtime(event) => self.handle_runtime(event),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return vec![Effect::Exit],
            KeyCode::Right | KeyCode::Tab => {
                self.switch_tab(1);
                return vec![];
            }
            KeyCode::Left | KeyCode::BackTab => {
                self.switch_tab(-1);
                return vec![];
            }
            _ => {}
        }

        match self.tab {
            Tab::Stt | Tab::Llm => self.handle_provider_key(key),
            Tab::Calendar => self.handle_calendar_key(key),
        }
    }

    fn handle_provider_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let tab = self.tab;
        let pt = self.provider_tab().unwrap();
        let count = pt.providers.len();

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = pt.list_state.selected().unwrap_or(0);
                pt.list_state.select(Some(i.saturating_sub(1)));
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = pt.list_state.selected().unwrap_or(0);
                pt.list_state
                    .select(Some((i + 1).min(count.saturating_sub(1))));
                vec![]
            }
            KeyCode::Enter => {
                let idx = pt.list_state.selected().unwrap_or(0);
                if let Some(provider) = pt.providers.get(idx).cloned() {
                    pt.current = Some(provider.clone());
                    vec![Effect::SaveProvider { tab, provider }]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let authorized = self.cal_permission == Some(CalendarPermissionState::Authorized);
        if !authorized || self.calendars.is_empty() {
            return vec![];
        }

        let item_count = self.calendars.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cal_cursor = self.cal_cursor.saturating_sub(1);
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cal_cursor = (self.cal_cursor + 1).min(item_count.saturating_sub(1));
                vec![]
            }
            KeyCode::Char(' ') => {
                if let Some(cal) = self.calendars.get_mut(self.cal_cursor) {
                    cal.enabled = !cal.enabled;
                }
                vec![]
            }
            KeyCode::Enter => {
                let calendars = self.calendars.clone();
                vec![Effect::SaveCalendars(calendars)]
            }
            _ => vec![],
        }
    }

    fn handle_runtime(&mut self, event: RuntimeEvent) -> Vec<Effect> {
        match event {
            RuntimeEvent::SettingsLoaded {
                current_stt,
                current_llm,
                stt_providers,
                llm_providers,
            } => {
                self.stt.current = current_stt;
                self.stt.providers = stt_providers;
                self.llm.current = current_llm;
                self.llm.providers = llm_providers;
                self.loading = false;
                self.reset_tab_state();
                vec![]
            }
            RuntimeEvent::CalendarsLoaded(mut calendars) => {
                calendars.sort_by(|a, b| a.source.cmp(&b.source));
                self.calendars = calendars;
                vec![]
            }
            RuntimeEvent::CalendarPermissionStatus(state) => {
                self.cal_permission = Some(state);
                vec![]
            }
            RuntimeEvent::Saved => {
                vec![]
            }
            RuntimeEvent::Error(msg) => {
                self.error = Some(msg);
                vec![]
            }
        }
    }

    fn switch_tab(&mut self, delta: i32) {
        let current = self.tab.index() as i32;
        let count = Tab::ALL.len() as i32;
        let next = (current + delta).rem_euclid(count) as usize;
        self.tab = Tab::ALL[next];
        self.reset_tab_state();
    }

    fn reset_tab_state(&mut self) {
        match self.tab {
            Tab::Stt => self.stt.reset_cursor(),
            Tab::Llm => self.llm.reset_cursor(),
            Tab::Calendar => {
                self.cal_cursor = 0;
            }
        }
    }
}
