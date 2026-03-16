use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_tui::textarea_input_from_key_event;
use rig::message::Message;
use tui_textarea::TextArea;

use crate::output::format_hhmmss;
use crate::theme::Theme;
use crate::widgets::ScrollState;

use super::action::Action;
use super::effect::Effect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Speaker {
    User,
    Assistant,
    Error,
}

pub(crate) struct VisibleMessage {
    pub(crate) speaker: Speaker,
    pub(crate) content: String,
}

pub(crate) struct App {
    model: String,
    session: Option<String>,
    api_history: Vec<Message>,
    transcript: Vec<VisibleMessage>,
    input: TextArea<'static>,
    pending_assistant: String,
    streaming: bool,
    status: String,
    last_error: Option<String>,
    started_at: Instant,
    scroll: ScrollState,
    autoscroll: bool,
}

impl App {
    pub(crate) fn new(model: String, session: Option<String>) -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("Type a message and press Enter...");
        input.set_placeholder_style(Theme::default().placeholder);

        let status = if session.is_some() {
            "Ready (session loaded)".to_string()
        } else {
            "Ready".to_string()
        };

        Self {
            model,
            session,
            api_history: Vec::new(),
            transcript: Vec::new(),
            input,
            pending_assistant: String::new(),
            streaming: false,
            status,
            last_error: None,
            started_at: Instant::now(),
            scroll: ScrollState::new(),
            autoscroll: true,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::StreamChunk(chunk) => {
                self.pending_assistant.push_str(&chunk);
                self.status = "Streaming response...".to_string();
                if self.autoscroll {
                    self.scroll.offset = self.scroll.max_scroll;
                }
                Vec::new()
            }
            Action::StreamCompleted(final_text) => {
                self.finish_stream(final_text);
                Vec::new()
            }
            Action::StreamFailed(error) => {
                self.fail_stream(error);
                Vec::new()
            }
        }
    }

    pub(crate) fn title(&self) -> String {
        let label = if self.streaming {
            "streaming"
        } else {
            self.status.as_str()
        };
        format!("char chat: {label} ({})", format_hhmmss(self.elapsed()))
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub(crate) fn input(&self) -> &TextArea<'static> {
        &self.input
    }

    pub(crate) fn input_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.input
    }

    pub(crate) fn transcript(&self) -> &[VisibleMessage] {
        &self.transcript
    }

    pub(crate) fn pending_assistant(&self) -> &str {
        &self.pending_assistant
    }

    pub(crate) fn streaming(&self) -> bool {
        self.streaming
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut ScrollState {
        if self.autoscroll {
            self.scroll.offset = self.scroll.max_scroll;
        }
        &mut self.scroll
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        match key.code {
            KeyCode::PageUp => {
                self.scroll_page_up();
                return Vec::new();
            }
            KeyCode::PageDown => {
                self.scroll_page_down();
                return Vec::new();
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_up();
                return Vec::new();
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_down();
                return Vec::new();
            }
            _ => {}
        }

        if self.streaming {
            return Vec::new();
        }

        match key.code {
            KeyCode::Enter => self.submit_input(),
            _ => {
                if let Some(input) = textarea_input_from_key_event(key, false) {
                    self.input.input(input);
                }
                Vec::new()
            }
        }
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.streaming {
            return Vec::new();
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.input.insert_str(&pasted);
        Vec::new()
    }

    fn submit_input(&mut self) -> Vec<Effect> {
        let input = self.input.lines().join("\n");
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let content = trimmed.to_string();
        self.input = TextArea::default();
        self.input
            .set_placeholder_text("Type a message and press Enter...");
        self.input
            .set_placeholder_style(Theme::default().placeholder);
        self.last_error = None;
        self.streaming = true;
        self.pending_assistant.clear();
        self.autoscroll = true;
        self.status = "Streaming response...".to_string();
        self.transcript.push(VisibleMessage {
            speaker: Speaker::User,
            content: content.clone(),
        });
        let history = self.api_history.clone();
        self.api_history.push(Message::user(content.clone()));

        vec![Effect::Submit {
            prompt: content,
            history,
        }]
    }

    fn finish_stream(&mut self, final_text: Option<String>) {
        self.streaming = false;
        self.status = "Ready".to_string();

        if self.pending_assistant.is_empty()
            && let Some(final_text) = final_text.as_deref()
            && !final_text.is_empty()
        {
            self.pending_assistant = final_text.to_string();
        } else if let Some(final_text) = final_text.as_deref()
            && final_text.starts_with(&self.pending_assistant)
            && final_text.len() > self.pending_assistant.len()
        {
            self.pending_assistant
                .push_str(&final_text[self.pending_assistant.len()..]);
        }

        if self.pending_assistant.is_empty() {
            self.last_error = Some("Empty response from model".to_string());
            self.status = "Error: empty response".to_string();
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Error,
                content: "No response content received from the model.".to_string(),
            });
            return;
        }

        let content = std::mem::take(&mut self.pending_assistant);
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Assistant,
            content: content.clone(),
        });
        self.api_history.push(Message::assistant(content));
    }

    fn fail_stream(&mut self, error: String) {
        self.streaming = false;
        if !self.pending_assistant.is_empty() {
            let content = std::mem::take(&mut self.pending_assistant);
            self.transcript.push(VisibleMessage {
                speaker: Speaker::Assistant,
                content: content.clone(),
            });
            self.api_history.push(Message::assistant(content));
        }
        self.last_error = Some(error.clone());
        self.status = format!("Error: {error}");
        self.transcript.push(VisibleMessage {
            speaker: Speaker::Error,
            content: error,
        });
    }

    fn scroll_up(&mut self) {
        self.scroll.offset = self.scroll.offset.saturating_sub(1);
        self.autoscroll = false;
    }

    fn scroll_down(&mut self) {
        self.scroll.offset = self
            .scroll
            .offset
            .saturating_add(1)
            .min(self.scroll.max_scroll);
        self.autoscroll = self.scroll.offset >= self.scroll.max_scroll;
    }

    fn scroll_page_up(&mut self) {
        self.scroll.offset = self.scroll.offset.saturating_sub(10);
        self.autoscroll = false;
    }

    fn scroll_page_down(&mut self) {
        self.scroll.offset = self
            .scroll
            .offset
            .saturating_add(10)
            .min(self.scroll.max_scroll);
        self.autoscroll = self.scroll.offset >= self.scroll.max_scroll;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_creates_request_effect() {
        let mut app = App::new("model".to_string(), None);
        app.input_mut().insert_str("hello");

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(matches!(effects.first(), Some(Effect::Submit { .. })));
        assert!(app.streaming);
        assert_eq!(app.transcript.len(), 1);
    }

    #[test]
    fn empty_submit_is_ignored() {
        let mut app = App::new("model".to_string(), None);

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));

        assert!(effects.is_empty());
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn stream_failure_preserves_partial_response() {
        let mut app = App::new("model".to_string(), None);
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamChunk("partial".to_string()));
        let _ = app.dispatch(Action::StreamFailed("boom".to_string()));

        assert_eq!(app.transcript.len(), 3);
        assert_eq!(app.transcript[1].content, "partial");
        assert_eq!(app.transcript[2].speaker, Speaker::Error);
    }

    #[test]
    fn empty_stream_completion_shows_error() {
        let mut app = App::new("model".to_string(), None);
        app.input_mut().insert_str("hello");
        let _ = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        let _ = app.dispatch(Action::StreamCompleted(None));

        assert!(!app.streaming);
        assert_eq!(app.transcript.len(), 2);
        assert_eq!(app.transcript[0].speaker, Speaker::User);
        assert_eq!(app.transcript[1].speaker, Speaker::Error);
        assert!(app.last_error.is_some());
    }
}
