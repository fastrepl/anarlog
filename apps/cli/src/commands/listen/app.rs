use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_listener_core::{
    DegradedError, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, State,
};
use hypr_listener2_core::BatchEvent;
use hypr_transcript::{FinalizedWord, PartialWord, TranscriptDelta, TranscriptProcessor};
use tui_textarea::TextArea;

use super::audio_drop::{AudioDropRequest, looks_like_audio_file, normalize_pasted_path};
use super::runtime::ListenerEvent;
use crate::frame::FrameRequester;
use crate::textarea_input::textarea_input_from_key_event;

const AUDIO_HISTORY_CAP: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Transcript,
    Memo,
}

pub struct MemoView {
    pub lines: Vec<String>,
    pub cursor_row: u16,
    pub cursor_col: u16,
}

pub struct App {
    pub should_quit: bool,
    pub state: State,
    pub status: String,
    pub degraded: Option<DegradedError>,
    pub errors: Vec<String>,
    pub mic_level: u16,
    pub speaker_level: u16,
    pub mic_history: VecDeque<u64>,
    pub speaker_history: VecDeque<u64>,
    pub mic_muted: bool,
    pub words: Vec<FinalizedWord>,
    pub partials: Vec<PartialWord>,
    transcript: TranscriptProcessor,
    pub started_at: std::time::Instant,
    pub scroll_offset: u16,
    frame_requester: FrameRequester,

    focus: Focus,
    notepad_width_percent: u16,
    transcript_max_scroll: u16,
    memo: TextArea<'static>,
    batch_running: bool,
}

impl App {
    fn init_memo() -> TextArea<'static> {
        TextArea::default()
    }

    pub fn new(frame_requester: FrameRequester) -> Self {
        Self {
            should_quit: false,
            state: State::Inactive,
            status: "Starting...".into(),
            degraded: None,
            errors: Vec::new(),
            mic_level: 0,
            speaker_level: 0,
            mic_history: VecDeque::with_capacity(AUDIO_HISTORY_CAP),
            speaker_history: VecDeque::with_capacity(AUDIO_HISTORY_CAP),
            mic_muted: false,
            words: Vec::new(),
            partials: Vec::new(),
            transcript: TranscriptProcessor::new(),
            started_at: std::time::Instant::now(),
            scroll_offset: 0,
            frame_requester,

            focus: Focus::Transcript,
            notepad_width_percent: 67,
            transcript_max_scroll: 0,
            memo: Self::init_memo(),
            batch_running: false,
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Left {
            self.adjust_notepad_width(-2);
            self.frame_requester.schedule_frame();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Right {
            self.adjust_notepad_width(2);
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Tab {
            self.toggle_focus();
            self.frame_requester.schedule_frame();
            return;
        }

        if self.memo_focused() {
            self.handle_memo_key(key);
        } else {
            self.handle_global_key(key);
        }
    }

    pub fn handle_paste(&mut self, pasted: String) -> Option<AudioDropRequest> {
        if !self.memo_focused() {
            return self.handle_transcript_paste(pasted);
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.memo.insert_str(&pasted);
        self.frame_requester.schedule_frame();
        None
    }

    pub fn handle_listener_event(&mut self, event: ListenerEvent) {
        match event {
            ListenerEvent::Lifecycle(e) => self.handle_lifecycle(e),
            ListenerEvent::Progress(e) => self.handle_progress(e),
            ListenerEvent::Error(e) => self.handle_error(e),
            ListenerEvent::Data(e) => self.handle_data(e),
        }
        self.frame_requester.schedule_frame();
    }

    pub fn handle_batch_event(&mut self, event: BatchEvent) {
        match event {
            BatchEvent::BatchStarted { .. } => {
                self.batch_running = true;
                self.status = "Transcribing dropped audio...".into();
            }
            BatchEvent::BatchCompleted { .. } => {
                self.batch_running = false;
                self.status = "Dropped audio transcription completed".into();
            }
            BatchEvent::BatchResponseStreamed {
                response,
                percentage,
                ..
            } => {
                if let Some(delta) = self.transcript.process(&response) {
                    self.apply_transcript_delta(delta);
                }

                self.status = format!("Transcribing dropped audio... {:.0}%", percentage * 100.0);

                if percentage >= 1.0 {
                    self.batch_running = false;
                    self.status = "Dropped audio transcription completed".into();
                }
            }
            BatchEvent::BatchResponse { response, .. } => {
                let delta = TranscriptProcessor::process_batch_response(&response);
                self.apply_transcript_delta(delta);
                self.batch_running = false;
                self.status = "Dropped audio transcription completed".into();
            }
            BatchEvent::BatchFailed { error, .. } => {
                self.batch_running = false;
                self.errors.push(format!("Batch: {error}"));
                self.status = format!("Dropped audio transcription failed: {error}");
            }
        }

        self.frame_requester.schedule_frame();
    }

    pub fn can_accept_audio_drop(&self) -> bool {
        self.transcript_focused()
            && self.state == State::Inactive
            && !self.batch_running
            && self.words.is_empty()
            && self.partials.is_empty()
    }

    pub fn memo_focused(&self) -> bool {
        self.focus == Focus::Memo
    }

    pub fn transcript_focused(&self) -> bool {
        self.focus == Focus::Transcript
    }

    pub fn memo_view(&self, max_rows: usize, max_cols: usize) -> MemoView {
        if max_rows == 0 || max_cols == 0 {
            return MemoView {
                lines: Vec::new(),
                cursor_row: 0,
                cursor_col: 0,
            };
        }

        let lines = self.memo.lines();
        let (memo_cursor_row, memo_cursor_col) = self.memo.cursor();
        let cursor_row = memo_cursor_row.min(lines.len().saturating_sub(1));
        let cursor_col = memo_cursor_col.min(current_line_len(lines, cursor_row));

        let row_start = (cursor_row + 1).saturating_sub(max_rows);

        let col_start = (cursor_col + 1).saturating_sub(max_cols);

        let row_end = (row_start + max_rows).min(lines.len());
        let lines = lines[row_start..row_end]
            .iter()
            .map(|line| {
                let end = (col_start + max_cols).min(line.chars().count());
                substring_by_char_range(line, col_start, end)
            })
            .collect();

        MemoView {
            lines,
            cursor_row: cursor_row.saturating_sub(row_start) as u16,
            cursor_col: cursor_col.saturating_sub(col_start) as u16,
        }
    }

    pub fn memo_is_empty(&self) -> bool {
        self.memo.is_empty()
    }

    pub fn update_transcript_max_scroll(&mut self, max_scroll: u16) {
        self.transcript_max_scroll = max_scroll;
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    pub fn notepad_width_percent(&self) -> u16 {
        self.notepad_width_percent
    }

    fn handle_global_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('m') => {
                self.focus = Focus::Memo;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(1)
                    .min(self.transcript_max_scroll);
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                self.frame_requester.schedule_frame();
            }
            _ => {}
        }
    }

    fn handle_memo_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.focus = Focus::Transcript;
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo = Self::init_memo();
            self.frame_requester.schedule_frame();
            return;
        }

        if let Some(input) = textarea_input_from_key_event(key, true) {
            self.memo.input(input);
        }

        self.frame_requester.schedule_frame();
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Memo => Focus::Transcript,
            Focus::Transcript => Focus::Memo,
        };
    }

    fn adjust_notepad_width(&mut self, delta: i16) {
        const MIN_NOTEPAD_WIDTH_PERCENT: u16 = 45;
        const MAX_NOTEPAD_WIDTH_PERCENT: u16 = 80;

        let next = (self.notepad_width_percent as i16 + delta).clamp(
            MIN_NOTEPAD_WIDTH_PERCENT as i16,
            MAX_NOTEPAD_WIDTH_PERCENT as i16,
        ) as u16;
        self.notepad_width_percent = next;
    }

    fn handle_lifecycle(&mut self, event: SessionLifecycleEvent) {
        match event {
            SessionLifecycleEvent::Active { error, .. } => {
                self.state = State::Active;
                self.degraded = error;
                if self.degraded.is_some() {
                    self.status = "Active (degraded)".into();
                } else {
                    self.status = "Listening".into();
                }
            }
            SessionLifecycleEvent::Inactive { error, .. } => {
                self.state = State::Inactive;
                if let Some(err) = error {
                    self.status = format!("Stopped: {err}");
                } else {
                    self.status = "Stopped".into();
                }
            }
            SessionLifecycleEvent::Finalizing { .. } => {
                self.state = State::Finalizing;
                self.status = "Finalizing...".into();
            }
        }
    }

    fn handle_progress(&mut self, event: SessionProgressEvent) {
        match event {
            SessionProgressEvent::AudioInitializing { .. } => {
                self.status = "Initializing audio...".into();
            }
            SessionProgressEvent::AudioReady { device, .. } => {
                if let Some(dev) = device {
                    self.status = format!("Audio ready ({dev})");
                } else {
                    self.status = "Audio ready".into();
                }
            }
            SessionProgressEvent::Connecting { .. } => {
                self.status = "Connecting...".into();
            }
            SessionProgressEvent::Connected { adapter, .. } => {
                self.status = format!("Connected via {adapter}");
            }
        }
    }

    fn handle_error(&mut self, event: SessionErrorEvent) {
        match event {
            SessionErrorEvent::AudioError { error, .. } => {
                self.errors.push(format!("Audio: {error}"));
            }
            SessionErrorEvent::ConnectionError { error, .. } => {
                self.errors.push(format!("Connection: {error}"));
            }
        }
    }

    fn handle_data(&mut self, event: SessionDataEvent) {
        match event {
            SessionDataEvent::AudioAmplitude { mic, speaker, .. } => {
                self.mic_level = mic;
                self.speaker_level = speaker;

                if self.mic_history.len() >= AUDIO_HISTORY_CAP {
                    self.mic_history.pop_front();
                }
                self.mic_history.push_back(mic as u64);

                if self.speaker_history.len() >= AUDIO_HISTORY_CAP {
                    self.speaker_history.pop_front();
                }
                self.speaker_history.push_back(speaker as u64);
            }
            SessionDataEvent::MicMuted { value, .. } => {
                self.mic_muted = value;
            }
            SessionDataEvent::StreamResponse { response, .. } => {
                if let Some(delta) = self.transcript.process(response.as_ref()) {
                    self.apply_transcript_delta(delta);
                }
            }
        }
    }

    fn apply_transcript_delta(&mut self, delta: TranscriptDelta) {
        if !delta.replaced_ids.is_empty() {
            self.words.retain(|w| !delta.replaced_ids.contains(&w.id));
        }
        self.words.extend(delta.new_words);
        self.partials = delta.partials;
    }

    fn handle_transcript_paste(&mut self, pasted: String) -> Option<AudioDropRequest> {
        if !self.can_accept_audio_drop() {
            return None;
        }

        let path = normalize_pasted_path(&pasted)?;

        if !looks_like_audio_file(&path) {
            return None;
        }

        if !path.is_file() {
            self.errors
                .push(format!("Dropped path is not a file: {}", path.display()));
            self.frame_requester.schedule_frame();
            return None;
        }

        self.batch_running = true;
        self.status = format!("Transcribing dropped audio: {}", path.display());
        self.frame_requester.schedule_frame();

        Some(AudioDropRequest {
            file_path: path.to_string_lossy().to_string(),
        })
    }
}

fn substring_by_char_range(s: &str, start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    let start_byte = s
        .char_indices()
        .nth(start)
        .map(|(i, _)| i)
        .unwrap_or_else(|| s.len());
    let end_byte = s
        .char_indices()
        .nth(end)
        .map(|(i, _)| i)
        .unwrap_or_else(|| s.len());
    s.get(start_byte..end_byte).unwrap_or("").to_string()
}

fn current_line_len(lines: &[String], row: usize) -> usize {
    lines.get(row).map(|line| line.chars().count()).unwrap_or(0)
}
