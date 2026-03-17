use std::collections::HashMap;
use std::time::Instant;

use owhisper_interface::stream::StreamResponse;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use hypr_transcript::{
    FinalizedWord, PartialWord, RuntimeSpeakerHint, SegmentBuilderOptions, TranscriptDelta,
    TranscriptProcessor, WordRef,
};

use super::action::Action;
use super::audio::{ChannelKind, DisplayMode};
use super::effect::Effect;
use super::runtime::RuntimeEvent;
use super::ui::TracingCapture;
use super::ui::shell::TranscribeShell;
use crate::cli::TranscribeMode;
use crate::theme::Theme;
use crate::widgets::build_segment_lines;

pub(crate) struct TranscriptView {
    pub title: &'static str,
    pub lines: Vec<Line<'static>>,
    pub placeholder: String,
    pub border_style: Style,
}

pub(crate) struct App {
    mode: TranscribeMode,
    shell: TranscribeShell,
    state: TranscriptState,
    terminal_message: Option<String>,
}

enum TranscriptState {
    Raw(RawState),
    Rich(RichState),
}

impl App {
    pub(crate) fn new(mode: TranscribeMode, tracing: std::sync::Arc<TracingCapture>) -> Self {
        let state = match mode {
            TranscribeMode::Raw => TranscriptState::Raw(RawState::new()),
            TranscribeMode::Rich => TranscriptState::Rich(RichState::new()),
        };

        Self {
            mode,
            shell: TranscribeShell::new(tracing),
            state,
            terminal_message: None,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => {
                if self.shell.handle_key(key) {
                    vec![Effect::Exit]
                } else {
                    Vec::new()
                }
            }
            Action::Runtime(event) => {
                match event {
                    RuntimeEvent::StreamResponse {
                        response,
                        display_mode,
                    } => {
                        self.terminal_message = None;
                        match &mut self.state {
                            TranscriptState::Raw(raw) => raw.apply_response(response, display_mode),
                            TranscriptState::Rich(rich) => rich.apply_response(response),
                        }
                    }
                    RuntimeEvent::StreamEnded => {
                        self.shell.stream_ended = true;
                    }
                    RuntimeEvent::Failed(error) => {
                        self.shell.stream_ended = true;
                        self.terminal_message = Some(error);
                    }
                }

                Vec::new()
            }
        }
    }

    pub(crate) fn shell_mut(&mut self) -> &mut TranscribeShell {
        &mut self.shell
    }

    pub(crate) fn title(&self) -> String {
        match self.mode {
            TranscribeMode::Raw => "debug transcribe (raw)".into(),
            TranscribeMode::Rich => "debug transcribe (rich)".into(),
        }
    }

    pub(crate) fn is_raw_mode(&self) -> bool {
        matches!(self.state, TranscriptState::Raw(_))
    }

    pub(crate) fn has_recent_words(&self) -> bool {
        match &self.state {
            TranscriptState::Rich(rich) => rich.has_recent_words(),
            TranscriptState::Raw(_) => false,
        }
    }

    pub(crate) fn transcript_view(&self, width: usize) -> TranscriptView {
        let placeholder = if let Some(message) = &self.terminal_message {
            message.clone()
        } else {
            match self.mode {
                TranscribeMode::Raw => "Stream ended.".to_string(),
                TranscribeMode::Rich => "Stream ended - no speech detected.".to_string(),
            }
        };

        match &self.state {
            TranscriptState::Raw(raw) => TranscriptView {
                title: "Transcript",
                lines: raw.lines(),
                placeholder,
                border_style: Style::new().fg(Color::Cyan),
            },
            TranscriptState::Rich(rich) => TranscriptView {
                title: "Transcript",
                lines: rich.lines(width),
                placeholder,
                border_style: rich.theme.border_focused,
            },
        }
    }
}

struct RawState {
    channels: Vec<ChannelTranscript>,
}

impl RawState {
    fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    fn apply_response(&mut self, response: StreamResponse, display_mode: DisplayMode) {
        self.ensure_channels(&display_mode);

        if let StreamResponse::TranscriptResponse {
            is_final,
            channel,
            channel_index,
            ..
        } = response
        {
            let text = channel
                .alternatives
                .first()
                .map(|a| a.transcript.as_str())
                .unwrap_or("");

            let channel = match display_mode {
                DisplayMode::Single(_) => 0,
                DisplayMode::Dual => {
                    channel_index.first().copied().unwrap_or(0).clamp(0, 1) as usize
                }
            };

            if let Some(transcript) = self.channels.get_mut(channel) {
                if is_final {
                    transcript.confirm(text);
                } else {
                    transcript.set_partial(text);
                }
            }
        }
    }

    fn ensure_channels(&mut self, mode: &DisplayMode) {
        if !self.channels.is_empty() {
            return;
        }

        let t0 = Instant::now();
        match mode {
            DisplayMode::Single(kind) => {
                self.channels.push(ChannelTranscript::new(t0, *kind));
            }
            DisplayMode::Dual => {
                self.channels
                    .push(ChannelTranscript::new(t0, ChannelKind::Mic));
                self.channels
                    .push(ChannelTranscript::new(t0, ChannelKind::Speaker));
            }
        }
    }

    fn lines(&self) -> Vec<Line<'static>> {
        self.channels
            .iter()
            .filter_map(ChannelTranscript::render_line)
            .collect()
    }
}

struct ChannelTranscript {
    segments: Vec<String>,
    partial: String,
    t0: Instant,
    kind: ChannelKind,
    last_confirmed: Option<String>,
}

impl ChannelTranscript {
    fn new(t0: Instant, kind: ChannelKind) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
            kind,
            last_confirmed: None,
        }
    }

    fn set_partial(&mut self, text: &str) {
        self.partial = text.to_string();
    }

    fn confirm(&mut self, text: &str) {
        if self.last_confirmed.as_deref() == Some(text) {
            return;
        }
        self.last_confirmed = Some(text.to_string());
        self.segments.push(text.to_string());
        self.partial.clear();
    }

    fn render_line(&self) -> Option<Line<'static>> {
        let confirmed = self.segments.join(" ");
        if confirmed.is_empty() && self.partial.is_empty() {
            return None;
        }

        let to = self.t0.elapsed().as_secs_f64();
        let from_str = if self.segments.is_empty() {
            "--:--".to_string()
        } else {
            fmt_ts(0.0)
        };

        let prefix = format!("[{} / {}]", from_str, fmt_ts(to));
        let theme = Theme::DEFAULT;
        let (confirmed_style, partial_style) = match self.kind {
            ChannelKind::Mic => (theme.raw_mic_confirmed, theme.raw_mic_partial),
            ChannelKind::Speaker => (theme.raw_speaker_confirmed, theme.raw_speaker_partial),
        };

        let mut spans = vec![
            Span::styled(prefix, Style::new().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(confirmed, confirmed_style),
        ];

        if !self.partial.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(self.partial.clone(), partial_style));
        }

        Some(Line::from(spans))
    }
}

struct RichState {
    words: Vec<FinalizedWord>,
    partials: Vec<PartialWord>,
    hints: Vec<RuntimeSpeakerHint>,
    partial_hints: Vec<RuntimeSpeakerHint>,
    transcript: TranscriptProcessor,
    word_first_seen: HashMap<String, Instant>,
    theme: Theme,
}

impl RichState {
    fn new() -> Self {
        Self {
            words: Vec::new(),
            partials: Vec::new(),
            hints: Vec::new(),
            partial_hints: Vec::new(),
            transcript: TranscriptProcessor::new(),
            word_first_seen: HashMap::new(),
            theme: Theme::DEFAULT,
        }
    }

    fn apply_response(&mut self, response: StreamResponse) {
        if let Some(delta) = self.transcript.process(&response) {
            self.apply_delta(delta);
        }
    }

    fn lines(&self, width: usize) -> Vec<Line<'static>> {
        let opts = SegmentBuilderOptions {
            max_gap_ms: Some(5000),
            ..Default::default()
        };
        let mut all_hints = self.hints.clone();
        let final_words_count = self.words.len();
        all_hints.extend(self.partial_hints.iter().cloned().map(|mut hint| {
            if let WordRef::RuntimeIndex(index) = &mut hint.target {
                *index += final_words_count;
            }
            hint
        }));

        let segments =
            hypr_transcript::build_segments(&self.words, &self.partials, &all_hints, Some(&opts));
        let word_age = |id: &str| self.word_age_secs(id);
        build_segment_lines(&segments, &self.theme, width, Some(&word_age))
    }

    fn has_recent_words(&self) -> bool {
        let now = Instant::now();
        self.word_first_seen
            .values()
            .any(|time| now.duration_since(*time).as_secs_f64() < 0.5)
    }

    fn apply_delta(&mut self, delta: TranscriptDelta) {
        if !delta.replaced_ids.is_empty() {
            self.words
                .retain(|word| !delta.replaced_ids.contains(&word.id));
            self.hints.retain(|hint| match &hint.target {
                WordRef::FinalWordId(word_id) => !delta.replaced_ids.contains(word_id),
                WordRef::RuntimeIndex(_) => true,
            });
        }

        let now = Instant::now();
        for word in &delta.new_words {
            self.word_first_seen.entry(word.id.clone()).or_insert(now);
        }

        self.words.extend(delta.new_words);
        self.hints.extend(delta.hints);
        self.partials = delta.partials;
        self.partial_hints = delta.partial_hints;
    }

    fn word_age_secs(&self, id: &str) -> f64 {
        self.word_first_seen
            .get(id)
            .map(|time| time.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX)
    }
}

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}
