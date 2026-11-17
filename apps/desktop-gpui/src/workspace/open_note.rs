pub(super) mod meeting_records;
mod memo;
mod summary;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use desktop_runtime::{
    CancellationToken, DocumentId, DocumentSnapshot, Generation, OpenSession, RenameSession,
    RuntimeHandle, SessionId,
};
use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Hsla, MouseButton, Render, SharedString, Subscription, Task, Window, canvas, div,
    prelude::*, px, rgb, svg,
};
use serde_json::{Value, json};

use super::{
    library::{OpenNote, folder_label},
    resource_list,
    templates::{self, Library},
};
use crate::{
    contracts::{
        EditorEvent, EditorInit, LaneContext, MeetingIntent, ProductRoute, WorkspaceEvent,
    },
    editor::{EditorPane, title::first_line_title, transcript::TranscriptView},
    meeting::{
        MeetingServices,
        live::{LiveState, SessionMode},
        player_view::PlayerView,
        widgets::{dancing_sticks, spinner},
    },
    product::sharing::{self, SharePopover, SharePopoverEvent},
    ui::{
        input::{InputEvent, TextInput},
        theme::{RADIUS, Theme, theme},
    },
};

/// Shipping's `red-500`, used by the record mark and the stop square.
const RECORD_RED: u32 = 0xef4444;

pub struct NoteView {
    runtime: RuntimeHandle,
    title: Entity<TextInput>,
    current: Option<Arc<OpenSession>>,
    content: Option<AnyView>,
    message: String,
    busy: bool,
    loading: bool,
    generation: Generation,
    cancellation: CancellationToken,
    rename_pending: bool,
    rename_error: bool,
    /// The open session's live capture mode.
    mode: SessionMode,
    /// Saved audio for the open session, which enables the transcript tab and its player.
    audio: Option<PathBuf>,
    player: Option<Entity<PlayerView>>,
    menu: bool,
    menu_index: Option<usize>,
    menu_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    details: Details,
    view: Option<NoteTab>,
    summary_editor: Option<(DocumentId, Entity<EditorPane>, Subscription)>,
    transcript: Option<Entity<TranscriptView>>,
    details_task: Option<Task<()>>,
    compact_folder: bool,
    share: Option<(SessionId, Entity<SharePopover>, Subscription)>,
    /// Summary generations by session; they outlive switching notes, as shipping's tasks do.
    jobs: HashMap<SessionId, summary::SummaryJob>,
    job_poll: Option<Task<()>>,
    model: Option<summary::ModelState>,
    picker: Option<summary::TemplatePicker>,
    templates: Arc<Library>,
    templates_cancel: CancellationToken,
    /// A template applied to the memo, recorded on it once saved.
    memo_template: Option<(SessionId, Arc<str>)>,
    memo_saved: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<WorkspaceEvent> for NoteView {}

#[derive(Clone)]
pub enum NoteEvent {
    Opened(Arc<OpenSession>),
    Failed,
    TitleUpdated(Arc<OpenSession>),
    RenameFailed,
    /// A library operation (move, delete, open in window) started from the note header.
    Request(OpenNote),
    /// The summary template picker's "See all templates" and "Create new template".
    OpenTemplates,
}

impl EventEmitter<NoteEvent> for NoteView {}

/// What the open note's header needs beyond the session row, kept live by one watch.
#[derive(Default)]
struct Details {
    folder: String,
    event_ended: bool,
    has_transcript: bool,
    summaries: Vec<DocumentSnapshot>,
    /// Each visible summary's stored title and template, for its tab.
    summary_meta: HashMap<Arc<str>, (Arc<str>, Arc<str>)>,
    /// The calendar event's title and description, and its participant count, which pick
    /// the memo's suggested templates.
    event_text: String,
    participants: usize,
}

/// The note's views, in shipping's switcher order: summaries, memos, transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
enum NoteTab {
    Summary(Arc<str>),
    Memos,
    Transcript,
}

fn note_tabs(summaries: &[Arc<str>], transcript: bool) -> Vec<NoteTab> {
    let mut tabs: Vec<NoteTab> = summaries.iter().cloned().map(NoteTab::Summary).collect();
    tabs.push(NoteTab::Memos);
    if transcript {
        tabs.push(NoteTab::Transcript);
    }
    tabs
}

/// Shipping's `computeCurrentNoteTab`: a missing choice opens the first summary, and a
/// live recording only keeps a choice that is still available.
fn current_tab(chosen: Option<&NoteTab>, live: bool, tabs: &[NoteTab]) -> NoteTab {
    let first_summary = tabs
        .iter()
        .find(|tab| matches!(tab, NoteTab::Summary(_)))
        .cloned();
    match chosen {
        Some(tab) if tabs.contains(tab) => tab.clone(),
        Some(NoteTab::Summary(_)) if !live => first_summary.unwrap_or(NoteTab::Memos),
        Some(_) => NoteTab::Memos,
        None if live => NoteTab::Memos,
        None => first_summary.unwrap_or(NoteTab::Memos),
    }
}

/// Mirrors shipping's `visibleSummaryDocuments` and `defaultSummaryDocumentId`: untouched
/// placeholder summaries hide behind real ones, and a finished transcript always offers one.
fn summary_ids(details: &[Value], session: &SessionId, transcript_done: bool) -> Vec<Arc<str>> {
    let placeholder = |row: &Value| {
        row["kind"] == "summary"
            && row["template_id"] == ""
            && row["body"] == ""
            && row["title"] == "Summary"
            && row["created_at"] == row["updated_at"]
            && row["generation_metadata_json"] == "{}"
    };
    let ids: Vec<Arc<str>> = details
        .iter()
        .filter(|row| !placeholder(row) || details.iter().all(&placeholder))
        .filter_map(|row| row["id"].as_str().map(Into::into))
        .collect();
    if ids.is_empty() && transcript_done {
        vec![format!("summary:{}", session.0).into()]
    } else {
        ids
    }
}

/// Shipping's `hasSummaryContent`: text beyond a leading heading that only repeats the title.
fn summary_has_content(body: &str, title: &str) -> bool {
    fn text(node: &Value) -> String {
        node["text"].as_str().map(str::to_owned).unwrap_or_else(|| {
            node["content"]
                .as_array()
                .map(|children| children.iter().map(text).collect())
                .unwrap_or_default()
        })
    }
    let trimmed = body.trim();
    let Ok(doc) = serde_json::from_str::<Value>(trimmed) else {
        return !trimmed.is_empty();
    };
    if !trimmed.starts_with('{') || doc["type"] != "doc" {
        return true;
    }
    let blocks = doc["content"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let title_block = blocks.first().is_some_and(|block| {
        !title.trim().is_empty()
            && block["type"] == "heading"
            && block["attrs"]["level"] == 1
            && text(block).trim() == title.trim()
    });
    blocks[usize::from(title_block)..]
        .iter()
        .any(|block| !text(block).trim().is_empty() || block["type"] == "image")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuAction {
    MeetingInfo,
    VersionHistory,
    Listen,
    Export,
    OpenWindow,
    AlwaysOnTop,
    Delete,
}

struct MenuItem {
    action: MenuAction,
    icon: &'static str,
    label: &'static str,
    /// The native preview has no implementation yet; shown disabled so nothing pretends to work.
    disabled: bool,
    separator_before: bool,
}

impl NoteView {
    pub fn new(runtime: RuntimeHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| TextInput::new("Untitled", cx).inline().breadcrumb());
        let input = cx.subscribe_in(&title, window, |this, _, event, window, cx| {
            match event {
                InputEvent::Submitted => {
                    this.rename(cx);
                    window.blur(cx);
                }
                InputEvent::Rejected => {
                    this.message = "Title input is limited to 4096 bytes.".into()
                }
                InputEvent::Changed => {}
            }
            cx.notify();
        });
        let blur = cx.on_blur(&title.focus_handle(cx), window, |this, _, cx| {
            this.rename(cx);
        });
        let mut subscriptions = vec![
            input,
            blur,
            cx.observe_global::<MeetingServices>(|this, cx| this.load_audio(cx)),
        ];
        if let Some(live) = LiveState::global(cx) {
            subscriptions.push(cx.observe(&live, |this, _, cx| this.live_changed(cx)));
        }
        let templates_cancel = CancellationToken::new();
        resource_list::watch(
            &runtime,
            templates::LIBRARY_SQL,
            templates_cancel.clone(),
            cx,
            |this, rows, cx| {
                if let Ok(rows) = rows {
                    this.templates = Arc::new(templates::decode_library(&rows));
                    cx.notify();
                }
            },
        );
        Self {
            runtime,
            title,
            current: None,
            content: None,
            message: "Select a note from your local library.".into(),
            busy: false,
            loading: false,
            generation: Generation::default(),
            cancellation: CancellationToken::new(),
            rename_pending: false,
            rename_error: false,
            mode: SessionMode::Inactive,
            audio: None,
            player: None,
            menu: false,
            menu_index: None,
            menu_focus: cx.focus_handle(),
            return_focus: None,
            details: Details::default(),
            view: None,
            summary_editor: None,
            transcript: None,
            details_task: None,
            compact_folder: false,
            share: None,
            jobs: HashMap::new(),
            job_poll: None,
            model: None,
            picker: None,
            templates: Arc::default(),
            templates_cancel,
            memo_template: None,
            memo_saved: None,
            _subscriptions: subscriptions,
        }
    }

    /// Shipping's `SessionShareButton` toggles its popover under the button.
    fn toggle_share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.current.as_ref().map(|s| s.summary.id.clone()) else {
            return;
        };
        if self.share.take().is_some() {
            cx.notify();
            return;
        }
        let runtime = self.runtime.clone();
        let popover = cx.new(|cx| SharePopover::new(session.clone(), runtime, window, cx));
        let subscription = cx.subscribe(&popover, |this, _, event, cx| {
            this.share = None;
            if let SharePopoverEvent::Open(route) = event {
                cx.emit(WorkspaceEvent::Product(route.clone()));
            }
            cx.notify();
        });
        self.share = Some((session, popover, subscription));
        cx.notify();
    }

    pub fn has_unsaved_title(&self, cx: &gpui::App) -> bool {
        self.busy
            || self.current.as_ref().is_some_and(|session| {
                self.title.read(cx).buffer.text.as_str() != session.summary.title.as_ref()
            })
    }

    fn live_changed(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.current else {
            return;
        };
        let mode = LiveState::mode(cx, &session.summary.id);
        let ended = mode == SessionMode::Inactive && self.mode != SessionMode::Inactive;
        if mode != self.mode || mode != SessionMode::Inactive {
            self.mode = mode;
            cx.notify();
        }
        if ended {
            // A resumed recording appends to the same file, so the player must reopen it.
            self.player = None;
            self.load_audio(cx);
        }
    }

    /// Shipping's `useAudioExists`: looks for the session's saved recording.
    fn load_audio(&mut self, cx: &mut Context<Self>) {
        let (Some(session), Some(services)) = (&self.current, cx.try_global::<MeetingServices>())
        else {
            return;
        };
        let id = session.summary.id.clone();
        let Ok(reply) = services.capture.audio_path(id.clone()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let path = reply.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |this, cx| {
                if this.current.as_ref().map(|session| &session.summary.id) != Some(&id)
                    || this.audio == path
                {
                    return;
                }
                this.audio = path;
                this.player = None;
                if let Some(transcript) = &this.transcript {
                    let audio = this.audio.clone();
                    transcript.update(cx, |transcript, cx| transcript.set_audio(audio, cx));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_content(
        &mut self,
        session_id: &SessionId,
        content: AnyView,
        cx: &mut Context<Self>,
    ) {
        if self
            .current
            .as_ref()
            .is_some_and(|session| &session.summary.id == session_id)
        {
            self.memo_saved = content.clone().downcast::<EditorPane>().ok().map(|editor| {
                let session = session_id.clone();
                cx.subscribe(&editor, move |this, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Saved(_)) {
                        this.memo_saved(&session, cx);
                    }
                })
            });
            self.content = Some(content);
            if !self.rename_error {
                self.message.clear();
            }
            cx.notify();
        }
    }

    pub fn cancel_open(&mut self) {
        self.cancellation.cancel();
        self.generation.advance();
        self.loading = false;
    }

    pub fn remove_sessions(&mut self, ids: &[SessionId], cx: &mut Context<Self>) {
        if self
            .current
            .as_ref()
            .is_some_and(|session| ids.contains(&session.summary.id))
        {
            self.cancel_open();
            self.current = None;
            self.reset_details();
            self.content = None;
            self.message.clear();
            self.title
                .update(cx, |title, cx| title.set_text(String::new(), cx));
            cx.notify();
        }
    }

    fn reset_details(&mut self) {
        self.details = Details::default();
        self.details_task = None;
        self.view = None;
        self.summary_editor = None;
        self.transcript = None;
        self.audio = None;
        self.player = None;
        self.picker = None;
    }

    fn restore_title(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.loading {
            return;
        }
        if self.rename_error {
            self.reload_title(cx);
            return;
        }
        if let Some(session) = &self.current {
            self.title.update(cx, |title, cx| {
                title.set_text(session.summary.title.to_string(), cx)
            });
            self.message.clear();
            self.rename_error = false;
            cx.notify();
        }
    }

    fn reload_title(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.current else {
            return;
        };
        let draft = self.title.read(cx).buffer.text.clone();
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = self
            .runtime
            .open_session(session.summary.id.clone(), self.cancellation.clone());
        self.loading = true;
        self.message = "Loading saved title…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(session) => {
                        if this.title.read(cx).buffer.text == draft {
                            this.title.update(cx, |title, cx| {
                                title.set_text(session.summary.title.to_string(), cx)
                            });
                        }
                        let session = Arc::new(session);
                        this.current = Some(session.clone());
                        this.rename_error = false;
                        this.message.clear();
                        cx.emit(NoteEvent::TitleUpdated(session));
                    }
                    Err(error) => {
                        this.message = format!(
                            "Could not load the saved title: {error}. Your input is preserved."
                        );
                        cx.emit(NoteEvent::RenameFailed);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open(&mut self, id: SessionId, cx: &mut Context<Self>) {
        if self.has_unsaved_title(cx) {
            self.message = "Save or restore your title before changing notes.".into();
            cx.notify();
            return;
        }
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = self.runtime.open_session(id, self.cancellation.clone());
        self.loading = true;
        self.message.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(session) => {
                        if this.has_unsaved_title(cx) {
                            this.message =
                                "The title changed while opening a note. Save or restore it first."
                                    .into();
                            cx.emit(NoteEvent::Failed);
                            cx.notify();
                        } else {
                            this.show(session, cx);
                        }
                    }
                    Err(error) => {
                        this.message = error.to_string();
                        cx.emit(NoteEvent::Failed);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub fn show(&mut self, session: OpenSession, cx: &mut Context<Self>) {
        self.content = None;
        self.menu = false;
        self.rename_pending = false;
        self.rename_error = false;
        self.reset_details();
        self.details.folder = folder_label(&session.summary.folder_path);
        self.watch_details(session.summary.id.clone(), cx);
        let session = Arc::new(session);
        cx.emit(NoteEvent::Opened(session.clone()));
        // A note without a memo shows shipping's empty editor; its first edit creates it.
        cx.emit(WorkspaceEvent::OpenEditor {
            session_id: session.summary.id.clone(),
            document: session
                .note
                .clone()
                .unwrap_or_else(|| DocumentSnapshot::unsaved_note(session.summary.id.clone())),
        });
        self.title.update(cx, |title, cx| {
            title.set_text(session.summary.title.to_string(), cx)
        });
        self.message.clear();
        self.mode = LiveState::mode(cx, &session.summary.id);
        self.current = Some(session);
        self.load_audio(cx);
        self.load_model(cx);
        cx.notify();
    }

    fn load_model(&mut self, cx: &mut Context<Self>) {
        let Ok(reply) = summary::load_model(&self.runtime) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            if let Ok(model) = reply.receive().await {
                let _ = this.update(cx, |this, cx| {
                    this.model = Some(model);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn watch_details(&mut self, id: SessionId, cx: &mut Context<Self>) {
        self.details_task = None;
        let reply = self.runtime.watch_query(
            "SELECT s.folder_path, s.event_json,
               EXISTS (SELECT 1 FROM transcripts t WHERE t.session_id = s.id AND t.deleted_at IS NULL
                 AND CASE WHEN json_valid(t.words_json) THEN json_array_length(t.words_json) ELSE 0 END > 0
               ) AS has_transcript,
               (SELECT json_group_array(json_object(
                  'id', d.id, 'session_id', d.session_id, 'kind', d.kind, 'title', d.title,
                  'template_id', d.template_id, 'body_format', d.body_format, 'body', d.body,
                  'created_at', d.created_at, 'updated_at', d.updated_at,
                  'generation_metadata_json', d.generation_metadata_json))
                FROM (SELECT * FROM session_documents WHERE session_id = s.id
                  AND kind IN ('summary', 'template_output') AND deleted_at IS NULL
                  ORDER BY sort_order, id) d
               ) AS summaries
             FROM sessions s WHERE s.id = ? AND s.deleted_at IS NULL"
                .into(),
            vec![json!(id)],
        );
        self.details_task = Some(cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.message = format!("Note details unavailable: {error}");
                        cx.notify();
                    });
                    return;
                }
            };
            loop {
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                let terminal = error.is_some();
                if this
                    .update(cx, |this, cx| {
                        if !this
                            .current
                            .as_ref()
                            .is_some_and(|session| session.summary.id == id)
                        {
                            return;
                        }
                        if let Some(error) = error {
                            this.message = format!("Note details unavailable: {error}");
                        } else if let Some(row) = rows.first() {
                            this.apply_details(row);
                        }
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
        }));
    }

    fn apply_details(&mut self, row: &Value) {
        let summaries: Vec<Value> = row["summaries"]
            .as_str()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let visible = summary_ids(&summaries, &SessionId("".into()), false);
        let summary_meta = summaries
            .iter()
            .filter_map(|row| {
                let text = |key: &str| Arc::<str>::from(row[key].as_str().unwrap_or_default());
                Some((
                    row["id"].as_str()?.into(),
                    (text("title"), text("template_id")),
                ))
            })
            .collect();
        let event = row["event_json"]
            .as_str()
            .and_then(|event| serde_json::from_str::<Value>(event).ok())
            .unwrap_or_default();
        self.details = Details {
            summary_meta,
            event_text: format!(
                "{} {}",
                event["title"].as_str().unwrap_or_default(),
                event["description"].as_str().unwrap_or_default()
            ),
            participants: event["participants"].as_array().map_or(0, Vec::len),
            folder: folder_label(row["folder_path"].as_str().unwrap_or_default()),
            event_ended: row["event_json"]
                .as_str()
                .and_then(|event| serde_json::from_str::<Value>(event).ok())
                .and_then(|event| {
                    chrono::DateTime::parse_from_rfc3339(event["ended_at"].as_str()?).ok()
                })
                .is_some_and(|ended| ended <= chrono::Utc::now()),
            has_transcript: row["has_transcript"].as_i64().unwrap_or(0) != 0
                || row["has_transcript"].as_bool().unwrap_or(false),
            summaries: summaries
                .into_iter()
                .filter(|row| {
                    row["id"]
                        .as_str()
                        .is_some_and(|id| visible.iter().any(|visible| visible.as_ref() == id))
                })
                .filter_map(|row| serde_json::from_value(row).ok())
                .collect(),
        };
    }

    fn tabs(&self) -> Vec<NoteTab> {
        let Some(session) = &self.current else {
            return Vec::new();
        };
        let ids: Vec<Arc<str>> = self
            .details
            .summaries
            .iter()
            .map(|summary| summary.id.0.clone())
            .collect();
        let inactive = self.mode == SessionMode::Inactive;
        let ids = if ids.is_empty() && self.details.has_transcript && inactive {
            summary_ids(&[], &session.summary.id, true)
        } else {
            ids
        };
        // Shipping's `getCanShowTranscript`.
        let transcript = self.details.has_transcript || self.audio.is_some() || !inactive;
        note_tabs(&ids, transcript)
    }

    /// Shipping's `useHasEditableSummary`: the shown view is a summary with content.
    pub fn editable_summary_shown(&self) -> bool {
        let Some(session) = &self.current else {
            return false;
        };
        let tabs = self.tabs();
        let NoteTab::Summary(id) =
            current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs)
        else {
            return false;
        };
        self.details
            .summaries
            .iter()
            .find(|summary| summary.id.0 == id)
            .is_some_and(|summary| summary_has_content(&summary.body, &session.summary.title))
    }

    fn meeting_over(&self) -> bool {
        self.mode != SessionMode::Active
            && (self.details.event_ended || self.details.has_transcript || self.audio.is_some())
    }

    pub(super) fn rename(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            self.rename_pending = true;
            return;
        }
        if self.loading {
            return;
        }
        let Some(session) = &self.current else {
            return;
        };
        let title: Arc<str> = self.title.read(cx).buffer.text.as_str().into();
        if title == session.summary.title {
            return;
        }
        let reply = self.runtime.rename_session(RenameSession {
            base: session.summary.clone(),
            title: title.clone(),
        });
        self.busy = true;
        self.rename_error = false;
        self.message.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(session) => {
                        this.current = Some(Arc::new(session));
                        cx.emit(NoteEvent::TitleUpdated(
                            this.current.as_ref().unwrap().clone(),
                        ));
                        this.message.clear();
                        if std::mem::take(&mut this.rename_pending) {
                            this.rename(cx);
                        }
                    }
                    Err(error) => {
                        this.rename_pending = false;
                        this.rename_error = true;
                        this.message =
                            format!("Title not saved: {error}. Your input is preserved.");
                        cx.emit(NoteEvent::RenameFailed);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn menu_items(&self) -> Vec<MenuItem> {
        let item = |action, icon, label, disabled, separator_before| MenuItem {
            action,
            icon,
            label,
            disabled,
            separator_before,
        };
        let (listen_icon, listen_label) = if self.mode != SessionMode::Inactive {
            ("editor/MicOff01Icon.svg", "Stop listening")
        } else if self.details.has_transcript {
            ("editor/Mic01Icon.svg", "Resume listening")
        } else {
            ("editor/Mic01Icon.svg", "Start listening")
        };
        vec![
            item(
                MenuAction::MeetingInfo,
                "editor/Calendar04Icon.svg",
                "Meeting info",
                true,
                false,
            ),
            item(
                MenuAction::VersionHistory,
                "editor/HistoryIcon.svg",
                "Version history",
                true,
                false,
            ),
            item(
                MenuAction::Listen,
                listen_icon,
                listen_label,
                self.mode == SessionMode::Finalizing,
                true,
            ),
            item(
                MenuAction::Export,
                "editor/FileDownloadIcon.svg",
                "Export",
                false,
                true,
            ),
            item(
                MenuAction::OpenWindow,
                "editor/AppWindowIcon.svg",
                "Open in New Window",
                false,
                true,
            ),
            item(
                MenuAction::AlwaysOnTop,
                "editor/PinIcon.svg",
                "Always on Top",
                true,
                false,
            ),
            item(
                MenuAction::Delete,
                "editor/TrashIcon.svg",
                "Delete",
                false,
                true,
            ),
        ]
    }

    fn step_menu(&mut self, forward: bool) {
        let items = self.menu_items();
        let enabled: Vec<usize> = (0..items.len())
            .filter(|index| !items[*index].disabled)
            .collect();
        let position = self
            .menu_index
            .and_then(|index| enabled.iter().position(|candidate| *candidate == index));
        self.menu_index = match position {
            Some(position) if forward => enabled.get((position + 1) % enabled.len()).copied(),
            Some(position) => enabled
                .get((position + enabled.len() - 1) % enabled.len())
                .copied(),
            None if forward => enabled.first().copied(),
            None => enabled.last().copied(),
        };
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = false;
        if self.menu_focus.is_focused(window)
            && let Some(focus) = self.return_focus.take()
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn choose_menu(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        let Some(session) = &self.current else {
            return;
        };
        let id = session.summary.id.clone();
        match action {
            MenuAction::Listen => self.toggle_recording(cx),
            MenuAction::Export => cx.emit(WorkspaceEvent::Product(ProductRoute::Export(id))),
            MenuAction::OpenWindow => cx.emit(NoteEvent::Request(OpenNote::Window(id))),
            MenuAction::Delete => cx.emit(NoteEvent::Request(OpenNote::Delete(vec![id].into()))),
            MenuAction::MeetingInfo | MenuAction::VersionHistory | MenuAction::AlwaysOnTop => {}
        }
    }

    fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.current {
            cx.emit(WorkspaceEvent::Meeting(
                if self.mode != SessionMode::Inactive {
                    MeetingIntent::Stop
                } else {
                    MeetingIntent::Start {
                        session_id: session.summary.id.clone(),
                    }
                },
            ));
        }
    }

    fn select_tab(&mut self, tab: NoteTab, cx: &mut Context<Self>) {
        self.view = Some(tab);
        cx.notify();
    }

    fn summary_view(
        &mut self,
        id: &Arc<str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let session = self.current.clone()?;
        let colors = theme(window);
        let document = self
            .details
            .summaries
            .iter()
            .find(|summary| &summary.id.0 == id)
            .cloned();
        let has_content = document.as_ref().is_some_and(|document| {
            if document.body_format.as_ref() == "markdown" {
                !document.body.trim().is_empty()
            } else {
                first_line_title(&document.body).is_some()
            }
        });
        let title = session.summary.title.to_string();
        if let Some(state) = self.summary_state(
            document.as_ref().map(|document| &document.id),
            has_content,
            &title,
            colors,
            cx,
        ) {
            return Some(state);
        }
        let document = document?;
        let editor = match &self.summary_editor {
            Some((current, editor, _)) if current == &document.id => editor.clone(),
            _ => {
                let return_focus = self.title.focus_handle(cx);
                let runtime = self.runtime.clone();
                let init = EditorInit {
                    session_id: session.summary.id.clone(),
                    document: document.clone(),
                    return_focus,
                };
                // Shipping's summary editor leads with the note title and shows no other
                // placeholder.
                let editor = cx.new(|cx| {
                    let mut editor =
                        EditorPane::titled(LaneContext { runtime }, init, title.into(), window, cx);
                    editor.set_placeholder(None);
                    editor
                });
                let saved = cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| {
                    if let EditorEvent::Saved(snapshot) = event {
                        this.rename_from_summary(&snapshot.body, cx);
                    }
                });
                self.summary_editor = Some((document.id.clone(), editor.clone(), saved));
                editor
            }
        };
        Some(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(editor)
                .into_any_element(),
        )
    }

    /// Shipping renames the note from the summary's first line on every save.
    fn rename_from_summary(&mut self, body: &str, cx: &mut Context<Self>) {
        let Some(title) = first_line_title(body) else {
            return;
        };
        if self
            .current
            .as_ref()
            .is_some_and(|session| session.summary.title.as_ref() != title)
        {
            self.title.update(cx, |input, cx| input.set_text(title, cx));
            self.rename(cx);
        }
    }

    fn folder_chip(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let folder = self.details.folder.clone();
        let compact = self.compact_folder;
        div()
            .id("note-folder")
            .h(px(28.))
            .flex()
            .flex_shrink_0()
            .items_center()
            .rounded(px(RADIUS))
            .text_color(colors.muted_foreground)
            .hover(|view| view.bg(colors.accent).text_color(colors.foreground))
            .cursor_pointer()
            .map(|view| {
                if folder.is_empty() || compact {
                    view.w(px(28.)).justify_center()
                } else {
                    view.max_w(px(144.)).min_w_0().gap_1().px(px(6.))
                }
            })
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(session) = &this.current {
                    cx.emit(NoteEvent::Request(OpenNote::Move(
                        vec![session.summary.id.clone()].into(),
                    )));
                }
            }))
            .group("note-folder")
            .child(
                icon("workspace/Folder01Icon.svg", 16., colors.muted_foreground)
                    .group_hover("note-folder", |style| style.text_color(colors.foreground)),
            )
            .when(!folder.is_empty() && !compact, |view| {
                view.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(neutral(colors, 0x525252, 0xd4d4d4))
                        .child(folder),
                )
            })
    }

    fn view_switcher(
        &self,
        tabs: &[NoteTab],
        current: &NoteTab,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let amplitude = self
            .current
            .as_ref()
            .zip(LiveState::global(cx))
            .and_then(|(session, live)| {
                live.read(cx)
                    .session(&session.summary.id)
                    .map(|live| live.amplitude)
            })
            .unwrap_or_default();
        let mode = self.mode;
        div()
            .flex()
            .flex_shrink_0()
            .h(px(28.))
            .p(px(2.))
            .gap(px(2.))
            .items_center()
            .rounded_full()
            .bg(colors.foreground.opacity(0.1))
            .children(tabs.iter().enumerate().map(|(index, tab)| {
                let active = tab == current;
                let summary = match tab {
                    NoteTab::Summary(id) => {
                        let (raw, template_id) = self
                            .details
                            .summary_meta
                            .get(id)
                            .cloned()
                            .unwrap_or_default();
                        let template = self
                            .templates
                            .templates
                            .iter()
                            .find(|template| template.id == template_id && !template_id.is_empty())
                            .cloned();
                        let job = self
                            .current
                            .as_ref()
                            .and_then(|session| self.jobs.get(&session.summary.id))
                            .filter(|job| job.shows(id));
                        Some((
                            summary::tab_title(&raw, template.as_ref(), &template_id),
                            template,
                            (!template_id.is_empty()).then_some(template_id),
                            job.is_some_and(|job| job.running()),
                            job.is_some_and(|job| !job.running()),
                        ))
                    }
                    _ => None,
                };
                let (path, size, label): (&str, f32, SharedString) = match tab {
                    NoteTab::Summary(_) => (
                        "editor/SparkleIcon.svg",
                        16.,
                        summary
                            .as_ref()
                            .map(|summary| summary.0.clone())
                            .unwrap_or_default(),
                    ),
                    NoteTab::Memos => ("editor/TextAlignLeftIcon.svg", 14., "Memos".into()),
                    NoteTab::Transcript => {
                        ("editor/TranscriptAudioIcon.svg", 16., "Transcript".into())
                    }
                };
                let target = tab.clone();
                let transcript = matches!(tab, NoteTab::Transcript);
                let recording = transcript && mode == SessionMode::Active;
                let finalizing = transcript && mode == SessionMode::Finalizing;
                let group = SharedString::from(format!("note-tab-{index}"));
                let generating = summary.as_ref().is_some_and(|summary| summary.3);
                let failed = summary.as_ref().is_some_and(|summary| summary.4);
                let summary_tab = summary.is_some();
                // Shipping's `canEdit`: an inactive session with a transcript.
                let editable_transcript = matches!(tab, NoteTab::Transcript)
                    && mode == SessionMode::Inactive
                    && self.details.has_transcript;
                let transcript_editing = editable_transcript
                    && self
                        .transcript
                        .as_ref()
                        .is_some_and(|transcript| transcript.read(cx).edit_mode());
                // Shipping marks the template a summary used (Auto when it has none).
                let used = summary
                    .as_ref()
                    .map(|summary| summary.2.clone())
                    .unwrap_or_default();
                let tab_icon = if let Some((_, template, _, _, _)) = &summary {
                    self.summary_glyph(
                        template.as_ref(),
                        generating,
                        if active {
                            colors.foreground
                        } else {
                            colors.muted_foreground.opacity(0.7)
                        },
                    )
                } else if recording {
                    dancing_sticks(
                        "note-tab-live",
                        amplitude,
                        rgb(RECORD_RED).into(),
                        (16., 16.),
                        (2., 1.),
                    )
                    .into_any_element()
                } else if finalizing {
                    spinner(
                        "note-tab-finalizing",
                        16.,
                        if active {
                            colors.foreground
                        } else {
                            colors.muted_foreground.opacity(0.7)
                        },
                    )
                    .into_any_element()
                } else if active {
                    icon(path, size, colors.foreground).into_any_element()
                } else {
                    icon(path, size, colors.muted_foreground.opacity(0.7))
                        .group_hover(group.clone(), |style| style.text_color(colors.foreground))
                        .into_any_element()
                };
                div()
                    .id(("note-tab", index))
                    .h(px(24.))
                    .px_2()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .when_else(
                        active,
                        |view| {
                            let summary_tab = matches!(tab, NoteTab::Summary(_));
                            view.min_w(px(if summary_tab { 62. } else { 40. }))
                                .max_w(px(if summary_tab { 224. } else { 160. }))
                                .gap(px(6.))
                                .bg(neutral(colors, 0xffffff, 0))
                                .when(is_dark(colors), |view| view.bg(colors.accent))
                                .shadow_xs()
                                .text_color(colors.foreground)
                        },
                        |view| {
                            view.text_color(colors.muted_foreground.opacity(0.7))
                                .hover(|view| {
                                    view.bg(colors.background.opacity(0.6))
                                        .text_color(colors.foreground)
                                })
                        },
                    )
                    // A live transcript tab is red and wide enough for its dancing sticks.
                    .when(active && recording, |view| {
                        let dark = is_dark(colors);
                        view.w(px(98.))
                            .min_w(px(98.))
                            .bg(if dark {
                                Hsla::from(rgb(0x450a0a)).opacity(0.5)
                            } else {
                                rgb(0xfef2f2).into()
                            })
                            .text_color(neutral(colors, RECORD_RED, 0xfca5a5))
                            .hover(move |view| {
                                view.bg(neutral_rgb(dark, 0xfee2e2, 0x450a0a))
                                    .text_color(neutral_rgb(dark, 0xdc2626, 0xfecaca))
                            })
                    })
                    .when(active && generating, |view| {
                        view.opacity(0.7).cursor_not_allowed()
                    })
                    .when(active && failed, |view| {
                        let red = self.error_color(colors);
                        view.text_color(red)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        // The active summary tab opens shipping's template picker.
                        if active && summary_tab {
                            if !generating && this.picker.is_none() {
                                this.open_picker(window, cx);
                            }
                        } else if active && editable_transcript {
                            if let Some(transcript) = &this.transcript {
                                transcript.update(cx, |transcript, cx| {
                                    let editing = transcript.edit_mode();
                                    transcript.set_edit_mode(!editing, cx)
                                });
                            }
                            cx.notify();
                        } else {
                            this.select_tab(target.clone(), cx)
                        }
                    }))
                    .group(group)
                    .child(tab_icon)
                    .when(active, |view| {
                        view.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .line_height(px(16.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(label),
                        )
                    })
                    // Shipping's transcript edit toggle: a pencil, a check while editing.
                    .when(active && editable_transcript, |view| {
                        view.child(icon(
                            if transcript_editing {
                                "editor/CheckmarkCircle02Icon.svg"
                            } else {
                                "editor/PencilEdit01Icon.svg"
                            },
                            14.,
                            colors.foreground,
                        ))
                    })
                    .when(active && summary_tab, |view| {
                        let color = if failed {
                            self.error_color(colors)
                        } else {
                            colors.foreground
                        };
                        view.relative()
                            .child(icon("editor/ArrowDown01Icon.svg", 14., color))
                            .when_some(
                                self.template_picker(used.clone(), generating, colors, cx),
                                |view, picker| {
                                    view.child(
                                        div().absolute().top(px(28.)).left_0().child(
                                            gpui::deferred(
                                                gpui::anchored().snap_to_window().child(picker),
                                            )
                                            .with_priority(1),
                                        ),
                                    )
                                },
                            )
                    })
            }))
    }

    /// Shipping's session top audio player, shown above the transcript once audio is saved.
    fn top_player(&mut self, colors: Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.mode != SessionMode::Inactive {
            return None;
        }
        let path = self.audio.clone()?;
        let playback = cx.try_global::<MeetingServices>()?.playback.clone();
        let player = self
            .player
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let mut player = PlayerView::new(playback, cx);
                    player.open(path, cx);
                    player
                })
            })
            .clone();
        Some(
            div().flex_shrink_0().px_1().pt_1().pb_2().child(
                div()
                    .overflow_hidden()
                    .rounded(px(22.))
                    .border_1()
                    .border_color(colors.border.opacity(0.7))
                    .bg(colors.card.opacity(0.8))
                    .child(player),
            ),
        )
    }

    /// Shipping's `HeaderMeetingControl`: Record, Stop while live, Share once the meeting is
    /// over, and nothing while the transcript finalizes.
    fn meeting_control(&self, colors: Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        (self.mode != SessionMode::Finalizing).then(|| self.meeting_button(colors, cx))
    }

    fn meeting_button(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let share = self.meeting_over();
        let recording = self.mode == SessionMode::Active;
        div()
            .id("record-note")
            .mx_1()
            .h(px(28.))
            .pl(px(6.))
            .pr(px(10.))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .rounded(px(RADIUS))
            .border_1()
            .border_color(colors.border)
            .when(recording, |view| view.bg(colors.card))
            .hover(|view| view.bg(colors.accent))
            .text_sm()
            .line_height(px(20.))
            .font_weight(crate::ui::theme::MEDIUM)
            .text_color(colors.foreground)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                if share {
                    this.toggle_share(window, cx);
                } else {
                    this.toggle_recording(cx);
                }
            }))
            .map(|view| {
                if share {
                    let popover = self
                        .share
                        .as_ref()
                        .filter(|(session, ..)| {
                            self.current.as_ref().map(|s| &s.summary.id) == Some(session)
                        })
                        .map(|(_, popover, _)| popover.clone());
                    view.relative()
                        .child(icon("editor/Share08Icon.svg", 14., colors.foreground))
                        .child("Share")
                        .when_some(popover, |view, popover| {
                            view.child(sharing::anchor(popover))
                        })
                } else if recording {
                    view.child(icon("editor/SquareIcon.svg", 12., rgb(RECORD_RED).into()))
                        .child("Stop")
                } else {
                    view.child(record_mark()).child("Record")
                }
            })
    }

    fn header(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let tabs = self.tabs();
        let current = current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs);
        let switcher = tabs.len() > 1;
        let breadcrumb = !switcher && self.mode == SessionMode::Inactive && !self.meeting_over();
        // Shipping's breadcrumb title is `w-fit` over the title and its placeholder, capped at
        // `max-w-56`; the input itself draws 14px text.
        let title_width = {
            let font = window.text_style().font();
            let measure = |text: &str| {
                let run = gpui::TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: colors.foreground,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window
                    .text_system()
                    .shape_line(text.to_owned().into(), px(14.), &[run], None)
                    .width
            };
            let title = self.title.read(cx).buffer.text.clone();
            (measure(&title).max(measure("Untitled")) + px(2.)).min(px(224.))
        };
        div()
            .flex()
            .h(px(48.))
            .flex_shrink_0()
            .pb(px(2.))
            .pl(px(12.))
            .pr(px(8.))
            .items_center()
            .gap(px(2.))
            .on_mouse_down(MouseButton::Left, |_, window, cx| window.blur(cx))
            .when(switcher, |view| {
                view.child(self.view_switcher(&tabs, &current, colors, cx))
            })
            .when(breadcrumb, |view| {
                view.child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap_1()
                        .child(self.folder_chip(colors, cx))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(colors.muted_foreground)
                                .child("/"),
                        )
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .w(title_width)
                                .child(self.title.clone()),
                        ),
                )
            })
            .child(div().flex_1().min_w_0())
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .when(!breadcrumb, |view| view.child(self.folder_chip(colors, cx)))
                    .children(self.meeting_control(colors, cx))
                    .child(
                        div()
                            .id("note-more")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(RADIUS))
                            .text_color(colors.muted_foreground)
                            .hover(|view| view.bg(colors.accent).text_color(colors.foreground))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.menu {
                                    this.close_menu(window, cx);
                                } else {
                                    this.return_focus = window.focused(cx);
                                    this.menu = true;
                                    this.menu_index = None;
                                    this.menu_focus.focus(window, cx);
                                    cx.notify();
                                }
                            }))
                            .group("note-more")
                            .child(
                                icon(
                                    "editor/MoreHorizontalIcon.svg",
                                    16.,
                                    colors.muted_foreground,
                                )
                                .group_hover("note-more", |style| {
                                    style.text_color(colors.foreground)
                                }),
                            ),
                    ),
            )
    }

    fn menu(&self, colors: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.menu_items();
        let red = neutral(colors, 0xdc2626, 0xf87171);
        div()
            .id("note-actions")
            .absolute()
            .top(px(39.))
            .right(px(8.))
            .w(px(224.))
            .occlude()
            .p(px(2.))
            .rounded(px(22.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .shadow_lg()
            .track_focus(&self.menu_focus)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_menu(window, cx)))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.close_menu(window, cx),
                    "up" => this.step_menu(false),
                    "down" => this.step_menu(true),
                    "enter" => {
                        let items = this.menu_items();
                        if let Some(item) = this.menu_index.and_then(|index| items.get(index)) {
                            this.choose_menu(item.action, window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .p(px(6.))
                    .rounded(px(20.))
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .text_color(colors.popover_foreground)
                    .children(items.into_iter().enumerate().flat_map(|(index, item)| {
                        let separator = item.separator_before.then(|| {
                            div()
                                .mx(px(-4.))
                                .my_1()
                                .h(px(1.))
                                .bg(colors.muted)
                                .into_any_element()
                        });
                        let destructive = item.action == MenuAction::Delete;
                        let highlighted = self.menu_index == Some(index);
                        let row = div()
                            .id(("note-action", index))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py(px(6.))
                            .rounded(px(14.))
                            .text_sm()
                            .line_height(px(20.))
                            .when(destructive, |view| view.text_color(red))
                            .when_else(
                                item.disabled,
                                |view| view.opacity(0.5),
                                |view| {
                                    view.cursor_pointer()
                                        .when(highlighted, |view| {
                                            view.bg(if destructive {
                                                neutral(colors, 0xfef2f2, 0x450a0a)
                                            } else {
                                                colors.accent
                                            })
                                        })
                                        .hover(|view| {
                                            view.bg(if destructive {
                                                neutral(colors, 0xfef2f2, 0x450a0a)
                                            } else {
                                                colors.accent
                                            })
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.choose_menu(item.action, window, cx)
                                        }))
                                },
                            )
                            .child(icon(
                                item.icon,
                                16.,
                                if destructive {
                                    red
                                } else {
                                    colors.popover_foreground
                                },
                            ))
                            .child(div().flex_1().child(item.label))
                            .when(item.action == MenuAction::MeetingInfo, |view| {
                                view.child(icon(
                                    "editor/ArrowRight01Icon.svg",
                                    16.,
                                    colors.popover_foreground,
                                ))
                            })
                            .into_any_element();
                        separator.into_iter().chain([row])
                    })),
            )
    }
}

/// Shipping's `RecordingIcon`: a red squircle dot over a halo running Tailwind's
/// `animate-ping` (scale to 2x and fade out over the first 75% of each second).
fn record_mark() -> impl IntoElement {
    let red = gpui::Hsla::from(rgb(RECORD_RED));
    div()
        .size(px(12.))
        .relative()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .child(div().absolute().bg(red.opacity(0.4)).with_animation(
            "record-ping",
            Animation::new(Duration::from_secs(1)).repeat(),
            |halo, delta| {
                let progress = 1. - (1. - (delta / 0.75).min(1.)).powi(3);
                let size = 10. * (1. + progress);
                halo.size(px(size))
                    .top(px(6. - size / 2.))
                    .left(px(6. - size / 2.))
                    .rounded(px(size * 0.3))
                    .opacity(1. - progress)
            },
        ))
        .child(div().size(px(8.)).rounded(px(2.4)).bg(red))
}

/// GPUI svgs do not inherit text color, so every icon names its own.
fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

fn is_dark(colors: Theme) -> bool {
    colors.background.l < 0.5
}

/// A fixed Tailwind neutral that shipping uses instead of a theme token.
fn neutral(colors: Theme, light: u32, dark: u32) -> Hsla {
    neutral_rgb(is_dark(colors), light, dark)
}

fn neutral_rgb(dark: bool, light: u32, dark_value: u32) -> Hsla {
    rgb(if dark { dark_value } else { light }).into()
}

/// Shipping's `SessionContentLoading` bars, without the pulse.
fn loading_skeleton(colors: Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .px_4()
        .py_5()
        .child(
            div()
                .h_5()
                .w(gpui::relative(0.6))
                .rounded_md()
                .bg(colors.muted),
        )
        .child(
            div()
                .h_4()
                .w(gpui::relative(0.8))
                .rounded_md()
                .bg(colors.muted.opacity(0.8)),
        )
        .child(
            div()
                .h_4()
                .w(gpui::relative(2. / 3.))
                .rounded_md()
                .bg(colors.muted.opacity(0.7)),
        )
}

impl Drop for NoteView {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.templates_cancel.cancel();
    }
}

impl Render for NoteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let compact_folder = self.compact_folder;
        let entity = cx.entity().downgrade();
        let ready = self.current.is_some() && !self.loading;
        let tabs = self.tabs();
        let current = current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs);
        let body = match &current {
            NoteTab::Summary(id) if ready => self.summary_view(&id.clone(), window, cx),
            NoteTab::Transcript if ready => self.current.clone().map(|session| {
                let runtime = self.runtime.clone();
                let audio = self.audio.clone();
                let transcript = self
                    .transcript
                    .get_or_insert_with(|| {
                        cx.new(|cx| {
                            let mut view =
                                TranscriptView::new(runtime, session.summary.id.clone(), cx);
                            view.set_audio(audio, cx);
                            view
                        })
                    })
                    .clone();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .when_some(self.top_player(colors, cx), |view, player| {
                        view.child(player)
                    })
                    .child(div().flex_1().min_h_0().child(transcript))
                    .into_any_element()
            }),
            _ => self.content.clone().map(|content| {
                div()
                    .relative()
                    .size_full()
                    .child(content)
                    .children(self.memo_templates_overlay(colors, cx))
                    .into_any_element()
            }),
        };
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && this.title.focus_handle(cx).is_focused(window)
                    && this.title.read(cx).buffer.marked.is_none()
                    && this.has_unsaved_title(cx)
                {
                    this.restore_title(cx);
                    cx.stop_propagation();
                }
            }))
            .when(ready, |view| view.child(self.header(window, cx)))
            .when(self.loading, |view| view.child(loading_skeleton(colors)))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let compact = bounds.size.width < px(480.);
                        if compact != compact_folder {
                            window.defer(cx, move |_, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    if this.compact_folder != compact {
                                        this.compact_folder = compact;
                                        cx.notify();
                                    }
                                });
                            });
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child(self.message.clone())
                        .when(self.rename_error, |view| {
                            view.child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("retry-title")
                                            .cursor_pointer()
                                            .child("Retry")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.rename(cx)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("restore-title")
                                            .cursor_pointer()
                                            .child("Restore")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.restore_title(cx)
                                                }),
                                            ),
                                    ),
                            )
                        }),
                )
            })
            .when_some(body, |view, content| {
                view.child(div().flex().flex_col().flex_1().min_h_0().child(content))
            })
            .when(self.menu, |view| {
                view.child(gpui::deferred(self.menu(colors, cx)))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_tabs_follow_shipping_switcher_rules() {
        let summary: Arc<str> = "summary-1".into();
        let tabs = note_tabs(std::slice::from_ref(&summary), true);
        assert_eq!(
            tabs,
            [
                NoteTab::Summary(summary.clone()),
                NoteTab::Memos,
                NoteTab::Transcript
            ]
        );
        assert_eq!(
            current_tab(None, false, &tabs),
            NoteTab::Summary(summary.clone())
        );
        assert_eq!(current_tab(None, true, &tabs), NoteTab::Memos);
        let gone = NoteTab::Summary("removed".into());
        assert_eq!(
            current_tab(Some(&gone), false, &tabs),
            NoteTab::Summary(summary)
        );
        assert_eq!(
            current_tab(Some(&NoteTab::Transcript), false, &note_tabs(&[], false)),
            NoteTab::Memos
        );
    }

    #[test]
    fn untouched_summary_placeholders_hide_behind_real_summaries() {
        let placeholder = json!({"id":"p","kind":"summary","template_id":"","body":"","title":"Summary",
            "created_at":"t","updated_at":"t","generation_metadata_json":"{}"});
        let real = json!({"id":"r","kind":"summary","template_id":"","body":"{}","title":"",
            "created_at":"t","updated_at":"u","generation_metadata_json":"{}"});
        let session = SessionId("s".into());
        assert_eq!(
            summary_ids(&[placeholder.clone(), real], &session, false),
            [Arc::<str>::from("r")]
        );
        assert_eq!(
            summary_ids(&[placeholder], &session, false),
            [Arc::<str>::from("p")]
        );
        assert_eq!(
            summary_ids(&[], &session, true),
            [Arc::<str>::from("summary:s")]
        );
    }
}
