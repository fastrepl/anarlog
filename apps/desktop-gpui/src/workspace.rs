mod ai_availability;
mod ai_settings;
mod attachments;
mod audio_player;
mod audio_retention;
mod automations_tab;
mod billing;
mod calendar_tab;
mod chat;
mod chat_cta;
mod chat_panels;
mod chat_tool_cards;
mod contact_summary;
mod contacts_tab;
mod deep_links;
mod developers_page;
mod dictation;
pub(crate) mod dictionary;
mod document_view;
mod edit_review;
mod enhance;
mod export;
mod filter_menu;
pub(crate) mod floating_bar;
mod folders_tab;
mod format_toolbar;
mod icon_picker;
mod instruction;
mod llm_models;
mod meeting_info;
mod mention_popup;
pub(crate) mod menu;
mod note;
mod note_search_bar;
mod notifications;
pub(crate) mod onboarding;
mod open_note;
mod overflow;
mod pre_meeting_brief;
mod recording;
mod scheduled_auto_start;
mod session_drag;
mod settings;
mod stt_selection;
pub(crate) use settings::SettingsTab;
mod share;
mod speaker_assign;
pub(crate) use overflow::find_session_dir;
mod folder_picker;
mod sidebar;
mod stats_page;
mod template_picker;
mod templates_tab;
mod timeline_selection;
mod title_bar;
mod toast;
mod tooltip;
mod transcript_edit;
mod transcript_selection;
mod transcript_tab;

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
const SIDEBAR_DEFAULT_WIDTH: f32 = crate::sidebar_layout::DEFAULT_WIDTH_PX;
const SIDEBAR_MIN_WIDTH: f32 = crate::sidebar_layout::MIN_WIDTH_PX;
const SIDEBAR_MAX_WIDTH: f32 = crate::sidebar_layout::MAX_WIDTH_PX;
/// The shell's own `store.json` scope for the sidebar share when the webview
/// has no layout to share.
const SIDEBAR_STORE_SCOPE: &str = "gpui";
const SIDEBAR_STORE_KEY: &str = "left_sidebar_fraction";
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
/// `useIgnoredEvents`' id sets, shared by the timeline and the calendar.
fn workspace_ignored(
    settings: &ProviderSettings,
    key: &str,
    field: &str,
) -> std::collections::HashSet<String> {
    calendar_tab::ignored_ids(settings, key, field)
}

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
    /// `EnhancedEditor`: the open summary's editor, keyed by its note id.
    enhanced_editor: Option<(String, gpui::Entity<BodyEditor>)>,
    font_family: Option<SharedString>,
    mono_font_family: Option<SharedString>,
    /// `@pierre/diffs`' mono stack for the edit review.
    diff_font_family: Option<SharedString>,
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
    /// `react-resizable-panels`' persisted layout: the sidebar's share of the
    /// panel group, `None` until the first frame sets the 200px default.
    sidebar_fraction: Option<f64>,
    /// The panel group width the last frame laid out.
    sidebar_group_width: f32,
    sidebar_drag: Option<SidebarDrag>,
    /// `/app/instruction`: the browser hand-off screen replacing the shell.
    instruction: Option<instruction::Instruction>,
    /// `autoSaveId="main-chat"`'s persisted layout: the chat panel's share of
    /// the panel group, `None` until a drag or a saved layout sets it.
    chat_panel_fraction: Option<f64>,
    chat_panel_drag: Option<chat_panels::ChatPanelDrag>,
    /// `useNoteSurfaceWindowWidthGuard`'s previous panel state, its resize
    /// baseline, and the window expansions still to restore.
    width_guard_state: crate::chat_panel_layout::PanelState,
    width_guard_last_body: Option<f32>,
    width_expansions: Vec<(f32, f32)>,
    /// `isAppWindowInactive`'s complement, kept current by the activation observer.
    window_active: bool,
    /// A `requestAppAttention` is outstanding until the window is focused.
    attention_requested: bool,
    open_menu: Option<Menu>,
    /// Radix's roving focus in the open menu, the items its keys act on (as
    /// this frame rendered them), the focus the open menu holds and the one
    /// it took it from.
    menu_keyboard: menu::MenuKeyboard,
    menu_runtime: std::cell::RefCell<Option<menu::MenuRuntime>>,
    menu_focus: FocusHandle,
    menu_previous_focus: Option<FocusHandle>,
    /// `editTargetRef`: the element focused when a title bar menu opened, so
    /// an Edit item runs on it after the press moved focus.
    menu_edit_target: Option<FocusHandle>,
    /// The editing context menu a right-click in an editable opened: where,
    /// and the editable it acts on.
    edit_context_menu: Option<(gpui::Point<Pixels>, FocusHandle)>,
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
    /// A mention chip asked for `/app/human|organization/<id>`; the next frame
    /// (which has the window) opens the Contacts tab on it.
    pending_contact: Option<contacts_tab::Selection>,
    /// `useMentionConfig` candidates shared with the editor's picker.
    mention_candidates:
        std::rc::Rc<std::cell::RefCell<Vec<crate::editor::mention_picker::MentionItem>>>,
    /// `chat/components/input/history.ts`: the sent drafts, newest first,
    /// shared across chats so a prompt can be recalled in a new one.
    chat_sent_history: Vec<crate::text_area::Draft>,
    mention_humans: Vec<crate::contacts::Human>,
    mention_organizations: Vec<crate::contacts::Organization>,
    auth: toast::Auth,
    /// `getDismissedToasts` from `store.json`.
    dismissed_toasts: Vec<String>,
    /// The `theme` setting: `light`, `dark`, or `system`.
    theme_preference: String,
    overflow_open: bool,
    overflow_submenu: Option<usize>,
    /// The settings tab while it is the active overlay tab.
    settings_tab: Option<settings::SettingsTab>,
    /// `returnToSlotId`: the settings tab an overlay tab (folders, templates,
    /// calendar, contacts, automations) was opened from, for `leaveOverlayTab`.
    overlay_return_settings: Option<settings::SettingsTab>,
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
    automations: Option<automations_tab::AutomationsState>,
    /// The Contacts tab while open.
    contacts: Option<contacts_tab::ContactsState>,
    /// The enhanced tab's template picker while open.
    template_picker: Option<template_picker::TemplatePicker>,
    folder_picker: Option<folder_picker::FolderPicker>,
    /// `useFolderPaths` / `useFolderIcons` for the header's folder button.
    folder_catalog: crate::folders::Catalog,
    /// `setSelectedPath` for the Folders tab the picker opens next.
    pending_folder_selection: Option<String>,
    /// `EnhancerService` and the `enhance` / `title` task states.
    enhancer: enhance::EnhancerState,
    /// `auto-enhance-started`: switch to this enhanced note on the next reload.
    pending_enhanced_tab: Option<String>,
    /// The template / folder icon picker while open.
    icon_picker: Option<icon_picker::IconPicker>,
    /// The Stats settings page's records and range.
    stats: Option<stats_page::StatsState>,
    /// The first-run flow while `OnboardingNeeded2` is not `false`.
    onboarding: Option<onboarding::OnboardingState>,
    /// The capture engine and the live session state.
    recording: recording::RecordingState,
    /// `ScheduledMeetingAutoStart`'s state.
    auto_start: scheduled_auto_start::AutoStartState,
    /// `joiningMeeting`: `Join & record` is opening the meeting and starting.
    joining_meeting: bool,
    /// The session audio player for the open transcript tab.
    audio_player: Option<audio_player::AudioPlayer>,
    /// The open transcript tab's scroll, follow and word hover state.
    transcript_view: transcript_tab::TranscriptView,
    /// The "Ask anything" pill is hovered (it grows into the prompt bar).
    chat_cta_hovered: bool,
    /// The find-in-note bar (`SearchProvider` state) while open.
    note_search: Option<note_search_bar::NoteSearch>,
    /// The onboarding's looping BGM while it is shown.
    onboarding_bgm: Option<crate::sfx::Sound>,
    /// The completion cue playing (dropping a `Sound` stops it).
    completion_cue: Option<crate::sfx::Sound>,
    /// `["models", provider, listModels]`: the Intelligence page's catalogues.
    llm_models: llm_models::LlmModelsCache,
    /// `["llm-health-check", model]`: the selected model's probe result.
    llm_health: llm_models::LlmHealthCache,
    /// `lastSelectedModelsRef`
    last_llm_models: llm_models::LastLlmModels,
    /// `useProviderAvailability` results by config.
    ai_availability: ai_availability::AvailabilityCache,
    /// The AI pages whose 5s local-server poll is running.
    availability_polls: std::collections::HashSet<ai_settings::ProviderKind>,
    /// `PersistAiSelection` is writing the default LLM selection.
    applying_llm_default: bool,
    /// The Transcription page's `lastSelectedModelsRef`.
    last_stt_models: std::collections::HashMap<String, String>,
    /// `PersistAiSelection` is writing the default STT selection.
    applying_stt_default: bool,
    /// `pendingProvider`: an STT provider picked without a model to show.
    pending_stt_provider: Option<String>,
    /// `changeMutation.isPending` of the storage row: the vault is being
    /// moved and the app is about to relaunch.
    storage_change_pending: bool,
    /// `changeMutation.error` under the storage row.
    storage_change_error: Option<String>,
    /// `useDeepgramHealth` results by API key.
    deepgram_health: std::collections::HashMap<String, stt_selection::SttHealth>,
    /// The Dictionary page's term field and the row being edited.
    dictionary_input: Option<gpui::Entity<TextInput>>,
    dictionary_edit: Option<dictionary::DictionaryEdit>,
    /// `chat.mode`
    chat_mode: chat::ChatMode,
    /// The active scope's chat (`general`, or `automations` on its tab) and
    /// the parked one.
    chat: chat::ChatState,
    parked_chat: chat::ChatState,
    chat_scroll: gpui::ScrollHandle,
    /// The `edit` tab: a chat `edit_memo` / `edit_summary` proposal under review.
    edit_review: Option<edit_review::EditReview>,
    /// The Share CTA's popover while open.
    share_popover: Option<share::SharePopover>,
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
    timeline_selection: timeline_selection::TimelineSelection,
    /// `showIgnored`: list ignored calendar events dimmed and struck through.
    show_ignored_events: bool,
    timeline_menu: Option<timeline_selection::TimelineMenu>,
    /// Session ids awaiting the `Delete N selected notes?` confirmation.
    pending_delete_selected: Vec<String>,
    /// Pending `scrollToAnchor`: the viewport ratio the current-time line
    /// should land at, applied over two frames once the row is measured.
    anchor_scroll: Option<f32>,
    /// `selectedNodeRef` → `scrollTimelineItemIntoView`: the session whose
    /// row scrolls into view once it is laid out.
    reveal_session: Option<String>,
    /// `useAutoScrollToAnchor`: the launch scroll happens once.
    anchor_scrolled_once: bool,
    /// Id of the chrome button under the pointer, so icons can take the
    /// `hover:text-foreground` colour their container cannot pass down.
    hovered: Option<&'static str>,
    /// `pre-meeting-brief-job.ts`'s `generating`: sessions whose brief is
    /// being written.
    brief_jobs: std::collections::HashSet<String>,
    /// `listInstalledApplications`, loaded when the Notifications page opens.
    installed_apps: Option<std::rc::Rc<Vec<anlg_detect::InstalledApp>>>,
    /// `listDefaultIgnoredBundleIds`
    default_ignored_apps: Vec<String>,
    /// The excluded-apps trigger's last painted bounds, for the popover flip.
    excluded_apps_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// The window's height as of the last frame, for `vh`-sized panels.
    viewport_height: f32,
    viewport_width: f32,
    /// The participant chip whose `Enhance contact` button is hovered.
    hovered_participant: Option<String>,
    /// The meeting info panel's and its participant row's last painted
    /// bounds, for the dropdown drawn over the panel's scrolling body.
    meeting_panel_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    participant_input_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// The open (or opening) Radix-style tooltip.
    tooltip: Option<tooltip::TooltipState>,
    tooltip_bounds: tooltip::TriggerBounds,
    /// When the last shown tooltip closed, for `skipDelayDuration`.
    tooltip_closed_at: Option<std::time::Instant>,
    tooltip_generation: u64,
    /// The pointer's last position over a `title` trigger; the platform
    /// tooltip opens below it.
    tooltip_pointer: gpui::Point<gpui::Pixels>,
    /// The frame being rendered, stamped on the triggers' painted bounds.
    tooltip_frame: u64,
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
        crate::ui::set_ui_font(font_family.clone());
        let mono_font_family =
            crate::theme::mono_font_family(cx.text_system()).map(SharedString::from);
        let diff_font_family =
            crate::theme::diff_mono_font_family(cx.text_system()).map(SharedString::from);
        tracing::info!(
            ui = ?font_family,
            mono = ?mono_font_family,
            diff = ?diff_font_family,
            "resolved font families"
        );
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
        let store_file = StoreFile::in_vault(store.vault_base());
        let sidebar_fraction = Self::load_sidebar_fraction(&store, &store_file);
        let chat_panel_fraction = Self::load_chat_panel_fraction(&store, &store_file);
        let mut this = Self {
            mode: mode.clone(),
            store,
            theme,
            focus_handle: cx.focus_handle(),
            title_input,
            editor: None,
            enhanced_editor: None,
            font_family,
            mono_font_family,
            diff_font_family,
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
            sidebar_fraction,
            sidebar_group_width: 0.0,
            sidebar_drag: None,
            instruction: None,
            chat_panel_fraction,
            chat_panel_drag: None,
            width_guard_state: Default::default(),
            width_guard_last_body: None,
            width_expansions: Vec::new(),
            window_active: true,
            attention_requested: false,
            open_menu: None,
            menu_keyboard: menu::MenuKeyboard::default(),
            menu_runtime: std::cell::RefCell::new(None),
            menu_focus: cx.focus_handle(),
            menu_previous_focus: None,
            menu_edit_target: None,
            edit_context_menu: None,
            open_note: None,
            recently_opened: Vec::new(),
            store_file,
            tabs: Vec::new(),
            provider_settings: ProviderSettings::default(),
            templates: Vec::new(),
            auto_focused_session: None,
            pending_editor_focus: None,
            pending_contact: None,
            mention_candidates: Default::default(),
            chat_sent_history: Vec::new(),
            mention_humans: Vec::new(),
            mention_organizations: Vec::new(),
            auth: toast::Auth::Loading,
            dismissed_toasts: Vec::new(),
            theme_preference: "system".to_string(),
            overflow_open: false,
            overflow_submenu: None,
            settings_tab: None,
            overlay_return_settings: None,
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
            automations: None,
            contacts: None,
            template_picker: None,
            folder_picker: None,
            folder_catalog: crate::folders::Catalog::default(),
            pending_folder_selection: None,
            enhancer: enhance::EnhancerState::default(),
            pending_enhanced_tab: None,
            icon_picker: None,
            stats: None,
            onboarding: None,
            recording: recording::RecordingState::default(),
            auto_start: Default::default(),
            joining_meeting: false,
            audio_player: None,
            transcript_view: transcript_tab::TranscriptView::default(),
            chat_cta_hovered: false,
            note_search: None,
            onboarding_bgm: None,
            completion_cue: None,
            llm_models: Default::default(),
            llm_health: Default::default(),
            last_llm_models: Default::default(),
            ai_availability: Default::default(),
            availability_polls: Default::default(),
            applying_llm_default: false,
            last_stt_models: Default::default(),
            applying_stt_default: false,
            pending_stt_provider: None,
            storage_change_pending: false,
            storage_change_error: None,
            deepgram_health: Default::default(),
            dictionary_input: None,
            dictionary_edit: None,
            chat_mode: Default::default(),
            chat: chat::ChatState::new(crate::chat::Scope::General),
            parked_chat: chat::ChatState::new(crate::chat::Scope::Automations),
            chat_scroll: gpui::ScrollHandle::new(),
            edit_review: None,
            share_popover: None,
            recent_emoji_ids: Vec::new(),
            note_scroll: gpui::ScrollHandle::new(),
            hovered_section: None,
            spoken_search: None,
            spoken_highlighted: None,
            pending_deletions: Vec::new(),
            timeline_selection: timeline_selection::TimelineSelection::default(),
            show_ignored_events: false,
            timeline_menu: None,
            pending_delete_selected: Vec::new(),
            anchor_scroll: None,
            reveal_session: None,
            anchor_scrolled_once: false,
            hovered: None,
            brief_jobs: std::collections::HashSet::new(),
            installed_apps: None,
            default_ignored_apps: anlg_detect::default_ignored_bundle_ids(),
            excluded_apps_bounds: std::rc::Rc::default(),
            viewport_height: 0.0,
            viewport_width: 0.0,
            hovered_participant: None,
            meeting_panel_bounds: std::rc::Rc::default(),
            participant_input_bounds: std::rc::Rc::default(),
            tooltip: None,
            tooltip_bounds: Default::default(),
            tooltip_closed_at: None,
            tooltip_generation: 0,
            tooltip_pointer: gpui::Point::default(),
            tooltip_frame: 0,
        };
        // Chips and the bottom fade depend on the scroll position.
        this.list_state
            .set_scroll_handler(cx.listener(|_, _: &gpui::ListScrollEvent, _, cx| cx.notify()));
        this.reload_sessions(cx);
        this.reload_settings(cx);
        this.watch_changes(cx);
        this.observe_window_activity(window, cx);
        match mode {
            Mode::Main => {
                this.restore_tabs(cx);
                this.start_onboarding_if_needed();
                this.spawn_recorder(cx);
                // `ScheduledMeetingAutoStart`: linked meetings record themselves.
                this.start_scheduled_auto_start(cx);
                // `LiveCaptureRecovery`: finalize captures a crash left behind.
                this.recover_captures(cx);
                this.start_enhancer(cx);
                // The task manager's retention tick; it also sweeps expired
                // voiceprint candidates a few times a day.
                this.start_audio_retention(cx);
                crate::voiceprint::spawn_cleanup_loop(&this.store);
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
            || self.automations_open()
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

    /// `on_window_event`'s `CloseRequested` for `AppWindow::Main`: the window
    /// leaves fullscreen and hides while the app stays alive behind the tray
    /// (its frame saved first, like the window-state plugin's save). A
    /// standalone note window closes.
    pub(crate) fn close_window(&self, window: &mut Window) {
        if self.is_standalone() {
            window.remove_window();
            return;
        }
        crate::window_state::save_main(self.store.identifier(), window.window_bounds());
        if window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        hide_window(window);
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
                    title: Some(crate::tray::app_name(self.store.identifier()).into()),
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
                        // `FloatingMeetingWindowSettingsSync`
                        this.sync_floating_bar(cx);
                        this.sync_tray(cx);
                        for kind in [
                            ai_settings::ProviderKind::Stt,
                            ai_settings::ProviderKind::Llm,
                        ] {
                            if this.ai_settings.contains_key(&kind) {
                                this.ensure_ai_availability(kind, false, cx);
                            }
                        }
                        this.ensure_llm_models(false, cx);
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
                        this.refresh_mention_candidates(cx);
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

    /// The identity of a sidebar row, stable across rebuilds: an item's id or
    /// a bucket's label.
    fn row_key(&self, index: usize) -> Option<String> {
        let Sessions::Ready(timeline) = &self.sessions else {
            return None;
        };
        match self.rows.get(index)? {
            SidebarRow::Spacer => Some(String::new()),
            SidebarRow::Header { bucket } => timeline
                .buckets
                .get(*bucket)
                .map(|bucket| format!("#{}", bucket.label)),
            SidebarRow::Session { bucket, item } => timeline
                .buckets
                .get(*bucket)
                .and_then(|bucket| bucket.items.get(*item))
                .map(|item| item.id.clone()),
        }
    }

    /// `buildTimelineBuckets` over the loaded rows with the current view.
    pub(crate) fn rebuild_timeline(&mut self, cx: &mut Context<Self>) {
        // The DOM list keeps its scroll position across live-query refreshes;
        // gpui's `splice` over every row would jump to the top, so remember
        // the row at the top of the viewport and put it back.
        let top = self.list_state.logical_scroll_top();
        let top_key = self.row_key(top.item_ix);
        let ignored_events =
            workspace_ignored(&self.provider_settings, "ignored_events", "tracking_id");
        let ignored_series =
            workspace_ignored(&self.provider_settings, "ignored_recurring_series", "id");
        self.sessions = Sessions::Ready(timeline::build_with(
            &self.session_rows,
            &self.event_rows,
            Utc::now(),
            &Local,
            timeline::View {
                group_by: self.group_by,
                order: self.sort_order,
                show_ignored: self.show_ignored_events,
            },
            |event| {
                ignored_events.contains(&event.tracking_id_event)
                    || (!event.recurrence_series_id.is_empty()
                        && ignored_series.contains(&event.recurrence_series_id))
            },
        ));
        self.rebuild_rows();
        if let Some(key) = top_key
            && let Some(item_ix) =
                (0..self.rows.len()).find(|ix| self.row_key(*ix) == Some(key.clone()))
        {
            self.list_state.scroll_to(gpui::ListOffset {
                item_ix,
                offset_in_item: top.offset_in_item,
            });
        }
        self.publish_tray_schedule(cx);
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

        // `openNew` / `openCurrent` of a sessions tab replaces an overlay tab
        // (settings, folders, templates, calendar, contacts, automations).
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        self.close_automations(cx);
        self.close_edit_review(cx);
        if self.selected.as_deref() == Some(session_id.as_str()) {
            return;
        }
        let previous = self.selected.replace(session_id.clone());
        // The chat's pending drop refs belong to the note they were dropped on.
        self.chat.pending_manual_refs.clear();
        self.reveal_session = Some(session_id.clone());
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

    /// `useEnsureDefaultSummary`: outside a live capture, a session with a
    /// transcript, a running batch, or a failed batch and no enhanced note
    /// gets its Summary document created.
    pub(crate) fn ensure_default_summary(&mut self, cx: &mut Context<Self>) {
        let Note::Ready { preview, .. } = &self.note else {
            return;
        };
        if !preview.enhanced.is_empty() {
            return;
        }
        let session_id = preview.session.id.clone();
        let mode = self.session_mode(&session_id);
        if matches!(
            mode,
            recording::SessionMode::Active | recording::SessionMode::Finalizing
        ) {
            return;
        }
        if preview.has_transcript || self.batch_state(&session_id).is_some() {
            self.ensure_summary(session_id, cx);
        }
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
                        let tab = this.current_tab_for(&preview);
                        // `title = draftTitle ?? storeTitle`
                        let title = preview.session.title.clone();
                        this.title_input.update(cx, |input, cx| {
                            if !input.is_dirty() {
                                input.set_text(title, cx);
                            }
                        });
                        this.sync_editor(&preview, cx);
                        this.sync_enhanced_editor(&preview, &tab, cx);
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
                this.ensure_default_summary(cx);
                // `ReadyScheduledSessionAutoStart` mounts with the loaded note.
                this.try_pending_auto_start(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `computeCurrentNoteTab`: keep the remembered tab while it still exists,
    /// otherwise the first enhanced note, otherwise the memo.
    fn current_tab_for(&mut self, preview: &NotePreview) -> NoteTab {
        // `updateSessionTabState({ view: { type: "enhanced", id } })` after
        // `auto-enhance-started` / a template swap.
        if let Some(note_id) = self.pending_enhanced_tab.take()
            && preview.enhanced.iter().any(|doc| doc.id == note_id)
        {
            return NoteTab::Enhanced(note_id);
        }
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
            } if self.can_show_transcript(preview) => NoteTab::Transcript,
            // A remembered transcript tab that no longer applies falls back to
            // the memo; a missing enhanced note or a fresh open lands on the
            // first enhanced note.
            Note::Ready {
                tab: NoteTab::Transcript,
                ..
            } => NoteTab::Memo,
            _ => first_enhanced.unwrap_or(NoteTab::Memo),
        }
    }

    fn set_tab(&mut self, tab: NoteTab, cx: &mut Context<Self>) {
        if let Note::Ready { tab: current, .. } = &mut self.note
            && *current != tab
        {
            *current = tab.clone();
            if let Note::Ready { preview, .. } = &self.note {
                let preview = preview.clone();
                self.sync_enhanced_editor(&preview, &tab, cx);
            }
            cx.notify();
        }
    }

    /// The editor behind the current tab: the memo's, or the open summary's.
    pub(crate) fn active_editor(&self) -> Option<&gpui::Entity<BodyEditor>> {
        match &self.note {
            Note::Ready {
                tab: NoteTab::Enhanced(id),
                ..
            } => self
                .enhanced_editor
                .as_ref()
                .filter(|(note_id, _)| note_id == id)
                .map(|(_, editor)| editor),
            _ => self.editor.as_ref(),
        }
    }

    /// `EnhancedEditor`'s `initialContent` / `key`: one editor per summary,
    /// opened on `ensureFirstLineTitle(content, sessionTitle)` and refreshed
    /// from the store while it has no pending edits.
    fn sync_enhanced_editor(
        &mut self,
        preview: &NotePreview,
        tab: &NoteTab,
        cx: &mut Context<Self>,
    ) {
        let NoteTab::Enhanced(note_id) = tab else {
            return;
        };
        let Some(doc) = preview.enhanced.iter().find(|doc| &doc.id == note_id) else {
            return;
        };
        let parsed = serde_json::from_str::<serde_json::Value>(&doc.body)
            .ok()
            .filter(|json| json.get("type").is_some())
            .or_else(|| {
                (!doc.body.trim().is_empty())
                    .then(|| anlg_tiptap::md_to_tiptap_json(&doc.body).ok())
                    .flatten()
            })
            .unwrap_or_else(|| serde_json::json!({ "type": "doc", "content": [] }));
        let body =
            crate::document::ensure_first_line_title(parsed, &preview.session.title).to_string();
        match &self.enhanced_editor {
            Some((current, editor)) if current == note_id => {
                editor.update(cx, |editor, cx| editor.replace_body(&body, cx));
            }
            _ => {
                if let Some((_, previous)) = self.enhanced_editor.take() {
                    previous.update(cx, |editor, cx| editor.flush(cx));
                }
                let search = mention_popup::search_over(
                    self.mention_candidates.clone(),
                    cx.global::<crate::search::Search>().0.clone(),
                    self.store.runtime().clone(),
                );
                let runtime = self.store.runtime().clone();
                let editor = cx.new(|cx| {
                    let mut editor = BodyEditor::new(note_id.clone(), &body, cx);
                    editor.set_mention_search(search);
                    editor.set_runtime(runtime);
                    editor.set_enforce_title_heading(true);
                    editor
                });
                let session_id = preview.session.id.clone();
                let stored = doc.body.clone();
                let session_title = preview.session.title.clone();
                cx.subscribe(
                    &editor,
                    move |this, editor, event: &EditorEvent, cx| match event {
                        EditorEvent::Flush(json) => {
                            let note_id = editor.read(cx).session_id.clone();
                            this.persist_enhanced_note(
                                session_id.clone(),
                                note_id,
                                &stored,
                                &session_title,
                                json.clone(),
                                cx,
                            );
                        }
                        EditorEvent::OpenMention { kind, id } => {
                            this.open_mention(kind, id.clone(), cx);
                        }
                        EditorEvent::Files(files) => {
                            this.attach_files(session_id.clone(), editor, files.clone(), cx);
                        }
                        EditorEvent::Dropped(paths) => {
                            this.drop_files(session_id.clone(), editor, paths.clone(), cx);
                        }
                    },
                )
                .detach();
                self.enhanced_editor = Some((note_id.clone(), editor));
            }
        }
    }

    /// `EnhancedEditor.handleChange`: skip the canonical empty document on an
    /// empty store body, then `updateEnhancedNoteContent` with the title from
    /// the first line (kept when neither the document nor the store has text).
    fn persist_enhanced_note(
        &mut self,
        session_id: String,
        note_id: String,
        stored_content: &str,
        session_title: &str,
        json: String,
        cx: &mut Context<Self>,
    ) {
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) else {
            return;
        };
        if stored_content.is_empty()
            && crate::document::is_canonical_empty_document(&parsed, session_title)
        {
            return;
        }
        let title = crate::document::extract_first_line_title(&parsed);
        let next_title =
            if title.is_some() || crate::document::has_stored_note_content(stored_content) {
                Some(title.unwrap_or_default())
            } else {
                None
            };
        let title_changed = next_title.is_some();
        let task =
            self.store
                .update_enhanced_note_content(note_id, session_id.clone(), json, next_title);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    if title_changed {
                        this.reload_sessions(cx);
                    }
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            }
            Ok(Err(error)) => {
                tracing::error!(%error, "[enhanced-editor] failed to persist summary")
            }
            Err(error) => tracing::error!(%error, "[enhanced-editor] failed to persist summary"),
        })
        .detach();
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
                let search = mention_popup::search_over(
                    self.mention_candidates.clone(),
                    cx.global::<crate::search::Search>().0.clone(),
                    self.store.runtime().clone(),
                );
                let runtime = self.store.runtime().clone();
                let editor = cx.new(|cx| {
                    let mut editor = BodyEditor::new(session_id, &body, cx);
                    editor.set_mention_search(search);
                    editor.set_runtime(runtime);
                    editor
                });
                self.load_mention_contacts(cx);
                cx.subscribe(
                    &editor,
                    |this, editor, event: &EditorEvent, cx| match event {
                        EditorEvent::Flush(json) => {
                            let session_id = editor.read(cx).session_id.clone();
                            this.persist_memo(session_id, json.clone(), cx);
                        }
                        EditorEvent::OpenMention { kind, id } => {
                            this.open_mention(kind, id.clone(), cx);
                        }
                        EditorEvent::Files(files) => {
                            let session_id = editor.read(cx).session_id.clone();
                            this.attach_files(session_id, editor, files.clone(), cx);
                        }
                        EditorEvent::Dropped(paths) => {
                            let session_id = editor.read(cx).session_id.clone();
                            this.drop_files(session_id, editor, paths.clone(), cx);
                        }
                    },
                )
                .detach();
                self.editor = Some(editor);
            }
        }
    }

    /// `/app/<type>/<id>` from a mention chip: sessions open in the current
    /// tab, people and organizations in the Contacts tab.
    fn open_mention(&mut self, kind: &str, id: String, cx: &mut Context<Self>) {
        match kind {
            "session" => self.select(id, cx),
            "human" => {
                self.pending_contact = Some(contacts_tab::Selection::Person(id));
                cx.notify();
            }
            "organization" => {
                self.pending_contact = Some(contacts_tab::Selection::Organization(id));
                cx.notify();
            }
            _ => {}
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

    /// `flushCanonicalSessionEditorChanges`: the memo editor's unsaved body
    /// for `session_id`, written now; the returned task completes with the
    /// write so a caller can sequence a snapshot read after it.
    pub(crate) fn flush_memo_editor(
        &mut self,
        session_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<tokio::task::JoinHandle<anyhow::Result<()>>> {
        let body = self
            .editor
            .as_ref()
            .filter(|editor| editor.read(cx).session_id == session_id)
            .and_then(|editor| editor.update(cx, |editor, _| editor.take_pending()))?;
        Some(self.store.update_memo(session_id.to_string(), body))
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

    /// The tray agenda row: `/app/new?calendarEventId=…&record=true` opens
    /// the event's session and starts recording.
    pub(crate) fn open_event_and_record(&mut self, event_id: String, cx: &mut Context<Self>) {
        if self.recording.live.is_some() || self.recording.starting {
            self.open_event(event_id, cx);
            return;
        }
        let task = self.store.open_event_session(event_id);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(session_id)) => {
                this.update(cx, |this, cx| {
                    this.reload_sessions(cx);
                    this.select(session_id.clone(), cx);
                    this.start_listening(session_id, cx);
                })
                .ok();
            }
            Ok(Err(error)) => tracing::error!(%error, "failed to open calendar event"),
            Err(error) => tracing::error!(%error, "failed to open calendar event"),
        })
        .detach();
    }

    /// `TrayQuitCompletely`: the confirmation before the app stops running in
    /// the background.
    pub(crate) fn confirm_quit_completely(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app_name = crate::tray::app_name(self.store.identifier()).to_string();
        // `app.dialog().message(..).title(..).buttons(OkCancelCustom(..))`: the
        // dialog plugin's native message box, kind `Info`.
        let answer = crate::dialogs::message(
            window,
            cx,
            crate::dialogs::MessageOptions {
                title: anlg_tray_core::labels::quit_completely_title(&app_name),
                description: anlg_tray_core::labels::quit_completely_message(&app_name),
                ok: anlg_tray_core::labels::QUIT_COMPLETELY_CONFIRM.to_string(),
                cancel: anlg_tray_core::labels::CANCEL.to_string(),
            },
        );
        cx.spawn(async move |_, cx| {
            if answer.await {
                cx.update(|cx| cx.quit()).ok();
            }
        })
        .detach();
    }

    /// `show_tray_icon` → `set_visible`.
    pub(super) fn sync_tray(&self, cx: &mut Context<Self>) {
        let visible = self.provider_settings.bool_setting(
            "show_tray_icon",
            &["general", "show_tray_icon"],
            true,
        );
        cx.global::<crate::tray::Tray>()
            .send(crate::tray::TrayCommand::Visible(visible));
    }

    /// `TrayScheduleSync`: the upcoming events for the tray agenda.
    fn publish_tray_schedule(&self, cx: &mut Context<Self>) {
        let ignored_events =
            workspace_ignored(&self.provider_settings, "ignored_events", "tracking_id");
        let ignored_series =
            workspace_ignored(&self.provider_settings, "ignored_recurring_series", "id");
        let events = crate::tray::schedule_events(
            &self.event_rows,
            |event| {
                ignored_events.contains(&event.tracking_id_event)
                    || (!event.recurrence_series_id.is_empty()
                        && ignored_series.contains(&event.recurrence_series_id))
            },
            Utc::now(),
            &Local,
        );
        cx.global::<crate::tray::Tray>()
            .send(crate::tray::TrayCommand::Schedule(events));
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

    fn set_menu(&mut self, menu: Option<Menu>, window: &Window, cx: &mut Context<Self>) {
        if self.open_menu != menu {
            // `rememberEditTarget` on the trigger's pointer down.
            if self.open_menu.is_none() {
                self.menu_edit_target = window.focused(cx);
            }
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
            // The drag moves the layout's share of the group; the rendered
            // width follows the flex split like the web view's.
            let fraction = crate::sidebar_layout::fraction_for(width, self.sidebar_group_width);
            let width = crate::sidebar_layout::width_for(fraction, self.sidebar_group_width);
            if width != self.sidebar_width || self.sidebar_fraction != Some(fraction) {
                self.sidebar_width = width;
                self.sidebar_fraction = Some(fraction);
                cx.notify();
            }
        }
    }

    /// The drag ended: the layout is saved where `autoSaveId` keeps it (the
    /// webview's localStorage, for the Tauri app to read back) and in the
    /// shell's own scope of `store.json`.
    fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_drag.take().is_some() {
            if let Some(fraction) = self.sidebar_fraction {
                self.save_sidebar_fraction(fraction);
            }
            cx.notify();
        }
    }

    fn save_sidebar_fraction(&self, fraction: f64) {
        if let Err(error) =
            self.store_file
                .set_scoped_f64(SIDEBAR_STORE_SCOPE, SIDEBAR_STORE_KEY, fraction)
        {
            tracing::warn!(%error, "failed to save the sidebar width");
        }
        let files =
            crate::webkit_local_storage::origin_files(self.store.path(), self.store.identifier());
        if files.is_empty() {
            return;
        }
        self.store.runtime().spawn(async move {
            let key = crate::sidebar_layout::STORAGE_KEY;
            let current = crate::webkit_local_storage::read(&files, key).await;
            let next = crate::sidebar_layout::with_layout_fraction(current.as_deref(), fraction);
            crate::webkit_local_storage::write(&files, key, &next).await;
        });
    }

    /// The saved share: the webview's `react-resizable-panels` layout first,
    /// then the shell's own copy.
    fn load_sidebar_fraction(store: &crate::db::Store, store_file: &StoreFile) -> Option<f64> {
        let files = crate::webkit_local_storage::origin_files(store.path(), store.identifier());
        let shared = if files.is_empty() {
            None
        } else {
            store
                .runtime()
                .block_on(crate::webkit_local_storage::read(
                    &files,
                    crate::sidebar_layout::STORAGE_KEY,
                ))
                .and_then(|document| crate::sidebar_layout::read_layout_fraction(&document))
        };
        shared.or_else(|| store_file.scoped_f64(SIDEBAR_STORE_SCOPE, SIDEBAR_STORE_KEY))
    }

    /// `createLeftSidebarPanelConstraints` + the panel's percentage layout on
    /// every frame: the share of the panel group (the main body: the window
    /// minus `pl-1`, less the docked chat panel), clamped to the pixel
    /// constraints; the first frame sets the 200px default when nothing was
    /// saved.
    fn sync_sidebar_width(&mut self, group: f32) {
        let group = group.max(1.0);
        self.sidebar_group_width = group;
        let fraction = *self
            .sidebar_fraction
            .get_or_insert_with(|| crate::sidebar_layout::default_fraction(group));
        if self.sidebar_drag.is_none() {
            self.sidebar_width = crate::sidebar_layout::width_for(fraction, group);
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
        self.viewport_height = f32::from(window.viewport_size().height);
        self.viewport_width = f32::from(window.viewport_size().width);
        self.begin_tooltip_frame(window, cx);
        // The menus rendered this frame record their items below.
        self.menu_runtime.borrow_mut().take();
        // `MainChatPanels` lays out the body before the sidebar group inside
        // it; the width guard then reacts to what fits.
        let main_layout = self.main_layout(window);
        self.sync_sidebar_width(main_layout.body);
        if !self.onboarding_open() {
            self.run_width_guard(main_layout, window, cx);
        }
        // `resolveIsDarkMode` on every frame: the setting or the system
        // appearance may have changed since the last one.
        if let Some(editor) = self.pending_editor_focus.take() {
            editor.update(cx, |editor, cx| editor.focus_start(window, cx));
        }
        if let Some(selection) = self.pending_contact.take() {
            self.open_contacts(window, cx);
            self.select_contact(Some(selection), window, cx);
        }
        self.prepare_contact_avatars();
        // The web view's shortcuts listen on `document`: when the focused
        // field unmounts (the chat closes, an overlay swaps out, the memo
        // editor gives way to the summary tab), GPUI would dispatch keys to
        // the bare root, past this element's bindings — the handle may still
        // be alive while its element is gone — so the workspace takes the
        // focus back whenever the focused handle is not under this element.
        // Checked once this frame is drawn: during `render` the tree is the
        // previous frame's, which would unfocus a field mounted this frame.
        let root_focus = self.focus_handle.clone();
        let menu_focus = self.menu_focus.clone();
        window.on_next_frame(move |window, cx| {
            if window.focused(cx).is_none_or(|focused| {
                focused != menu_focus && !root_focus.contains(&focused, window)
            }) {
                window.focus(&root_focus);
            }
        });
        let system_dark = cx.global::<crate::system_theme::SystemTheme>().dark(window);
        let resolved = Theme::resolve(&self.theme_preference, system_dark);
        if resolved != self.theme {
            self.theme = resolved;
            crate::dialogs::set_prefer_dark(resolved == Theme::dark());
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

        // `syncContent`'s focus rule: an external body parked while a note
        // editor had focus lands once it blurs.
        for editor in self.editor.clone().into_iter().chain(
            self.enhanced_editor
                .as_ref()
                .map(|(_, editor)| editor.clone()),
        ) {
            let focused = editor.read(cx).is_focused(window);
            editor.update(cx, |editor, cx| editor.sync_focus(focused, cx));
        }
        // `/app/instruction` sits outside the shell layout: no title bar,
        // sidebar or toasts while the browser hand-off is pending.
        if self.instruction_open() {
            return self.render_instruction(window, cx);
        }
        // `isOnboarding`: the onboarding tab takes the whole shell surface,
        // without the title bar or sidebar.
        if self.onboarding_open() {
            return div()
                .id("workspace-root")
                .size_full()
                .track_focus(&self.focus_handle)
                .text_color(theme.foreground)
                .tw_text_sm()
                // `font-sans` on `body`: without the resolved family GPUI's
                // fallback font ignores the semibold headings.
                .when_some(self.font_family.clone(), |root, family| {
                    root.font_family(family)
                })
                .child(self.render_onboarding(window, cx))
                .into_any_element();
        }

        // `ShellFrame`: title bar (Windows/Linux) above the `shell-scaffold`
        // row of sidebar + main surface, all on `bg-background`.
        let sidebar_dragging = self.sidebar_drag.is_some();
        let chat_panel_dragging = self.chat_panel_dragging();
        let section_resizing = self.section_resizing();
        let root = div()
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
                cx.listener(|this, _: &actions::SignIn, window, cx| this.sign_in(window, cx)),
            )
            .on_action(cx.listener(|this, _: &actions::UpgradeToPro, window, cx| {
                this.upgrade_to_pro(window, cx)
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
            .on_action(
                cx.listener(|this, _: &actions::CloseWindow, window, _| this.close_window(window)),
            )
            // `useCloseStandaloneNoteWindowOnEscape`
            .on_action(cx.listener(|this, _: &actions::ToggleChat, _, cx| this.toggle_chat(cx)))
            .on_action(cx.listener(|this, _: &actions::SelectAll, _, cx| {
                this.select_all_timeline_notes(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::DeleteSelected, _, cx| {
                this.request_delete_selected(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::TogglePlayback, _, cx| {
                if !this.toggle_transcript_playback(cx) {
                    cx.propagate();
                }
            }))
            // `mod+f` / `mod+h` of the note's `SearchProvider`; on the Contacts
            // tab `mod+f` focuses and selects the sidebar's search field instead
            // (`sidebar/contacts.tsx`), and the other overlay tabs bind nothing.
            .on_action(cx.listener(|this, _: &actions::FocusSearch, window, cx| {
                if this.contacts_open() {
                    this.focus_contacts_search(window, cx);
                } else if !this.overlay_tab_open() {
                    this.toggle_note_search(false, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &actions::ToggleReplace, window, cx| {
                if !this.overlay_tab_open() {
                    this.toggle_note_search(true, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &actions::Escape, window, cx| {
                if this.close_open_menus(cx) {
                    return;
                }
                // `useHotkeys("esc")` in edit mode drops the section selection,
                // and the `SearchProvider`'s closes the find bar.
                if this.clear_transcript_selection(cx) || this.close_note_search(Some(cx)) {
                    return;
                }
                if !this.pending_delete_selected.is_empty() {
                    this.pending_delete_selected.clear();
                    cx.notify();
                } else if this.timeline_menu.is_some() {
                    this.timeline_menu = None;
                    cx.notify();
                } else if this.chat_mode == chat::ChatMode::FloatingOpen {
                    // `esc` closes the floating panel only (`isVisible`).
                    this.close_chat(cx);
                    window.focus(&this.focus_handle);
                } else if this.share_popover_open() {
                    this.close_share_popover(cx);
                } else if this.export_dialog.is_some() {
                    this.close_export_dialog(cx);
                } else if this.badge_dialog_open() {
                    this.close_badge_dialog(cx);
                } else if this.folder_dialog_open() {
                    this.close_folder_dialogs(cx);
                } else if this.calendar_popover_open() {
                    this.close_calendar_popover(cx);
                } else if this.icon_picker_open() {
                    this.close_icon_picker(window, cx);
                } else if this.template_picker_open() {
                    this.close_template_picker(window, cx);
                } else if this.folder_picker_open() {
                    this.close_folder_picker(window, cx);
                } else if this.overlay_tab_open() {
                    // `useMainEscapeShortcutAction` → `leaveOverlayTab`.
                    this.leave_overlay_tab(window, cx);
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
            // An editable's right mouse down (which ran first) asked for the
            // webview's editing context menu.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                    if let Some(request) = crate::edit_menu::take(cx) {
                        this.edit_context_menu = Some(request);
                        cx.notify();
                    }
                }),
            )
            // `shouldClearTimelineSelectionOnPointerDown`: any press outside the
            // timeline root, its dialog, or its (OS-level in Tauri) context menu
            // drops the selection, whatever element claims the pointer.
            .capture_any_mouse_down(cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                this.dismiss_tooltip(cx);
            }))
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                this.tooltip_key_down(event, cx);
            }))
            .on_modifiers_changed(cx.listener(
                |this, event: &gpui::ModifiersChangedEvent, _, cx| {
                    this.tooltip_modifiers_changed(event, cx);
                },
            ))
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                if this.timeline_menu.is_some()
                    || !this.pending_delete_selected.is_empty()
                    || this.list_state.viewport_bounds().contains(&event.position)
                {
                    return;
                }
                this.clear_timeline_selection(cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .tw_text_sm()
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .when(sidebar_dragging, |root| {
                root.cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_sidebar_drag(event.position.x, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_sidebar_drag(cx)),
                    )
            })
            .when(chat_panel_dragging, |root| {
                root.cursor_col_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_chat_panel_drag(event.position.x, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_chat_panel_drag(cx)),
                    )
            })
            .when(section_resizing, |root| {
                root.cursor_ns_resize()
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_section_resize(event.position.y, cx);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_section_resize(cx)),
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
                        } else if self.automations_open() {
                            self.render_automations_sidebar(cx).into_any_element()
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
            .children(self.render_tooltip(window))
            .children(self.render_filter_menu(window, cx))
            .children(self.render_audio_player_menu(window, cx))
            .children(self.render_calendar_context_menu(window, cx))
            .children(self.render_automations_context_menu(window, cx))
            .children(self.render_timeline_context_menu(window, cx))
            .children(self.render_mention_popup(window, cx))
            .children(self.render_composer_mention_popup(window, cx))
            .children(self.render_format_toolbar(window, cx))
            .children(self.render_delete_selected_dialog(cx))
            .children(self.render_open_menu(window, cx))
            .children(self.render_edit_context_menu(cx))
            .children(self.render_export_dialog(window, cx))
            .children(self.render_badge_dialog(window, cx))
            .children(self.render_folder_dialogs(window, cx))
            .children(
                self.render_recording_toast(cx)
                    .map(|toast| gpui::deferred(toast).with_priority(10)),
            )
            .children(
                self.render_flash_toast(cx)
                    .map(|toast| gpui::deferred(toast).with_priority(11)),
            )
            .children(self.render_open_note_dialog(window, cx))
            .into_any_element();
        self.sync_menu_focus(window, cx);
        root
    }
}

/// `window.hide()`: withdrawn on X11 so it leaves the taskbar like Tauri's
/// GTK window; gpui has no hide, so elsewhere the window iconifies.
pub(crate) fn hide_window(window: &mut Window) {
    #[cfg(target_os = "linux")]
    {
        let size = window.viewport_size();
        let scale = window.scale_factor();
        if crate::x11::withdraw(
            (f32::from(size.width) * scale).round() as u32,
            (f32::from(size.height) * scale).round() as u32,
        ) {
            return;
        }
    }
    window.minimize_window();
}

/// `window.show()` + `set_focus()`: maps a hidden window again and raises
/// it.
pub(crate) fn show_window(window: &mut Window) {
    #[cfg(target_os = "linux")]
    crate::x11::map_withdrawn();
    window.activate_window();
}
