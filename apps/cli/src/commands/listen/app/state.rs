use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::Instant;

use hypr_listener_core::{
    DegradedError, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, State,
};
use hypr_listener2_core::BatchEvent;
use hypr_transcript::{
    FinalizedWord, PartialWord, RuntimeSpeakerHint, Segment, TranscriptDelta, TranscriptProcessor,
    WordRef,
};

use crate::commands::listen::runtime::RuntimeEvent;

const AUDIO_HISTORY_CAP: usize = 64;

pub(super) struct ListenState {
    state: State,
    status: String,
    degraded: Option<DegradedError>,
    errors: Vec<String>,
    mic_level: u16,
    speaker_level: u16,
    mic_history: VecDeque<u64>,
    speaker_history: VecDeque<u64>,
    mic_muted: bool,
    words: Vec<FinalizedWord>,
    partials: Vec<PartialWord>,
    hints: Vec<RuntimeSpeakerHint>,
    partial_hints: Vec<RuntimeSpeakerHint>,
    transcript: TranscriptProcessor,
    started_at: Instant,
    batch_running: bool,
    word_first_seen: HashMap<String, Instant>,
}

impl ListenState {
    pub(super) fn new() -> Self {
        Self {
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
            hints: Vec::new(),
            partial_hints: Vec::new(),
            transcript: TranscriptProcessor::new(),
            started_at: Instant::now(),
            batch_running: false,
            word_first_seen: HashMap::new(),
        }
    }

    pub(super) fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub(super) fn handle_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Lifecycle(e) => self.handle_lifecycle(e),
            RuntimeEvent::Progress(e) => self.handle_progress(e),
            RuntimeEvent::Error(e) => self.handle_error(e),
            RuntimeEvent::Data(e) => self.handle_data(e),
        }
    }

    pub(super) fn handle_batch_event(&mut self, event: BatchEvent) {
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
    }

    pub(super) fn listener_state(&self) -> State {
        self.state.clone()
    }

    pub(super) fn status(&self) -> &str {
        &self.status
    }

    pub(super) fn degraded(&self) -> Option<&DegradedError> {
        self.degraded.as_ref()
    }

    pub(super) fn errors(&self) -> &[String] {
        &self.errors
    }

    pub(super) fn mic_muted(&self) -> bool {
        self.mic_muted
    }

    pub(super) fn mic_history(&self) -> &VecDeque<u64> {
        &self.mic_history
    }

    pub(super) fn speaker_history(&self) -> &VecDeque<u64> {
        &self.speaker_history
    }

    pub(super) fn word_count(&self) -> usize {
        self.words.len()
    }

    pub(super) fn batch_running(&self) -> bool {
        self.batch_running
    }

    pub(super) fn is_transcript_empty(&self) -> bool {
        self.words.is_empty() && self.partials.is_empty()
    }

    pub(super) fn push_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub(super) fn begin_audio_drop(&mut self, path: &Path) {
        self.batch_running = true;
        self.status = format!("Transcribing dropped audio: {}", path.display());
    }

    pub(super) fn segments(&self) -> Vec<Segment> {
        let opts = hypr_transcript::SegmentBuilderOptions {
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
        hypr_transcript::build_segments(&self.words, &self.partials, &all_hints, Some(&opts))
    }

    pub(super) fn word_age_secs(&self, id: &str) -> f64 {
        self.word_first_seen
            .get(id)
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX)
    }

    pub(super) fn has_recent_words(&self) -> bool {
        let now = Instant::now();
        self.word_first_seen
            .values()
            .any(|t| now.duration_since(*t).as_secs_f64() < 0.5)
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
}
