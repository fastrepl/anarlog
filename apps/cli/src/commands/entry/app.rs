use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use tui_textarea::TextArea;

use crate::frame::FrameRequester;
use crate::textarea_input::textarea_input_from_key_event;

const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../../assets/char.png");

#[derive(Clone, Copy)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/sessions",
        description: "Switch session",
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

#[derive(Clone, Copy)]
pub enum EntryAction {
    Listen,
    Quit,
}

pub struct SessionEntry {
    pub title: String,
    pub time_label: String,
    pub day_label: String,
    pub notes: String,
    pub transcript: String,
}

pub struct SessionsOverlay {
    pub search: TextArea<'static>,
    pub entries: Vec<SessionEntry>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub status_message: Option<String>,
    pub viewing_session: Option<usize>,
}

impl SessionsOverlay {
    fn new() -> Self {
        let mut overlay = Self {
            search: TextArea::default(),
            entries: demo_sessions(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            status_message: None,
            viewing_session: None,
        };
        overlay.recompute_filter();
        overlay
    }

    pub fn search_text(&self) -> String {
        self.search
            .lines()
            .first()
            .cloned()
            .unwrap_or_else(String::new)
    }

    pub fn search_cursor_col(&self) -> usize {
        self.search.cursor().1
    }

    fn recompute_filter(&mut self) {
        let query = self.search_text().trim().to_ascii_lowercase();
        self.filtered_indices = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if query.is_empty() {
                    return true;
                }

                entry.title.to_ascii_lowercase().contains(&query)
                    || entry.day_label.to_ascii_lowercase().contains(&query)
                    || entry.time_label.to_ascii_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect();

        self.selected_index = self
            .selected_index
            .min(self.filtered_indices.len().saturating_sub(1));
    }

    fn selected_session_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected_index).copied()
    }

    fn normalize_search_single_line(&mut self) {
        let current = self
            .search
            .lines()
            .first()
            .cloned()
            .unwrap_or_else(String::new);
        if self.search.lines().len() == 1 {
            return;
        }
        self.search = TextArea::from([current]);
    }
}

pub struct EntryApp {
    pub should_quit: bool,
    action: Option<EntryAction>,
    input: TextArea<'static>,
    filtered_commands: Vec<usize>,
    selected_index: usize,
    popup_visible: bool,
    sessions_overlay: Option<SessionsOverlay>,
    pub status_message: Option<String>,
    frame_requester: FrameRequester,
    logo_protocol: Option<StatefulProtocol>,
}

impl EntryApp {
    pub fn new(frame_requester: FrameRequester, status_message: Option<String>) -> Self {
        let mut app = Self {
            should_quit: false,
            action: None,
            input: TextArea::default(),
            filtered_commands: Vec::new(),
            selected_index: 0,
            popup_visible: false,
            sessions_overlay: None,
            status_message,
            frame_requester,
            logo_protocol: load_logo_protocol(),
        };
        app.recompute_popup();
        app
    }

    pub fn action(&self) -> Option<EntryAction> {
        self.action
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

    pub fn sessions_overlay(&self) -> Option<&SessionsOverlay> {
        self.sessions_overlay.as_ref()
    }

    pub fn logo_protocol(&mut self) -> Option<&mut StatefulProtocol> {
        self.logo_protocol.as_mut()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.action = Some(EntryAction::Quit);
            self.should_quit = true;
            return;
        }

        if self.sessions_overlay.is_some() {
            self.handle_sessions_overlay_key(key);
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Esc {
            self.input = TextArea::default();
            self.status_message = None;
            self.recompute_popup();
            self.frame_requester.schedule_frame();
            return;
        }

        if self.popup_visible {
            match key.code {
                KeyCode::Up => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    self.frame_requester.schedule_frame();
                    return;
                }
                KeyCode::Down => {
                    let max = self.filtered_commands.len().saturating_sub(1);
                    self.selected_index = (self.selected_index + 1).min(max);
                    self.frame_requester.schedule_frame();
                    return;
                }
                KeyCode::Tab => {
                    if let Some(cmd) = self.selected_command_name() {
                        self.set_input_text(cmd.to_string());
                        self.recompute_popup();
                        self.frame_requester.schedule_frame();
                    }
                    return;
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
            self.dispatch_command(&command);
            self.frame_requester.schedule_frame();
            return;
        }

        if let Some(input) = textarea_input_from_key_event(key, false) {
            self.input.input(input);
            self.normalize_single_line();
            self.status_message = None;
            self.recompute_popup();
            self.frame_requester.schedule_frame();
        }
    }

    pub fn handle_paste(&mut self, pasted: String) {
        if let Some(overlay) = self.sessions_overlay.as_mut() {
            let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
            let first_line = pasted.lines().next().unwrap_or("");
            if !first_line.is_empty() {
                overlay.search.insert_str(first_line);
                overlay.normalize_search_single_line();
                overlay.status_message = None;
                overlay.recompute_filter();
                self.frame_requester.schedule_frame();
            }
            return;
        }

        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        let first_line = pasted.lines().next().unwrap_or("");
        if !first_line.is_empty() {
            self.input.insert_str(first_line);
            self.normalize_single_line();
            self.status_message = None;
            self.recompute_popup();
            self.frame_requester.schedule_frame();
        }
    }

    fn selected_command_name(&self) -> Option<&'static str> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        Some(COMMANDS.get(selected)?.name)
    }

    fn set_input_text(&mut self, value: String) {
        self.input = TextArea::from([value]);
    }

    fn dispatch_command(&mut self, command: &str) {
        let normalized = command.trim().trim_start_matches('/').to_ascii_lowercase();

        match normalized.as_str() {
            "listen" => {
                self.action = Some(EntryAction::Listen);
                self.should_quit = true;
            }
            "sessions" => {
                self.open_sessions_overlay();
            }
            "exit" | "quit" => {
                self.action = Some(EntryAction::Quit);
                self.should_quit = true;
            }
            "auth" => {
                self.status_message = match crate::commands::auth::run() {
                    Ok(()) => Some("Opened auth page in browser".into()),
                    Err(error) => Some(error.to_string()),
                };
                self.input = TextArea::default();
                self.recompute_popup();
            }
            "desktop" => {
                let message = match crate::commands::desktop::run() {
                    Ok(crate::commands::desktop::DesktopAction::OpenedApp) => {
                        "Opened desktop app".to_string()
                    }
                    Ok(crate::commands::desktop::DesktopAction::OpenedDownloadPage) => {
                        "Desktop app not found. Opened download page".to_string()
                    }
                    Err(error) => error.to_string(),
                };
                self.status_message = Some(message);
                self.input = TextArea::default();
                self.recompute_popup();
            }
            _ if normalized.is_empty() => {}
            _ => {
                self.status_message = Some(format!("Unknown command: {}", command.trim()));
            }
        }
    }

    fn open_sessions_overlay(&mut self) {
        self.sessions_overlay = Some(SessionsOverlay::new());
        self.status_message = None;
        self.input = TextArea::default();
        self.popup_visible = false;
        self.filtered_commands.clear();
    }

    fn handle_sessions_overlay_key(&mut self, key: KeyEvent) {
        let Some(overlay) = self.sessions_overlay.as_mut() else {
            return;
        };

        if key.code == KeyCode::Esc {
            if overlay.viewing_session.is_some() {
                overlay.viewing_session = None;
            } else {
                self.sessions_overlay = None;
            }
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
            if let Some(index) = overlay.selected_session_index() {
                let title = overlay.entries[index].title.clone();
                overlay.entries.remove(index);
                overlay.status_message = Some(format!("Deleted session: {title}"));
                if let Some(viewing) = overlay.viewing_session {
                    if viewing == index {
                        overlay.viewing_session = None;
                    } else if viewing > index {
                        overlay.viewing_session = Some(viewing - 1);
                    }
                }
                overlay.recompute_filter();
            }
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            if let Some(index) = overlay.selected_session_index() {
                let title = overlay.entries[index].title.clone();
                let next_title = if title.ends_with(" (renamed)") {
                    title
                } else {
                    format!("{title} (renamed)")
                };
                overlay.entries[index].title = next_title.clone();
                overlay.status_message = Some(format!("Renamed session to: {next_title}"));
                overlay.recompute_filter();
            }
            return;
        }

        if key.code == KeyCode::Enter {
            if let Some(index) = overlay.selected_session_index() {
                overlay.viewing_session = Some(index);
                overlay.status_message = None;
            }
            return;
        }

        if key.code == KeyCode::Up {
            overlay.selected_index = overlay.selected_index.saturating_sub(1);
            return;
        }

        if key.code == KeyCode::Down {
            let max = overlay.filtered_indices.len().saturating_sub(1);
            overlay.selected_index = (overlay.selected_index + 1).min(max);
            return;
        }

        if overlay.viewing_session.is_some() {
            return;
        }

        if let Some(input) = textarea_input_from_key_event(key, false) {
            overlay.search.input(input);
            overlay.normalize_search_single_line();
            overlay.status_message = None;
            overlay.recompute_filter();
        }
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
        "sessions" => &["session"],
        _ => &[],
    }
}

fn demo_sessions() -> Vec<SessionEntry> {
    vec![
        SessionEntry {
            title: "Adding version at bottom-right in CLI UI?".into(),
            time_label: "9:39 PM".into(),
            day_label: "Today".into(),
            notes: "Pin version string to lower-right and keep alignment stable on resize."
                .into(),
            transcript: "Let's add the version text to the footer area and reserve width so it does not shift with status updates.".into(),
        },
        SessionEntry {
            title: "Search item match discontinuation fix".into(),
            time_label: "9:37 PM".into(),
            day_label: "Today".into(),
            notes: "Investigate why matching resets after a delete key event.".into(),
            transcript: "The issue happens when the query becomes empty and we do not recompute list ordering.".into(),
        },
        SessionEntry {
            title: "App CLI: check/install/open web page for char.com/download".into(),
            time_label: "9:31 PM".into(),
            day_label: "Today".into(),
            notes: "Desktop command should open app if installed, otherwise open download page."
                .into(),
            transcript: "We can probe common install locations first, then fall back to browser launch for download.".into(),
        },
        SessionEntry {
            title: "Exit message for listen session".into(),
            time_label: "9:30 PM".into(),
            day_label: "Today".into(),
            notes: "Print session id, duration, and word count after leaving listen mode.".into(),
            transcript: "Exit summary now includes compact timing and total finalized words.".into(),
        },
        SessionEntry {
            title: "OpenCode: Char alignment vs OpenCode screenshot comparison".into(),
            time_label: "9:27 PM".into(),
            day_label: "Today".into(),
            notes: "Adjust spacing and typography in command picker to match reference.".into(),
            transcript: "Main differences are title spacing and list highlight contrast.".into(),
        },
        SessionEntry {
            title: "UI/CLI input alignment and design cleanup".into(),
            time_label: "9:06 PM".into(),
            day_label: "Today".into(),
            notes: "Center command input and keep logo vertical rhythm consistent.".into(),
            transcript: "The centered layout works better when popup height changes dynamically.".into(),
        },
        SessionEntry {
            title: "Centralized theme setup in CLI".into(),
            time_label: "7:48 PM".into(),
            day_label: "Yesterday".into(),
            notes: "Move all shared color/typography to a single theme struct.".into(),
            transcript: "This makes focused border and muted styles consistent across screens.".into(),
        },
    ]
}

pub fn command_highlight_indices(query: &str, command: &str) -> Vec<usize> {
    let query = query.trim().to_ascii_lowercase();
    let command = command.trim_start_matches('/').to_ascii_lowercase();

    if query.is_empty() {
        return Vec::new();
    }

    if command.starts_with(&query) {
        return (0..query.chars().count()).collect();
    }

    if let Some(start) = command.find(&query) {
        let width = query.chars().count();
        return (start..start + width).collect();
    }

    let mut query_chars = query.chars();
    let mut target = match query_chars.next() {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let mut indices = Vec::new();

    for (i, ch) in command.chars().enumerate() {
        if ch != target {
            continue;
        }

        indices.push(i);
        if let Some(next) = query_chars.next() {
            target = next;
        } else {
            return indices;
        }
    }

    Vec::new()
}
