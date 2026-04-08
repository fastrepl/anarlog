use std::{
    collections::BTreeSet,
    io,
    ops::RangeInclusive,
    path::Path,
    sync::{
        Arc,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossterm::{
    event::{
        self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind,
    },
    execute,
};
use futures_util::StreamExt;
use hypr_activity_capture::{
    ActivityCapture, ActivityScreenshotCoordinator, Capabilities, CaptureError, CaptureStream,
    EventCoalescer, LatestCaptureSink, LatestCaptureState, PendingCapture, PlatformCapture,
    PolicyUpdate, ScreenCoreCapturer, StableSegmentScreenshotPolicy, Transition, WatchOptions,
};
use ratatui::{DefaultTerminal, layout::Rect, widgets::ListState};
use tokio::sync::oneshot;

use crate::{
    event_row::{EventRow, RowStatus},
    export::{ExportScope, RawRecord, copy_records, save_records, save_screenshot_image},
    options::{CaptureRuntimeMode, Options},
    theme::Theme,
    ui::{self, ScreenData},
};

const UI_IDLE_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    List,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetailTab {
    Details,
    Raw,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionStats {
    pub(crate) event_count: usize,
    pub(crate) distinct_apps: usize,
    pub(crate) focus_count: usize,
    pub(crate) update_count: usize,
    pub(crate) idle_count: usize,
    pub(crate) screenshot_count: usize,
}

pub(crate) fn run(options: Options, color_enabled: bool) -> io::Result<()> {
    let _mouse_capture = MouseCaptureGuard::enable()?;
    ratatui::run(|terminal| {
        let app = ActivityApp::new(options, Theme::new(color_enabled))?;
        app.run(terminal)
    })
}

struct MouseCaptureGuard;

impl MouseCaptureGuard {
    fn enable() -> io::Result<Self> {
        execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCaptureGuard {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    }
}

enum RuntimeEvent {
    Transition(Transition),
    Error(CaptureError),
    Ended,
}

enum CaptureDriver {
    Once,
    Polling {
        next_capture_at: Option<Instant>,
    },
    Watch {
        receiver: Receiver<RuntimeEvent>,
        stop_tx: Option<oneshot::Sender<()>>,
        handle: Option<thread::JoinHandle<()>>,
        active: bool,
    },
    Stopped,
}

impl CaptureDriver {
    fn polling(next_capture_at: Option<Instant>) -> Self {
        Self::Polling { next_capture_at }
    }

    fn watch(stream: CaptureStream) -> io::Result<Self> {
        let (event_tx, receiver) = mpsc::channel();
        let (stop_tx, stop_rx) = oneshot::channel();

        let handle = thread::Builder::new()
            .name("activity-capture-dev-watch".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = event_tx.send(RuntimeEvent::Error(CaptureError::platform(
                            error.to_string(),
                        )));
                        return;
                    }
                };

                runtime.block_on(async move {
                    let mut stop_rx = stop_rx;
                    let mut stream = stream;

                    loop {
                        tokio::select! {
                            _ = &mut stop_rx => break,
                            item = stream.next() => {
                                match item {
                                    Some(Ok(transition)) => {
                                        if event_tx.send(RuntimeEvent::Transition(transition)).is_err() {
                                            break;
                                        }
                                    }
                                    Some(Err(error)) => {
                                        let _ = event_tx.send(RuntimeEvent::Error(error));
                                        break;
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                });
            })
            .map_err(io::Error::other)?;

        Ok(Self::Watch {
            receiver,
            stop_tx: Some(stop_tx),
            handle: Some(handle),
            active: true,
        })
    }

    fn poll_timeout(&self) -> Duration {
        match self {
            Self::Polling {
                next_capture_at: Some(deadline),
            } => deadline.saturating_duration_since(Instant::now()),
            Self::Polling {
                next_capture_at: None,
            }
            | Self::Watch { .. }
            | Self::Once
            | Self::Stopped => UI_IDLE_POLL,
        }
    }

    fn tick(&mut self) -> bool {
        match self {
            Self::Polling { next_capture_at } => {
                next_capture_at.is_some_and(|deadline| Instant::now() >= deadline)
            }
            _ => false,
        }
    }

    fn schedule_next_poll(&mut self, interval: Duration) {
        if let Self::Polling { next_capture_at } = self {
            *next_capture_at = Some(Instant::now() + interval);
        }
    }

    fn runtime_events(&mut self) -> Vec<RuntimeEvent> {
        let mut ready = Vec::new();
        let Self::Watch {
            receiver, active, ..
        } = self
        else {
            return ready;
        };

        loop {
            match receiver.try_recv() {
                Ok(event) => ready.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if *active {
                        *active = false;
                        ready.push(RuntimeEvent::Ended);
                    }
                    break;
                }
            }
        }

        ready
    }

    fn is_watch_live(&self) -> bool {
        matches!(self, Self::Watch { active: true, .. })
    }

    fn stop(&mut self) {
        let mut driver = CaptureDriver::Stopped;
        std::mem::swap(self, &mut driver);

        if let CaptureDriver::Watch {
            stop_tx,
            handle,
            active: _,
            receiver: _,
        } = driver
        {
            if let Some(stop_tx) = stop_tx {
                let _ = stop_tx.send(());
            }
            if let Some(handle) = handle {
                let _ = handle.join();
            }
        }
    }
}

struct ActivityApp {
    capture: PlatformCapture,
    options: Options,
    capabilities: Capabilities,
    theme: Theme,
    coalescer: EventCoalescer,
    capture_driver: CaptureDriver,
    runtime_label: String,
    events: Vec<EventRow>,
    raw_records: Vec<RawRecord>,
    list_state: ListState,
    selection_anchor: Option<usize>,
    status_message: Option<String>,
    view: View,
    detail_tab: DetailTab,
    should_exit: bool,
    list_inner_area: Rect,
    screenshot_coordinator: ActivityScreenshotCoordinator,
    screenshot_state: Arc<LatestCaptureState>,
    pending_screenshot: Option<PendingCapture>,
}

impl Drop for ActivityApp {
    fn drop(&mut self) {
        self.capture_driver.stop();
    }
}

impl ActivityApp {
    fn new(options: Options, theme: Theme) -> io::Result<Self> {
        let capture = PlatformCapture::with_policy(options.policy());
        let capabilities = capture.capabilities();
        let resolved_runtime = if options.once {
            CaptureRuntimeMode::Poll
        } else {
            options
                .resolve_runtime_mode(capabilities)
                .map_err(io::Error::other)?
        };
        let runtime_label = if options.once {
            "once".to_string()
        } else {
            options.runtime_label(resolved_runtime)
        };
        let screenshot_state = Arc::new(LatestCaptureState::default());
        let mut app = Self {
            capture,
            options,
            capabilities,
            theme,
            coalescer: EventCoalescer::default(),
            capture_driver: CaptureDriver::Once,
            runtime_label,
            events: Vec::new(),
            raw_records: Vec::new(),
            list_state: ListState::default(),
            selection_anchor: None,
            status_message: None,
            view: View::List,
            detail_tab: DetailTab::Details,
            should_exit: false,
            list_inner_area: Rect::default(),
            screenshot_coordinator: ActivityScreenshotCoordinator::new(
                Box::new(StableSegmentScreenshotPolicy::default()),
                Arc::new(LatestCaptureSink::new(Arc::clone(&screenshot_state))),
                Arc::new(ScreenCoreCapturer),
            ),
            screenshot_state,
            pending_screenshot: None,
        };

        if app.options.once {
            app.capture_once()?;
            return Ok(app);
        }

        match resolved_runtime {
            CaptureRuntimeMode::Watch => {
                let stream = app
                    .capture
                    .watch(WatchOptions {
                        poll_interval: app.options.poll_interval(),
                        emit_initial: !app.options.no_emit_initial,
                    })
                    .map_err(|error| io::Error::other(error.to_string()))?;
                app.capture_driver = CaptureDriver::watch(stream)?;
            }
            CaptureRuntimeMode::Poll => {
                app.capture_once()?;
                app.capture_driver =
                    CaptureDriver::polling(Some(Instant::now() + app.options.poll_interval()));
            }
        }

        Ok(app)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_exit {
            self.drain_runtime_events();
            self.check_pending_screenshot();
            ui::render(terminal, self.screen_data())?;

            let timeout = self.screenshot_poll_timeout();
            if event::poll(timeout)? {
                self.handle_terminal_event(event::read()?);
            } else if self.capture_driver.tick() {
                self.capture_once()?;
                self.capture_driver
                    .schedule_next_poll(self.options.poll_interval());
            }
        }

        Ok(())
    }

    fn drain_runtime_events(&mut self) {
        for event in self.capture_driver.runtime_events() {
            match event {
                RuntimeEvent::Transition(transition) => self.push_transition(transition),
                RuntimeEvent::Error(error) => {
                    self.status_message = Some(format!(
                        "capture error ({:?}): {}",
                        error.kind, error.message
                    ));
                    self.capture_driver.stop();
                }
                RuntimeEvent::Ended => {
                    self.status_message = Some("watch stream ended".to_string());
                }
            }
        }
    }

    fn capture_once(&mut self) -> io::Result<()> {
        let snapshot = self
            .capture
            .snapshot()
            .map_err(|error| io::Error::other(error.to_string()))?;

        match snapshot {
            Some(snapshot) => {
                if let Some(transition) = self.coalescer.push(Some(snapshot)) {
                    self.push_transition(transition);
                }
            }
            None if self.events.is_empty() => {
                let row = EventRow::idle(SystemTime::now(), None);
                let record = RawRecord::placeholder(&row, "capture returned no snapshot");
                self.push_row(row, record);
            }
            None => {
                if let Some(transition) = self.coalescer.push(None) {
                    self.push_transition(transition);
                }
            }
        }

        Ok(())
    }

    fn handle_terminal_event(&mut self, event: CrosstermEvent) {
        match event {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
            CrosstermEvent::Mouse(mouse) => self.handle_mouse(mouse),
            CrosstermEvent::Resize(_, _) => {}
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.view {
            View::List => self.handle_list_key(key),
            View::Details => self.handle_details_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('v') => self.toggle_selection_anchor(),
            KeyCode::Char('y') => self.copy_selection(),
            KeyCode::Char('s') => self.save_selection(),
            KeyCode::Char('S') => self.save_session(),
            KeyCode::Char('r') => {
                self.detail_tab = DetailTab::Raw;
                self.view = View::Details;
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                self.detail_tab = DetailTab::Details;
                self.view = View::Details;
            }
            _ => {}
        }
    }

    fn handle_details_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_exit = true,
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                self.view = View::List;
            }
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('d') => self.detail_tab = DetailTab::Details,
            KeyCode::Char('r') | KeyCode::Tab => self.toggle_detail_tab(),
            KeyCode::Char('y') => self.copy_selection(),
            KeyCode::Char('s') => self.save_selection(),
            KeyCode::Char('S') => self.save_session(),
            _ => {}
        }
    }

    fn toggle_detail_tab(&mut self) {
        self.detail_tab = match self.detail_tab {
            DetailTab::Details => DetailTab::Raw,
            DetailTab::Raw => DetailTab::Details,
        };
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.view != View::List {
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(_) => {
                if let Some(index) = self.row_at(mouse.column, mouse.row) {
                    self.select_index(index);
                    self.detail_tab = DetailTab::Details;
                    self.view = View::Details;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.row_hit_area(mouse.column, mouse.row) {
                    self.select_next();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.row_hit_area(mouse.column, mouse.row) {
                    self.select_previous();
                }
            }
            _ => {}
        }
    }

    fn row_hit_area(&self, column: u16, row: u16) -> bool {
        let area = self.list_inner_area;
        column >= area.x
            && column < area.x.saturating_add(area.width)
            && row >= area.y
            && row < area.y.saturating_add(area.height)
    }

    fn row_at(&self, column: u16, row: u16) -> Option<usize> {
        if !self.row_hit_area(column, row) {
            return None;
        }

        let relative_row = usize::from(row.saturating_sub(self.list_inner_area.y));
        let index = self.list_state.offset() + relative_row;
        (index < self.events.len()).then_some(index)
    }

    fn push_transition(&mut self, transition: Transition) {
        let update = self
            .screenshot_coordinator
            .handle_transition(&transition, unix_ms_now());
        self.apply_screenshot_update(update);

        if let Some(row) = EventRow::from_transition(&transition) {
            let record = RawRecord::from_transition(&row, transition);
            self.push_row(row, record);
        }
    }

    fn apply_screenshot_update(&mut self, update: PolicyUpdate) {
        match update {
            PolicyUpdate::None => {}
            PolicyUpdate::CancelPending => {
                self.pending_screenshot = None;
            }
            PolicyUpdate::Schedule(pending) => {
                self.pending_screenshot = Some(pending);
            }
            PolicyUpdate::CancelAndSchedule(pending) => {
                self.pending_screenshot = Some(pending);
            }
        }
    }

    fn check_pending_screenshot(&mut self) {
        let Some(pending) = self.pending_screenshot.as_ref() else {
            return;
        };
        let now = unix_ms_now();
        if now < pending.due_at_ms {
            return;
        }
        let pending_id = pending.pending_id;
        match self
            .screenshot_coordinator
            .fire_pending_capture(pending_id, now)
        {
            Ok(true) => {
                self.pending_screenshot = None;
                if let Some(capture) = self.screenshot_state.latest() {
                    let saved_path = match save_screenshot_image(&capture) {
                        Ok(path) => {
                            let label = file_label(&path);
                            self.status_message = Some(format!("screenshot saved to {label}"));
                            Some(label)
                        }
                        Err(error) => {
                            self.status_message = Some(format!("screenshot save failed: {error}"));
                            None
                        }
                    };
                    let row = EventRow::screenshot(&capture, saved_path.as_deref());
                    let record = RawRecord::screenshot(&row, &capture);
                    self.push_row(row, record);
                }
            }
            Ok(false) => {
                self.pending_screenshot = None;
            }
            Err(error) => {
                self.pending_screenshot = None;
                self.status_message = Some(format!("screenshot capture failed: {error}"));
            }
        }
    }

    fn screenshot_poll_timeout(&self) -> Duration {
        let base = self.capture_driver.poll_timeout();
        let Some(pending) = self.pending_screenshot.as_ref() else {
            return base;
        };
        let now = unix_ms_now();
        if now >= pending.due_at_ms {
            return Duration::ZERO;
        }
        let remaining = Duration::from_millis((pending.due_at_ms - now) as u64);
        base.min(remaining)
    }

    fn push_row(&mut self, row: EventRow, raw_record: RawRecord) {
        let should_follow_tail = matches!(self.view, View::List)
            && self
                .selected_index()
                .is_none_or(|index| index + 1 >= self.events.len());

        self.events.push(row);
        self.raw_records.push(raw_record);

        if should_follow_tail || self.events.len() == 1 {
            self.select_last();
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .filter(|index| *index < self.events.len())
    }

    fn select_index(&mut self, index: usize) {
        if self.events.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state
                .select(Some(index.min(self.events.len() - 1)));
        }
    }

    fn select_first(&mut self) {
        if !self.events.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn select_last(&mut self) {
        if !self.events.is_empty() {
            self.list_state.select(Some(self.events.len() - 1));
        }
    }

    fn select_next(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let next = self
            .selected_index()
            .map_or(0, |index| (index + 1).min(self.events.len() - 1));
        self.select_index(next);
    }

    fn select_previous(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let previous = self
            .selected_index()
            .map_or(0, |index| index.saturating_sub(1));
        self.select_index(previous);
    }

    fn toggle_selection_anchor(&mut self) {
        match (self.selection_anchor, self.selected_index()) {
            (Some(_), _) => {
                self.selection_anchor = None;
                self.status_message = Some("selection cleared".to_string());
            }
            (None, Some(index)) => {
                self.selection_anchor = Some(index);
                self.status_message = Some(format!("selection started at row {}", index + 1));
            }
            (None, None) => {
                self.status_message = Some("nothing selected".to_string());
            }
        }
    }

    fn copy_selection(&mut self) {
        let Some(range) = self.export_range() else {
            self.status_message = Some("nothing to copy".to_string());
            return;
        };

        let count = range_len(&range);
        match copy_records(&self.raw_records, range, ExportScope::Selection) {
            Ok(_) => {
                self.status_message = Some(format!("copied {count} raw record(s) as JSON"));
            }
            Err(error) => {
                self.status_message = Some(format!("copy failed: {error}"));
            }
        }
    }

    fn save_selection(&mut self) {
        let Some(range) = self.export_range() else {
            self.status_message = Some("nothing to save".to_string());
            return;
        };

        let count = range_len(&range);
        match save_records(&self.raw_records, range, ExportScope::Selection) {
            Ok(path) => {
                self.status_message = Some(format!(
                    "saved {count} raw record(s) to {}",
                    file_label(&path)
                ));
            }
            Err(error) => {
                self.status_message = Some(format!("save failed: {error}"));
            }
        }
    }

    fn save_session(&mut self) {
        if self.raw_records.is_empty() {
            self.status_message = Some("nothing to save".to_string());
            return;
        }

        let range = 0..=self.raw_records.len() - 1;
        match save_records(&self.raw_records, range.clone(), ExportScope::Session) {
            Ok(path) => {
                self.status_message = Some(format!(
                    "saved {} raw record(s) to {}",
                    range_len(&range),
                    file_label(&path)
                ));
            }
            Err(error) => {
                self.status_message = Some(format!("save failed: {error}"));
            }
        }
    }

    fn export_range(&self) -> Option<RangeInclusive<usize>> {
        let current = self.selected_index()?;
        let anchor = self.selection_anchor.unwrap_or(current);
        Some(anchor.min(current)..=anchor.max(current))
    }

    fn selection_summary(&self) -> Option<String> {
        let range = self.export_range()?;
        self.selection_anchor.map(|_| {
            format!(
                "range={}..{} ({} rows)",
                range.start() + 1,
                range.end() + 1,
                range_len(&range)
            )
        })
    }

    fn runtime_summary(&self) -> String {
        if self.options.once {
            return self.runtime_label.clone();
        }

        let live = if self.capture_driver.is_watch_live() {
            "live"
        } else if matches!(self.capture_driver, CaptureDriver::Watch { .. }) {
            "stopped"
        } else {
            "active"
        };

        format!("{} {live}", self.runtime_label)
    }

    fn session_stats(&self) -> SessionStats {
        let mut apps = BTreeSet::new();
        let mut focus_count = 0;
        let mut update_count = 0;
        let mut idle_count = 0;
        let mut screenshot_count = 0;

        for row in &self.events {
            if row.app_name != "-" {
                apps.insert(row.app_name.as_str());
            }

            match row.status {
                RowStatus::Focus => focus_count += 1,
                RowStatus::Update => update_count += 1,
                RowStatus::Idle => idle_count += 1,
                RowStatus::Screenshot => screenshot_count += 1,
            }
        }

        SessionStats {
            event_count: self.events.len(),
            distinct_apps: apps.len(),
            focus_count,
            update_count,
            idle_count,
            screenshot_count,
        }
    }

    fn selected_raw_json(&self) -> Option<String> {
        self.selected_index()
            .and_then(|index| self.raw_records.get(index))
            .map(RawRecord::pretty_json)
    }

    fn screen_data(&mut self) -> ScreenData<'_> {
        let selected_index = self.selected_index();
        let selected_raw_json = self.selected_raw_json();
        let selection_summary = self.selection_summary();
        let policy_label = self.options.policy_label();
        let browser_policy_label = self.options.browser_policy_label();
        let runtime_summary = self.runtime_summary();
        let session_stats = self.session_stats();

        ScreenData {
            options: &self.options,
            capabilities: self.capabilities,
            theme: self.theme,
            view: self.view,
            detail_tab: self.detail_tab,
            runtime_summary,
            policy_label,
            browser_policy_label,
            session_stats,
            events: &self.events,
            selected_index,
            selected_range: self.export_range(),
            selection_summary,
            status_message: self.status_message.clone(),
            selected_raw_json,
            list_state: &mut self.list_state,
            list_inner_area: &mut self.list_inner_area,
        }
    }
}

fn range_len(range: &RangeInclusive<usize>) -> usize {
    range.end() - range.start() + 1
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
