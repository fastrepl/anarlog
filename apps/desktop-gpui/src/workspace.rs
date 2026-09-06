mod ai_settings;
mod calendar_tab;
mod contacts_tab;
mod developers_page;
mod document_view;
mod export;
mod filter_menu;
mod folders_tab;
mod icon_picker;
mod meeting_info;
mod menu;
mod note;
pub(crate) mod onboarding;
mod open_note;
mod overflow;
mod recording;
mod settings;
pub(crate) use overflow::find_session_dir;
mod sidebar;
mod stats_page;
mod template_picker;
mod templates_tab;
mod title_bar;
mod toast;

use std::sync::Arc;

use chrono::{Local, Utc};
use gpui::{
    Context, Decorations, FocusHandle, ListAlignment, ListState, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, Render, SharedString, Window, div, prelude::*, px,
};

use crate::actions;
use crate::db::{NotePreview, ProviderSettings, Store};
use crate::editor::{BodyEditor, EditorEvent};
use crate::store_file::StoreFile;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::Theme;
use crate::timeline::{self, Timeline};
use crate::ui::TailwindText as _;

/// `apps/desktop/src/main/left-sidebar-panel.ts`.
const SIDEBAR_DEFAULT_WIDTH: f32 = 200.0;
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 360.0;
const RESIZE_EDGE: f32 = 5.0;

/// Which title bar menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    File,
    Edit,
    View,
    Help,
}

struct SidebarDrag {
    start_x: Pixels,
    start_width: f32,
}

/// Which window this workspace drives: the main window, or a standalone
/// note window (`/app/note/$sessionId`, server decorations, no sidebar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    Main,
    StandaloneNote(String),
}

/// `note-<sessionId>` windows, so opening a note twice focuses the first.
#[derive(Default)]
pub(crate) struct NoteWindows(pub std::collections::HashMap<String, gpui::WindowHandle<Workspace>>);

impl gpui::Global for NoteWindows {}

enum Sessions {
    Loading,
    Ready(Timeline),
    Failed(String),
}

/// One line of the sidebar list; buckets and their rows share one flat list
/// so the variable-height `list` element can virtualize them together.
#[derive(Clone)]
enum SidebarRow {
    /// `data-sidebar-timeline-top-spacer`: room for the floating chips.
    Spacer,
    Header {
        bucket: usize,
    },
    Session {
        bucket: usize,
        item: usize,
    },
}

/// `computeCurrentNoteTab` with no remembered tab and no live session.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NoteTab {
    Memo,
    Enhanced(String),
    Transcript,
}

enum Note {
    Empty,
    Loading,
    Ready {
        preview: Box<NotePreview>,
        tab: NoteTab,
    },
    Failed(String),
}

pub struct Workspace {
    mode: Mode,
    store: Arc<Store>,
    theme: Theme,
    focus_handle: FocusHandle,
    title_input: gpui::Entity<TextInput>,
    /// The memo editor for the selected session.
    editor: Option<gpui::Entity<BodyEditor>>,
    font_family: Option<SharedString>,
    mono_font_family: Option<SharedString>,
    sessions: Sessions,
    /// Every non-deleted session (`useSessionSummaries`), for the open-note dialog.
    session_rows: Vec<timeline::SessionRow>,
    event_rows: Vec<timeline::EventRow>,
    /// `useSidebarNotes`: grouping and ordering of the timeline.
    group_by: timeline::GroupBy,
    sort_order: timeline::SortOrder,
    filter_menu_open: bool,
    filter_submenu: Option<usize>,
    rows: Vec<SidebarRow>,
    list_state: ListState,
    selected: Option<String>,
    note: Note,
    sidebar_expanded: bool,
    sidebar_width: f32,
    sidebar_drag: Option<SidebarDrag>,
    open_menu: Option<Menu>,
    open_note: Option<open_note::OpenNoteDialog>,
    /// `recentlyOpenedSessionIds`, newest first, persisted to `store.json`.
    recently_opened: Vec<String>,
    store_file: StoreFile,
    /// The tab store's session tabs, in order; the tab strip itself is not
    /// shown, but `openNew` vs `openCurrent` decide which note gets closed.
    tabs: Vec<String>,
    provider_settings: ProviderSettings,
    /// `useUserTemplates`, for the empty-memo suggestions.
    templates: Vec<crate::db::Template>,
    /// `useAutoFocusEditor`: the session whose editor was focused on open, and
    /// the editor waiting for the next frame to receive that focus.
    auto_focused_session: Option<String>,
    pending_editor_focus: Option<gpui::Entity<BodyEditor>>,
    auth: toast::Auth,
    /// `getDismissedToasts` from `store.json`.
    dismissed_toasts: Vec<String>,
    /// The `theme` setting: `light`, `dark`, or `system`.
    theme_preference: String,
    overflow_open: bool,
    overflow_submenu: Option<usize>,
    /// The settings tab while it is the active overlay tab.
    settings_tab: Option<settings::SettingsTab>,
    settings_search: Option<gpui::Entity<TextInput>>,
    /// The settings `Select` whose popover is open.
    open_select: Option<settings::OpenSelect>,
    /// Transcription / Intelligence page state, created when a page opens.
    ai_settings: std::collections::HashMap<ai_settings::ProviderKind, ai_settings::AiSettings>,
    /// Provider cards whose Advanced disclosure is expanded.
    ai_advanced_open: std::collections::HashSet<(ai_settings::ProviderKind, &'static str)>,
    /// `usePermission` state for the Permissions page, keyed by permission.
    permissions: std::collections::HashMap<&'static str, settings::PermissionState>,
    /// The Meeting info submenu's data for the note whose overflow menu is open.
    meeting_info: Option<meeting_info::MeetingInfo>,
    /// The Export dialog while open.
    export_dialog: Option<export::ExportDialog>,
    /// A transient success / error toast.
    flash: Option<toast::FlashToast>,
    /// Developers page state, created when the page opens.
    developers: Option<developers_page::DevelopersState>,
    /// The Folders tab while open.
    folders: Option<folders_tab::FoldersState>,
    /// The Templates tab while open.
    templates_tab: Option<templates_tab::TemplatesState>,
    /// The Calendar tab while open.
    calendar: Option<calendar_tab::CalendarState>,
    /// The Contacts tab while open.
    contacts: Option<contacts_tab::ContactsState>,
    /// The enhanced tab's template picker while open.
    template_picker: Option<template_picker::TemplatePicker>,
    /// The template / folder icon picker while open.
    icon_picker: Option<icon_picker::IconPicker>,
    /// The Stats settings page's records and range.
    stats: Option<stats_page::StatsState>,
    /// The first-run flow while `OnboardingNeeded2` is not `false`.
    onboarding: Option<onboarding::OnboardingState>,
    /// The capture engine and the live session state.
    recording: recording::RecordingState,
    /// `anarlog.template-picker.recent-emojis` (kept for the session).
    recent_emoji_ids: Vec<String>,
    /// The note column's scroll position, for its WebKit-style scrollbar.
    note_scroll: gpui::ScrollHandle,
    /// The template section under the pointer (`group-hover`).
    hovered_section: Option<u64>,
    /// The `SpokenLanguagesView` chip input, created with the settings tab.
    spoken_search: Option<gpui::Entity<TextInput>>,
    spoken_highlighted: Option<usize>,
    pending_deletions: Vec<overflow::PendingDeletion>,
    /// Pending `scrollToAnchor`: the viewport ratio the current-time line
    /// should land at, applied over two frames once the row is measured.
    anchor_scroll: Option<f32>,
    /// `useAutoScrollToAnchor`: the launch scroll happens once.
    anchor_scrolled_once: bool,
    /// Id of the chrome button under the pointer, so icons can take the
    /// `hover:text-foreground` colour their container cannot pass down.
    hovered: Option<&'static str>,
}

impl Workspace {
    pub fn new(store: Arc<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_mode(store, Mode::Main, window, cx)
    }

    /// `StandaloneNoteWindow`: the note surface alone, showing `session_id`.
    pub fn standalone(
        store: Arc<Store>,
        session_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_mode(store, Mode::StandaloneNote(session_id), window, cx)
    }

    fn with_mode(
        store: Arc<Store>,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let font_family = crate::theme::ui_font_family(cx.text_system()).map(SharedString::from);
        let mono_font_family =
            crate::theme::mono_font_family(cx.text_system()).map(SharedString::from);
        tracing::info!(ui = ?font_family, mono = ?mono_font_family, "resolved font families");
        let theme = Theme::light();
        let title_input = cx.new(|cx| {
            TextInput::new(
                "Untitled",
                TextInputStyle {
                    text: theme.title,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: true,
                    masked: false,
                },
                window,
                cx,
            )
        });
        cx.subscribe(&title_input, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Committed {
                this.persist_title(cx);
            }
        })
        .detach();
        let store_file = StoreFile::next_to(store.path());
        let mut this = Self {
            mode: mode.clone(),
            store,
            theme,
            focus_handle: cx.focus_handle(),
            title_input,
            editor: None,
            font_family,
            mono_font_family,
            sessions: Sessions::Loading,
            session_rows: Vec::new(),
            event_rows: Vec::new(),
            group_by: timeline::GroupBy::Date,
            sort_order: timeline::SortOrder::Newest,
            filter_menu_open: false,
            filter_submenu: None,
            rows: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, px(400.0)),
            selected: None,
            note: Note::Empty,
            sidebar_expanded: true,
            sidebar_width: SIDEBAR_DEFAULT_WIDTH,
            sidebar_drag: None,
            open_menu: None,
            open_note: None,
            recently_opened: Vec::new(),
            store_file,
            tabs: Vec::new(),
            provider_settings: ProviderSettings::default(),
            templates: Vec::new(),
            auto_focused_session: None,
            pending_editor_focus: None,
            auth: toast::Auth::Loading,
            dismissed_toasts: Vec::new(),
            theme_preference: "system".to_string(),
            overflow_open: false,
            overflow_submenu: None,
            settings_tab: None,
            settings_search: None,
            open_select: None,
            ai_settings: std::collections::HashMap::new(),
            ai_advanced_open: std::collections::HashSet::new(),
            permissions: std::collections::HashMap::new(),
            meeting_info: None,
            export_dialog: None,
            flash: None,
            developers: None,
            folders: None,
            templates_tab: None,
            calendar: None,
            contacts: None,
            template_picker: None,
            icon_picker: None,
            stats: None,
            onboarding: None,
            recording: recording::RecordingState::default(),
            recent_emoji_ids: Vec::new(),
            note_scroll: gpui::ScrollHandle::new(),
            hovered_section: None,
            spoken_search: None,
            spoken_highlighted: None,
            pending_deletions: Vec::new(),
            anchor_scroll: None,
            anchor_scrolled_once: false,
            hovered: None,
        };
        // Chips and the bottom fade depend on the scroll position.
        this.list_state
            .set_scroll_handler(cx.listener(|_, _: &gpui::ListScrollEvent, _, cx| cx.notify()));
        this.reload_sessions(cx);
        this.reload_settings(cx);
        this.watch_changes(cx);
        match mode {
            Mode::Main => {
                this.restore_tabs(cx);
                this.start_onboarding_if_needed();
                this.spawn_recorder(cx);
            }
            Mode::StandaloneNote(session_id) => {
                this.tabs.push(session_id.clone());
                this.selected = Some(session_id.clone());
                this.note = Note::Loading;
                this.reload_note(session_id, cx);
            }
        }
        this
    }

    /// `hasCustomSidebarTab`: settings, folders, templates, calendar, and
    /// contacts swap the timeline for their own sidebar.
    pub(crate) fn custom_sidebar_open(&self) -> bool {
        self.settings_open()
            || self.folders_open()
            || self.templates_open()
            || self.calendar_open()
            || self.contacts_open()
    }

    /// `leftSidebarPanelStyle` without `canResizeLeftSidebarPanel`: custom
    /// sidebars lay out at `LEFT_SIDEBAR_DEFAULT_WIDTH_PX` regardless of the
    /// timeline's resized width.
    pub(crate) fn custom_sidebar_width(&self) -> f32 {
        if self.custom_sidebar_open() {
            SIDEBAR_DEFAULT_WIDTH
        } else {
            self.sidebar_width
        }
    }

    pub(crate) fn is_standalone(&self) -> bool {
        matches!(self.mode, Mode::StandaloneNote(_))
    }

    /// `openStandaloneNoteWindow`: a 720×820 server-decorated window (min
    /// 420×500) per session; an existing one is brought forward.
    pub(crate) fn open_note_window(&mut self, session_id: String, cx: &mut Context<Self>) {
        if let Some(handle) = cx
            .default_global::<NoteWindows>()
            .0
            .get(&session_id)
            .cloned()
        {
            let focused = handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok();
            if focused {
                return;
            }
        }
        let store = self.store.clone();
        let id = session_id.clone();
        let bounds = gpui::Bounds::centered(None, gpui::size(px(720.0), px(820.0)), cx);
        let result = cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Anarlog".into()),
                    ..Default::default()
                }),
                window_decorations: Some(gpui::WindowDecorations::Server),
                app_id: Some(crate::APP_ID.to_string()),
                window_min_size: Some(gpui::size(px(420.0), px(500.0))),
                ..Default::default()
            },
            move |window, cx| {
                let workspace = cx.new(|cx| Workspace::standalone(store, id, window, cx));
                workspace.read(cx).focus_handle().focus(window);
                workspace
            },
        );
        match result {
            Ok(handle) => {
                cx.default_global::<NoteWindows>()
                    .0
                    .insert(session_id, handle);
            }
            Err(error) => tracing::error!(%error, "failed to open note window"),
        }
    }

    /// `closeSessionNoteWindows`: deleting a note closes its windows.
    fn close_note_windows(&mut self, session_id: &str, cx: &mut Context<Self>) {
        if let Some(handle) = cx.default_global::<NoteWindows>().0.remove(session_id) {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
    }

    /// `initializeDesktopTabs`: pinned session tabs come back through
    /// `openNew` (the last one active) and the recent list is reloaded; with
    /// nothing pinned the empty view shows.
    fn restore_tabs(&mut self, cx: &mut Context<Self>) {
        self.recently_opened = self.store_file.recently_opened_sessions();
        self.dismissed_toasts = self.store_file.dismissed_toasts();
        for tab in self.store_file.pinned_session_tabs() {
            self.open_new(tab.id, cx);
        }
    }

    fn reload_settings(&mut self, cx: &mut Context<Self>) {
        let task = self.store.load_provider_settings();
        let templates = self.store.list_templates();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(settings)) = task.await {
                this.update(cx, |this, cx| {
                    if this.provider_settings != settings {
                        this.theme_preference = settings.theme.clone();
                        this.provider_settings = settings;
                        cx.notify();
                    }
                })
                .ok();
            }
            if let Ok(Ok(templates)) = templates.await {
                this.update(cx, |this, cx| {
                    if this.templates != templates {
                        this.templates = templates;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// `handleApplyTemplate`: the memo becomes one `h2` + empty paragraph per
    /// titled section, persisted with `raw_template_id` in the same write.
    fn apply_template(
        &mut self,
        template: &crate::db::Template,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content: Vec<serde_json::Value> = template
            .section_titles
            .iter()
            .flat_map(|title| {
                [
                    serde_json::json!({
                        "type": "heading",
                        "attrs": { "level": 2 },
                        "content": [{ "type": "text", "text": title }],
                    }),
                    serde_json::json!({ "type": "paragraph" }),
                ]
            })
            .collect();
        if content.is_empty() {
            return;
        }
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let session_id = editor.read(cx).session_id.clone();
        let body = serde_json::json!({ "type": "doc", "content": content }).to_string();
        editor.update(cx, |editor, cx| {
            editor.replace_body(&body, cx);
            // `replaceContent` leaves the selection at the end of the new document.
            editor.place_caret_at_end(window, cx);
        });
        let task =
            self.store
                .update_memo_with_template(session_id.clone(), body, template.id.clone());
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to apply template"),
            Err(error) => tracing::error!(%error, "failed to apply template"),
        })
        .detach();
    }

    /// Re-reads the list and the open note whenever the Tauri app commits.
    fn watch_changes(&self, cx: &mut Context<Self>) {
        let mut changes = self.store.changes();
        cx.spawn(async move |this, cx| {
            while changes.changed().await.is_ok() {
                let keep_going = this
                    .update(cx, |this, cx| {
                        this.reload_sessions(cx);
                        this.reload_settings(cx);
                        this.reload_folders_from_watcher(cx);
                        this.reload_templates_from_watcher(cx);
                        this.reload_contacts_from_watcher(cx);
                        this.reload_stats(cx);
                        if let Some(selected) = this.selected.clone() {
                            this.reload_note(selected, cx);
                        }
                    })
                    .is_ok();
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    fn reload_sessions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.sessions, Sessions::Ready(_)) {
            self.sessions = Sessions::Loading;
        }
        let task = self.store.list_timeline();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok((rows, events))) => {
                        this.session_rows = rows;
                        this.event_rows = events;
                        this.rebuild_timeline(cx);
                    }
                    Ok(Err(error)) => {
                        this.sessions = Sessions::Failed(error.to_string());
                        this.rebuild_rows();
                    }
                    Err(error) => {
                        this.sessions = Sessions::Failed(error.to_string());
                        this.rebuild_rows();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `buildTimelineBuckets` over the loaded rows with the current view.
    pub(crate) fn rebuild_timeline(&mut self, cx: &mut Context<Self>) {
        self.sessions = Sessions::Ready(timeline::build_with(
            &self.session_rows,
            &self.event_rows,
            Utc::now(),
            &Local,
            self.group_by,
            self.sort_order,
        ));
        self.rebuild_rows();
        cx.notify();
    }

    fn rebuild_rows(&mut self) {
        let previous_len = self.rows.len();
        self.rows.clear();
        if let Sessions::Ready(timeline) = &self.sessions {
            if timeline.has_more_future_items {
                self.rows.push(SidebarRow::Spacer);
            }
            for (bucket_ix, bucket) in timeline.buckets.iter().enumerate() {
                self.rows.push(SidebarRow::Header { bucket: bucket_ix });
                for item_ix in 0..bucket.items.len() {
                    self.rows.push(SidebarRow::Session {
                        bucket: bucket_ix,
                        item: item_ix,
                    });
                }
            }
        }
        // Splicing keeps the scroll position across reloads the way the DOM
        // list does; `reset` would jump back to the top.
        if previous_len == 0 {
            self.list_state.reset(self.rows.len());
        } else {
            self.list_state.splice(0..previous_len, self.rows.len());
        }
    }

    /// The list row that draws the current-time line and whether it sits at
    /// the row's bottom edge (after a header or the last past item) or its
    /// top edge (before the first past item).
    pub(crate) fn anchor_row(&self) -> Option<(usize, bool)> {
        let Sessions::Ready(timeline) = &self.sessions else {
            return None;
        };
        let now = Utc::now();
        for (index, row) in self.rows.iter().enumerate() {
            match row {
                SidebarRow::Header { bucket } => {
                    let bucket = &timeline.buckets[*bucket];
                    if bucket.label == "Today"
                        && !bucket.items.is_empty()
                        && matches!(
                            timeline::indicator_placement(&bucket.items, now, self.sort_order),
                            timeline::IndicatorPlacement::Before { index: 0 }
                        )
                    {
                        return Some((index, true));
                    }
                }
                SidebarRow::Session { bucket, item } => {
                    let bucket = &timeline.buckets[*bucket];
                    if bucket.label != "Today" {
                        continue;
                    }
                    match timeline::indicator_placement(&bucket.items, now, self.sort_order) {
                        timeline::IndicatorPlacement::Before { index: at }
                            if at == *item && at > 0 =>
                        {
                            return Some((index, false));
                        }
                        timeline::IndicatorPlacement::After if *item + 1 == bucket.items.len() => {
                            return Some((index, true));
                        }
                        _ => {}
                    }
                }
                SidebarRow::Spacer => {}
            }
        }
        None
    }

    /// `openCurrent`: reuse the tab if the note is already open, otherwise
    /// replace the active slot, which closes the note that was there.
    fn select(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.open_tab(session_id, false, cx);
    }

    /// `openNew`: the note opens in a new tab; the previous one stays open in
    /// the (invisible) tab list, so it is not closed or cleaned up.
    pub(crate) fn open_new(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.open_tab(session_id, true, cx);
    }

    fn open_tab(&mut self, session_id: String, force_new: bool, cx: &mut Context<Self>) {
        // `addRecentlyOpened`, saved through the store like the main window does.
        self.recently_opened.retain(|id| id != &session_id);
        self.recently_opened.insert(0, session_id.clone());
        self.recently_opened
            .truncate(open_note::MAX_RECENT_SESSIONS);
        if let Err(error) = self
            .store_file
            .save_recently_opened_sessions(&self.recently_opened)
        {
            tracing::warn!(%error, "failed to save recently opened sessions");
        }

        if self.selected.as_deref() == Some(session_id.as_str()) {
            return;
        }
        let previous = self.selected.replace(session_id.clone());
        let already_open = self.tabs.contains(&session_id);
        if !already_open {
            match previous
                .as_ref()
                .and_then(|id| self.tabs.iter().position(|t| t == id))
            {
                Some(slot) if !force_new => {
                    let closed = std::mem::replace(&mut self.tabs[slot], session_id.clone());
                    self.close_tab(closed, cx);
                }
                _ => self.tabs.push(session_id.clone()),
            }
        }
        self.note = Note::Loading;
        cx.notify();
        self.reload_note(session_id, cx);
    }

    /// `openCurrent` replaces the tab, so the previous note goes through the
    /// tab close handler: pending edits are written first, then an untouched
    /// note is soft-deleted.
    fn close_tab(&mut self, session_id: String, cx: &mut Context<Self>) {
        let pending = self
            .editor
            .as_ref()
            .filter(|editor| editor.read(cx).session_id == session_id)
            .and_then(|editor| editor.update(cx, |editor, _| editor.take_pending()));
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            if let Some(body) = pending
                && let Err(error) = store.update_memo(session_id.clone(), body).await
            {
                tracing::error!(%error, "failed to persist note");
            }
            match store.close_empty_session(session_id).await {
                Ok(Ok(true)) => {
                    this.update(cx, |this, cx| this.reload_sessions(cx)).ok();
                }
                Ok(Ok(false)) => {}
                Ok(Err(error)) => tracing::error!(%error, "session close cleanup"),
                Err(error) => tracing::error!(%error, "session close cleanup"),
            }
        })
        .detach();
    }

    fn ensure_summary(&mut self, session_id: String, cx: &mut Context<Self>) {
        let task = self.store.ensure_summary_document(session_id.clone());
        cx.spawn(async move |this, cx| {
            match task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result)
            {
                Ok(true) => {
                    this.update(cx, |this, cx| {
                        if this.selected.as_deref() == Some(session_id.as_str()) {
                            this.reload_note(session_id.clone(), cx);
                        }
                    })
                    .ok();
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::error!(%error, "[enhancer] failed to create default summary")
                }
            }
        })
        .detach();
    }

    fn reload_note(&mut self, session_id: String, cx: &mut Context<Self>) {
        let task = self.store.load_note(session_id.clone());
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // A newer selection may have raced this load; keep the latest.
                if this.selected.as_deref() != Some(session_id.as_str()) {
                    return;
                }
                this.note = match result {
                    Ok(Ok(Some(preview))) => {
                        // `useEnsureDefaultSummary`: a transcript without an
                        // enhanced note gets its Summary document created.
                        if preview.has_transcript && preview.enhanced.is_empty() {
                            this.ensure_summary(session_id.clone(), cx);
                        }
                        let tab = this.current_tab_for(&preview);
                        // `title = draftTitle ?? storeTitle`
                        let title = preview.session.title.clone();
                        this.title_input.update(cx, |input, cx| {
                            if !input.is_dirty() {
                                input.set_text(title, cx);
                            }
                        });
                        this.sync_editor(&preview, cx);
                        // `useAutoFocusEditor`: focus the memo once per opened
                        // session, at the document start.
                        if tab == NoteTab::Memo
                            && this.auto_focused_session.as_deref() != Some(session_id.as_str())
                            && let Some(editor) = this.editor.clone()
                        {
                            this.auto_focused_session = Some(session_id.clone());
                            this.pending_editor_focus = Some(editor);
                        }
                        Note::Ready {
                            preview: Box::new(preview),
                            tab,
                        }
                    }
                    Ok(Ok(None)) => Note::Failed("This note no longer exists.".to_string()),
                    Ok(Err(error)) => Note::Failed(error.to_string()),
                    Err(error) => Note::Failed(error.to_string()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `computeCurrentNoteTab`: keep the remembered tab while it still exists,
    /// otherwise the first enhanced note, otherwise the memo.
    fn current_tab_for(&self, preview: &NotePreview) -> NoteTab {
        let first_enhanced = preview
            .enhanced
            .first()
            .map(|doc| NoteTab::Enhanced(doc.id.clone()));
        match &self.note {
            Note::Ready {
                tab: NoteTab::Memo, ..
            } => NoteTab::Memo,
            Note::Ready {
                tab: NoteTab::Enhanced(id),
                ..
            } if preview.enhanced.iter().any(|doc| &doc.id == id) => NoteTab::Enhanced(id.clone()),
            Note::Ready {
                tab: NoteTab::Transcript,
                ..
            } if preview.has_transcript => NoteTab::Transcript,
            _ => first_enhanced.unwrap_or(NoteTab::Memo),
        }
    }

    fn set_tab(&mut self, tab: NoteTab, cx: &mut Context<Self>) {
        if let Note::Ready { tab: current, .. } = &mut self.note
            && *current != tab
        {
            *current = tab;
            cx.notify();
        }
    }

    pub(crate) fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if self.is_standalone() {
            return;
        }
        self.sidebar_expanded = !self.sidebar_expanded;
        cx.notify();
    }

    /// Keeps one `BodyEditor` per selected session, fed from the store unless
    /// it has unsaved edits.
    fn sync_editor(&mut self, preview: &NotePreview, cx: &mut Context<Self>) {
        let session_id = preview.session.id.clone();
        let body = preview.memo_body.clone();
        match &self.editor {
            Some(editor) if editor.read(cx).session_id == session_id => {
                editor.update(cx, |editor, cx| editor.replace_body(&body, cx));
            }
            _ => {
                if let Some(previous) = self.editor.take() {
                    previous.update(cx, |editor, cx| editor.flush(cx));
                }
                let editor = cx.new(|cx| BodyEditor::new(session_id, &body, cx));
                cx.subscribe(&editor, |this, editor, event: &EditorEvent, cx| {
                    let EditorEvent::Flush(json) = event;
                    let session_id = editor.read(cx).session_id.clone();
                    this.persist_memo(session_id, json.clone(), cx);
                })
                .detach();
                self.editor = Some(editor);
            }
        }
    }

    /// `updateSession({ raw_md })` from the editor's debounced flush.
    fn persist_memo(&mut self, session_id: String, body: String, cx: &mut Context<Self>) {
        let task = self.store.update_memo(session_id.clone(), body);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to persist note"),
            Err(error) => tracing::error!(%error, "failed to persist note"),
        })
        .detach();
    }

    /// `persistTitle`: the title input's blur/Enter writes the draft.
    fn persist_title(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        let title = self.title_input.read(cx).text().to_string();
        let task = self.store.update_title(session_id.clone(), title);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to persist title"),
            Err(error) => tracing::error!(%error, "failed to persist title"),
        })
        .detach();
    }

    /// `useNewNote`: create the session, then open it as the current tab.
    /// `useNewNoteAndListen`
    pub(crate) fn new_note_and_listen(&mut self, cx: &mut Context<Self>) {
        if self.recording.live.is_some() || self.recording.starting {
            return;
        }
        let task = self.store.create_note();
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(session_id)) = task.await {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.open_new(session_id.clone(), cx);
                    this.start_listening(session_id, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn new_note(&mut self, cx: &mut Context<Self>) {
        let task = self.store.create_note();
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(session_id)) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.open_new(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to create note"),
            Err(error) => tracing::error!(%error, "failed to create note"),
        })
        .detach();
    }

    /// Clicking a calendar event opens (creating if needed) its session.
    pub(crate) fn open_event(&mut self, event_id: String, cx: &mut Context<Self>) {
        let task = self.store.open_event_session(event_id);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(session_id)) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.select(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to open calendar event"),
            Err(error) => tracing::error!(%error, "failed to open calendar event"),
        })
        .detach();
    }

    fn set_menu(&mut self, menu: Option<Menu>, cx: &mut Context<Self>) {
        if self.open_menu != menu {
            self.open_menu = menu;
            cx.notify();
        }
    }

    /// `ResizableHandle` drag: clamp to the panel's min/max like the app.
    fn begin_sidebar_drag(&mut self, x: Pixels, cx: &mut Context<Self>) {
        self.sidebar_drag = Some(SidebarDrag {
            start_x: x,
            start_width: self.sidebar_width,
        });
        cx.notify();
    }

    fn update_sidebar_drag(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if let Some(drag) = &self.sidebar_drag {
            let width = (drag.start_width + f32::from(x - drag.start_x))
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
            if width != self.sidebar_width {
                self.sidebar_width = width;
                cx.notify();
            }
        }
    }

    fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_drag.take().is_some() {
            cx.notify();
        }
    }

    /// Switch between note views (`mod+alt+left/right`), in tab-strip order:
    /// enhanced notes first, then the memo.
    fn step_view(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Note::Ready { preview, tab } = &self.note else {
            return;
        };
        let mut tabs: Vec<NoteTab> = preview
            .enhanced
            .iter()
            .map(|doc| NoteTab::Enhanced(doc.id.clone()))
            .collect();
        tabs.push(NoteTab::Memo);
        if preview.has_transcript {
            tabs.push(NoteTab::Transcript);
        }
        let Some(index) = tabs.iter().position(|t| t == tab) else {
            return;
        };
        let next = (index as isize + delta).rem_euclid(tabs.len() as isize) as usize;
        let next = tabs[next].clone();
        self.set_tab(next, cx);
    }

    fn set_hovered(&mut self, id: &'static str, hovered: bool, cx: &mut Context<Self>) {
        let next = hovered.then_some(id);
        if self.hovered != next && (hovered || self.hovered == Some(id)) {
            self.hovered = next;
            cx.notify();
        }
    }

    /// Icon colour for a chrome button: muted, or foreground while hovered.
    fn chrome_icon_color(&self, id: &'static str) -> gpui::Rgba {
        if self.hovered == Some(id) {
            self.theme.foreground
        } else {
            self.theme.muted_foreground
        }
    }

    /// `chrome_button` wired to hover tracking.
    fn tracked_chrome_button(
        &self,
        id: &'static str,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        crate::ui::chrome_button(id, self.theme, self.hovered == Some(id))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hovered(id, *hovered, cx);
            }))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `resolveIsDarkMode` on every frame: the setting or the system
        // appearance may have changed since the last one.
        if let Some(editor) = self.pending_editor_focus.take() {
            editor.update(cx, |editor, cx| editor.focus_start(window, cx));
        }
        self.prepare_contact_avatars();
        let resolved = Theme::resolve(&self.theme_preference, window.appearance());
        if resolved != self.theme {
            self.theme = resolved;
            self.title_input.update(cx, |input, cx| {
                input.set_style(
                    TextInputStyle {
                        text: resolved.title,
                        placeholder: resolved.muted_foreground,
                        selection: resolved.selection,
                        underline_when_focused: true,
                        masked: false,
                    },
                    cx,
                )
            });
        }
        let theme = self.theme;
        let client_decorations = matches!(window.window_decorations(), Decorations::Client { .. });

        // `isOnboarding`: the onboarding tab takes the whole shell surface,
        // without the title bar or sidebar.
        if self.onboarding_open() {
            return div()
                .id("workspace-root")
                .size_full()
                .track_focus(&self.focus_handle)
                .child(self.render_onboarding(window, cx))
                .into_any_element();
        }

        // `ShellFrame`: title bar (Windows/Linux) above the `shell-scaffold`
        // row of sidebar + main surface, all on `bg-background`.
        let dragging = self.sidebar_drag.is_some();
        div()
            .id("window")
            .track_focus(&self.focus_handle)
            .key_context(actions::KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &actions::NewNote, _, cx| this.new_note(cx)))
            .on_action(
                cx.listener(|this, _: &actions::OpenNoteDialog, window, cx| {
                    this.open_note_dialog(window, cx)
                }),
            )
            // `mod+shift+n` → `useNewNoteAndListen`: a new note that starts
            // capturing as soon as it opens.
            .on_action(cx.listener(|this, _: &actions::StartRecording, _, cx| {
                this.new_note_and_listen(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::OpenSettings, window, cx| {
                this.open_settings(settings::SettingsTab::App, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &actions::OpenTranscriptionSettings, window, cx| {
                    this.open_settings(settings::SettingsTab::Transcription, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &actions::OpenIntelligenceSettings, window, cx| {
                    this.open_settings(settings::SettingsTab::Intelligence, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToggleSidebar, _, cx| this.toggle_sidebar(cx)),
            )
            .on_action(cx.listener(|this, _: &actions::PreviousView, _, cx| this.step_view(-1, cx)))
            .on_action(cx.listener(|this, _: &actions::NextView, _, cx| this.step_view(1, cx)))
            .on_action(|_: &actions::ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_action(|_: &actions::CloseWindow, window, _| window.remove_window())
            // `useCloseStandaloneNoteWindowOnEscape`
            .on_action(cx.listener(|this, _: &actions::Escape, window, cx| {
                if this.export_dialog.is_some() {
                    this.close_export_dialog(cx);
                } else if this.folder_dialog_open() {
                    this.close_folder_dialogs(cx);
                } else if this.calendar_popover_open() {
                    this.close_calendar_popover(cx);
                } else if this.icon_picker_open() {
                    this.close_icon_picker(window, cx);
                } else if this.template_picker_open() {
                    this.close_template_picker(window, cx);
                } else if this.is_standalone() {
                    window.remove_window();
                }
            }))
            // A press on anything that does not claim the pointer moves focus
            // to the shell, blurring inputs the way a click on the page does.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseDownEvent, window, _| {
                    if !this.focus_handle.is_focused(window) {
                        this.focus_handle.focus(window);
                    }
                }),
            )
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .tw_text_sm()
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .when(dragging, |root| {
                root.cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_sidebar_drag(event.position.x, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_sidebar_drag(cx)),
                    )
            })
            .when(client_decorations, |root| {
                root.on_mouse_down(MouseButton::Left, |event, window, _cx| {
                    let size = window.window_bounds().get_bounds().size;
                    if let Some(edge) =
                        title_bar::resize_edge(event.position, px(RESIZE_EDGE), size)
                    {
                        window.start_window_resize(edge);
                    }
                })
            })
            .when(
                title_bar::uses_windows_style_title_bar() && !self.is_standalone(),
                |root| root.child(self.render_title_bar(window, cx)),
            )
            // `shell-scaffold`: `pl-1` only while the main surface has its left
            // chrome; collapsing the sidebar switches to `top-borderless`. A
            // standalone note window is the surface alone.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.sidebar_expanded && !self.is_standalone(), |shell| {
                        let sidebar = if self.contacts_open() {
                            self.render_contacts_sidebar(window, cx).into_any_element()
                        } else if self.calendar_open() {
                            self.render_calendar_sidebar(cx).into_any_element()
                        } else if self.templates_open() {
                            self.render_templates_sidebar(cx).into_any_element()
                        } else if self.folders_open() {
                            self.render_folders_sidebar(cx).into_any_element()
                        } else if self.settings_open() {
                            self.render_settings_nav(cx).into_any_element()
                        } else {
                            self.render_sidebar(window, cx).into_any_element()
                        };
                        // `canResizeLeftSidebarPanel` holds only for the
                        // timeline: custom sidebars are pinned to the default
                        // width and the `ResizableHandle` is not rendered.
                        shell
                            .pl_1()
                            .child(sidebar)
                            .when(!self.custom_sidebar_open(), |shell| {
                                shell.child(self.render_sidebar_handle(cx))
                            })
                    })
                    .when(!self.sidebar_expanded && !self.is_standalone(), |shell| {
                        shell.gap_1()
                    })
                    .child(self.render_main_surface(window, cx)),
            )
            // sonner's toaster sits at `z-index: 999999999`, above every
            // popover, menu, and dialog.
            .children(
                self.render_toast_host(window, cx)
                    .map(|toast| gpui::deferred(toast).with_priority(10)),
            )
            .children(
                self.render_undo_toast(cx)
                    .map(|toast| gpui::deferred(toast).with_priority(10)),
            )
            .children(
                self.render_settings_alert_toast()
                    .map(|toast| gpui::deferred(toast).with_priority(10)),
            )
            .children(self.render_overflow_menu(window, cx))
            .children(self.render_filter_menu(window, cx))
            .children(self.render_open_menu(window, cx))
            .children(self.render_export_dialog(cx))
            .children(self.render_folder_dialogs(cx))
            .children(
                self.render_recording_toast(cx)
                    .map(|toast| gpui::deferred(toast).with_priority(10)),
            )
            .children(
                self.render_flash_toast()
                    .map(|toast| gpui::deferred(toast).with_priority(11)),
            )
            .children(self.render_open_note_dialog(window, cx))
            .into_any_element()
    }
}
