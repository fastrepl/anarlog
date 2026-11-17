use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use desktop_runtime::{
    CancellationToken, Generation, LibraryPage, LibraryQuery, Reply, RuntimeHandle, SessionId,
};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    AnyView, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    Subscription, Window,
};

use super::{
    automations::view::{AutomationsEvent, AutomationsView},
    calendar::{CalendarFolders, CalendarOpen, CalendarView, Clock},
    calendar_sidebar::{CalendarSidebar, CalendarSidebarEvent},
    contacts::{Contacts, ContactsEvent, Selection},
    folders_view::{FoldersEvent, FoldersView},
    library::{LibraryView, OpenNote},
    navigation::{Navigation, Route, SidebarState, SlotId},
    open_note::{NoteEvent, NoteView},
    picker::{NotePicker, PickerEvent},
    templates_view::{TemplatesEvent, TemplatesView},
};
use crate::{
    chat::{ChatView, state::ChatEvent},
    contracts::{LaneContext, MeetingIntent, ProductRoute, WorkspaceEvent},
};

#[derive(Clone)]
pub enum WorkspaceAction {
    SidebarChanged,
    OpenShared(Arc<str>),
    NewNoteWindow,
    OpenNoteWindow(SessionId),
    PinnedChanged(Arc<[Route]>),
    /// The note menu's "Open floating panel".
    OpenFloatingPanel,
}

#[derive(Clone)]
pub(super) enum Navigate {
    Open(Route, bool),
    Select(SlotId),
    History(bool),
}

pub struct WorkspaceView {
    pub(super) runtime: RuntimeHandle,
    pub(super) focus: FocusHandle,
    pub(super) library: Entity<LibraryView>,
    pub(super) note: Entity<NoteView>,
    pub(super) picker: Entity<NotePicker>,
    pub(super) calendar: Entity<CalendarView>,
    pub(super) calendar_sidebar: Entity<CalendarSidebar>,
    calendar_sync: Entity<super::calendar_accounts::CalendarSync>,
    pub(super) templates: Entity<TemplatesView>,
    pub(super) folders: Entity<FoldersView>,
    pub(super) contacts: Option<Entity<Contacts>>,
    pub(super) chat: Entity<ChatView>,
    pub(super) automations: Entity<AutomationsView>,
    pub(super) navigation: Navigation,
    pub(super) sidebar: SidebarState,
    pub(super) query: LibraryQuery,
    pub(super) page: Option<Arc<LibraryPage>>,
    generation: Generation,
    cancellation: CancellationToken,
    watch_cancel: CancellationToken,
    events_cancel: CancellationToken,
    events_loaded: Option<std::time::Instant>,
    creating: bool,
    ready: bool,
    pub(super) picker_open: bool,
    pub(super) sort_menu: Option<super::sidebar::SortMenu>,
    pub(super) return_focus: Option<FocusHandle>,
    pub(super) resizing: bool,
    pending: Option<Navigate>,
    pub(super) titles: HashMap<SessionId, Arc<str>>,
    pub(super) title_order: VecDeque<SessionId>,
    pub(super) dirty: HashSet<SessionId>,
    pub(super) recording: HashSet<SessionId>,
    viewer: Option<Arc<str>>,
    auto_start: Option<SessionId>,
    pub(super) route_content: Option<(Route, AnyView)>,
    pub(super) pin_persistence: bool,
    pub(super) deletions: Vec<super::undo_delete::Pending>,
    /// Shipping asks before deleting a multi-selection.
    pub(super) confirm_delete: Option<Arc<[SessionId]>>,
    automation_client: Option<super::automation_runner::AutomationClient>,
    pub(super) edit_review: Option<Entity<super::edit_review::EditReview>>,
    subscriptions: Vec<Subscription>,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceView {}
impl EventEmitter<WorkspaceAction> for WorkspaceView {}

impl Focusable for WorkspaceView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WorkspaceView {
    pub fn set_timeline_metadata(
        &mut self,
        show_folder: bool,
        show_tags: bool,
        cx: &mut Context<Self>,
    ) {
        self.library.update(cx, |library, cx| {
            library.set_metadata(show_folder, show_tags, cx)
        });
    }

    pub fn set_floating_bar_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.note
            .update(cx, |note, cx| note.set_floating_bar_enabled(enabled, cx));
    }

    pub fn set_timeline_clock(
        &mut self,
        use_24_hour_time: bool,
        timezone: Option<chrono_tz::Tz>,
        cx: &mut Context<Self>,
    ) {
        self.library.update(cx, |library, cx| {
            library.set_clock(use_24_hour_time, timezone, cx)
        });
        self.note.update(cx, |note, cx| {
            note.set_clock(
                super::open_note::Clock {
                    use_24_hour_time,
                    timezone,
                },
                cx,
            )
        });
        self.calendar.update(cx, |calendar, cx| {
            calendar.set_clock(
                Clock {
                    use_24_hour_time,
                    timezone,
                },
                cx,
            )
        });
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar.toggle();
        self.sort_menu = None;
        cx.notify();
    }

    pub fn sidebar_expanded(&self) -> bool {
        self.sidebar.expanded
    }

    pub fn run_automations(
        &mut self,
        trigger: super::automation_runner::Trigger,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        let runtime = self.runtime.clone();
        let client = self.automation_client.clone();
        let starters = super::automation_runner::run_starters(
            &runtime,
            trigger,
            session.clone(),
            client.clone(),
        );
        let reply =
            super::automation_runner::matching_workflows(&runtime, trigger, session.clone());
        cx.spawn(async move |this, cx| {
            let result = async {
                // Like shipping, a failing starter does not stop the workflows.
                let mut messages = match starters {
                    Ok(reply) => reply
                        .receive()
                        .await
                        .unwrap_or_else(|error| vec![error.to_string()]),
                    Err(error) => vec![error.to_string()],
                };
                let workflows = reply?.receive().await?;
                for id in workflows {
                    let result = super::automation_runner::run(
                        &runtime,
                        id,
                        session.clone(),
                        client.clone(),
                    )?
                    .receive()
                    .await;
                    messages.push(match result {
                        Ok(detail) => detail,
                        Err(error) => error.to_string(),
                    });
                }
                Ok::<_, desktop_runtime::ServiceError>(messages.join(" · "))
            }
            .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(message) if !message.is_empty() => crate::application::toasts::info(message, cx),
                Err(error) => this.set_status(error.to_string(), cx),
                _ => {}
            });
        })
        .detach();
    }
    /// The host's calendar account services, once native services are ready.
    pub fn set_calendar_accounts(
        &mut self,
        accounts: Option<Arc<dyn super::calendar_accounts::CalendarAccounts>>,
        cx: &mut Context<Self>,
    ) {
        self.calendar_sync
            .update(cx, |sync, cx| sync.set_services(accounts, cx));
    }

    /// Refreshes connected calendars unless a refresh is already running.
    pub fn sync_calendars(&mut self, cx: &mut Context<Self>) {
        self.calendar_sync.update(cx, |sync, cx| sync.schedule(cx));
    }

    pub fn set_automation_client(
        &mut self,
        client: Option<super::automation_runner::AutomationClient>,
        cx: &mut Context<Self>,
    ) {
        self.automation_client = client.clone();
        self.automations
            .update(cx, |automations, cx| automations.set_client(client, cx));
    }

    /// An integration callback arrived; connected accounts may have changed.
    pub fn integrations_changed(&mut self, cx: &mut Context<Self>) {
        self.automations
            .update(cx, |automations, cx| automations.refresh_connections(cx));
    }
    pub fn new(
        context: LaneContext,
        ready: Reply<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let library = cx.new(LibraryView::new);
        let note = cx.new(|cx| NoteView::new(context.runtime.clone(), window, cx));
        let picker = cx.new(|cx| NotePicker::new(context.runtime.clone(), cx));
        let calendar_sync = cx.new(|_| super::calendar_accounts::CalendarSync::new());
        let calendar =
            cx.new(|cx| CalendarView::new(context.runtime.clone(), calendar_sync.clone(), cx));
        let calendar_sidebar =
            cx.new(|cx| CalendarSidebar::new(context.runtime.clone(), calendar_sync.clone(), cx));
        let templates = cx.new(|cx| TemplatesView::new(context.runtime.clone(), cx));
        let folders = cx.new(|cx| FoldersView::new(context.runtime.clone(), cx));
        let templates_subscription = cx.subscribe(&templates, |this, _, event, cx| match event {
            TemplatesEvent::Back => this.leave_overlay(cx),
            TemplatesEvent::Product(route) => cx.emit(WorkspaceEvent::Product(route.clone())),
            TemplatesEvent::Navigate(route) => {
                this.navigate(Navigate::Open(route.clone(), false), cx)
            }
        });
        let automations = cx.new(|cx| AutomationsView::new(context.runtime.clone(), cx));
        let automations_subscription =
            cx.subscribe(&automations, |this, _, event, cx| match event {
                AutomationsEvent::Back => this.leave_overlay(cx),
                AutomationsEvent::Product(route) => cx.emit(WorkspaceEvent::Product(route.clone())),
                AutomationsEvent::Navigate(route) => {
                    this.navigate(Navigate::Open(route.clone(), false), cx)
                }
            });
        let folders_subscription = cx.subscribe(&folders, |this, _, event, cx| match event {
            FoldersEvent::Back => this.leave_overlay(cx),
            FoldersEvent::SignIn => cx.emit(WorkspaceEvent::Product(ProductRoute::Account)),
            FoldersEvent::Navigate(route) => {
                this.navigate(Navigate::Open(route.clone(), false), cx)
            }
        });
        let workspace = cx.weak_entity();
        let chat = cx.new(|cx| ChatView::new(context.runtime.clone(), workspace, cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let open_subscription = cx.subscribe(&library, |this, _, event: &OpenNote, cx| {
            this.handle_note(event, cx)
        });
        let forward_subscription = cx.subscribe(&note, |_, _, event: &WorkspaceEvent, cx| {
            cx.emit(event.clone())
        });
        let note_subscription = cx.subscribe(&note, |this, _, event: &NoteEvent, cx| match event {
            NoteEvent::Opened(session) => {
                this.cache_title(session.summary.id.clone(), session.summary.title.clone());
                this.reload(cx);
                if let Some(intent) = this.pending.take() {
                    this.apply_navigation(intent, cx);
                }
                this.picker.update(cx, |picker, cx| {
                    picker.remember(session.summary.id.clone(), cx)
                });
                if this.auto_start.as_ref() == Some(&session.summary.id) {
                    this.auto_start = None;
                    cx.emit(WorkspaceEvent::Meeting(MeetingIntent::Start {
                        session_id: session.summary.id.clone(),
                    }));
                }
                cx.notify();
            }
            NoteEvent::TitleUpdated(session) => {
                this.cache_title(session.summary.id.clone(), session.summary.title.clone());
                this.reload(cx);
                if let Some(intent) = this.pending.take() {
                    this.navigate(intent, cx);
                }
                cx.notify();
            }
            NoteEvent::RenameFailed => this.pending = None,
            NoteEvent::Request(request) => this.handle_note(request, cx),
            NoteEvent::OpenTemplates => this.navigate(Navigate::Open(Route::Templates, false), cx),
            NoteEvent::Open(route) => this.open_route(route.clone(), true, cx),
            NoteEvent::OpenFloatingPanel => cx.emit(WorkspaceAction::OpenFloatingPanel),
            NoteEvent::OpenReview(id) => {
                this.navigate(Navigate::Open(Route::Edit(id.clone()), true), cx)
            }
            NoteEvent::Failed => {
                this.pending = None;
                this.set_status(
                    "Could not open the destination. The current tab is unchanged.".into(),
                    cx,
                );
            }
        });
        let picker_subscription = cx.subscribe_in(
            &picker,
            window,
            |this, _, event: &PickerEvent, window, cx| {
                this.picker_open = false;
                if let Some(focus) = this.return_focus.take() {
                    focus.focus(window, cx);
                }
                if let PickerEvent::Open(route, new_tab) = event {
                    this.navigate(Navigate::Open(route.clone(), *new_tab), cx);
                }
                cx.notify();
            },
        );
        let calendar_subscription = cx.subscribe(&calendar, |this, _, event: &CalendarOpen, cx| {
            this.open_session(event.0.clone(), cx);
        });
        let calendar_note_subscription = cx
            .subscribe(&calendar, |this, _, event: &OpenNote, cx| {
                this.handle_note(event, cx)
            });
        let calendar_folders_subscription = cx
            .subscribe(&calendar, |this, _, _: &CalendarFolders, cx| {
                this.open_route(Route::Folders, true, cx)
            });
        let calendar_sidebar_subscription = cx.subscribe(
            &calendar_sidebar,
            |this, _, event: &CalendarSidebarEvent, cx| match event {
                CalendarSidebarEvent::Back => this.leave_overlay(cx),
                CalendarSidebarEvent::Connect => {
                    if this.can_navigate(cx) {
                        cx.emit(WorkspaceEvent::Product(ProductRoute::Calendar));
                    }
                }
            },
        );
        cx.spawn(async move |this, cx| {
            let result = ready.receive().await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.ready = true;
                    this.picker.update(cx, |picker, cx| picker.load(cx));
                    this.reload(cx);
                    this.watch(cx);
                    this.watch_events(cx);
                    this.tick(cx);
                }
                Err(error) => {
                    this.set_status(format!("Could not open the isolated library: {error}"), cx)
                }
            });
        })
        .detach();
        let mut navigation = Navigation::default();
        navigation.open(Route::Empty, true, false);
        Self {
            runtime: context.runtime,
            focus,
            library,
            note,
            picker,
            calendar,
            calendar_sidebar,
            calendar_sync,
            templates,
            folders,
            contacts: None,
            chat,
            automations,
            navigation,
            sidebar: SidebarState::default(),
            // Shipping's timeline lists the whole library; the runtime orders it.
            query: LibraryQuery {
                limit: u32::MAX,
                ..LibraryQuery::default()
            },
            page: None,
            generation: Generation::default(),
            cancellation: CancellationToken::new(),
            watch_cancel: CancellationToken::new(),
            events_cancel: CancellationToken::new(),
            events_loaded: None,
            creating: false,
            ready: false,
            picker_open: false,
            sort_menu: None,
            return_focus: None,
            resizing: false,
            pending: None,
            titles: HashMap::new(),
            title_order: VecDeque::new(),
            dirty: HashSet::new(),
            recording: HashSet::new(),
            viewer: None,
            auto_start: None,
            route_content: None,
            pin_persistence: false,
            deletions: Vec::new(),
            confirm_delete: None,
            edit_review: None,
            automation_client: None,
            subscriptions: vec![
                open_subscription,
                forward_subscription,
                note_subscription,
                picker_subscription,
                calendar_subscription,
                calendar_sidebar_subscription,
                calendar_folders_subscription,
                calendar_note_subscription,
                templates_subscription,
                automations_subscription,
                folders_subscription,
            ],
        }
    }

    /// Workspace errors appear as shipping's error toasts.
    pub fn set_status(&mut self, message: String, cx: &mut Context<Self>) {
        crate::application::toasts::error(message, cx);
    }

    fn handle_note(&mut self, event: &OpenNote, cx: &mut Context<Self>) {
        let Some(authenticated) = super::note_lock::guard(event, cx) else {
            return self.handle_unlocked_note(event, cx);
        };
        let event = event.clone();
        cx.spawn(async move |this, cx| {
            if authenticated.await {
                let _ = this.update(cx, |this, cx| this.handle_unlocked_note(&event, cx));
            }
        })
        .detach();
    }

    fn handle_unlocked_note(&mut self, event: &OpenNote, cx: &mut Context<Self>) {
        match event {
            OpenNote::Current(id) => self.open_session(id.clone(), cx),
            OpenNote::Calendar => self.navigate(Navigate::Open(Route::Calendar, true), cx),
            OpenNote::StopListening => cx.emit(WorkspaceEvent::Meeting(MeetingIntent::Stop)),
            OpenNote::Event(event) => {
                if !self.can_navigate(cx) || self.library.read(cx).opening() {
                    return;
                }
                let reply =
                    super::calendar::open_event(&self.runtime, event.clone(), self.viewer.clone());
                // Like shipping, the event shows a spinner until its note opens.
                self.library.update(cx, |library, cx| {
                    library.set_opening(Some(event.clone()), cx)
                });
                cx.spawn(async move |this, cx| {
                    let result = match reply {
                        Ok(reply) => reply.receive().await,
                        Err(error) => Err(error),
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.library
                            .update(cx, |library, cx| library.set_opening(None, cx));
                        match result {
                            Ok(id) => this.open_session(id, cx),
                            Err(error) => {
                                this.set_status(format!("Could not open event: {error}"), cx)
                            }
                        }
                    });
                })
                .detach();
            }
            OpenNote::Window(id) => {
                if self.can_navigate(cx) {
                    cx.emit(WorkspaceAction::OpenNoteWindow(id.clone()));
                }
            }
            OpenNote::Ignore(kind, id) => {
                let reply = super::timeline::ignore(&self.runtime, *kind, id.clone());
                cx.spawn(async move |this, cx| {
                    if let Err(error) = async { reply?.receive().await }.await {
                        let _ = this.update(cx, |this, cx| {
                            this.set_status(format!("Could not delete event: {error}"), cx);
                        });
                    }
                })
                .detach();
            }
            OpenNote::Reveal(id) => {
                let reply = super::notes::directory(&self.runtime, id.clone());
                let job = cx.background_executor().spawn(async move {
                    let path = reply?.receive().await?;
                    open::that(path).map_err(super::mutations::failure)
                });
                cx.spawn(async move |this, cx| {
                    if let Err(error) = job.await {
                        let _ = this.update(cx, |this, cx| {
                            this.set_status(format!("Could not show note folder: {error}"), cx);
                        });
                    }
                })
                .detach();
            }
            OpenNote::Delete(ids) => self.delete_notes(ids, false, cx),
            OpenNote::DeleteSelected(ids) => self.request_delete_selected(ids.clone(), cx),
            OpenNote::Move {
                ids,
                folder,
                create,
            } => self.move_notes(ids.clone(), folder.clone(), *create, cx),
        }
    }

    /// The folder picker's choice: files the notes at once, like shipping's `updateSession`.
    fn move_notes(
        &mut self,
        ids: Arc<[SessionId]>,
        folder: Arc<str>,
        create: bool,
        cx: &mut Context<Self>,
    ) {
        // Filing moves the note's folder on disk, which a live recording is writing into.
        if !self.can_navigate(cx) {
            return;
        }
        if ids.iter().any(|id| self.recording.contains(id)) {
            self.set_status("Stop the recording before moving this note.".into(), cx);
            return;
        }
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let result = super::folder_picker::move_notes(&runtime, ids, folder, create).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.reload(cx),
                    Err(error) => this.set_status(format!("Could not move the note: {error}"), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn chat(&self) -> &Entity<ChatView> {
        &self.chat
    }

    /// Whether the open note shows a summary with content, for chat's rewrite suggestions.
    pub fn editable_summary_shown(&self, cx: &gpui::App) -> bool {
        self.note.read(cx).editable_summary_shown()
    }

    /// Closes every tab showing `route`, like shipping's `closeProposalReviewTab`.
    pub fn close_route(&mut self, route: &Route, cx: &mut Context<Self>) {
        let slots = self
            .navigation
            .tabs
            .iter()
            .filter(|tab| &tab.route == route)
            .map(|tab| tab.slot)
            .collect::<Vec<_>>();
        if slots.is_empty() {
            return;
        }
        for slot in slots {
            self.navigation.close(slot);
        }
        if self.navigation.tabs.is_empty() {
            self.navigation.open(Route::Empty, true, false);
        }
        // The tab now shown may be another note than the one last opened.
        if let Some(Route::Session(id)) = self.navigation.current().map(|tab| tab.route.clone())
            && self.note.read(cx).session_id() != Some(&id)
        {
            self.note.update(cx, |note, cx| note.open(id, cx));
        }
        self.sync_route(cx);
        cx.notify();
    }

    pub fn shown_summary(&self, cx: &gpui::App) -> Option<Arc<str>> {
        self.note.read(cx).shown_summary()
    }

    pub fn current_route(&self) -> Route {
        self.navigation
            .current()
            .map(|tab| tab.route.clone())
            .unwrap_or(Route::Empty)
    }

    pub fn open_native_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.navigate(Navigate::Open(route, false), cx);
    }

    pub fn set_editor_content(
        &mut self,
        session_id: &SessionId,
        content: AnyView,
        cx: &mut Context<Self>,
    ) {
        self.note
            .update(cx, |note, cx| note.set_content(session_id, content, cx));
    }

    pub fn set_route_content(&mut self, route: Route, content: AnyView, cx: &mut Context<Self>) {
        if self
            .navigation
            .current()
            .is_some_and(|tab| tab.route == route)
        {
            self.route_content = Some((route, content));
            cx.notify();
        }
    }

    pub fn set_session_dirty(&mut self, id: SessionId, dirty: bool, cx: &mut Context<Self>) {
        if dirty && self.pending.is_some() {
            self.pending = None;
            self.note.update(cx, |note, _| note.cancel_open());
            self.set_status(
                "Navigation was cancelled to preserve newer edits.".into(),
                cx,
            );
        }
        if dirty {
            self.dirty.insert(id);
        } else {
            self.dirty.remove(&id);
        }
        cx.notify();
    }

    pub fn set_recording(&mut self, id: SessionId, active: bool, cx: &mut Context<Self>) {
        self.library.update(cx, |library, cx| {
            library.set_recording(id.clone(), active, cx)
        });
        if active {
            self.recording.insert(id);
        } else {
            self.recording.remove(&id);
        }
        cx.notify();
    }

    pub fn set_viewer(&mut self, viewer: Option<Arc<str>>, cx: &mut Context<Self>) {
        if self.viewer == viewer {
            return;
        }
        self.viewer = viewer.clone();
        self.set_automation_client(None, cx);
        self.picker
            .update(cx, |picker, cx| picker.set_viewer(viewer.clone(), cx));
        self.calendar
            .update(cx, |calendar, _| calendar.viewer = viewer.clone());
        self.calendar_sidebar.update(cx, |sidebar, cx| {
            sidebar.set_signed_in(viewer.is_some(), cx)
        });
        self.templates.update(cx, |templates, cx| {
            templates.set_signed_in(viewer.is_some(), cx)
        });
        self.folders.update(cx, |folders, cx| {
            folders.set_signed_in(viewer.is_some(), cx)
        });
        if let Some(contacts) = &self.contacts {
            contacts.update(cx, |contacts, cx| contacts.set_viewer(viewer.clone(), cx));
        }
    }

    pub fn set_calendar_week_start(&mut self, week_start: u8, cx: &mut Context<Self>) {
        self.calendar
            .update(cx, |calendar, cx| calendar.set_week_start(week_start, cx));
    }

    pub fn set_pin_persistence(&mut self, enabled: bool) {
        self.pin_persistence = enabled;
    }

    fn cache_title(&mut self, id: SessionId, title: Arc<str>) {
        self.title_order.retain(|cached| cached != &id);
        self.title_order.push_back(id.clone());
        self.titles.insert(id, title);
        if self.title_order.len() > 512
            && let Some(oldest) = self.title_order.pop_front()
        {
            self.titles.remove(&oldest);
        }
    }

    pub fn open_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        self.navigate(Navigate::Open(Route::Session(id), false), cx);
    }

    pub fn open_route(&mut self, route: Route, new_slot: bool, cx: &mut Context<Self>) {
        self.navigate(Navigate::Open(route, new_slot), cx);
    }

    pub fn restore_pinned(&mut self, routes: &[Route], cx: &mut Context<Self>) {
        for route in routes.iter().filter(|route| route.persistent_pin()) {
            let slot = self.navigation.open(route.clone(), true, false);
            self.navigation.pin(slot, true);
        }
        self.navigation.active = self
            .navigation
            .tabs
            .iter()
            .find(|tab| tab.route == Route::Empty)
            .map(|tab| tab.slot);
        cx.notify();
    }

    pub fn invalidate(&mut self, route: &Route, cx: &mut Context<Self>) {
        if let Route::Session(id) = route
            && (self.dirty.contains(id) || self.note.read(cx).has_unsaved_title(cx))
        {
            self.set_status("The note was removed remotely. Unsaved local work is retained; resolve it before leaving.".into(), cx);
            return;
        }
        self.navigation.invalidate(route);
        self.sync_route(cx);
    }

    pub fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.recording.is_empty() {
            self.set_status(
                "Stop recording and finish saving before closing the window.".into(),
                cx,
            );
            return false;
        }
        self.can_navigate(cx)
    }

    fn can_navigate(&mut self, cx: &mut Context<Self>) -> bool {
        if self.creating || self.note.read(cx).has_unsaved_title(cx) || !self.dirty.is_empty() {
            self.set_status(
                "Save or restore unsaved edits before closing or changing tabs.".into(),
                cx,
            );
            false
        } else {
            true
        }
    }

    fn destination(&self, intent: &Navigate) -> Option<Route> {
        match intent {
            Navigate::Open(route, _) => Some(route.clone()),
            Navigate::Select(slot) => self
                .navigation
                .tabs
                .iter()
                .find(|tab| tab.slot == *slot)
                .map(|tab| tab.route.clone()),
            Navigate::History(forward) => self.navigation.history_target(*forward),
        }
    }

    pub(super) fn navigate(&mut self, intent: Navigate, cx: &mut Context<Self>) {
        if self.note.read(cx).has_unsaved_title(cx) {
            self.pending = Some(intent);
            self.note.update(cx, |note, cx| note.rename(cx));
            return;
        }
        if !self.can_navigate(cx) {
            return;
        }
        let Some(route) = self.destination(&intent) else {
            return;
        };
        self.note.update(cx, |note, _| note.cancel_open());
        if matches!(&route, Route::Session(_))
            && self
                .navigation
                .current()
                .is_some_and(|tab| tab.route == route)
        {
            self.pending = None;
            self.apply_navigation(intent, cx);
            return;
        }
        if let Route::Session(id) = &route {
            self.pending = Some(intent);
            self.note.update(cx, |note, cx| note.open(id.clone(), cx));
        } else {
            self.pending = None;
            self.apply_navigation(intent, cx);
        }
    }

    fn apply_navigation(&mut self, intent: Navigate, cx: &mut Context<Self>) {
        match intent {
            Navigate::Open(route, new_slot) => {
                let protect = self.navigation.current().is_some_and(
                    |tab| matches!(&tab.route, Route::Session(id) if self.recording.contains(id)),
                );
                self.navigation.open(route, new_slot, protect);
            }
            Navigate::Select(slot) => {
                self.navigation.select(slot);
            }
            Navigate::History(forward) => {
                self.navigation.travel(forward);
            }
        }
        self.sync_route(cx);
    }

    pub(super) fn sync_route(&mut self, cx: &mut Context<Self>) {
        let route = self
            .navigation
            .current()
            .map(|tab| tab.route.clone())
            .unwrap_or(Route::Empty);
        self.sidebar.set_route(&route);
        self.chat
            .update(cx, |chat, cx| chat.set_route(route.clone(), cx));
        self.library.update(cx, |library, cx| {
            library.set_active(
                match &route {
                    Route::Session(id) => Some(id.clone()),
                    _ => None,
                },
                cx,
            )
        });
        self.route_content = None;
        // Leaving an edit review unmounts it, as in shipping.
        if !matches!(&route, Route::Edit(id) if self.edit_review.as_ref().is_some_and(|review| review.read(cx).id == *id))
        {
            self.edit_review = None;
        }
        let calendar_route = route == Route::Calendar;
        self.calendar.update(cx, |calendar, cx| {
            if calendar_route {
                calendar.activate(cx);
            } else {
                calendar.suspend();
            }
        });
        self.calendar_sidebar.update(cx, |sidebar, cx| {
            if calendar_route {
                sidebar.activate(cx);
            } else {
                sidebar.suspend();
            }
        });
        let templates_route = route == Route::Templates;
        self.templates.update(cx, |templates, cx| {
            if templates_route {
                templates.activate(cx);
            } else {
                templates.suspend(cx);
            }
        });
        let folders_route = matches!(route, Route::Folders | Route::Folder(_));
        self.folders.update(cx, |folders, cx| {
            if folders_route {
                folders.activate(cx);
            } else {
                folders.suspend(cx);
            }
        });
        self.sync_contacts(&route, cx);
        let automations_route = route == Route::Automations;
        self.automations.update(cx, |automations, cx| {
            if automations_route {
                automations.activate(cx);
            } else {
                automations.suspend(cx);
            }
        });
        if let Route::Folder(id) = &route {
            self.folders
                .update(cx, |folders, cx| folders.select_id(id.clone(), cx));
        }
        match &route {
            Route::Settings(section) => cx.emit(WorkspaceEvent::Product(match section.as_ref() {
                "billing" => ProductRoute::Billing,
                "permissions" => ProductRoute::Permissions,
                "sync" => ProductRoute::CloudSync,
                "imports" => ProductRoute::Import,
                _ => ProductRoute::Settings,
            })),
            Route::Onboarding => cx.emit(WorkspaceEvent::Product(ProductRoute::Onboarding)),
            Route::SharedSession(id) => cx.emit(WorkspaceAction::OpenShared(id.clone())),
            _ => {}
        }
        self.titles.retain(|id, _| {
            self.navigation
                .tabs
                .iter()
                .any(|tab| tab.route == Route::Session(id.clone()))
        });
        cx.notify();
    }

    /// Contacts, people and organizations share shipping's contacts tab.
    fn sync_contacts(&mut self, route: &Route, cx: &mut Context<Self>) {
        let selection = match route {
            Route::Contacts => None,
            Route::Human(id) => Some(Selection::Person(id.to_string().into())),
            Route::Organization(id) => Some(Selection::Organization(id.to_string().into())),
            _ => {
                if let Some(contacts) = &self.contacts {
                    contacts.update(cx, |contacts, _| contacts.suspend());
                }
                return;
            }
        };
        let contacts = match &self.contacts {
            Some(contacts) => contacts.clone(),
            None => {
                let contacts = cx.new(|cx| Contacts::new(self.runtime.clone(), cx));
                contacts.update(cx, |contacts, cx| {
                    contacts.set_viewer(self.viewer.clone(), cx)
                });
                self.subscriptions.push(cx.subscribe(
                    &contacts,
                    |this, _, event: &ContactsEvent, cx| match event {
                        ContactsEvent::Back => this.leave_overlay(cx),
                        ContactsEvent::OpenNote(id) => this.open_session(id.clone(), cx),
                    },
                ));
                self.contacts = Some(contacts.clone());
                contacts
            }
        };
        contacts.update(cx, |contacts, cx| {
            contacts.activate(cx);
            if selection.is_some() {
                contacts.select(selection, cx);
            }
        });
    }

    fn watch(&mut self, cx: &mut Context<Self>) {
        let reply = self.runtime.watch_library();
        let cancel = self.watch_cancel.clone();
        cx.spawn(async move |this, cx| {
            let mut watch = match reply {
                Ok(reply) => match reply.receive().await {
                    Ok(watch) => watch,
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| this.set_status(error.to_string(), cx));
                        return;
                    }
                },
                Err(error) => {
                    let _ = this.update(cx, |this, cx| this.set_status(error.to_string(), cx));
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                watch.snapshots.borrow_and_update();
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if this
                    .update(cx, |this, cx| {
                        if let Some(error) = error {
                            this.set_status(format!("Library watch failed: {error}"), cx);
                        } else {
                            this.reload(cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
                if watch.terminal_error().is_some() {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    fn reload_events(&mut self, cx: &mut Context<Self>) {
        self.events_cancel.cancel();
        self.events_cancel = CancellationToken::new();
        self.events_loaded = Some(std::time::Instant::now());
        let reply = super::timeline::load_events(&self.runtime, self.events_cancel.clone());
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| match result {
                Ok(events) => this
                    .library
                    .update(cx, |library, cx| library.set_events(events, cx)),
                Err(desktop_runtime::ServiceError::Cancelled) => {}
                Err(error) => this.set_status(format!("Calendar events failed: {error}"), cx),
            });
        })
        .detach();
    }

    /// Reloads timeline events whenever events, their notes or ignored events change.
    fn watch_events(&mut self, cx: &mut Context<Self>) {
        let reply = super::timeline::watch_events(&self.runtime);
        let cancel = self.watch_cancel.clone();
        cx.spawn(async move |this, cx| {
            let mut watch = match reply {
                Ok(reply) => match reply.receive().await {
                    Ok(watch) => watch,
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| this.set_status(error.to_string(), cx));
                        return;
                    }
                },
                Err(error) => {
                    let _ = this.update(cx, |this, cx| this.set_status(error.to_string(), cx));
                    return;
                }
            };
            loop {
                watch.snapshots.borrow_and_update();
                if cancel.is_cancelled()
                    || this.update(cx, |this, cx| this.reload_events(cx)).is_err()
                    || watch.terminal_error().is_some()
                {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    /// Keeps the current-time line, muted future items and the meeting countdown current. The
    /// event window is re-read periodically because it is relative to now.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let cancel = self.watch_cancel.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(delay) = this.update(cx, |this, cx| {
                    if this.events_loaded.is_some_and(|loaded| {
                        loaded.elapsed() > std::time::Duration::from_secs(1800)
                    }) {
                        this.reload_events(cx);
                    }
                    this.library.update(cx, |library, cx| library.tick(cx))
                }) else {
                    break;
                };
                match select(
                    cx.background_executor().timer(delay).boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left(_) => {}
                    Either::Right(_) => break,
                }
            }
        })
        .detach();
    }

    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        if !self.ready {
            return;
        }
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = self
            .runtime
            .library(self.query.clone(), self.cancellation.clone());
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        if this.page.as_ref().is_some_and(|previous| {
                            previous.offset == page.offset
                                && previous.has_more == page.has_more
                                && previous.items.len() == page.items.len()
                                && previous.items.iter().zip(page.items.iter()).all(|(a, b)| {
                                    a.id == b.id
                                        && a.title == b.title
                                        && a.created_at == b.created_at
                                        && a.folder_path == b.folder_path
                                        && a.tag_line == b.tag_line
                                        && a.event_start == b.event_start
                                        && a.event_end == b.event_end
                                })
                        }) {
                            return;
                        }
                        let page = Arc::new(page);
                        this.page = Some(page.clone());
                        this.library
                            .update(cx, |library, cx| library.set_page(page, cx));
                    }
                    Err(error) => this.set_status(
                        format!("Library refresh failed: {error}. Previous notes retained."),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn create(&mut self, listen: bool, cx: &mut Context<Self>) {
        if !self.ready || !self.can_navigate(cx) {
            return;
        }
        let reply = self.runtime.create_note("Untitled note".into());
        self.creating = true;
        self.note.update(cx, |note, _| note.cancel_open());
        self.pending = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply { Ok(reply) => reply.receive().await, Err(error) => Err(error) };
            let _ = this.update(cx, |this, cx| {
                this.creating = false;
                match result {
                    Ok(session) => {
                        if this.can_navigate(cx) {
                            let id = session.summary.id.clone();
                            let new_slot = this.navigation.current().is_none_or(|tab| tab.route != Route::Empty);
                            this.pending = Some(Navigate::Open(Route::Session(id.clone()), new_slot));
                            this.auto_start = listen.then_some(id);
                            this.note.update(cx, |note, cx| note.show(session, cx));
                        } else {
                            this.set_status("The new note was created and is available in the library. Save current edits before opening it.".into(), cx);
                        }
                    }
                    Err(error) => this.set_status(format!("Note not created: {error}"), cx),
                }
                cx.notify();
            });
        }).detach();
    }

    pub(super) fn show_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return;
        }
        self.return_focus = window.focused(cx);
        self.picker_open = true;
        // Shipping centers it on the app shell's first panel, which starts after the 4px left
        // padding the shell has while the sidebar is open.
        let offset = gpui::px(if self.sidebar.expanded { 2. } else { 0. });
        self.picker
            .update(cx, |picker, cx| picker.open(offset, window, cx));
        cx.notify();
    }

    pub(super) fn shortcuts(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if self.sort_menu_key(event, cx) {
            cx.stop_propagation();
            return;
        }
        if self.confirm_delete.is_some() {
            if event.keystroke.key == "escape" {
                self.close_delete_confirmation(cx);
            }
            cx.stop_propagation();
            return;
        }
        // Shipping's ⌘⌥←/→ switch the open note's views, even from its editors; plain ⌥←/→
        // stay with text editing.
        if modifiers.alt
            && modifiers.secondary(cx)
            && matches!(event.keystroke.key.as_str(), "left" | "right")
        {
            if matches!(self.current_route(), Route::Session(_)) {
                let forward = event.keystroke.key == "right";
                self.note.update(cx, |note, cx| note.step_view(forward, cx));
                cx.stop_propagation();
            }
            return;
        }
        if !modifiers.secondary(cx) {
            return;
        }
        match event.keystroke.key.as_str() {
            "k" => self.show_picker(window, cx),
            "j" => self
                .chat
                .update(cx, |chat, cx| chat.event(ChatEvent::Toggle, window, cx)),
            "n" => self.create(modifiers.shift, cx),
            "," => self.navigate(Navigate::Open(Route::settings("app"), false), cx),
            "\\" => self.toggle_sidebar(cx),
            "[" => self.navigate(Navigate::History(false), cx),
            "]" => self.navigate(Navigate::History(true), cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    pub(super) fn dismiss_overlay(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if event.keystroke.key == "escape"
            && !modifiers.secondary(cx)
            && !modifiers.alt
            && !modifiers.shift
        {
            if self.chat.read(cx).is_open() {
                self.chat
                    .update(cx, |chat, cx| chat.event(ChatEvent::Close, window, cx));
            } else {
                self.leave_overlay(cx);
            }
            cx.stop_propagation();
        }
    }

    fn leave_overlay(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.navigation.current() else {
            return;
        };
        if matches!(current.route, Route::Empty | Route::Onboarding) {
            return;
        }
        if let Some((slot, route)) = &current.return_to {
            if *slot != current.slot
                && self
                    .navigation
                    .tabs
                    .iter()
                    .any(|tab| tab.slot == *slot && &tab.route == route)
            {
                self.navigate(Navigate::Select(*slot), cx);
                return;
            }
            if *slot == current.slot && current.can_back() {
                self.navigate(Navigate::History(false), cx);
                return;
            }
        }
        if let Some(home) = self
            .navigation
            .tabs
            .iter()
            .find(|tab| tab.route == Route::Empty)
        {
            self.navigate(Navigate::Select(home.slot), cx);
        } else {
            self.navigate(Navigate::Open(Route::Empty, false), cx);
        }
    }
}

impl Drop for WorkspaceView {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.watch_cancel.cancel();
        self.events_cancel.cancel();
        self.subscriptions.clear();
    }
}
