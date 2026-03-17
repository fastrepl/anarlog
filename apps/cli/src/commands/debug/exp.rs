use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen_inline};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, LineGauge, List, ListItem, Paragraph};
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

// --- Constants ---

const NUM_DOWNLOADS: usize = 10;
const NUM_WORKERS: usize = 4;

type DownloadId = usize;
type WorkerId = usize;

// --- Action / Effect ---

enum Action {
    Key(crossterm::event::KeyEvent),
    Runtime(RuntimeEvent),
}

enum Effect {
    Exit,
}

// --- Runtime ---

enum RuntimeEvent {
    DownloadUpdate(WorkerId, DownloadId, f64),
    DownloadDone(WorkerId, DownloadId, u128),
}

struct Download {
    id: DownloadId,
    size: usize,
}

struct Runtime {
    worker_txs: Vec<mpsc::UnboundedSender<Download>>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Runtime {
    fn start(tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        let mut worker_txs = Vec::new();
        let mut tasks = Vec::new();

        for id in 0..NUM_WORKERS {
            let (worker_tx, mut worker_rx) = mpsc::unbounded_channel::<Download>();
            let tx = tx.clone();

            let task = tokio::spawn(async move {
                while let Some(download) = worker_rx.recv().await {
                    let mut remaining = download.size;
                    let start = Instant::now();
                    while remaining > 0 {
                        let wait = (remaining as u64).min(10);
                        tokio::time::sleep(Duration::from_millis(wait * 10)).await;
                        remaining = remaining.saturating_sub(10);
                        let progress =
                            (download.size - remaining) as f64 * 100.0 / download.size as f64;
                        let _ = tx.send(RuntimeEvent::DownloadUpdate(id, download.id, progress));
                    }
                    let elapsed = start.elapsed().as_millis();
                    let _ = tx.send(RuntimeEvent::DownloadDone(id, download.id, elapsed));
                }
            });

            worker_txs.push(worker_tx);
            tasks.push(task);
        }

        Self { worker_txs, tasks }
    }

    fn send_download(&self, worker_id: WorkerId, download: Download) {
        let _ = self.worker_txs[worker_id].send(download);
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

// --- App (pure state machine) ---

struct DownloadInProgress {
    id: DownloadId,
    started_at: Instant,
    progress: f64,
}

struct App {
    pending: VecDeque<Download>,
    in_progress: BTreeMap<WorkerId, DownloadInProgress>,
    done: usize,
    finished_lines: Vec<Line<'static>>,
    all_done: bool,
}

impl App {
    fn new() -> Self {
        let pending = (0..NUM_DOWNLOADS)
            .map(|id| Download {
                id,
                size: 50 + (id * 37 % 100),
            })
            .collect();

        Self {
            pending,
            in_progress: BTreeMap::new(),
            done: 0,
            finished_lines: Vec::new(),
            all_done: false,
        }
    }

    fn next_download(&mut self, worker_id: WorkerId) -> Option<Download> {
        self.pending.pop_front().map(|d| {
            self.in_progress.insert(
                worker_id,
                DownloadInProgress {
                    id: d.id,
                    started_at: Instant::now(),
                    progress: 0.0,
                },
            );
            d
        })
    }

    fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    vec![Effect::Exit]
                } else {
                    Vec::new()
                }
            }
            Action::Runtime(event) => {
                match event {
                    RuntimeEvent::DownloadUpdate(worker_id, _download_id, progress) => {
                        if let Some(download) = self.in_progress.get_mut(&worker_id) {
                            download.progress = progress;
                        }
                    }
                    RuntimeEvent::DownloadDone(worker_id, download_id, elapsed_ms) => {
                        self.in_progress.remove(&worker_id);
                        self.done += 1;
                        self.finished_lines.push(Line::from(vec![
                            Span::raw("Finished "),
                            Span::styled(
                                format!("download {download_id}"),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(format!(" in {elapsed_ms}ms")),
                        ]));

                        if self.pending.is_empty() && self.in_progress.is_empty() {
                            self.all_done = true;
                        }
                    }
                }
                Vec::new()
            }
        }
    }
}

// --- Screen ---

struct ExpScreen {
    app: App,
    runtime: Runtime,
}

impl ExpScreen {
    fn new(mut app: App, runtime: Runtime) -> Self {
        for worker_id in 0..NUM_WORKERS {
            if let Some(download) = app.next_download(worker_id) {
                runtime.send_download(worker_id, download);
            }
        }
        Self { app, runtime }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ExpScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = ();

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(_) | TuiEvent::Draw => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let worker_id = match &event {
            RuntimeEvent::DownloadDone(wid, _, _) => Some(*wid),
            _ => None,
        };

        let effects = self.app.dispatch(Action::Runtime(event));

        if let Some(wid) = worker_id {
            if let Some(download) = self.app.next_download(wid) {
                self.runtime.send_download(wid, download);
            }
            if self.app.all_done {
                return ScreenControl::Exit(());
            }
        }

        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        draw(frame, &self.app);
    }

    fn title(&self) -> String {
        "exp".to_string()
    }

    fn next_frame_delay(&self) -> Duration {
        Duration::from_millis(100)
    }
}

// --- UI ---

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();

    let block = Block::new().title(Line::from("Progress").centered());
    frame.render_widget(block, area);

    let finished_height = app.finished_lines.len() as u16;
    let vertical = Layout::vertical([
        Constraint::Length(finished_height),
        Constraint::Length(2),
        Constraint::Length(NUM_WORKERS as u16),
    ])
    .margin(1);
    let [finished_area, progress_area, main] = area.layout(&vertical);

    if !app.finished_lines.is_empty() {
        let finished = Paragraph::new(app.finished_lines.clone());
        frame.render_widget(finished, finished_area);
    }

    let progress = LineGauge::default()
        .filled_style(Style::default().fg(Color::Blue))
        .label(format!("{}/{NUM_DOWNLOADS}", app.done))
        .ratio(app.done as f64 / NUM_DOWNLOADS as f64);
    frame.render_widget(progress, progress_area);

    let horizontal = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);
    let [list_area, gauge_area] = main.layout(&horizontal);

    let items: Vec<ListItem> = app
        .in_progress
        .values()
        .map(|download| {
            ListItem::new(Line::from(vec![
                Span::raw(symbols::DOT),
                Span::styled(
                    format!(" download {:>2}", download.id),
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    " ({}ms)",
                    download.started_at.elapsed().as_millis()
                )),
            ]))
        })
        .collect();
    frame.render_widget(List::new(items), list_area);

    for (i, (_, download)) in app.in_progress.iter().enumerate() {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Yellow))
            .ratio(download.progress / 100.0);
        let y = gauge_area.top().saturating_add(i as u16);
        if y >= area.bottom() {
            continue;
        }
        frame.render_widget(
            gauge,
            Rect {
                x: gauge_area.left(),
                y,
                width: gauge_area.width,
                height: 1,
            },
        );
    }
}

// --- Entry point ---

pub async fn run() -> CliResult<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::start(tx);
    let app = App::new();
    let screen = ExpScreen::new(app, runtime);

    let height = (NUM_DOWNLOADS + NUM_WORKERS + 6) as u16;
    run_screen_inline(screen, height, Some(rx))
        .await
        .map_err(|e| CliError::operation_failed("run exp screen", e.to_string()))
}
