mod brief;
mod export_modal;
mod meeting_info;
pub(super) mod meeting_records;
mod memo;
mod summary;
mod versions;

pub use meeting_info::Clock;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use desktop_runtime::{
    CancellationToken, DocumentId, DocumentSnapshot, Generation, OpenSession, RenameSession,
    RuntimeHandle, SessionId,
};
use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Hsla, MouseButton, MouseDownEvent, Pixels, Point, Render, SharedString,
    Subscription, Task, Window, canvas, div, prelude::*, px, rgb, svg,
};
use serde_json::{Value, json};

use super::{
    folder_picker::{FolderPicker, FolderPickerEvent},
    library::{OpenNote, folder_label},
    navigation::Route,
    resource_list::{self, Glyph},
    templates::{self, Library, Template},
};
use crate::{
    application::toasts,
    contracts::{EditorEvent, EditorInit, LaneContext, MeetingIntent, WorkspaceEvent},
    editor::{
        EditorPane,
        find::{FindBar, FindTarget},
        title::first_line_title,
        transcript::{TranscriptEvent, TranscriptView},
    },
    meeting::{
        MeetingServices,
        live::{LiveState, SessionMode},
        player_view::PlayerView,
        upload::{self, Job, UploadEvent, Uploads},
        widgets::{dancing_sticks, spinner},
    },
    product::sharing::{self, SharePopover, SharePopoverEvent},
    ui::{
        input::{InputEvent, TextInput},
        theme::{RADIUS, Theme, theme},
    },
};
use export_modal::{ExportEvent, ExportModal};
use meeting_info::{MeetingInfo, MeetingInfoEvent};

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
    /// The open session's upload or batch transcription (shipping's `running_batch`).
    batch: Option<Job>,
    /// Shipping's `useCurrentNoteHasContent`, read when the note menu opens.
    menu_note_content: bool,
    /// The transcript view's Resume listening, for as long as that view lives.
    transcript_events: Option<Subscription>,
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
    folder_picker: Entity<FolderPicker>,
    folder_open: bool,
    folder_chip_hovered: bool,
    clock: Clock,
    floating_bar_enabled: bool,
    /// The window's Always on Top state, read when the menu opens; `None` when unknown.
    always_on_top: Option<bool>,
    /// The open "Meeting info" submenu.
    meeting_info: Option<(Entity<MeetingInfo>, Subscription)>,
    export: Option<(Entity<ExportModal>, Subscription)>,
    /// A note view tab's right-click menu, at the pointer.
    tab_menu: Option<(Point<Pixels>, TabMenu)>,
    pending_edits: Entity<super::edit_review::PendingEdits>,
    versions: versions::Versions,
    /// Shown in its own window, which has no "Open in New Window" and clears the traffic lights.
    standalone: bool,
    /// Shipping's ⌘F / ⌘H find & replace bar over the shown tab.
    find: Entity<FindBar>,
    /// The open note's pre-meeting brief sources, and a brief being written.
    brief: Option<Arc<brief::Brief>>,
    brief_job: Option<brief::BriefJob>,
    _subscriptions: Vec<Subscription>,
}

/// The right-click menus of the active Memos and Summary tabs.
#[derive(Clone, Debug, PartialEq)]
enum TabMenu {
    Memo,
    Summary(Arc<str>),
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
    /// A route opened in a new tab, like a participant's contact or "See all folders".
    Open(Route),
    /// "Open floating panel" brings the live meeting's floating bar forward.
    OpenFloatingPanel,
    /// The pending-edits banner's "Review memo" / "Review summary".
    OpenReview(Arc<str>),
}

impl EventEmitter<NoteEvent> for NoteView {}

/// What the open note's header needs beyond the session row, kept live by one watch.
#[derive(Default)]
struct Details {
    folder: String,
    folder_path: Arc<str>,
    folder_icon: Option<Glyph>,
    /// The memo document's stored body and format, for its tab's Copy.
    memo: (String, String),
    event_ended: bool,
    has_transcript: bool,
    summaries: Vec<DocumentSnapshot>,
    /// Each visible summary's stored title and template, for its tab.
    summary_meta: HashMap<Arc<str>, (Arc<str>, Arc<str>)>,
    /// The calendar event's title and description, and its participant count, which pick
    /// the memo's suggested templates.
    event_text: String,
    participants: usize,
    lock_row: Option<(&'static str, &'static str, bool)>,
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
    Retranscribe,
    UploadAudio,
    UploadTranscript,
    ShowAudio,
    Export,
    OpenWindow,
    FloatingPanel,
    AlwaysOnTop,
    Delete,
    Lock,
}

struct MenuItem {
    action: MenuAction,
    icon: &'static str,
    label: &'static str,
    disabled: bool,
    /// Shows a trailing check (Always on Top).
    checked: bool,
    separator_before: bool,
}

impl MenuItem {
    fn new(action: MenuAction, icon: &'static str, label: &'static str) -> Self {
        Self {
            action,
            icon,
            label,
            disabled: false,
            checked: false,
            separator_before: false,
        }
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Shipping's overflow menu groups, in order; an empty group adds no separator.
fn menu_rows(groups: Vec<Vec<MenuItem>>) -> Vec<MenuItem> {
    let mut rows = Vec::new();
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
        let separator = !rows.is_empty();
        for (index, mut item) in group.into_iter().enumerate() {
            item.separator_before = separator && index == 0;
            rows.push(item);
        }
    }
    rows
}

/// Shipping's `getStoredNoteMarkdown`: stored Markdown as is, TipTap JSON converted.
fn stored_markdown(body: &str, format: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if format == "markdown" || !trimmed.starts_with('{') {
        return trimmed.to_owned();
    }
    serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|doc| anlg_tiptap::tiptap_json_to_md(&doc).ok())
        .map(|markdown| markdown.trim().to_owned())
        .unwrap_or_default()
}

/// The view `forward` (or back) from `current`, wrapping like shipping's ⌘⌥←/→.
fn adjacent_tab(tabs: &[NoteTab], current: &NoteTab, forward: bool) -> Option<NoteTab> {
    let index = tabs.iter().position(|tab| tab == current)?;
    let next = if forward {
        (index + 1) % tabs.len()
    } else {
        (index + tabs.len() - 1) % tabs.len()
    };
    tabs.get(next).cloned()
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
        let uploads = Uploads::global(cx);
        let mut subscriptions = vec![
            input,
            blur,
            cx.observe_global::<MeetingServices>(|this, cx| this.load_audio(cx)),
            cx.observe(&uploads, |this, _, cx| this.sync_batch(cx)),
            cx.subscribe(&uploads, |this, _, event, cx| {
                let UploadEvent::AudioChanged(session) = event;
                if this.current.as_ref().map(|current| &current.summary.id) == Some(session) {
                    this.load_audio(cx);
                }
            }),
        ];
        if let Some(live) = LiveState::global(cx) {
            subscriptions.push(cx.observe(&live, |this, _, cx| this.live_changed(cx)));
        }
        let folder_picker = cx.new(|cx| FolderPicker::new(runtime.clone(), cx));
        subscriptions.push(cx.subscribe_in(
            &folder_picker,
            window,
            |this, _, event, window, cx| this.folder_picked(event, window, cx),
        ));
        let pending_edits = cx.new(|cx| super::edit_review::PendingEdits::new(runtime.clone(), cx));
        subscriptions.push(cx.subscribe(
            &pending_edits,
            |_, _, event: &super::edit_review::OpenReview, cx| {
                cx.emit(NoteEvent::OpenReview(event.0.clone()))
            },
        ));
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
            batch: None,
            menu_note_content: false,
            transcript_events: None,
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
            folder_picker,
            folder_open: false,
            folder_chip_hovered: false,
            clock: Clock::default(),
            floating_bar_enabled: true,
            always_on_top: None,
            meeting_info: None,
            export: None,
            tab_menu: None,
            pending_edits,
            versions: Default::default(),
            standalone: false,
            find: cx.new(FindBar::new),
            brief: None,
            brief_job: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_standalone(&mut self) {
        self.standalone = true;
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

    fn sync_batch(&mut self, cx: &mut Context<Self>) {
        let batch = self.current.as_ref().and_then(|session| {
            Uploads::global(cx)
                .read(cx)
                .job(&session.summary.id)
                .cloned()
        });
        if batch != self.batch {
            self.batch = batch;
            cx.notify();
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
            if let Ok(editor) = content.clone().downcast::<EditorPane>() {
                self.adopt_brief_editor(&editor, cx);
            }
            self.memo_saved = content.clone().downcast::<EditorPane>().ok().map(|editor| {
                let session = session_id.clone();
                cx.subscribe(&editor, move |this, _, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Saved(_)) {
                        this.memo_saved(&session, cx);
                    }
                })
            });
            self.observe_memo(&content, cx);
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
        self.folder_open = false;
        self.meeting_info = None;
        self.export = None;
        self.tab_menu = None;
        self.details = Details::default();
        self.details_task = None;
        self.view = None;
        self.summary_editor = None;
        self.transcript = None;
        self.audio = None;
        self.player = None;
        self.picker = None;
        self.versions = Default::default();
        self.brief = None;
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
        self.find.update(cx, |find, cx| find.close(cx));
        self.content = None;
        self.menu = false;
        self.rename_pending = false;
        self.rename_error = false;
        self.reset_details();
        self.details.folder = folder_label(&session.summary.folder_path);
        self.details.folder_path = session.summary.folder_path.clone();
        self.watch_details(session.summary.id.clone(), cx);
        self.watch_conflicts(session.summary.id.clone(), cx);
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
        let id = session.summary.id.0.clone();
        self.pending_edits
            .update(cx, |banner, cx| banner.set_session(Some(id), cx));
        self.current = Some(session);
        self.sync_batch(cx);
        self.load_audio(cx);
        self.load_model(cx);
        self.load_brief(cx);
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
               (SELECT icon_json FROM folders f WHERE f.path = s.folder_path AND f.deleted_at IS NULL
                 LIMIT 1) AS folder_icon,
               (SELECT json_object('body', m.body, 'body_format', m.body_format)
                FROM session_documents m WHERE m.session_id = s.id AND m.kind = 'note'
                  AND m.deleted_at IS NULL
                ORDER BY m.id = m.session_id DESC, m.created_at, m.id LIMIT 1
               ) AS memo,
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
        let memo = row["memo"]
            .as_str()
            .and_then(|memo| serde_json::from_str::<Value>(memo).ok())
            .unwrap_or_default();
        let folder_path: Arc<str> = row["folder_path"].as_str().unwrap_or_default().into();
        self.details = Details {
            folder_icon: (!folder_path.is_empty()).then(|| {
                row["folder_icon"]
                    .as_str()
                    .map_or_else(Glyph::folder_default, Glyph::folder)
            }),
            folder_path,
            memo: (
                memo["body"].as_str().unwrap_or_default().to_owned(),
                memo["body_format"].as_str().unwrap_or_default().to_owned(),
            ),
            summary_meta,
            event_text: format!(
                "{} {}",
                event["title"].as_str().unwrap_or_default(),
                event["description"].as_str().unwrap_or_default()
            ),
            participants: event["participants"].as_array().map_or(0, Vec::len),
            lock_row: self.details.lock_row,
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
        let transcript = self.details.has_transcript
            || self.audio.is_some()
            || !inactive
            || self.batch.is_some();
        note_tabs(&ids, transcript)
    }

    pub fn session_id(&self) -> Option<&SessionId> {
        self.current.as_ref().map(|session| &session.summary.id)
    }

    /// The summary the note shows, which chat edits and corrections target by default.
    pub fn shown_summary(&self) -> Option<Arc<str>> {
        self.current.as_ref()?;
        let tabs = self.tabs();
        match current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs) {
            NoteTab::Summary(id) => Some(id),
            _ => None,
        }
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

    pub(crate) fn rename(&mut self, cx: &mut Context<Self>) {
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

    /// Shipping's `OverflowButton`, group by group. Other note actions add rows to their group.
    fn menu_items(&self) -> Vec<MenuItem> {
        use MenuAction::*;
        let live = self.mode != SessionMode::Inactive;
        let (listen_icon, listen_label) = if live {
            ("editor/MicOff01Icon.svg", "Stop listening")
        } else if self.details.has_transcript || self.audio.is_some() {
            ("editor/Mic01Icon.svg", "Resume listening")
        } else {
            ("editor/Mic01Icon.svg", "Start listening")
        };
        let mut always_on_top = MenuItem::new(AlwaysOnTop, "editor/PinIcon.svg", "Always on Top")
            .disabled(self.always_on_top.is_none());
        always_on_top.checked = self.always_on_top == Some(true);
        menu_rows(vec![
            vec![
                MenuItem::new(MeetingInfo, "editor/Calendar04Icon.svg", "Meeting info"),
                MenuItem::new(VersionHistory, "editor/HistoryIcon.svg", "Version history"),
            ],
            // Shipping's recording rows follow Listen in its group.
            std::iter::once(
                MenuItem::new(Listen, listen_icon, listen_label)
                    .disabled(self.mode == SessionMode::Finalizing),
            )
            .chain(self.recording_menu_items())
            .collect(),
            vec![MenuItem::new(
                Export,
                "editor/FileDownloadIcon.svg",
                "Export",
            )],
            [
                // A standalone note window has no "Open in New Window".
                (!self.standalone).then(|| {
                    MenuItem::new(OpenWindow, "editor/AppWindowIcon.svg", "Open in New Window")
                }),
                // Shipping's `canOpenFloatingPanel`.
                (self.floating_bar_enabled && self.mode == SessionMode::Active).then(|| {
                    MenuItem::new(
                        FloatingPanel,
                        "chat/PictureInPictureIcon.svg",
                        "Open floating panel",
                    )
                }),
                Some(always_on_top),
            ]
            .into_iter()
            .flatten()
            .collect(),
            // Shipping's Lock Note shares Delete's group.
            self.details
                .lock_row
                .map(|(icon, label, disabled)| MenuItem::new(Lock, icon, label).disabled(disabled))
                .into_iter()
                .chain([MenuItem::new(Delete, "editor/TrashIcon.svg", "Delete")])
                .collect(),
        ])
    }

    pub fn set_clock(&mut self, clock: Clock, cx: &mut Context<Self>) {
        self.clock = clock;
        if let Some((info, _)) = &self.meeting_info {
            info.update(cx, |info, cx| info.set_clock(clock, cx));
        }
    }

    pub fn set_floating_bar_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.floating_bar_enabled != enabled {
            self.floating_bar_enabled = enabled;
            cx.notify();
        }
    }

    /// Shipping's ⌘⌥←/→: the previous or next of Summary, Memos and Transcript, wrapping.
    pub fn step_view(&mut self, forward: bool, cx: &mut Context<Self>) {
        let tabs = self.tabs();
        let current = current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs);
        if let Some(tab) = adjacent_tab(&tabs, &current, forward) {
            self.picker = None;
            self.select_tab(tab, cx);
        }
    }

    fn open_meeting_info(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.current.as_ref().map(|s| s.summary.id.clone()) else {
            return;
        };
        if self.meeting_info.is_some() {
            return;
        }
        let runtime = self.runtime.clone();
        let clock = self.clock;
        let info = cx.new(|cx| MeetingInfo::new(runtime, session, clock, cx));
        let subscription = cx.subscribe(&info, |this, _, event, cx| match event {
            MeetingInfoEvent::OpenContact(human) => {
                this.menu = false;
                this.meeting_info = None;
                cx.emit(NoteEvent::Open(Route::Human(human.clone())));
                cx.notify();
            }
        });
        self.meeting_info = Some((info, subscription));
        cx.notify();
    }

    fn toggle_always_on_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(previous) = self.always_on_top else {
            return;
        };
        let levels = crate::platform::window_level::window_level(cx);
        match levels.set_always_on_top(window, !previous) {
            Ok(()) => self.always_on_top = Some(!previous),
            Err(error) => tracing::warn!(%error, "always on top failed"),
        }
        cx.notify();
    }

    fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.current.clone() else {
            return;
        };
        let tabs = self.tabs();
        let summary = match current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs)
        {
            NoteTab::Summary(id)
                if self
                    .details
                    .summaries
                    .iter()
                    .any(|summary| summary.id.0 == id) =>
            {
                Some(DocumentId(id))
            }
            _ => None,
        };
        let runtime = self.runtime.clone();
        let editors = self
            .content
            .clone()
            .and_then(|content| content.downcast::<EditorPane>().ok())
            .into_iter()
            .chain(
                self.summary_editor
                    .as_ref()
                    .map(|(_, editor, _)| editor.clone()),
            )
            .map(|editor| editor.downgrade())
            .collect();
        let modal = cx.new(|cx| {
            ExportModal::new(
                runtime,
                session.summary.id.clone(),
                session.summary.title.clone(),
                summary,
                editors,
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(&modal, window, |this, _, event, window, cx| {
            let ExportEvent::Close = event;
            this.export = None;
            if let Some(focus) = this.return_focus.take() {
                focus.focus(window, cx);
            }
            cx.notify();
        });
        self.export = Some((modal, subscription));
        cx.notify();
    }

    fn folder_picked(
        &mut self,
        event: &FolderPickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.folder_open = false;
        match event {
            FolderPickerEvent::Choose { path, create } => {
                if let Some(session) = &self.current {
                    cx.emit(NoteEvent::Request(OpenNote::Move {
                        ids: vec![session.summary.id.clone()].into(),
                        folder: path.clone(),
                        create: *create,
                    }));
                }
            }
            FolderPickerEvent::SeeAll => {
                let path = self.details.folder_path.clone();
                cx.emit(NoteEvent::Open(if path.is_empty() {
                    Route::Folders
                } else {
                    Route::Folder(path)
                }));
            }
            FolderPickerEvent::Dismiss => {}
        }
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn tab_menu_items(&self, menu: &TabMenu) -> Vec<(&'static str, bool, bool)> {
        match menu {
            TabMenu::Memo => {
                let (body, format) = &self.details.memo;
                vec![("Copy", stored_markdown(body, format).is_empty(), false)]
            }
            TabMenu::Summary(id) => {
                let generating = self
                    .current
                    .as_ref()
                    .and_then(|session| self.jobs.get(&session.summary.id))
                    .is_some_and(|job| job.shows(id) && job.running());
                let empty = self
                    .details
                    .summaries
                    .iter()
                    .find(|summary| &summary.id.0 == id)
                    .is_none_or(|summary| {
                        stored_markdown(&summary.body, &summary.body_format).is_empty()
                    });
                let primary = self.tabs().into_iter().find_map(|tab| match tab {
                    NoteTab::Summary(id) => Some(id),
                    _ => None,
                });
                let mut items = vec![("Copy", empty, false), ("Regenerate", generating, false)];
                if primary.as_ref() != Some(id) {
                    items.push(("Remove", generating, true));
                }
                items
            }
        }
    }

    fn choose_tab_menu(&mut self, menu: TabMenu, label: &str, cx: &mut Context<Self>) {
        self.tab_menu = None;
        match (menu, label) {
            (TabMenu::Memo, "Copy") => {
                let (body, format) = &self.details.memo;
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(stored_markdown(
                    body, format,
                )));
                toasts::success("Memo copied to clipboard", cx);
            }
            (TabMenu::Summary(id), "Copy") => {
                if let Some(summary) = self.details.summaries.iter().find(|s| s.id.0 == id) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(stored_markdown(
                        &summary.body,
                        &summary.body_format,
                    )));
                    let title = self.summary_title(&id);
                    toasts::success(format!("{title} copied to clipboard"), cx);
                }
            }
            (TabMenu::Summary(_), "Regenerate") => self.regenerate(cx),
            (TabMenu::Summary(id), "Remove") => self.remove_summary(id, cx),
            _ => {}
        }
        cx.notify();
    }

    fn summary_template(&self, template_id: &str) -> Option<&Template> {
        self.templates
            .templates
            .iter()
            .find(|template| &*template.id == template_id && !template_id.is_empty())
    }

    /// Shipping's `useEnhancedViewTitle`: the template's title, else the stored title.
    fn summary_title(&self, id: &str) -> SharedString {
        let (raw, template_id) = self
            .details
            .summary_meta
            .get(id)
            .cloned()
            .unwrap_or_default();
        summary::tab_title(&raw, self.summary_template(&template_id), &template_id)
    }

    /// Shipping's `deleteEnhancedNote`; the removed tab hands over to the one before it.
    fn remove_summary(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        let tabs = self.tabs();
        let current = current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs);
        if current == NoteTab::Summary(id.clone())
            && let Some(index) = tabs.iter().position(|tab| *tab == current)
            && index > 0
        {
            self.view = Some(tabs[index - 1].clone());
        }
        let reply = self.runtime.submit(move |services| async move {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            services
                .executor
                .execute(
                    "UPDATE session_documents SET deleted_at = ?, updated_at = ? WHERE id = ?
                     AND kind IN ('summary', 'template_output') AND deleted_at IS NULL"
                        .into(),
                    vec![json!(now), json!(now), json!(id)],
                )
                .await
                .map_err(super::mutations::failure)?;
            Ok(())
        });
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Could not remove the summary: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Shipping's overflow recording rows: Re-transcribe saved audio, upload into an empty
    /// note, and reveal the audio folder.
    fn recording_menu_items(&self) -> Vec<MenuItem> {
        let row = MenuItem::new;
        let running = matches!(self.batch, Some(Job::Running { .. }));
        let inactive = self.mode == SessionMode::Inactive && !running;
        let mut items = Vec::new();
        if self.audio.is_some() && inactive {
            items.push(row(
                MenuAction::Retranscribe,
                "editor/RefreshIcon.svg",
                "Re-transcribe",
            ));
        }
        if self.audio.is_none()
            && !self.details.has_transcript
            && !self.menu_note_content
            && self.mode == SessionMode::Inactive
        {
            items.push(row(
                MenuAction::UploadAudio,
                "editor/AudioWaveformIcon.svg",
                "Upload audio",
            ));
            items.push(row(
                MenuAction::UploadTranscript,
                "editor/FileTextIcon.svg",
                "Upload transcript",
            ));
        }
        if self.audio.is_some() {
            items.push(row(
                MenuAction::ShowAudio,
                "meeting/FolderOpenIcon.svg",
                if cfg!(target_os = "macos") {
                    "Show Audio in Finder"
                } else if cfg!(target_os = "windows") {
                    "Show Audio in Explorer"
                } else {
                    "Show Audio in Folder"
                },
            ));
        }
        items
    }

    fn current_note_has_content(&self, cx: &gpui::App) -> bool {
        let tabs = self.tabs();
        match current_tab(self.view.as_ref(), self.mode == SessionMode::Active, &tabs) {
            NoteTab::Summary(_) => self.editable_summary_shown(),
            NoteTab::Transcript => self.details.has_transcript,
            NoteTab::Memos => self
                .memo_editor()
                .is_some_and(|editor| !editor.read(cx).is_pristine()),
        }
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
        // Participants edited in Meeting info decide whether a brief is offered.
        if self.meeting_info.take().is_some() {
            self.load_brief(cx);
        }
        if self.menu_focus.is_focused(window)
            && let Some(focus) = self.return_focus.take()
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn choose_menu(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        // Meeting info opens its submenu, and Always on Top toggles in place, like shipping.
        match action {
            MenuAction::MeetingInfo => return self.open_meeting_info(cx),
            MenuAction::AlwaysOnTop => return self.toggle_always_on_top(window, cx),
            _ => {}
        }
        let keep_focus = action == MenuAction::Export;
        let return_focus = keep_focus.then(|| self.return_focus.clone()).flatten();
        self.close_menu(window, cx);
        if keep_focus {
            self.return_focus = return_focus;
        }
        let Some(session) = &self.current else {
            return;
        };
        let id = session.summary.id.clone();
        match action {
            MenuAction::Listen => self.toggle_recording(cx),
            MenuAction::Retranscribe => {
                let runtime = self.runtime.clone();
                Uploads::global(cx)
                    .update(cx, |uploads, cx| uploads.retranscribe(&runtime, id, cx));
            }
            MenuAction::UploadAudio => {
                let runtime = self.runtime.clone();
                Uploads::global(cx)
                    .update(cx, |uploads, cx| uploads.upload_audio(&runtime, id, cx));
            }
            MenuAction::UploadTranscript => {
                let runtime = self.runtime.clone();
                Uploads::global(cx).update(cx, |uploads, cx| {
                    uploads.upload_transcript(&runtime, id, cx)
                });
            }
            MenuAction::ShowAudio => {
                Uploads::global(cx).update(cx, |uploads, cx| uploads.show_audio(id, cx));
            }
            MenuAction::Export => self.open_export(window, cx),
            MenuAction::OpenWindow => cx.emit(NoteEvent::Request(OpenNote::Window(id))),
            MenuAction::FloatingPanel => cx.emit(NoteEvent::OpenFloatingPanel),
            MenuAction::VersionHistory => self.open_history(window, cx),
            MenuAction::Delete => cx.emit(NoteEvent::Request(OpenNote::Delete(vec![id].into()))),
            MenuAction::Lock => super::note_lock::toggle_from_menu(id, cx),
            MenuAction::MeetingInfo | MenuAction::AlwaysOnTop => {}
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
        if self.view.as_ref() != Some(&tab) {
            self.find.update(cx, |find, cx| find.close(cx));
        }
        self.view = Some(tab);
        cx.notify();
    }

    fn searchable(&self) -> bool {
        let tab = current_tab(
            self.view.as_ref(),
            self.mode == SessionMode::Active,
            &self.tabs(),
        );
        self.current.is_some() && self.find_target(&tab).is_some()
    }

    /// Shipping's note search keys (`note-input/search/context.tsx`): ⌘F toggles the find bar
    /// and ⌘H opens it with replace. Returns whether the note took the key; a taken ⌘H never
    /// reaches the app menu's Hide.
    pub(crate) fn find_shortcut(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        let replace = match event.keystroke.key.as_str() {
            "f" => false,
            "h" => true,
            _ => return false,
        };
        if !modifiers.secondary(cx) || modifiers.shift || modifiers.alt || !self.searchable() {
            return false;
        }
        self.find.update(cx, |find, cx| {
            if replace {
                find.toggle_replace(window, cx)
            } else {
                find.toggle(window, cx)
            }
        });
        true
    }

    /// What the find bar searches on `tab`, and whether it may replace there.
    fn find_target(&self, tab: &NoteTab) -> Option<(FindTarget, bool)> {
        match tab {
            NoteTab::Summary(id) => self
                .summary_editor
                .as_ref()
                .filter(|(document, _, _)| &document.0 == id)
                .map(|(_, editor, _)| (FindTarget::Editor(editor.downgrade()), true)),
            NoteTab::Memos => self
                .content
                .clone()
                .and_then(|content| content.downcast::<EditorPane>().ok())
                .map(|editor| (FindTarget::Editor(editor.downgrade()), true)),
            NoteTab::Transcript => self
                .transcript
                .as_ref()
                .filter(|_| self.current.is_some())
                .map(|view| (FindTarget::Transcript(view.downgrade()), false)),
        }
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

    /// Shipping's `FolderPicker` trigger: the folder's icon and path, opening the picker under
    /// it (`align` end when the chip sits on the right).
    fn folder_chip(
        &self,
        align_end: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let folder = self.details.folder.clone();
        let compact = self.compact_folder;
        let open = self.folder_open;
        let glyph = self.details.folder_icon.clone();
        div()
            .relative()
            .flex_shrink_0()
            .min_w_0()
            .child(
                div()
                    .id("note-folder")
                    .h(px(28.))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .rounded(px(RADIUS))
                    .text_color(colors.muted_foreground)
                    .hover(|view| view.bg(colors.accent).text_color(colors.foreground))
                    .when(open, |view| {
                        view.bg(colors.accent).text_color(colors.foreground)
                    })
                    .cursor_pointer()
                    .map(|view| {
                        if folder.is_empty() || compact {
                            view.w(px(28.)).justify_center()
                        } else {
                            view.max_w(px(144.)).min_w_0().gap_1().px(px(6.))
                        }
                    })
                    .on_hover(cx.listener(|this, hovering: &bool, _, _| {
                        this.folder_chip_hovered = *hovering;
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.folder_open {
                            this.folder_open = false;
                        } else if this.current.is_some() {
                            this.return_focus = window.focused(cx);
                            this.folder_open = true;
                            let path = this.details.folder_path.clone();
                            this.folder_picker
                                .update(cx, |picker, cx| picker.open(path, window, cx));
                        }
                        cx.notify();
                    }))
                    .group("note-folder")
                    .child(match glyph {
                        Some(glyph) => glyph.render(16.),
                        None => icon("workspace/Folder01Icon.svg", 16., colors.muted_foreground)
                            .group_hover("note-folder", |style| style.text_color(colors.foreground))
                            .when(open, |icon| icon.text_color(colors.foreground))
                            .into_any_element(),
                    })
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
                    }),
            )
            .when(open, |view| {
                let popover = resource_list::floating_menu(224., colors)
                    .id("note-folder-picker")
                    .pb_0()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(|event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                        }
                    })
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        // The chip's own click toggles it closed.
                        if !this.folder_chip_hovered {
                            this.folder_open = false;
                            cx.notify();
                        }
                    }))
                    .child(self.folder_picker.clone());
                view.child(
                    div()
                        .absolute()
                        .top(px(32.))
                        .map(|view| {
                            if align_end {
                                view.right_0()
                            } else {
                                view.left_0()
                            }
                        })
                        .child(
                            gpui::deferred(
                                gpui::anchored()
                                    .anchor(if align_end {
                                        gpui::Anchor::TopRight
                                    } else {
                                        gpui::Anchor::TopLeft
                                    })
                                    .snap_to_window()
                                    .child(popover),
                            )
                            .with_priority(1),
                        ),
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
                        let template_id = self
                            .details
                            .summary_meta
                            .get(id)
                            .map(|(_, template_id)| template_id.clone())
                            .unwrap_or_default();
                        let template = self.summary_template(&template_id).cloned();
                        let job = self
                            .current
                            .as_ref()
                            .and_then(|session| self.jobs.get(&session.summary.id))
                            .filter(|job| job.shows(id));
                        Some((
                            self.summary_title(id),
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
                // Shipping's `shouldShowTranscriptTabSpinner`: finalizing or running a batch.
                let finalizing = transcript
                    && (mode == SessionMode::Finalizing
                        || matches!(self.batch, Some(Job::Running { .. })));
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
                    .when(
                        active && matches!(tab, NoteTab::Summary(_) | NoteTab::Memos),
                        |view| {
                            let menu = match tab {
                                NoteTab::Summary(id) => TabMenu::Summary(id.clone()),
                                _ => TabMenu::Memo,
                            };
                            view.on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.picker = None;
                                    this.tab_menu = Some((event.position, menu.clone()));
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                        },
                    )
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
                    .when(active && transcript, |view| {
                        view.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                                if let Some(transcript) = &this.transcript {
                                    transcript.update(cx, |transcript, cx| {
                                        transcript.open_tab_menu(event.position, cx)
                                    });
                                }
                                cx.stop_propagation();
                            }),
                        )
                    })
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
                let recording = self
                    .current
                    .as_ref()
                    .map(|session| (self.runtime.clone(), session.summary.id.clone()));
                cx.new(|cx| {
                    let mut player = PlayerView::new(playback, cx);
                    player.open(path, cx);
                    if let Some((runtime, session)) = recording {
                        player.set_recording(runtime, session);
                    }
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
            // A standalone window's header starts after the macOS traffic lights (76px).
            .pl(px(if self.standalone && cfg!(target_os = "macos") {
                80.
            } else {
                12.
            }))
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
                        .child(self.folder_chip(false, colors, cx))
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
                    .when(!breadcrumb, |view| {
                        view.child(self.folder_chip(true, colors, cx))
                    })
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
                                    this.menu_note_content = this.current_note_has_content(cx);
                                    this.menu = true;
                                    this.menu_index = None;
                                    this.always_on_top =
                                        crate::platform::window_level::window_level(cx)
                                            .always_on_top(window)
                                            .ok();
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
        let submenu = self.meeting_info.as_ref().map(|(info, _)| info.clone());
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
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.close_menu(window, cx),
                    "up" => this.step_menu(false),
                    "down" => this.step_menu(true),
                    "right" => {
                        let items = this.menu_items();
                        if this
                            .menu_index
                            .and_then(|index| items.get(index))
                            .map(|item| item.action)
                            == Some(MenuAction::MeetingInfo)
                        {
                            this.open_meeting_info(cx);
                        }
                    }
                    "left" => this.meeting_info = None,
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
                        let sub_open =
                            item.action == MenuAction::MeetingInfo && self.meeting_info.is_some();
                        let highlighted = self.menu_index == Some(index) || sub_open;
                        let action = item.action;
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
                            // Radix opens a submenu on hover and closes it over any other row.
                            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                                if !*hovering {
                                    return;
                                }
                                if action == MenuAction::MeetingInfo {
                                    this.open_meeting_info(cx);
                                } else if this.meeting_info.take().is_some() {
                                    cx.notify();
                                }
                            }))
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
                                            this.choose_menu(action, window, cx)
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
                            .when(item.checked, |view| {
                                view.child(icon(
                                    "editor/CheckIcon.svg",
                                    16.,
                                    colors.popover_foreground,
                                ))
                            })
                            .into_any_element();
                        separator.into_iter().chain([row])
                    })),
            )
            // Shipping's `w-72` submenu flips to the menu's left, level with its first row.
            .when_some(submenu, |view, info| {
                view.child(
                    div()
                        .id("meeting-info-submenu")
                        .absolute()
                        .top(px(9.))
                        .right(px(217.))
                        .w(px(288.))
                        .occlude()
                        .p(px(2.))
                        .rounded(px(22.))
                        .border_1()
                        .border_color(colors.app_floating_border)
                        .bg(colors.app_floating_chrome)
                        .shadow_lg()
                        .child(info),
                )
            })
    }

    /// Radix menus are modal: a press outside closes the menu and goes nowhere else.
    fn menu_backdrop(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::deferred(
            gpui::anchored()
                .position(gpui::point(px(0.), px(0.)))
                .child(
                    div()
                        .id("note-actions-backdrop")
                        .w(window.viewport_size().width)
                        .h(window.viewport_size().height)
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.close_menu(window, cx)),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, _, window, cx| this.close_menu(window, cx)),
                        ),
                ),
        )
        .with_priority(1)
    }

    /// The Memos and Summary tabs' right-click menus (shipping shows them as native menus).
    fn tab_menu_view(
        &self,
        position: Point<Pixels>,
        menu: TabMenu,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let items = self.tab_menu_items(&menu);
        gpui::deferred(
            gpui::anchored().position(position).snap_to_window().child(
                resource_list::context_menu(colors)
                    .id("note-tab-menu")
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.tab_menu = None;
                        cx.notify();
                    }))
                    .children(items.into_iter().flat_map(|(label, disabled, separated)| {
                        let menu = menu.clone();
                        let separator = separated
                            .then(|| div().my_1().h(px(1.)).bg(colors.border).into_any_element());
                        let row = div()
                            .id(SharedString::from(format!("note-tab-menu-{label}")))
                            .h(px(30.))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_sm()
                            .rounded(px(4.))
                            .when_else(
                                disabled,
                                |row| row.opacity(0.5),
                                |row| {
                                    row.cursor_pointer()
                                        .hover(|style| style.bg(colors.accent))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_tab_menu(menu.clone(), label, cx)
                                        }))
                                },
                            )
                            .child(label)
                            .into_any_element();
                        separator.into_iter().chain([row])
                    })),
            ),
        )
        .with_priority(1)
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
        let summary = self.current.as_ref().map(|session| &session.summary);
        if let Some(gate) = super::note_lock::gate(summary, window, cx) {
            return gate;
        }
        self.details.lock_row =
            summary.and_then(|summary| super::note_lock::menu_row(&summary.id, cx));
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
                let transcript = match &self.transcript {
                    Some(transcript) => transcript.clone(),
                    None => {
                        let transcript = cx.new(|cx| {
                            let mut view =
                                TranscriptView::new(runtime, session.summary.id.clone(), cx);
                            view.set_audio(audio, cx);
                            view
                        });
                        self.transcript_events =
                            Some(cx.subscribe(&transcript, |this, _, event, cx| match event {
                                TranscriptEvent::ResumeListening => {
                                    if this.mode == SessionMode::Inactive {
                                        this.toggle_recording(cx)
                                    }
                                }
                            }));
                        self.transcript = Some(transcript.clone());
                        transcript
                    }
                };
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
        // Shipping's `AudioDropTarget` wraps the memo and summary editors.
        let body = match (body, &self.current) {
            (Some(content), Some(session)) if current != NoteTab::Transcript => Some(
                upload::audio_drop_target(
                    content,
                    colors,
                    self.runtime.clone(),
                    session.summary.id.clone(),
                    window,
                    cx,
                )
                .into_any_element(),
            ),
            (body, _) => body,
        };
        let target = ready.then(|| self.find_target(&current)).flatten();
        let searchable = target.is_some();
        self.find.update(cx, |find, cx| {
            let replace = target.as_ref().is_some_and(|(_, replace)| *replace);
            find.set_target(target.map(|(target, _)| target), replace, cx)
        });
        let find_shown = searchable && self.find.read(cx).visible();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let handled = if event.keystroke.key == "escape" {
                    let find = this.find.clone();
                    let close = this.searchable() && find.read(cx).visible();
                    if close {
                        find.update(cx, |find, cx| find.dismiss(window, cx));
                    }
                    close
                } else {
                    this.find_shortcut(event, window, cx)
                };
                if handled {
                    cx.stop_propagation();
                }
            }))
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
            .when(ready, |view| view.child(self.pending_edits.clone()))
            .when(find_shown, |view| {
                view.child(div().px_3().pt_1().child(self.find.clone()))
            })
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
            .children(ready.then(|| self.conflict_banner(colors, cx)).flatten())
            .when_some(body, |view, content| {
                view.child(div().flex().flex_col().flex_1().min_h_0().child(content))
            })
            .when(self.menu, |view| {
                view.child(self.menu_backdrop(window, cx))
                    .child(gpui::deferred(self.menu(colors, cx)).with_priority(2))
            })
            .when_some(self.tab_menu.clone(), |view, (position, menu)| {
                view.child(self.tab_menu_view(position, menu, colors, cx))
            })
            .when_some(
                self.export.as_ref().map(|(modal, _)| modal.clone()),
                |view, modal| view.child(modal),
            )
            .children(self.history_dialog(colors, window, cx))
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
    fn overflow_menu_separates_only_groups_that_have_rows() {
        let row = |action| MenuItem::new(action, "", "");
        let rows = menu_rows(vec![
            vec![row(MenuAction::MeetingInfo)],
            vec![],
            vec![row(MenuAction::Export), row(MenuAction::OpenWindow)],
        ]);
        assert_eq!(
            rows.iter()
                .map(|row| (row.action, row.separator_before))
                .collect::<Vec<_>>(),
            [
                (MenuAction::MeetingInfo, false),
                (MenuAction::Export, true),
                (MenuAction::OpenWindow, false)
            ]
        );
    }

    #[test]
    fn command_option_arrows_cycle_the_note_views() {
        let tabs = note_tabs(&["summary".into()], true);
        assert_eq!(
            adjacent_tab(&tabs, &NoteTab::Transcript, true),
            Some(NoteTab::Summary("summary".into()))
        );
        assert_eq!(
            adjacent_tab(&tabs, &NoteTab::Memos, false),
            Some(NoteTab::Summary("summary".into()))
        );
    }

    struct FakeLevel(std::cell::Cell<bool>);

    impl crate::platform::window_level::WindowLevel for FakeLevel {
        fn always_on_top(&self, _: &Window) -> desktop_runtime::Result<bool> {
            Ok(self.0.get())
        }

        fn set_always_on_top(&self, _: &Window, on: bool) -> desktop_runtime::Result<()> {
            self.0.set(on);
            Ok(())
        }
    }

    #[gpui::test]
    fn always_on_top_sets_the_window_level_and_stays_open(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let level = std::rc::Rc::new(FakeLevel(std::cell::Cell::new(false)));
        cx.update(|cx| cx.set_global(crate::platform::window_level::WindowLevels(level.clone())));
        let window = cx.add_window(|window, cx| NoteView::new(runtime.clone(), window, cx));
        window
            .update(cx, |note, window, cx| {
                note.menu = true;
                note.always_on_top = crate::platform::window_level::window_level(cx)
                    .always_on_top(window)
                    .ok();
                note.choose_menu(MenuAction::AlwaysOnTop, window, cx);
                assert!(note.menu);
                let item = note
                    .menu_items()
                    .into_iter()
                    .find(|item| item.action == MenuAction::AlwaysOnTop)
                    .unwrap();
                assert!(item.checked && !item.disabled);
            })
            .unwrap();
        assert!(level.0.get());
        futures::executor::block_on(runtime.shutdown()).unwrap();
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
