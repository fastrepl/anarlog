mod calendar;
mod form;

pub(crate) use self::calendar::{CalendarComponent, CalendarPhase};
pub(crate) use self::form::{FormField, FormFieldId, validate_base_url};

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::commands::connect::{ConnectProvider, ConnectionType};

use self::calendar::ComponentResult;
use self::form::{FormOutcome, FormState};
use super::action::Action;
use super::effect::{Effect, SaveData};
use super::providers::PROVIDERS;
use super::runtime::RuntimeEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    SelectProvider,
    InputForm,
    Calendar,
    Done,
}

pub(crate) struct App {
    step: Step,
    type_filter: Option<ConnectionType>,
    provider: Option<ConnectProvider>,
    base_url: Option<String>,
    api_key: Option<String>,
    list_state: ListState,
    search_query: String,
    form: FormState,
    calendar: CalendarComponent,
    configured_providers: HashSet<String>,
    current_stt_provider: Option<String>,
    current_llm_provider: Option<String>,
}

impl App {
    pub fn new(
        type_filter: Option<ConnectionType>,
        provider: Option<ConnectProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> (Self, Vec<Effect>) {
        Self::new_with_configured(
            type_filter,
            provider,
            base_url,
            api_key,
            HashSet::new(),
            None,
            None,
        )
    }

    pub fn new_with_configured(
        type_filter: Option<ConnectionType>,
        provider: Option<ConnectProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
        configured_providers: HashSet<String>,
        current_stt_provider: Option<String>,
        current_llm_provider: Option<String>,
    ) -> (Self, Vec<Effect>) {
        let mut app = Self {
            step: Step::SelectProvider,
            type_filter,
            provider,
            base_url,
            api_key,
            list_state: ListState::default(),
            search_query: String::new(),
            form: FormState::empty(),
            calendar: CalendarComponent::new(),
            configured_providers,
            current_stt_provider,
            current_llm_provider,
        };
        let effects = app.advance();
        (app, effects)
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(text) => self.handle_paste(&text),
            Action::Runtime(event) => self.handle_runtime_event(event),
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn provider(&self) -> Option<ConnectProvider> {
        self.provider
    }

    pub fn form_fields(&self) -> &[FormField] {
        self.form.fields()
    }

    pub fn focused_field(&self) -> usize {
        self.form.focused_field()
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn configured_providers(&self) -> &HashSet<String> {
        &self.configured_providers
    }

    pub fn current_stt_provider(&self) -> Option<&str> {
        self.current_stt_provider.as_deref()
    }

    pub fn current_llm_provider(&self) -> Option<&str> {
        self.current_llm_provider.as_deref()
    }

    pub fn calendar(&self) -> &CalendarComponent {
        &self.calendar
    }

    pub fn calendar_mut(&mut self) -> &mut CalendarComponent {
        &mut self.calendar
    }

    pub fn filtered_providers(&self) -> Vec<ConnectProvider> {
        let query = self.search_query.to_ascii_lowercase();
        PROVIDERS
            .iter()
            .map(|m| m.provider)
            .filter(|p| {
                if let Some(ct) = self.type_filter {
                    if !p.valid_for(ct) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                p.id().to_ascii_lowercase().contains(&query)
                    || p.display_name().to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn breadcrumb(&self) -> String {
        match self.provider {
            Some(p) => p.display_name().to_string(),
            None => String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return vec![Effect::Exit];
        }

        match self.step {
            Step::SelectProvider => self.handle_provider_key(key),
            Step::InputForm => match self.form.handle_key(key) {
                FormOutcome::Nothing => Vec::new(),
                FormOutcome::Confirmed { base_url, api_key } => {
                    self.base_url = base_url;
                    self.api_key = api_key;
                    self.step = Step::Done;
                    self.advance()
                }
            },
            Step::Calendar => {
                let provider = self.provider.unwrap();
                match self.calendar.handle_key(key, provider) {
                    ComponentResult::Effects(e) => e,
                    ComponentResult::Done(e) => {
                        self.step = Step::Done;
                        let mut all = e;
                        all.extend(self.advance());
                        all
                    }
                }
            }
            Step::Done => Vec::new(),
        }
    }

    fn handle_paste(&mut self, text: &str) -> Vec<Effect> {
        match self.step {
            Step::InputForm => {
                self.form.handle_paste(text);
            }
            Step::SelectProvider => {
                self.search_query.push_str(text);
                self.list_state.select(Some(0));
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_provider_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let filtered = self.filtered_providers();
        let len = filtered.len();

        match key.code {
            KeyCode::Up => {
                self.list_navigate(-1, len);
                Vec::new()
            }
            KeyCode::Down => {
                self.list_navigate(1, len);
                Vec::new()
            }
            KeyCode::Enter => {
                if len == 0 {
                    return Vec::new();
                }
                let idx = self.list_state.selected().unwrap_or(0);
                if let Some(&provider) = filtered.get(idx) {
                    if provider.is_disabled() {
                        return Vec::new();
                    }
                    self.provider = Some(provider);
                    self.step = Step::InputForm;
                    self.advance()
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.list_state.select(Some(0));
                Vec::new()
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.list_state.select(Some(0));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn list_navigate(&mut self, direction: isize, len: usize) {
        let current = self.list_state.selected().unwrap_or(0);
        let next = current as isize + direction;
        if next >= 0 && (next as usize) < len {
            self.list_state.select(Some(next as usize));
        }
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) -> Vec<Effect> {
        match self.calendar.handle_runtime_event(event) {
            ComponentResult::Effects(effects) => effects,
            ComponentResult::Done(effects) => {
                self.step = Step::Done;
                let mut all = effects;
                all.extend(self.advance());
                all
            }
        }
    }

    fn advance(&mut self) -> Vec<Effect> {
        loop {
            match self.step {
                Step::SelectProvider => {
                    if self.provider.is_some() {
                        self.step = Step::InputForm;
                        continue;
                    }
                    self.list_state = ListState::default().with_selected(Some(0));
                    return Vec::new();
                }
                Step::InputForm => {
                    let provider = self.provider.unwrap();

                    if provider.is_local() && provider.is_calendar_provider() {
                        self.step = Step::Calendar;
                        return vec![Effect::CheckCalendarPermission];
                    }

                    let form = FormState::setup(provider, &mut self.base_url, &self.api_key);
                    if form.fields().is_empty() {
                        self.step = Step::Done;
                        continue;
                    }

                    self.form = form;
                    return Vec::new();
                }
                Step::Calendar => {
                    return Vec::new();
                }
                Step::Done => {
                    let provider = self.provider.unwrap();
                    let mut connection_types = provider.capabilities();
                    if let Some(ct) = self.type_filter {
                        connection_types.retain(|t| *t == ct);
                    }
                    return vec![Effect::Save(SaveData {
                        connection_types,
                        provider,
                        base_url: self.base_url.clone(),
                        api_key: self.api_key.clone(),
                    })];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::runtime::CalendarPermissionState;
    use super::*;

    #[test]
    fn all_args_provided_produces_save() {
        let (app, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Deepgram),
            Some("https://api.deepgram.com/v1".to_string()),
            Some("key123".to_string()),
        );
        assert_eq!(app.step(), Step::Done);
        assert!(matches!(effects.as_slice(), [Effect::Save(_)]));
    }

    #[test]
    fn no_args_starts_at_select_provider() {
        let (app, effects) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert!(effects.is_empty());
    }

    #[test]
    fn provider_with_default_url_shows_api_key_only() {
        let (app, effects) = App::new(None, Some(ConnectProvider::Deepgram), None, None);
        assert_eq!(app.step(), Step::InputForm);
        assert!(effects.is_empty());
        assert_eq!(app.form_fields().len(), 1);
        assert_eq!(app.form_fields()[0].id, FormFieldId::ApiKey);
    }

    #[test]
    fn custom_provider_shows_both_fields() {
        let (app, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);
        assert!(effects.is_empty());
        assert_eq!(app.form_fields().len(), 2);
        assert_eq!(app.form_fields()[0].id, FormFieldId::BaseUrl);
        assert_eq!(app.form_fields()[1].id, FormFieldId::ApiKey);
    }

    #[test]
    fn local_provider_skips_form() {
        let (app, effects) = App::new(None, Some(ConnectProvider::Ollama), None, None);
        assert_eq!(app.step(), Step::Done);
        assert!(matches!(effects.as_slice(), [Effect::Save(_)]));
    }

    #[test]
    fn search_filters_providers() {
        let (mut app, _) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('m'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('i'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('s'))));

        let filtered = app.filtered_providers();
        assert!(filtered.contains(&ConnectProvider::Mistral));
        assert!(!filtered.contains(&ConnectProvider::Deepgram));
    }

    #[test]
    fn dual_capability_provider_produces_both_types() {
        let (_, effects) = App::new(
            None,
            Some(ConnectProvider::Openai),
            Some("https://api.openai.com/v1".to_string()),
            Some("key".to_string()),
        );
        if let Effect::Save(data) = &effects[0] {
            assert!(data.connection_types.contains(&ConnectionType::Stt));
            assert!(data.connection_types.contains(&ConnectionType::Llm));
        } else {
            panic!("expected Save effect");
        }
    }

    #[test]
    fn type_filter_restricts_connection_types() {
        let (_, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Openai),
            Some("https://api.openai.com/v1".to_string()),
            Some("key".to_string()),
        );
        if let Effect::Save(data) = &effects[0] {
            assert_eq!(data.connection_types, vec![ConnectionType::Stt]);
        } else {
            panic!("expected Save effect");
        }
    }

    #[test]
    fn select_provider_then_input() {
        let (mut app, _) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert_eq!(app.list_state_mut().selected(), Some(0));

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        // First provider (Deepgram) has a default URL, so form shows only API key
        assert_eq!(app.step(), Step::InputForm);
        assert_eq!(app.form_fields().len(), 1);
        assert_eq!(app.form_fields()[0].id, FormFieldId::ApiKey);
    }

    #[test]
    fn base_url_validation_rejects_invalid() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);

        for c in "not-a-url".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert!(app.form_fields()[0].error.is_some());

        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);

        for c in "ftp://example.com".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(
            app.form_fields()[0].error.as_deref(),
            Some("invalid URL: scheme must be http or https")
        );
    }

    #[test]
    fn esc_exits() {
        let (mut app, _) = App::new(None, None, None, None);
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Esc)));
        assert!(matches!(effects.as_slice(), [Effect::Exit]));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_calendar_goes_to_calendar_step() {
        let (app, effects) = App::new(None, Some(ConnectProvider::AppleCalendar), None, None);
        assert_eq!(app.step(), Step::Calendar);
        assert_eq!(app.calendar().phase(), CalendarPhase::Permission);
        assert!(matches!(
            effects.as_slice(),
            [Effect::CheckCalendarPermission]
        ));
    }

    #[test]
    fn authorized_calendar_permission_auto_loads_calendars() {
        let (mut app, _) = App::new(None, Some(ConnectProvider::AppleCalendar), None, None);

        let effects = app.dispatch(Action::Runtime(RuntimeEvent::CalendarPermissionStatus(
            CalendarPermissionState::Authorized,
        )));

        assert_eq!(app.step(), Step::Calendar);
        assert_eq!(app.calendar().phase(), CalendarPhase::Select);
        assert!(app.calendar().loading());
        assert!(matches!(effects.as_slice(), [Effect::LoadCalendars]));
    }

    #[test]
    fn tab_cycles_form_fields() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);
        assert_eq!(app.form_fields().len(), 2);
        assert_eq!(app.focused_field(), 0);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(app.focused_field(), 1);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(app.focused_field(), 0);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::BackTab)));
        assert_eq!(app.focused_field(), 1);
    }
}
