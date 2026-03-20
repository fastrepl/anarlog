mod commands;
mod search;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::{Editor, KeyResult};
use ratatui_image::protocol::StatefulProtocol;

use crate::commands::connect;
use crate::commands::meetings;
use crate::commands::model;
use crate::commands::timeline;

pub(crate) use commands::{ALL_COMMANDS, Command, CommandEntry};

use commands::{load_logo_protocol, lookup, pick_tip};
use search::command_match_score;

use super::action::Action;
use super::effect::Effect;

enum MeetingsIntent {
    View,
    ChatResume,
}

pub(crate) enum Overlay {
    None,
    Connect(connect::app::App),
    Meetings(meetings::app::App),
    Models(model::app::App),
    Timeline(timeline::app::App),
}

pub(crate) struct App {
    input: Editor,
    filtered_commands: Vec<usize>,
    selected_index: usize,
    popup_visible: bool,
    pub(crate) status_message: Option<String>,
    pub(crate) tip: &'static str,
    logo_protocol: Option<StatefulProtocol>,
    pub(crate) stt_provider: Option<String>,
    pub(crate) llm_provider: Option<String>,
    overlay: Overlay,
    meetings_intent: MeetingsIntent,
}

impl App {
    pub(crate) fn new(
        status_message: Option<String>,
        stt_provider: Option<String>,
        llm_provider: Option<String>,
    ) -> Self {
        let mut app = Self {
            input: Editor::single_line(),
            filtered_commands: Vec::new(),
            selected_index: 0,
            popup_visible: false,
            status_message,
            tip: pick_tip(&stt_provider, &llm_provider),
            logo_protocol: load_logo_protocol(),
            stt_provider,
            llm_provider,
            overlay: Overlay::None,
            meetings_intent: MeetingsIntent::View,
        };
        app.recompute_popup();
        app
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::SubmitCommand(command) => self.submit_command(&command),
            Action::StatusMessage(message) => {
                self.status_message = Some(message);
                self.input = Editor::single_line();
                self.recompute_popup();
                Vec::new()
            }
            Action::ConnectRuntime(event) => {
                if let Overlay::Connect(ref mut app) = self.overlay {
                    let effects = app.dispatch(connect::action::Action::Runtime(event));
                    self.translate_connect_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::MeetingsLoaded(meetings) => {
                if let Overlay::Meetings(ref mut app) = self.overlay {
                    let effects = app.dispatch(meetings::action::Action::MeetingsLoaded(meetings));
                    self.translate_meetings_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::MeetingsLoadError(msg) => {
                if let Overlay::Meetings(ref mut app) = self.overlay {
                    let effects = app.dispatch(meetings::action::Action::LoadError(msg));
                    self.translate_meetings_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::EventsLoaded(events) => {
                if let Overlay::Meetings(ref mut app) = self.overlay {
                    let effects = app.dispatch(meetings::action::Action::EventsLoaded(events));
                    self.translate_meetings_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::CalendarNotConfigured => {
                if let Overlay::Meetings(ref mut app) = self.overlay {
                    let effects = app.dispatch(meetings::action::Action::CalendarNotConfigured);
                    self.translate_meetings_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::ModelsLoaded(models) => {
                if let Overlay::Models(ref mut app) = self.overlay {
                    let effects = app.dispatch(model::action::Action::Loaded(models));
                    self.translate_models_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::ModelsLoadError(msg) => {
                if let Overlay::Models(ref mut app) = self.overlay {
                    let effects = app.dispatch(model::action::Action::LoadError(msg));
                    self.translate_models_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::TimelineContactsLoaded { orgs, humans } => {
                if let Overlay::Timeline(ref mut app) = self.overlay {
                    let effects =
                        app.dispatch(timeline::action::Action::ContactsLoaded { orgs, humans });
                    self.translate_timeline_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::TimelineContactsLoadError(msg) => {
                if let Overlay::Timeline(ref mut app) = self.overlay {
                    let effects = app.dispatch(timeline::action::Action::ContactsLoadError(msg));
                    self.translate_timeline_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::TimelineEntriesLoaded(entries) => {
                if let Overlay::Timeline(ref mut app) = self.overlay {
                    let effects = app.dispatch(timeline::action::Action::EntriesLoaded(entries));
                    self.translate_timeline_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::TimelineEntriesLoadError(msg) => {
                if let Overlay::Timeline(ref mut app) = self.overlay {
                    let effects = app.dispatch(timeline::action::Action::EntriesLoadError(msg));
                    self.translate_timeline_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::ConnectSaved {
                connection_types,
                provider_id,
            } => {
                for ct in &connection_types {
                    match ct {
                        crate::commands::connect::ConnectionType::Stt => {
                            self.stt_provider = Some(provider_id.clone());
                        }
                        crate::commands::connect::ConnectionType::Llm => {
                            self.llm_provider = Some(provider_id.clone());
                        }
                        _ => {}
                    }
                }
                self.tip = pick_tip(&self.stt_provider, &self.llm_provider);
                self.status_message = Some("Provider configured".into());
                Vec::new()
            }
        }
    }

    pub(crate) fn reload_logo(&mut self) {
        self.logo_protocol = load_logo_protocol();
    }

    pub(crate) fn logo_protocol(&mut self) -> Option<&mut StatefulProtocol> {
        self.logo_protocol.as_mut()
    }

    pub(crate) fn cursor_col(&self) -> usize {
        self.input.cursor().1
    }

    pub(crate) fn input_text(&self) -> String {
        self.input.lines().first().cloned().unwrap_or_default()
    }

    pub(crate) fn query(&self) -> String {
        self.input_text()
            .trim()
            .trim_start_matches('/')
            .to_ascii_lowercase()
    }

    pub(crate) fn overlay_mut(&mut self) -> &mut Overlay {
        &mut self.overlay
    }

    pub(crate) fn has_overlay(&self) -> bool {
        !matches!(self.overlay, Overlay::None)
    }

    pub(crate) fn popup_visible(&self) -> bool {
        self.popup_visible
    }

    pub(crate) fn visible_commands(&self) -> Vec<CommandEntry> {
        self.filtered_commands
            .iter()
            .filter_map(|&i| {
                let cmd = ALL_COMMANDS.get(i)?;
                Some(CommandEntry {
                    name: cmd.name(),
                    description: cmd.description(),
                    group: cmd.group(),
                    disabled_reason: cmd.disabled_reason(&self.stt_provider, &self.llm_provider),
                })
            })
            .collect()
    }

    fn is_command_disabled(&self, cmd: Command) -> bool {
        cmd.disabled_reason(&self.stt_provider, &self.llm_provider)
            .is_some()
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match self.overlay {
            Overlay::Connect(ref mut app) => {
                let effects = app.dispatch(connect::action::Action::Key(key));
                return self.translate_connect_effects(effects);
            }
            Overlay::Meetings(ref mut app) => {
                let effects = app.dispatch(meetings::action::Action::Key(key));
                return self.translate_meetings_effects(effects);
            }
            Overlay::Models(ref mut app) => {
                let effects = app.dispatch(model::action::Action::Key(key));
                return self.translate_models_effects(effects);
            }
            Overlay::Timeline(ref mut app) => {
                let effects = app.dispatch(timeline::action::Action::Key(key));
                return self.translate_timeline_effects(effects);
            }
            Overlay::None => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        if key.code == KeyCode::Esc {
            self.reset_input();
            return Vec::new();
        }

        if self.popup_visible {
            match key.code {
                KeyCode::Up => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    return Vec::new();
                }
                KeyCode::Down => {
                    let max = self.filtered_commands.len().saturating_sub(1);
                    self.selected_index = (self.selected_index + 1).min(max);
                    return Vec::new();
                }
                KeyCode::Tab => {
                    if let Some(cmd) = self.selected_command_name() {
                        self.set_input_text(cmd.to_string());
                        self.recompute_popup();
                    }
                    return Vec::new();
                }
                _ => {}
            }
        }

        if key.code == KeyCode::Enter {
            if self.popup_visible
                && let Some(cmd) = self.selected_command_name()
            {
                self.set_input_text(cmd.to_string());
            }

            let command = self.input_text().trim().to_string();
            return self.submit_command(&command);
        }

        if self.input.handle_key(key) == KeyResult::Consumed {
            self.status_message = None;
            self.recompute_popup();
        }

        Vec::new()
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        match self.overlay {
            Overlay::Connect(ref mut app) => {
                let effects = app.dispatch(connect::action::Action::Paste(pasted));
                return self.translate_connect_effects(effects);
            }
            Overlay::Meetings(_) => return Vec::new(),
            Overlay::Models(_) => return Vec::new(),
            Overlay::Timeline(_) => return Vec::new(),
            Overlay::None => {}
        }

        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        if !pasted.is_empty() {
            self.input.insert_str(&pasted);
            self.status_message = None;
            self.recompute_popup();
        }
        Vec::new()
    }

    fn submit_command(&mut self, command: &str) -> Vec<Effect> {
        let normalized = command.trim().trim_start_matches('/').to_ascii_lowercase();
        if normalized.is_empty() {
            return Vec::new();
        }

        let Some((cmd, rest)) = lookup(&normalized) else {
            self.status_message = Some(format!("Unknown command: {}", command.trim()));
            return Vec::new();
        };
        if self.is_command_disabled(cmd) {
            return Vec::new();
        }
        self.dispatch_command(cmd, rest)
    }

    fn dispatch_command(&mut self, cmd: Command, rest: &str) -> Vec<Effect> {
        use crate::commands::model::Commands as ModelCommands;

        match cmd {
            Command::MeetingsNew => vec![Effect::Launch(super::EntryCommand::MeetingsNew)],
            Command::Chat => vec![Effect::Launch(super::EntryCommand::Chat {
                session_id: None,
            })],
            Command::ChatResume => {
                self.reset_input();
                self.meetings_intent = MeetingsIntent::ChatResume;
                self.overlay = Overlay::Meetings(meetings::app::App::new());
                vec![Effect::LoadMeetings]
            }
            Command::Meetings => {
                self.reset_input();
                self.meetings_intent = MeetingsIntent::View;
                self.overlay = Overlay::Meetings(meetings::app::App::new());
                vec![Effect::LoadMeetings]
            }
            Command::Timeline => {
                self.reset_input();
                self.overlay = Overlay::Timeline(timeline::app::App::new());
                vec![Effect::LoadTimelineContacts]
            }
            Command::Connect => {
                let (connect_app, initial_effects) = connect::app::App::new(None, None, None, None);
                self.reset_input();
                self.overlay = Overlay::Connect(connect_app);
                self.translate_connect_effects(initial_effects)
            }
            Command::Auth => {
                self.reset_input();
                vec![Effect::OpenAuth]
            }
            Command::Bug => {
                self.reset_input();
                vec![Effect::OpenBug]
            }
            Command::Hello => {
                self.reset_input();
                vec![Effect::OpenHello]
            }
            Command::Desktop => {
                self.reset_input();
                vec![Effect::OpenDesktop]
            }
            Command::Models => {
                self.reset_input();
                self.overlay = Overlay::Models(model::app::App::new());
                vec![Effect::LoadModels]
            }
            Command::ModelsDownload => {
                let name = rest.to_string();
                if name.is_empty() {
                    self.reset_input();
                    self.status_message = Some("Usage: /models download <name>".to_string());
                    Vec::new()
                } else {
                    vec![Effect::RunModel(ModelCommands::Download { name })]
                }
            }
            Command::ModelsDelete => {
                let name = rest.to_string();
                if name.is_empty() {
                    self.reset_input();
                    self.status_message = Some("Usage: /models delete <name>".to_string());
                    Vec::new()
                } else {
                    vec![Effect::RunModel(ModelCommands::Delete { name })]
                }
            }
            Command::ModelsPaths => vec![Effect::RunModel(ModelCommands::Paths)],
            Command::Exit => vec![Effect::Exit],
        }
    }

    fn translate_connect_effects(&mut self, effects: Vec<connect::effect::Effect>) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                connect::effect::Effect::Save(data) => {
                    self.reset_input();
                    result.push(Effect::SaveConnect {
                        connection_types: data.connection_types,
                        provider: data.provider,
                        base_url: data.base_url,
                        api_key: data.api_key,
                    });
                }
                connect::effect::Effect::Exit => {
                    self.reset_input();
                }
                connect::effect::Effect::CheckCalendarPermission => {
                    result.push(Effect::CheckCalendarPermission);
                }
                connect::effect::Effect::RequestCalendarPermission => {
                    result.push(Effect::RequestCalendarPermission);
                }
                connect::effect::Effect::ResetCalendarPermission => {
                    result.push(Effect::ResetCalendarPermission);
                }
                connect::effect::Effect::LoadCalendars => {
                    result.push(Effect::LoadCalendars);
                }
                connect::effect::Effect::SaveCalendars(data) => {
                    result.push(Effect::SaveCalendars(data));
                }
            }
        }
        result
    }

    fn translate_meetings_effects(
        &mut self,
        effects: Vec<meetings::effect::Effect>,
    ) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                meetings::effect::Effect::Select(id) => {
                    let cmd = match self.meetings_intent {
                        MeetingsIntent::View => super::EntryCommand::View { session_id: id },
                        MeetingsIntent::ChatResume => super::EntryCommand::Chat {
                            session_id: Some(id),
                        },
                    };
                    self.reset_input();
                    result.push(Effect::Launch(cmd));
                }
                meetings::effect::Effect::Exit => {
                    self.reset_input();
                }
            }
        }
        result
    }

    fn translate_timeline_effects(
        &mut self,
        effects: Vec<timeline::effect::Effect>,
    ) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                timeline::effect::Effect::LoadTimeline(human_id) => {
                    result.push(Effect::LoadTimelineEntries(human_id));
                }
                timeline::effect::Effect::ViewSession(session_id) => {
                    self.reset_input();
                    result.push(Effect::Launch(super::EntryCommand::View { session_id }));
                }
                timeline::effect::Effect::Exit => {
                    self.reset_input();
                }
            }
        }
        result
    }

    fn translate_models_effects(&mut self, effects: Vec<model::effect::Effect>) -> Vec<Effect> {
        for effect in effects {
            match effect {
                model::effect::Effect::Exit => {
                    self.reset_input();
                }
            }
        }
        Vec::new()
    }

    fn reset_input(&mut self) {
        let had_overlay = self.has_overlay();
        self.overlay = Overlay::None;
        self.input = Editor::single_line();
        self.status_message = None;
        self.recompute_popup();
        if had_overlay {
            self.reload_logo();
        }
    }

    fn selected_command_name(&self) -> Option<&'static str> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        Some(ALL_COMMANDS.get(selected)?.name())
    }

    fn set_input_text(&mut self, value: String) {
        self.input = Editor::single_line();
        self.input.insert_str(&value);
    }

    fn recompute_popup(&mut self) {
        let input = self.input_text();
        let input = input.trim();

        if input.is_empty() {
            self.popup_visible = false;
            self.filtered_commands.clear();
            self.selected_index = 0;
            return;
        }

        self.popup_visible = true;
        let query = input.trim_start_matches('/');
        let mut ranked = ALL_COMMANDS
            .iter()
            .enumerate()
            .filter_map(|(i, command)| {
                command_match_score(query, command.name()).map(|score| (i, score))
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|(left_i, left_score), (right_i, right_score)| {
            right_score.cmp(left_score).then_with(|| {
                ALL_COMMANDS[*left_i]
                    .name()
                    .cmp(ALL_COMMANDS[*right_i].name())
            })
        });

        self.filtered_commands = ranked.into_iter().map(|(i, _)| i).collect();

        use super::ui::command_popup::GROUP_ORDER;
        self.filtered_commands.sort_by_key(|&i| {
            let group = ALL_COMMANDS[i].group();
            GROUP_ORDER
                .iter()
                .position(|&g| g == group)
                .unwrap_or(usize::MAX)
        });

        if self.filtered_commands.is_empty() {
            self.filtered_commands = (0..ALL_COMMANDS.len()).collect();
        }

        self.selected_index = self
            .selected_index
            .min(self.filtered_commands.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_runtime_events_update_connect_overlay() {
        let mut app = App::new(None, None, None);
        let (connect_app, _) = connect::app::App::new(
            None,
            Some(crate::commands::connect::ConnectProvider::AppleCalendar),
            None,
            None,
        );
        app.overlay = Overlay::Connect(connect_app);

        let effects = app.dispatch(Action::ConnectRuntime(
            connect::runtime::RuntimeEvent::CalendarPermissionStatus(
                connect::runtime::CalendarPermissionState::NotDetermined,
            ),
        ));

        assert!(effects.is_empty());

        match app.overlay {
            Overlay::Connect(ref connect_app) => {
                assert_eq!(
                    connect_app.calendar().auth_status(),
                    Some(connect::runtime::CalendarPermissionState::NotDetermined)
                );
            }
            Overlay::None | Overlay::Models(_) | Overlay::Meetings(_) | Overlay::Timeline(_) => {
                panic!("expected connect overlay")
            }
        }
    }

    #[test]
    fn connect_calendar_effects_are_forwarded() {
        let mut app = App::new(None, None, None);

        let effects = app.translate_connect_effects(vec![
            connect::effect::Effect::CheckCalendarPermission,
            connect::effect::Effect::LoadCalendars,
        ]);

        assert_eq!(effects.len(), 2);
        assert!(matches!(effects[0], Effect::CheckCalendarPermission));
        assert!(matches!(effects[1], Effect::LoadCalendars));
    }
}
