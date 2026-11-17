//! The open note's Transcript view: shipping's `TranscriptContent` screens (listening,
//! finalizing, batch, empty) and its segment list (speaker label in its segment color, then the
//! words), rendered by the same `anlg_transcript` segmenter. While a meeting is live, stored
//! segments merge with the live ones and partial words show faded and italic. Uploads and
//! re-transcription run through `meeting::upload`; the view also owns the Transcript tab's
//! right-click menu, Space playback and segment selection (⌘⇧↑/↓).

mod speakers;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{
    ChannelProfile, RenderTranscriptHuman, RenderTranscriptRequest, RenderedTranscriptSegment,
    SegmentKey, SegmentWord, SpeakerLabeler, render_transcript_segments,
};
use desktop_runtime::{RuntimeHandle, SessionId};
use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveText, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, Point, Render, ScrollHandle, SharedString, StyledText, Subscription,
    Task, Window, div, point, prelude::*, px, rgb, rgba, svg,
};
use serde_json::{Value, json};

use super::find::FindOptions;
use crate::{
    meeting::{
        MeetingServices,
        live::{CaptureMode, LiveMeeting, LiveState, SessionMode},
        model::Transcript,
        playback::{Command, State},
        store::TranscriptStore,
        upload::{Job, Phase, Uploads},
        widgets::{dancing_sticks, spinner},
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{RADIUS, Theme, theme},
    },
    workspace::resource_list,
};

pub struct TranscriptView {
    runtime: RuntimeHandle,
    session: SessionId,
    segments: Vec<RenderedTranscriptSegment>,
    /// Each stored word's transcript, for edits and speaker assignment.
    word_transcripts: HashMap<String, Arc<str>>,
    loaded: bool,
    audio: Option<PathBuf>,
    error: Option<String>,
    live: Option<Entity<LiveState>>,
    scroll: ScrollHandle,
    /// Shipping's transcript edit mode (the pencil on the Transcript tab).
    edit_mode: bool,
    editors: HashMap<String, SegmentEditor>,
    /// The word under the pointer, by row and word index.
    hovered: Option<(usize, usize)>,
    /// Playback position in this session's audio, for the current-line highlight.
    playing_ms: Option<i64>,
    picker: Option<speakers::SpeakerPicker>,
    /// The note's find query, its options and the current match.
    find: Option<(String, FindOptions, usize)>,
    /// The current match's row and word, resolved once per frame.
    found_word: Option<(usize, usize)>,
    focus: FocusHandle,
    uploads: Entity<Uploads>,
    /// The Transcript tab's right-click menu, at its window position.
    tab_menu: Option<Point<Pixels>>,
    /// Shipping's segment multi-selection: selected row ids and the last one clicked.
    selected: HashSet<String>,
    anchor: Option<String>,
    /// The rendered segment ids in order, for range selection.
    order: Vec<String>,
    _playback: Option<Task<()>>,
    _watch: Task<()>,
    _live: Option<Subscription>,
    _uploads: Subscription,
}

pub enum TranscriptEvent {
    /// The Transcript tab menu's "Resume listening".
    ResumeListening,
}

impl EventEmitter<TranscriptEvent> for TranscriptView {}

/// One segment's text while editing (shipping's `EditableSegmentText`).
struct SegmentEditor {
    input: Entity<TextInput>,
    original: String,
    transcript: Arc<str>,
    word_ids: Vec<String>,
    /// Bumped per keystroke; a pause in typing saves, so closing the note keeps the edit.
    revision: u64,
    _subscriptions: [Subscription; 2],
}

impl TranscriptView {
    pub fn new(runtime: RuntimeHandle, session: SessionId, cx: &mut Context<Self>) -> Self {
        // Humans named by speaker hints label their segments, as shipping's renderer does.
        let reply = runtime.watch_query(
            "SELECT t.id, t.started_at_ms, t.words_json, t.speaker_hints_json,
               (SELECT json_group_array(json_object('id', h.id, 'name', h.name)) FROM humans h
                WHERE h.deleted_at IS NULL AND instr(t.speaker_hints_json, h.id) > 0) AS humans
             FROM transcripts t
             WHERE t.session_id = ? AND t.deleted_at IS NULL ORDER BY t.started_at_ms, t.id"
                .into(),
            vec![json!(session)],
        );
        let watch = cx.spawn(async move |this, cx| {
            let mut watch = match async { reply?.receive().await }.await {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            loop {
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let rendered = cx
                    .background_executor()
                    .spawn(async move { segments(&rows) })
                    .await;
                let error = watch.terminal_error().map(|error| error.to_string());
                let terminal = error.is_some();
                if this
                    .update(cx, |this, cx| {
                        match rendered {
                            Ok((segments, words)) => {
                                this.segments = segments;
                                this.word_transcripts = words;
                            }
                            Err(error) => this.error = Some(error),
                        }
                        if let Some(error) = error {
                            this.error = Some(error);
                        }
                        this.loaded = true;
                        this.follow(cx);
                        cx.notify();
                    })
                    .is_err()
                    || terminal
                    || watch.snapshots.changed().await.is_err()
                {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        });
        let live = LiveState::global(cx);
        let observe = live.as_ref().map(|live| {
            cx.observe(live, |this, _, cx| {
                this.follow(cx);
                cx.notify();
            })
        });
        let uploads = Uploads::global(cx);
        let observe_uploads = cx.observe(&uploads, |_, _, cx| cx.notify());
        Self {
            runtime,
            session,
            segments: Vec::new(),
            word_transcripts: HashMap::new(),
            loaded: false,
            audio: None,
            error: None,
            live,
            scroll: ScrollHandle::new(),
            edit_mode: false,
            editors: HashMap::new(),
            hovered: None,
            playing_ms: None,
            picker: None,
            find: None,
            found_word: None,
            focus: cx.focus_handle(),
            uploads,
            tab_menu: None,
            selected: HashSet::new(),
            anchor: None,
            order: Vec::new(),
            _playback: None,
            _watch: watch,
            _live: observe,
            _uploads: observe_uploads,
        }
    }

    /// The session's saved audio: it decides the empty screen's copy, makes words seekable
    /// and follows playback for the current line.
    pub fn set_audio(&mut self, audio: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.audio == audio {
            return;
        }
        self.audio = audio;
        self.playing_ms = None;
        self._playback = self.audio.clone().and_then(|path| {
            let playback = cx.try_global::<MeetingServices>()?.playback.clone();
            Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let snapshot = playback.snapshot();
                    let ms = (snapshot.path.as_ref() == Some(&path))
                        .then_some(snapshot.position.as_millis() as i64)
                        .filter(|ms| *ms > 0);
                    if this
                        .update(cx, |this, cx| {
                            if this.playing_ms != ms {
                                this.playing_ms = ms;
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }))
        });
        cx.notify();
    }

    pub fn edit_mode(&self) -> bool {
        self.edit_mode
    }

    /// Leaving edit mode saves every changed segment, as shipping's blur does.
    pub fn set_edit_mode(&mut self, edit_mode: bool, cx: &mut Context<Self>) {
        if self.edit_mode == edit_mode {
            return;
        }
        if !edit_mode {
            let ids: Vec<String> = self.editors.keys().cloned().collect();
            for id in ids {
                self.commit(&id, cx);
            }
            self.editors.clear();
        }
        self.edit_mode = edit_mode;
        cx.notify();
    }

    fn editor(
        &mut self,
        row: &Row,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TextInput>> {
        if let Some(editor) = self.editors.get(&row.id) {
            return Some(editor.input.clone());
        }
        let word_ids: Vec<String> = row
            .words
            .iter()
            .filter_map(|word| word.id.clone())
            .collect();
        let transcript = self.word_transcripts.get(word_ids.first()?)?.clone();
        let original = normalize(
            &row.words
                .iter()
                .map(|word| word.text.trim())
                .collect::<Vec<_>>()
                .join(" "),
        );
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline().multiline();
            input.set_text(original.clone(), cx);
            input
        });
        let id = row.id.clone();
        let blur = cx.on_blur(&input.focus_handle(cx), window, {
            let id = id.clone();
            move |this, _, cx| this.commit(&id, cx)
        });
        // Cmd/Ctrl-Enter finishes the segment, like shipping's editor.
        let submit = cx.subscribe_in(&input, window, {
            let id = id.clone();
            move |this, _, event, window, cx| match event {
                InputEvent::Submitted => window.blur(cx),
                InputEvent::Changed => this.save_after_pause(&id, cx),
                InputEvent::Rejected => {}
            }
        });
        self.editors.insert(
            id,
            SegmentEditor {
                input: input.clone(),
                original,
                transcript,
                word_ids,
                revision: 0,
                _subscriptions: [blur, submit],
            },
        );
        Some(input)
    }

    fn save_after_pause(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(id) else {
            return;
        };
        editor.revision += 1;
        let revision = editor.revision;
        let id = id.to_owned();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(800))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .editors
                    .get(&id)
                    .is_some_and(|editor| editor.revision == revision)
                {
                    this.commit(&id, cx);
                }
            });
        })
        .detach();
    }

    /// Saves a segment's edited text when it changed (shipping's blur handler).
    fn commit(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(id) else {
            return;
        };
        let text = normalize(&editor.input.read(cx).buffer.text);
        if text == editor.original || editor.word_ids.is_empty() {
            return;
        }
        editor.original = text.clone();
        let reply = TranscriptStore(self.runtime.clone()).edit_segment_text(
            editor.transcript.clone(),
            editor.word_ids.clone(),
            text,
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Transcript edit not saved: {error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Shipping's `seekAndPlay` for a clicked word.
    fn seek(&mut self, start_ms: i64, cx: &mut Context<Self>) {
        let (Some(_), Some(services)) = (&self.audio, cx.try_global::<MeetingServices>()) else {
            return;
        };
        let playback = services.playback.clone();
        let _ = playback.send(Command::Seek(Duration::from_millis(start_ms.max(0) as u64)));
        let _ = playback.send(Command::Play);
    }

    fn live(&self, cx: &App) -> Option<LiveMeeting> {
        self.live
            .as_ref()
            .and_then(|live| live.read(cx).session(&self.session).cloned())
            .filter(LiveMeeting::live)
    }

    /// Shipping keeps a live transcript pinned to its end while the reader is near the bottom.
    fn follow(&mut self, cx: &App) {
        if self.live(cx).is_some() {
            let offset = self.scroll.offset().y;
            let max = self.scroll.max_offset().y;
            if max + offset < px(120.) {
                self.scroll.scroll_to_bottom();
            }
        }
    }
}

/// Shipping's `useTranscriptScreen`.
#[derive(Debug, PartialEq)]
enum Screen {
    RunningBatch {
        phase: Phase,
        percentage: f64,
    },
    Listening {
        finalizing: bool,
    },
    Batch {
        requested_live: bool,
    },
    Empty {
        audio: bool,
        error: Option<SharedString>,
    },
    Ready,
}

fn screen(job: Option<&Job>, live: Option<&LiveMeeting>, has_words: bool, audio: bool) -> Screen {
    if let Some(Job::Running { phase, percentage }) = job {
        return Screen::RunningBatch {
            phase: *phase,
            percentage: *percentage,
        };
    }
    let visible = has_words || live.is_some_and(|live| !live.segments.is_empty());
    let active = live.is_some_and(|live| live.mode == SessionMode::Active);
    if active && live.is_some_and(|live| live.capture == CaptureMode::RecordOnly) {
        return Screen::Batch {
            requested_live: false,
        };
    }
    if !visible && live.is_some_and(|live| live.paused.is_some()) {
        return Screen::Batch {
            requested_live: true,
        };
    }
    if !visible && let Some(live) = live {
        return Screen::Listening {
            finalizing: live.mode == SessionMode::Finalizing,
        };
    }
    if !visible {
        // Like shipping's `TranscriptEmptyState`, a failed run's error stays on the empty screen.
        let error = match job {
            Some(Job::Failed(error)) => Some(error.clone()),
            _ => None,
        };
        return Screen::Empty { audio, error };
    }
    Screen::Ready
}

type Rendered = (Vec<RenderedTranscriptSegment>, HashMap<String, Arc<str>>);

fn segments(rows: &[Value]) -> Result<Rendered, String> {
    let parse = |row: &Value, key: &str| -> Result<Vec<Value>, String> {
        serde_json::from_str(row[key].as_str().unwrap_or("[]")).map_err(|error| error.to_string())
    };
    let mut transcripts = Vec::new();
    let mut word_transcripts = HashMap::new();
    let mut humans = Vec::new();
    for row in rows {
        let id: Arc<str> = row["id"].as_str().unwrap_or_default().into();
        for human in parse(row, "humans")? {
            if let (Some(human_id), Some(name)) = (human["id"].as_str(), human["name"].as_str())
                && !humans
                    .iter()
                    .any(|known: &RenderTranscriptHuman| known.human_id == human_id)
            {
                humans.push(RenderTranscriptHuman {
                    human_id: human_id.into(),
                    name: name.into(),
                });
            }
        }
        let words = parse(row, "words_json")?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for word in &words {
            let word: &crate::meeting::model::Word = word;
            word_transcripts.insert(word.id.clone(), id.clone());
        }
        let transcript = Transcript {
            id: id.clone(),
            started_at: row["started_at_ms"].as_i64().unwrap_or(0),
            revision: 0,
            sequence: 0,
            words,
            hints: parse(row, "speaker_hints_json")?,
        };
        transcripts.push(
            transcript
                .render_input()
                .map_err(|error| error.to_string())?,
        );
    }
    let segments = render_transcript_segments(RenderTranscriptRequest {
        speaker_context: None,
        preview: None,
        transcripts,
        participant_human_ids: Vec::new(),
        self_human_id: None,
        humans,
    });
    Ok((segments, word_transcripts))
}

/// Shipping's `normalizeEditableTranscriptText`.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One rendered segment: who spoke, and their words.
#[derive(Debug)]
struct Row {
    id: String,
    key: SegmentKey,
    label: SharedString,
    start_ms: i64,
    words: Vec<SegmentWord>,
    text: String,
}

/// Shipping's `mergeRenderedAndLiveSegments` then `mergeAdjacentSpeakerSegments`: stored words
/// the live stream still carries come from the live side, and consecutive segments from one
/// speaker join.
fn merge(stored: &[RenderedTranscriptSegment], live: &[Arc<LiveTranscriptSegment>]) -> Vec<Row> {
    let live_ids: HashSet<&str> = live.iter().map(|segment| segment.id.as_str()).collect();
    let live_words: HashSet<&str> = live
        .iter()
        .flat_map(|segment| segment.words.iter().filter_map(|word| word.id.as_deref()))
        .collect();
    let mut labeler = SpeakerLabeler::new();
    let mut rows: Vec<Row> = Vec::new();
    for segment in stored {
        if segment.key.speaker_human_id.is_none() {
            labeler.unknown_speaker_number(&segment.key);
        }
        if live_ids.contains(segment.id.as_str()) {
            continue;
        }
        let words: Vec<SegmentWord> = segment
            .words
            .iter()
            .filter(|word| !word.id.as_deref().is_some_and(|id| live_words.contains(id)))
            .cloned()
            .collect();
        if words.is_empty() && !segment.words.is_empty() {
            continue;
        }
        rows.push(Row {
            id: segment.id.clone(),
            key: segment.key.clone(),
            label: segment.speaker_label.clone().into(),
            start_ms: segment.start_ms,
            words,
            text: segment.text.clone(),
        });
    }
    for segment in live {
        let label = rows
            .iter()
            .find(|row| row.key == segment.key)
            .map(|row| row.label.clone())
            .unwrap_or_else(|| {
                format!("Speaker {}", labeler.unknown_speaker_number(&segment.key)).into()
            });
        rows.push(Row {
            id: segment.id.clone(),
            key: segment.key.clone(),
            label,
            start_ms: segment.start_ms,
            words: segment.words.clone(),
            text: segment.text.clone(),
        });
    }
    if live.is_empty() {
        return rows;
    }
    rows.sort_by_key(|row| row.start_ms);
    let mut merged: Vec<Row> = Vec::new();
    for row in rows {
        match merged.last_mut() {
            Some(last) if last.key == row.key => {
                last.words.extend(row.words);
                last.text.push(' ');
                last.text.push_str(&row.text);
            }
            _ => merged.push(row),
        }
    }
    merged
}

type RowText = (
    String,
    Vec<(Range<usize>, HighlightStyle)>,
    Vec<Range<usize>>,
);

/// A segment's words joined by spaces: partial words `opacity-60 italic`, the sentence being
/// played on yellow (shipping's `data-line-current`), the hovered seekable word on the accent.
/// Also returns each word's byte range, for clicks.
fn row_text(
    row: &Row,
    current_ms: Option<i64>,
    hovered: Option<usize>,
    dark: bool,
    accent: Hsla,
) -> RowText {
    if row.words.is_empty() {
        let text = row.text.split_whitespace().collect::<Vec<_>>().join(" ");
        return (text, Vec::new(), Vec::new());
    }
    let partial = HighlightStyle {
        font_style: Some(FontStyle::Italic),
        fade_out: Some(0.4),
        ..Default::default()
    };
    let current_line = HighlightStyle {
        background_color: Some(
            if dark {
                rgba(0x713f124d)
            } else {
                rgba(0xfef9c380)
            }
            .into(),
        ),
        ..Default::default()
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut ranges = Vec::new();
    for line in sentence_lines(&row.words) {
        let start = text.len();
        let words = &row.words[line.clone()];
        for word in words {
            if !text.is_empty() {
                text.push(' ');
            }
            let begin = text.len();
            text.push_str(word.text.trim());
            ranges.push(begin..text.len());
        }
        let first = words.first().map_or(0, |word| word.start_ms);
        let last = words.last().map_or(0, |word| word.end_ms);
        if current_ms.is_some_and(|ms| ms >= first && ms <= last) {
            highlights.push((start..text.len(), current_line));
        }
    }
    for (index, word) in row.words.iter().enumerate() {
        if !word.is_final {
            highlights.push((ranges[index].clone(), partial));
        }
    }
    if let Some(index) = hovered.filter(|index| seekable(&row.words[*index])) {
        highlights.push((
            ranges[index].clone(),
            HighlightStyle {
                background_color: Some(accent.opacity(0.6)),
                ..Default::default()
            },
        ));
    }
    (text, highlights, ranges)
}

/// Shipping's `groupWordsIntoLines`: sentences end at `.`, `?` or `!`.
fn sentence_lines(words: &[SegmentWord]) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, word) in words.iter().enumerate() {
        if word.text.trim().ends_with(['.', '?', '!']) {
            lines.push(start..index + 1);
            start = index + 1;
        }
    }
    if start < words.len() {
        lines.push(start..words.len());
    }
    lines
}

/// Shipping's `isTranscriptWordSeekable` rejects only synthetic text timing, which rendered
/// segments do not carry; partial live words are never played back.
fn seekable(word: &SegmentWord) -> bool {
    word.is_final
}

/// Shipping's `getSegmentColor`: golden-angle hues in OKLCH, remote speakers rotated 180°.
fn segment_color(key: &SegmentKey, dark: bool) -> Hsla {
    let mut index = key.speaker_index.unwrap_or(0) as u32 as f64;
    if let Some(human) = &key.speaker_human_id {
        let mut hash = 0u32;
        for unit in human.encode_utf16() {
            hash = hash.wrapping_mul(31).wrapping_add(u32::from(unit));
        }
        index = f64::from(hash);
    }
    let offset = if key.speaker_human_id.is_none() && key.channel == ChannelProfile::RemoteParty {
        180.
    } else {
        0.
    };
    let hue = (10. + index * 137.508 + offset) % 360.;
    oklch(if dark { 0.72 } else { 0.55 }, 0.15, hue)
}

fn oklch(lightness: f64, chroma: f64, hue: f64) -> Hsla {
    let (a, b) = (
        chroma * hue.to_radians().cos(),
        chroma * hue.to_radians().sin(),
    );
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    let channel = |linear: f64| {
        let linear = linear.clamp(0., 1.);
        let value = if linear <= 0.003_130_8 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1. / 2.4) - 0.055
        };
        (value * 255.).round() as u32
    };
    let red = channel(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s);
    let green = channel(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s);
    let blue = channel(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s);
    rgb((red << 16) | (green << 8) | blue).into()
}

/// Shipping's centered transcript status: icon, `text-base` title and muted description.
fn status(
    colors: Theme,
    icon: gpui::AnyElement,
    title: &'static str,
    description: impl Into<SharedString>,
) -> gpui::Div {
    // Shipping's transcript container adds `pt-2` above every screen.
    div()
        .size_full()
        .min_h(px(400.))
        .pt_2()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px_6()
        .text_center()
        .child(div().mb_5().child(icon))
        .child(
            div()
                .max_w(px(448.))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_base()
                        .line_height(px(24.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(px(22.75))
                        .text_color(colors.muted_foreground)
                        .child(description.into()),
                ),
        )
}

fn waveform(colors: Theme) -> gpui::AnyElement {
    svg()
        .path("meeting/AudioWaveformIcon.svg")
        .size(px(36.))
        .text_color(colors.muted_foreground)
        .into_any_element()
}

/// Shipping's `LiveTranscriptInterruptedNotice`.
fn paused_notice(colors: Theme, message: SharedString) -> impl IntoElement {
    div().w_full().flex().justify_center().child(
        div()
            .max_w(px(448.))
            .px_6()
            .text_center()
            .text_sm()
            .line_height(px(22.75))
            .text_color(colors.muted_foreground)
            .child(message),
    )
}

/// A small shadcn `Button` (`size="sm"`): primary, or outline with an optional icon.
fn button(
    id: &'static str,
    colors: Theme,
    label: &'static str,
    icon: Option<(&'static str, f32)>,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    let color = if primary {
        colors.primary_foreground
    } else {
        colors.foreground
    };
    div()
        .id(id)
        .h_7()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(px(RADIUS))
        .text_xs()
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .shadow_xs()
        .text_color(color)
        .map(|button| {
            if primary {
                button
                    .bg(colors.primary)
                    .hover(move |style| style.bg(colors.primary.opacity(0.9)))
            } else {
                button
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .hover(move |style| style.bg(colors.accent))
            }
        })
        .when_some(icon, |button, (path, size)| {
            button.child(svg().path(path).size(px(size)).text_color(color))
        })
        .child(label)
}

/// Shipping's batching `TranscriptEmptyState`: spinner, phase, progress and, once audio is
/// imported, Stop transcription.
fn batching(
    colors: Theme,
    phase: Phase,
    percentage: f64,
    stop: Option<gpui::Stateful<gpui::Div>>,
) -> gpui::Div {
    div()
        .size_full()
        .min_h(px(400.))
        .pt_2()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px_6()
        .text_center()
        .child(div().mb_5().pb(px(6.5)).child(spinner(
            "transcript-batching",
            36.,
            colors.muted_foreground,
        )))
        .child(
            div()
                .when(stop.is_some(), |view| view.mb_6())
                .child(
                    div()
                        .text_base()
                        .line_height(px(24.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(colors.foreground)
                        .child(match phase {
                            Phase::Importing => "Importing audio...",
                            Phase::Transcribing => "Generating transcript...",
                        }),
                )
                .when(percentage > 0., |view| {
                    view.child(
                        div()
                            .mt_2()
                            .text_sm()
                            .line_height(px(22.75))
                            .text_color(colors.muted_foreground)
                            .child(format!("{}% complete", (percentage * 100.).round())),
                    )
                }),
        )
        .children(stop)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabAction {
    Copy,
    ResumeListening,
    Retranscribe,
    DeleteRecording,
}

/// Shipping's `formatTranscriptExportSegments`.
fn transcript_text(segments: &[RenderedTranscriptSegment]) -> String {
    segments
        .iter()
        .map(|segment| format!("{}: {}", segment.speaker_label, segment.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Shipping's segment selection click: with Shift, the range from the anchor joins the
/// selection; otherwise the segment toggles.
fn select(
    selected: &mut HashSet<String>,
    anchor: Option<&str>,
    order: &[String],
    target: &str,
    range: bool,
) {
    let position = |id: &str| order.iter().position(|candidate| candidate == id);
    if range && let (Some(from), Some(to)) = (anchor.and_then(position), position(target)) {
        selected.extend(order[from.min(to)..=from.max(to)].iter().cloned());
        return;
    }
    if !selected.remove(target) {
        selected.insert(target.to_owned());
    }
}

/// Shipping's ⌘⇧↑ / ⌘⇧↓: everything from the first segment to the anchor, or from the anchor
/// to the last.
fn extend(order: &[String], anchor: &str, up: bool) -> Option<HashSet<String>> {
    let index = order.iter().position(|id| id == anchor)?;
    let range = if up {
        &order[..=index]
    } else {
        &order[index..]
    };
    Some(range.iter().cloned().collect())
}

/// Shipping's transcript search (`getTranscriptWordMatches`): each match belongs to a word,
/// whose matching text shows on `bg-yellow-500` when current and `bg-yellow-200/50` otherwise.
impl TranscriptView {
    pub fn find(&mut self, query: &str, options: FindOptions, cx: &mut Context<Self>) {
        let query = query.trim();
        self.find = (!query.is_empty()).then(|| (query.to_owned(), options, 0));
        self.reveal_found(cx);
        cx.notify();
    }

    pub fn find_status(&self, cx: &App) -> (usize, usize) {
        let count = self.found(cx).len();
        let active = self.find.as_ref().map_or(0, |find| find.2);
        (active.min(count.saturating_sub(1)), count)
    }

    pub fn find_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = self.found(cx).len();
        if let Some(find) = &mut self.find
            && count > 0
        {
            find.2 = (find.2.min(count - 1) + if forward { 1 } else { count - 1 }) % count;
            self.reveal_found(cx);
            cx.notify();
        }
    }

    /// Each match's row and word, in transcript order.
    fn found(&self, cx: &App) -> Vec<(usize, usize)> {
        let Some((query, options, _)) = &self.find else {
            return Vec::new();
        };
        let live = self.live(cx);
        let rows = merge(
            &self.segments,
            live.as_ref().map(|live| &live.segments[..]).unwrap_or(&[]),
        );
        let words: Vec<(usize, usize, &str)> = rows
            .iter()
            .enumerate()
            .flat_map(|(row, segment)| {
                segment
                    .words
                    .iter()
                    .enumerate()
                    .map(move |(word, value)| (row, word, value.text.trim()))
            })
            .collect();
        let texts: Vec<&str> = words.iter().map(|(_, _, text)| *text).collect();
        super::find::transcript_matches(&texts, query, *options)
            .into_iter()
            .map(|index| (words[index].0, words[index].1))
            .collect()
    }

    fn reveal_found(&mut self, cx: &App) {
        let active = self.find.as_ref().map_or(0, |find| find.2);
        if let Some((row, _)) = self.found(cx).get(active) {
            self.scroll
                .scroll_to_item(row + usize::from(self.error.is_some()));
        }
    }

    /// Lays the find highlights for row `index` over its other highlights.
    fn found_highlights(
        &self,
        index: usize,
        row: &Row,
        ranges: &[Range<usize>],
        text: &str,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let Some((query, options, _)) = &self.find else {
            return highlights;
        };
        let found: Vec<_> = ranges
            .iter()
            .enumerate()
            .take(row.words.len())
            .flat_map(|(word, range)| {
                let style = HighlightStyle {
                    background_color: Some(super::find::transcript_match_color(
                        self.found_word == Some((index, word)),
                    )),
                    ..Default::default()
                };
                super::find::highlight_ranges(&text[range.clone()], query, *options)
                    .into_iter()
                    .map(move |found| (range.start + found.start..range.start + found.end, style))
            })
            .collect();
        if found.is_empty() {
            return highlights;
        }
        gpui::combine_highlights(highlights, found.clone())
            .map(|(range, mut style)| {
                if let Some((_, found)) = found
                    .iter()
                    .find(|(found, _)| found.start <= range.start && range.end <= found.end)
                {
                    style.background_color = found.background_color;
                }
                (range, style)
            })
            .collect()
    }
}
impl Render for TranscriptView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.found_word = self.find.as_ref().and_then(|find| {
            let found = self.found(cx);
            found
                .get(find.2.min(found.len().saturating_sub(1)))
                .copied()
        });
        let colors = theme(window);
        let dark = colors.background.l < 0.5;
        let live = self.live(cx);
        if !self.loaded && self.error.is_none() {
            return div().id("note-transcript").size_full();
        }
        let has_words = self
            .segments
            .iter()
            .any(|segment| !segment.words.is_empty());
        let job = self.uploads.read(cx).job(&self.session).cloned();
        let screen = screen(job.as_ref(), live.as_ref(), has_words, self.audio.is_some());
        let frame = div()
            .id("note-transcript")
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down));
        let frame = match screen {
            Screen::RunningBatch { phase, percentage } => {
                let stop = (phase == Phase::Transcribing).then(|| {
                    button(
                        "transcript-stop",
                        colors,
                        "Stop transcription",
                        Some(("meeting/SquareIcon.svg", 12.)),
                        false,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let session = this.session.clone();
                        this.uploads
                            .update(cx, |uploads, cx| uploads.stop(&session, cx));
                    }))
                });
                frame.child(batching(colors, phase, percentage, stop))
            }
            Screen::Listening { finalizing } => frame.child(if finalizing {
                status(
                    colors,
                    // Shipping's spinner is an inline-block, so the line adds its descent below.
                    div()
                        .pb(px(6.5))
                        .child(spinner(
                            "transcript-finalizing",
                            36.,
                            colors.muted_foreground,
                        ))
                        .into_any_element(),
                    "Finalizing transcript...",
                    "Transcript is still being written.",
                )
            } else {
                status(
                    colors,
                    waveform(colors),
                    "Listening...",
                    "Transcript will appear here when the first segment arrives.",
                )
            }),
            Screen::Batch {
                requested_live: true,
            } => frame.child(
                div()
                    .size_full()
                    .min_h(px(400.))
                    .pt_2()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(live.and_then(|live| live.paused), |view, message| {
                        view.child(paused_notice(colors, message))
                    }),
            ),
            Screen::Batch {
                requested_live: false,
            } => frame.child(status(
                colors,
                dancing_sticks(
                    "transcript-batch",
                    live.map(|live| live.amplitude).unwrap_or_default(),
                    rgb(0xa3a3a3).into(),
                    (80., 36.),
                    (3., 3.),
                )
                .into_any_element(),
                "Batch transcription mode",
                "Recording continues. Your transcript will be generated after you stop.",
            )),
            Screen::Empty {
                audio: _,
                error: Some(error),
            } => frame.child(
                status(
                    colors,
                    svg()
                        .path("meeting/AlertCircleIcon.svg")
                        .size(px(36.))
                        .text_color(colors.muted_foreground)
                        .into_any_element(),
                    "Transcription failed",
                    error,
                )
                .child(div().mt_6().child(self.retranscribe_button(colors, cx))),
            ),
            Screen::Empty { audio, error: None } => frame.child(
                status(
                    colors,
                    waveform(colors),
                    if audio {
                        "Audio available"
                    } else {
                        "No transcript available"
                    },
                    if audio {
                        "Re-transcribe this audio, or upload a transcript file."
                    } else {
                        "Upload audio or a transcript file to populate this note."
                    },
                )
                .child(
                    div()
                        .mt_6()
                        .flex()
                        .items_center()
                        .gap_2()
                        .map(|row| {
                            if audio {
                                row.child(self.retranscribe_button(colors, cx))
                            } else {
                                row.child(
                                    button(
                                        "transcript-upload-audio",
                                        colors,
                                        "Upload audio",
                                        None,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            let (runtime, session) =
                                                (this.runtime.clone(), this.session.clone());
                                            this.uploads.update(cx, |uploads, cx| {
                                                uploads.upload_audio(&runtime, session, cx)
                                            });
                                        },
                                    )),
                                )
                            }
                        })
                        .child(
                            button(
                                "transcript-upload-transcript",
                                colors,
                                "Upload transcript",
                                None,
                                false,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let (runtime, session) =
                                    (this.runtime.clone(), this.session.clone());
                                this.uploads.update(cx, |uploads, cx| {
                                    uploads.upload_transcript(&runtime, session, cx)
                                });
                            })),
                        ),
                ),
            ),
            Screen::Ready => {
                let paused = live.as_ref().and_then(|live| live.paused.clone());
                let rows = merge(
                    &self.segments,
                    live.as_ref().map(|live| &live.segments[..]).unwrap_or(&[]),
                );
                self.order = rows.iter().map(|row| row.id.clone()).collect();
                // Shipping edits and seeks only once the meeting is over.
                let editing = self.edit_mode && live.is_none();
                let audio = self.audio.is_some() && live.is_none();
                // Shipping's Space shortcut listens on the document: with nothing more specific
                // focused, the transcript takes the keyboard.
                if !editing
                    && self.picker.is_none()
                    && !self.focus.is_focused(window)
                    && self.focus.within_focused(window, cx)
                {
                    let focus = self.focus.clone();
                    window.defer(cx, move |window, cx| focus.focus(window, cx));
                }
                let mut children = Vec::with_capacity(rows.len());
                for (index, row) in rows.iter().enumerate() {
                    let editor = if editing {
                        self.editor(row, window, cx)
                    } else {
                        None
                    };
                    let selected = self.selected.contains(&row.id);
                    let header =
                        self.segment_header(index, row, editing, selected, dark, colors, cx);
                    let body = div()
                        .mt(px(6.))
                        .text_sm()
                        .line_height(px(22.75))
                        .text_color(colors.foreground);
                    let body = match editor {
                        Some(input) => body
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(input)
                            .into_any_element(),
                        None => body
                            .child(self.words(index, row, audio, colors, cx))
                            .into_any_element(),
                    };
                    let id = row.id.clone();
                    children.push(
                        div()
                            .when(index > 0, |view| view.pt_4())
                            .child(
                                div()
                                    .id(("segment", index))
                                    .relative()
                                    .px_2()
                                    .rounded(px(8.))
                                    .when(editing, |view| view.cursor_pointer())
                                    .when(selected, |view| {
                                        view.bg(colors.primary.opacity(0.1)).child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .rounded(px(8.))
                                                .border_1()
                                                .border_color(colors.primary.opacity(0.3)),
                                        )
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |this, event: &MouseDownEvent, window, cx| {
                                                this.click_segment(&id, event, window, cx)
                                            },
                                        ),
                                    )
                                    .child(header)
                                    .child(body),
                            )
                            .into_any_element(),
                    );
                }
                frame
                    .child(
                        div()
                            .id("note-transcript-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .px_1()
                            .pt_2()
                            .pb(px(80.))
                            .when_some(self.error.clone(), |view, error| {
                                view.child(
                                    div()
                                        .px_2()
                                        .text_sm()
                                        .text_color(colors.muted_foreground)
                                        .child(format!("Transcript unavailable: {error}")),
                                )
                            })
                            .children(children)
                            .when_some(paused, |view, message| {
                                view.child(div().pt_8().child(paused_notice(colors, message)))
                            }),
                    )
                    .children(self.scroll_controls(colors, cx))
            }
        };
        let menu = self
            .tab_menu
            .map(|position| self.tab_menu_view(position, colors, cx));
        frame.children(menu)
    }
}

impl TranscriptView {
    /// Shipping's `SegmentHeader`: the select circle while editing, then the speaker label,
    /// which opens the speaker picker.
    #[allow(clippy::too_many_arguments)]
    fn segment_header(
        &self,
        index: usize,
        row: &Row,
        editing: bool,
        selected: bool,
        dark: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = segment_color(&row.key, dark);
        let open = self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.segment == row.id);
        let target = speakers::Target::new(row, &self.word_transcripts);
        div()
            .relative()
            .py_1()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .line_height(px(16.))
            .font_weight(FontWeight::LIGHT)
            .when(editing, |header| {
                header.child(
                    div()
                        .size_4()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .map(|circle| {
                            if selected {
                                circle
                                    .border_color(colors.primary)
                                    .bg(colors.primary)
                                    .child(
                                        svg()
                                            .path("editor/CheckIcon.svg")
                                            .size(px(10.))
                                            .text_color(colors.primary_foreground),
                                    )
                            } else {
                                circle.border_color(colors.muted_foreground.opacity(0.4))
                            }
                        }),
                )
            })
            .child(
                div()
                    .relative()
                    // Shipping's selection ignores clicks on the speaker label and its picker.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .id(("speaker", index))
                            .my(px(-2.))
                            .py(px(2.))
                            .pr_2()
                            .cursor_pointer()
                            .text_color(color)
                            .when(open, |label| label.underline())
                            .hover(|style| style.underline())
                            .when_some(target, |label, target| {
                                label.on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_picker(target.clone(), window, cx)
                                }))
                            })
                            .child(row.label.clone()),
                    )
                    .when(open, |view| {
                        view.children(self.speaker_picker(colors, cx).map(|picker| {
                            div().absolute().top(px(-2.)).left_full().child(
                                gpui::deferred(gpui::anchored().snap_to_window().child(picker))
                                    .with_priority(1),
                            )
                        }))
                    }),
            )
            .into_any_element()
    }

    /// The segment's words; with audio, each seeks playback (shipping's `WordSpan`).
    fn words(
        &self,
        index: usize,
        row: &Row,
        audio: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self
            .hovered
            .filter(|(row, _)| audio && *row == index)
            .map(|(_, word)| word);
        let current = self.playing_ms.filter(|_| audio);
        let dark = colors.background.l < 0.5;
        let (text, highlights, ranges) = row_text(row, current, hovered, dark, colors.accent);
        let highlights = self.found_highlights(index, row, &ranges, &text, highlights);
        let text = StyledText::new(text).with_highlights(highlights);
        if !audio || ranges.is_empty() {
            return text.into_any_element();
        }
        let starts: Vec<i64> = row.words.iter().map(|word| word.start_ms).collect();
        let view = cx.entity().downgrade();
        let hover_view = view.clone();
        let word_ranges = ranges.clone();
        InteractiveText::new(("transcript-words", index), text)
            .on_click(ranges, move |word, window, cx| {
                // Modifier clicks select segments instead (shipping's capture handler).
                let modifiers = window.modifiers();
                if modifiers.shift || modifiers.platform || modifiers.control {
                    return;
                }
                let start = starts[word];
                let _ = view.update(cx, |this, cx| this.seek(start, cx));
            })
            .on_hover(move |position, _, _, cx| {
                let word = position.and_then(|position| {
                    word_ranges
                        .iter()
                        .position(|range| range.contains(&position))
                });
                let _ = hover_view.update(cx, |this, cx| {
                    let hovered = word.map(|word| (index, word));
                    if this.hovered != hovered {
                        this.hovered = hovered;
                        cx.notify();
                    }
                });
            })
            .into_any_element()
    }

    fn retranscribe_button(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        button(
            "transcript-retranscribe",
            colors,
            "Re-transcribe",
            Some(("meeting/RefreshIcon.svg", 16.)),
            true,
        )
        .on_click(cx.listener(|this, _, _, cx| this.tab_action(TabAction::Retranscribe, cx)))
    }

    fn click_segment(
        &mut self,
        id: &str,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clicks on segment editors and the speaker picker never reach here.
        self.focus.focus(window, cx);
        let modifiers = event.modifiers;
        let modified = modifiers.shift || modifiers.platform || modifiers.control;
        if !self.edit_mode && !modified {
            if !self.selected.is_empty() {
                self.selected.clear();
                self.anchor = None;
                cx.notify();
            }
            return;
        }
        select(
            &mut self.selected,
            self.anchor.as_deref(),
            &self.order,
            id,
            modifiers.shift,
        );
        self.anchor = Some(id.to_owned());
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if keystroke.key == "escape" && self.tab_menu.take().is_some() {
            cx.stop_propagation();
            cx.notify();
            return;
        }
        // Keys typed into a segment editor or the speaker search stay there.
        if !self.focus.is_focused(window) {
            return;
        }
        let secondary = if cfg!(target_os = "macos") {
            modifiers.platform
        } else {
            modifiers.control
        };
        match keystroke.key.as_str() {
            "space" if !modifiers.modified() && !self.edit_mode => {
                self.toggle_playback(cx);
                cx.stop_propagation();
            }
            "escape" if self.edit_mode && !self.selected.is_empty() => {
                self.selected.clear();
                self.anchor = None;
                cx.stop_propagation();
                cx.notify();
            }
            "up" | "down" if modifiers.shift && secondary && !self.selected.is_empty() => {
                if let Some(selected) = self
                    .anchor
                    .as_deref()
                    .and_then(|anchor| extend(&self.order, anchor, keystroke.key == "up"))
                {
                    self.selected = selected;
                    cx.stop_propagation();
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    /// Shipping's Space: pause while playing, otherwise play this note's recording.
    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        let (Some(audio), Some(services)) =
            (self.audio.clone(), cx.try_global::<MeetingServices>())
        else {
            return;
        };
        if self.live(cx).is_some() {
            return;
        }
        let playback = services.playback.clone();
        let snapshot = playback.snapshot();
        if snapshot.path.as_ref() != Some(&audio) {
            let _ = playback.send(Command::Open(audio));
            let _ = playback.send(Command::Play);
            return;
        }
        let _ = playback.send(match snapshot.state() {
            State::Playing => Command::Pause,
            State::Paused | State::Stopped => Command::Play,
        });
    }

    /// Opens shipping's Transcript tab context menu (`HeaderViewTranscriptActive`).
    pub fn open_tab_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.tab_menu = Some(position);
        cx.notify();
    }

    fn tab_menu_items(&self, cx: &App) -> Vec<(TabAction, &'static str, bool)> {
        // Shipping offers listening and re-transcription only while the session is inactive
        // (listening also while a batch runs).
        let inactive = self
            .live
            .as_ref()
            .is_none_or(|live| live.read(cx).session(&self.session).is_none());
        let uploads = self.uploads.read(cx);
        let has_words = self
            .segments
            .iter()
            .any(|segment| !segment.words.is_empty());
        let mut items = vec![(TabAction::Copy, "Copy", !has_words)];
        if inactive {
            items.push((TabAction::ResumeListening, "Resume listening", false));
        }
        if self.audio.is_some() && inactive && !uploads.running(&self.session) {
            items.push((TabAction::Retranscribe, "Re-transcribe", false));
        }
        if self.audio.is_some() {
            items.push((
                TabAction::DeleteRecording,
                "Delete recording",
                uploads.deleting(&self.session),
            ));
        }
        items
    }

    fn tab_menu_view(
        &self,
        position: Point<Pixels>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = self.tab_menu_items(cx);
        gpui::deferred(
            gpui::anchored().position(position).snap_to_window().child(
                resource_list::context_menu(colors)
                    .id("transcript-tab-menu")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.tab_menu = None;
                        cx.notify();
                    }))
                    .children(items.into_iter().map(|(action, label, disabled)| {
                        div()
                            .id(label)
                            .h(px(30.))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_sm()
                            .rounded(px(4.))
                            .text_color(colors.foreground)
                            .when_else(
                                disabled,
                                |item| item.opacity(0.5),
                                |item| {
                                    item.cursor_pointer()
                                        .hover(move |style| style.bg(colors.accent))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.tab_menu = None;
                                            this.tab_action(action, cx);
                                        }))
                                },
                            )
                            .child(label)
                    })),
            ),
        )
        .with_priority(2)
        .into_any_element()
    }

    fn tab_action(&mut self, action: TabAction, cx: &mut Context<Self>) {
        let (runtime, session) = (self.runtime.clone(), self.session.clone());
        match action {
            TabAction::Copy => {
                let text = transcript_text(&self.segments);
                if !text.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    crate::application::toasts::success("Transcript copied to clipboard", cx);
                }
            }
            TabAction::ResumeListening => cx.emit(TranscriptEvent::ResumeListening),
            TabAction::Retranscribe => self.uploads.update(cx, |uploads, cx| {
                uploads.retranscribe(&runtime, session, cx)
            }),
            TabAction::DeleteRecording => self.uploads.update(cx, |uploads, cx| {
                uploads.delete_recording(&runtime, session, cx)
            }),
        }
        cx.notify();
    }

    /// Shipping's scroll-to-top and scroll-to-bottom controls, shown once the transcript scrolls.
    fn scroll_controls(&self, colors: Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let max = self.scroll.max_offset().y;
        if max <= px(0.) {
            return None;
        }
        let offset = -self.scroll.offset().y;
        let (at_top, at_bottom) = (offset <= px(0.), offset >= max);
        let button = |id: &'static str, path: &'static str, disabled: bool| {
            div()
                .id(id)
                .size_8()
                .flex()
                .items_center()
                .justify_center()
                .when_else(
                    disabled,
                    |button| button.opacity(0.3),
                    move |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.muted.opacity(0.55)))
                    },
                )
                .child(
                    svg()
                        .path(path)
                        .size(px(14.))
                        .text_color(colors.muted_foreground.opacity(0.45))
                        .group_hover("transcript-scroll", move |style| {
                            style.text_color(colors.foreground)
                        }),
                )
        };
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_1()
                .flex()
                .items_center()
                .child(
                    div()
                        .id("transcript-scroll-controls")
                        .group("transcript-scroll")
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(gpui::transparent_black())
                        .hover(move |style| {
                            style
                                .border_color(colors.border.opacity(0.5))
                                .bg(colors.background.opacity(0.65))
                                .shadow_sm()
                        })
                        .child(
                            button("transcript-scroll-top", "editor/ArrowUp02Icon.svg", at_top)
                                .when(!at_top, |button| {
                                    button.on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll.set_offset(point(px(0.), px(0.)));
                                        cx.notify();
                                    }))
                                }),
                        )
                        .child(
                            div()
                                .h(px(1.))
                                .w_full()
                                .bg(colors.border.opacity(0.2))
                                .group_hover("transcript-scroll", move |style| {
                                    style.bg(colors.border.opacity(0.6))
                                }),
                        )
                        .child(
                            button(
                                "transcript-scroll-bottom",
                                "editor/ArrowDown02Icon.svg",
                                at_bottom,
                            )
                            .when(!at_bottom, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| {
                                    this.scroll.scroll_to_bottom();
                                    cx.notify();
                                }))
                            }),
                        ),
                ),
        )
    }
}

impl Focusable for TranscriptView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_rows_render_into_speaker_segments() {
        let rows = [json!({
            "id": "t",
            "started_at_ms": 0,
            "words_json": json!([
                {"id":"a","text":"Hello","start_ms":0,"end_ms":200,"channel":0},
                {"id":"b","text":" there.","start_ms":200,"end_ms":400,"channel":0},
                {"id":"c","text":"Hi","start_ms":3000,"end_ms":3200,"channel":1}
            ]).to_string(),
            "speaker_hints_json": "[]"
        })];
        let (segments, words) = segments(&rows).unwrap();
        assert_eq!(words.get("c").map(AsRef::as_ref), Some("t"));
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text.trim(), "Hello there.");
        assert_eq!(segments[1].key.channel, ChannelProfile::RemoteParty);
    }

    #[test]
    fn segment_colors_match_shipping_oklch_palette() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        // chroma.oklch(0.55, 0.15, hue).hex() in shipping.
        assert_eq!(segment_color(&key, false), Hsla::from(rgb(0xb7445d)));
        let remote = SegmentKey {
            channel: ChannelProfile::RemoteParty,
            ..key
        };
        assert_eq!(segment_color(&remote, false), Hsla::from(rgb(0x008b85)));
    }

    fn word(id: Option<&str>, text: &str, final_: bool) -> SegmentWord {
        SegmentWord {
            text: text.into(),
            start_ms: 0,
            end_ms: 0,
            channel: ChannelProfile::DirectMic,
            is_final: final_,
            id: id.map(Into::into),
        }
    }

    #[test]
    fn live_words_replace_stored_ones_and_one_speaker_stays_one_segment() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        let stored = vec![RenderedTranscriptSegment {
            provisional_speaker: None,
            id: "stored".into(),
            key: key.clone(),
            speaker_label: "Speaker 1".into(),
            start_ms: 0,
            end_ms: 900,
            text: "Hello there".into(),
            words: vec![
                word(Some("a"), "Hello", true),
                word(Some("b"), " there", true),
            ],
        }];
        let live = vec![Arc::new(LiveTranscriptSegment {
            id: "live".into(),
            key,
            start_ms: 500,
            end_ms: 1500,
            text: String::new(),
            words: vec![word(Some("b"), " there", true), word(None, " frie", false)],
        })];
        let rows = merge(&stored, &live);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_ref(), "Speaker 1");
        let (text, highlights, _) = row_text(&rows[0], None, None, false, gpui::black());
        assert_eq!(text, "Hello there frie");
        assert_eq!(highlights.len(), 1);
        assert_eq!(&text[highlights[0].0.clone()], "frie");
    }

    #[test]
    fn the_played_sentence_is_highlighted_and_words_keep_their_ranges() {
        let key = SegmentKey {
            channel: ChannelProfile::DirectMic,
            speaker_index: None,
            speaker_human_id: None,
        };
        let timed = |id: &str, text: &str, start_ms: i64, end_ms: i64| SegmentWord {
            start_ms,
            end_ms,
            ..word(Some(id), text, true)
        };
        let row = Row {
            id: "s".into(),
            key,
            label: "Speaker 1".into(),
            start_ms: 0,
            words: vec![
                timed("a", "One.", 0, 100),
                timed("b", " Two", 200, 300),
                timed("c", " three.", 300, 400),
            ],
            text: String::new(),
        };
        let (text, highlights, ranges) = row_text(&row, Some(250), Some(0), false, gpui::black());
        assert_eq!(text, "One. Two three.");
        assert_eq!(&text[ranges[1].clone()], "Two");
        let playing = highlights
            .iter()
            .find(|(_, style)| style.background_color.is_some_and(|color| color.a < 0.6))
            .map(|(range, _)| &text[range.clone()]);
        // Shipping's line span holds the space before it too.
        assert_eq!(playing, Some(" Two three."));
        assert_eq!(
            highlights.len(),
            2,
            "the played sentence and the hovered word"
        );
        assert_eq!(normalize("  Hi \n there "), "Hi there");
    }

    #[test]
    fn screens_follow_shipping_capture_states() {
        let mut live = LiveMeeting::new(SessionId("s".into()), SessionMode::Active);
        assert_eq!(
            screen(None, Some(&live), false, false),
            Screen::Listening { finalizing: false }
        );
        live.capture = CaptureMode::RecordOnly;
        assert_eq!(
            screen(None, Some(&live), true, false),
            Screen::Batch {
                requested_live: false
            }
        );
        live.capture = CaptureMode::FallbackRecordOnly;
        live.paused = Some("paused".into());
        assert_eq!(
            screen(None, Some(&live), false, false),
            Screen::Batch {
                requested_live: true
            }
        );
        assert_eq!(screen(None, Some(&live), true, false), Screen::Ready);
        live.mode = SessionMode::Finalizing;
        live.paused = None;
        assert_eq!(
            screen(None, Some(&live), false, false),
            Screen::Listening { finalizing: true }
        );
        assert_eq!(
            screen(None, None, false, true),
            Screen::Empty {
                audio: true,
                error: None
            }
        );
        assert_eq!(screen(None, None, true, false), Screen::Ready);
        // A running upload or re-transcription replaces every other screen; a failure shows
        // only while there are no words.
        let running = Job::Running {
            phase: Phase::Transcribing,
            percentage: 0.4,
        };
        assert_eq!(
            screen(Some(&running), None, true, true),
            Screen::RunningBatch {
                phase: Phase::Transcribing,
                percentage: 0.4
            }
        );
        let failed = Job::Failed("No speech was detected in the audio.".into());
        assert_eq!(
            screen(Some(&failed), None, false, true),
            Screen::Empty {
                audio: true,
                error: Some("No speech was detected in the audio.".into())
            }
        );
        assert_eq!(screen(Some(&failed), None, true, true), Screen::Ready);
    }

    #[test]
    fn segment_selection_toggles_ranges_and_extends_from_the_anchor() {
        let order: Vec<String> = ["a", "b", "c", "d"].map(String::from).into();
        let mut selected = HashSet::new();
        select(&mut selected, None, &order, "b", false);
        select(&mut selected, Some("b"), &order, "d", true);
        assert_eq!(selected, ["b", "c", "d"].map(String::from).into());
        select(&mut selected, Some("d"), &order, "c", false);
        assert!(!selected.contains("c"));
        assert_eq!(
            extend(&order, "b", true),
            Some(["a", "b"].map(String::from).into())
        );
        assert_eq!(
            extend(&order, "b", false),
            Some(["b", "c", "d"].map(String::from).into())
        );
    }
}
