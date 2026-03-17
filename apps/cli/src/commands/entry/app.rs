use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use tui_textarea::TextArea;

use hypr_cli_tui::textarea_input_from_key_event;

use super::action::Action;
use super::effect::Effect;

const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../../assets/char.png");

const TIPS_UNCONFIGURED: &[&str] = &[
    "Run /connect to set up a provider",
    "Use /auth to sign in, then /connect to configure",
    "Press Tab to auto-fill the selected command",
];

const TIPS_READY: &[&str] = &[
    "Type /listen to start a live transcription session",
    "Use /desktop to open or install the desktop app",
    "Press Tab to auto-fill the selected command",
    "Press Esc to clear the input field",
];

#[derive(Clone, Copy)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/connect",
        description: "Connect provider",
    },
    SlashCommand {
        name: "/listen",
        description: "Start live transcription",
    },
    SlashCommand {
        name: "/auth",
        description: "Open auth in browser",
    },
    SlashCommand {
        name: "/desktop",
        description: "Open desktop app or download page",
    },
    SlashCommand {
        name: "/exit",
        description: "Exit",
    },
];

pub struct App {
    input: TextArea<'static>,
    filtered_commands: Vec<usize>,
    selected_index: usize,
    popup_visible: bool,
    pub status_message: Option<String>,
    pub tip: &'static str,
    logo_protocol: Option<StatefulProtocol>,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
}

impl App {
    pub fn new(
        status_message: Option<String>,
        stt_provider: Option<String>,
        llm_provider: Option<String>,
    ) -> Self {
        let mut app = Self {
            input: TextArea::default(),
            filtered_commands: Vec::new(),
            selected_index: 0,
            popup_visible: false,
            status_message,
            tip: pick_tip(&stt_provider, &llm_provider),
            logo_protocol: load_logo_protocol(),
            stt_provider,
            llm_provider,
        };
        app.recompute_popup();
        app
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::SubmitCommand(command) => self.submit_command(&command),
            Action::StatusMessage(message) => {
                self.status_message = Some(message);
                self.input = TextArea::default();
                self.recompute_popup();
                Vec::new()
            }
        }
    }

    pub fn logo_protocol(&mut self) -> Option<&mut StatefulProtocol> {
        self.logo_protocol.as_mut()
    }

    pub fn cursor_col(&self) -> usize {
        self.input.cursor().1
    }

    pub fn input_text(&self) -> String {
        self.input
            .lines()
            .first()
            .cloned()
            .unwrap_or_else(String::new)
    }

    pub fn query(&self) -> String {
        self.input_text()
            .trim()
            .trim_start_matches('/')
            .to_ascii_lowercase()
    }

    pub fn popup_visible(&self) -> bool {
        self.popup_visible
    }

    pub fn popup_height(&self) -> u16 {
        let rows = self.filtered_commands.len().clamp(1, 6) as u16;
        rows + 2
    }

    pub fn filtered_commands(&self) -> &[usize] {
        &self.filtered_commands
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn selected_command(&self) -> Option<SlashCommand> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        COMMANDS.get(selected).copied()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        if key.code == KeyCode::Esc {
            self.input = TextArea::default();
            self.status_message = None;
            self.recompute_popup();
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

        if let Some(input) = textarea_input_from_key_event(key, false) {
            self.input.input(input);
            self.normalize_single_line();
            self.status_message = None;
            self.recompute_popup();
        }

        Vec::new()
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        let first_line = pasted.lines().next().unwrap_or("");
        if !first_line.is_empty() {
            self.input.insert_str(first_line);
            self.normalize_single_line();
            self.status_message = None;
            self.recompute_popup();
        }
        Vec::new()
    }

    fn submit_command(&mut self, command: &str) -> Vec<Effect> {
        let normalized = command.trim().trim_start_matches('/').to_ascii_lowercase();

        match normalized.as_str() {
            "connect" => vec![Effect::LaunchConnect],
            "listen" => vec![Effect::LaunchListen],
            "exit" | "quit" => vec![Effect::Exit],
            "auth" => {
                self.input = TextArea::default();
                self.status_message = None;
                self.recompute_popup();
                vec![Effect::OpenAuth]
            }
            "desktop" => {
                self.input = TextArea::default();
                self.status_message = None;
                self.recompute_popup();
                vec![Effect::OpenDesktop]
            }
            _ if normalized.is_empty() => Vec::new(),
            _ => {
                self.status_message = Some(format!("Unknown command: {}", command.trim()));
                Vec::new()
            }
        }
    }

    fn selected_command_name(&self) -> Option<&'static str> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        Some(COMMANDS.get(selected)?.name)
    }

    fn set_input_text(&mut self, value: String) {
        self.input = TextArea::from([value]);
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
        let mut ranked = COMMANDS
            .iter()
            .enumerate()
            .filter_map(|(i, command)| {
                command_match_score(query, command.name).map(|score| (i, score))
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|(left_i, left_score), (right_i, right_score)| {
            right_score
                .cmp(left_score)
                .then_with(|| COMMANDS[*left_i].name.cmp(COMMANDS[*right_i].name))
        });

        self.filtered_commands = ranked.into_iter().map(|(i, _)| i).collect();

        if self.filtered_commands.is_empty() {
            self.filtered_commands = (0..COMMANDS.len()).collect();
        }

        self.selected_index = self
            .selected_index
            .min(self.filtered_commands.len().saturating_sub(1));
    }

    fn normalize_single_line(&mut self) {
        let current = self
            .input
            .lines()
            .first()
            .cloned()
            .unwrap_or_else(String::new);
        if self.input.lines().len() == 1 {
            return;
        }
        self.input = TextArea::from([current]);
    }
}

fn pick_tip(stt_provider: &Option<String>, llm_provider: &Option<String>) -> &'static str {
    let tips = if stt_provider.is_none() || llm_provider.is_none() {
        TIPS_UNCONFIGURED
    } else {
        TIPS_READY
    };
    let index = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as usize % tips.len())
        .unwrap_or(0);
    tips[index]
}

fn load_logo_protocol() -> Option<StatefulProtocol> {
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let image = image::load_from_memory(LOGO_PNG_BYTES).ok()?;
    Some(picker.new_resize_protocol(image))
}

fn command_match_score(query: &str, command: &str) -> Option<i32> {
    let query = query.trim().to_ascii_lowercase();
    let command = command.trim_start_matches('/').to_ascii_lowercase();

    let direct_score = single_command_match_score(&query, &command);
    let alias_score = command_aliases(&command)
        .iter()
        .filter_map(|alias| single_command_match_score(&query, alias).map(|score| score - 25))
        .max();

    let best_score = match (direct_score, alias_score) {
        (Some(direct), Some(alias)) => Some(direct.max(alias)),
        (Some(direct), None) => Some(direct),
        (None, Some(alias)) => Some(alias),
        (None, None) => None,
    };

    if query.is_empty() {
        return Some(1);
    }

    best_score
}

fn single_command_match_score(query: &str, command: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(1);
    }

    if command.starts_with(query) {
        let penalty = (command.len() as i32 - query.len() as i32).max(0);
        return Some(500 - penalty);
    }

    if let Some(pos) = command.find(query) {
        return Some(350 - pos as i32);
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next()?;
    let mut score = 200;
    let mut matched = 0usize;
    let mut prev_index = None;

    for (i, ch) in command.chars().enumerate() {
        if ch != current {
            continue;
        }

        matched += 1;
        if let Some(prev) = prev_index {
            if i == prev + 1 {
                score += 8;
            } else {
                score -= (i - prev) as i32;
            }
        }
        prev_index = Some(i);

        if let Some(next) = query_chars.next() {
            current = next;
        } else {
            score -= (command.len() as i32 - matched as i32).max(0);
            return Some(score);
        }
    }

    None
}

fn command_aliases(command: &str) -> &'static [&'static str] {
    match command {
        "exit" => &["quit"],
        _ => &[],
    }
}
