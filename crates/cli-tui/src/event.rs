use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind};
use std::future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Draw,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<TuiEvent>,
    broker: Arc<EventBroker>,
    _task: tokio::task::JoinHandle<()>,
}

struct EventBroker {
    paused: AtomicBool,
    wake: Notify,
}

impl EventBroker {
    fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            wake: Notify::new(),
        }
    }

    fn pause_events(&self) {
        self.paused.store(true, Ordering::SeqCst);
        self.wake.notify_waiters();
    }

    fn resume_events(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.wake.notify_waiters();
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

impl EventHandler {
    pub fn new(mut draw_rx: broadcast::Receiver<()>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let broker = Arc::new(EventBroker::new());
        let task_broker = broker.clone();

        let task = tokio::spawn(async move {
            let mut event_stream = Some(EventStream::new());

            loop {
                if task_broker.is_paused() {
                    event_stream = None;
                } else if event_stream.is_none() {
                    event_stream = Some(EventStream::new());
                }

                let event = tokio::select! {
                    draw = draw_rx.recv() => {
                        match draw {
                            Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => TuiEvent::Draw,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    _ = task_broker.wake.notified() => {
                        continue;
                    }
                    maybe_ct_event = next_crossterm_event(&mut event_stream), if event_stream.is_some() => {
                        match maybe_ct_event {
                            Some(Ok(ct_event)) => {
                                match ct_event {
                                    CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                                        TuiEvent::Key(key)
                                    }
                                    CrosstermEvent::Paste(pasted) => TuiEvent::Paste(pasted),
                                    CrosstermEvent::Resize(_, _) => TuiEvent::Draw,
                                    _ => continue,
                                }
                            }
                            Some(Err(_)) | None => {
                                event_stream = Some(EventStream::new());
                                continue;
                            }
                        }
                    }
                    else => break,
                };

                if tx.send(event).is_err() {
                    break;
                }
            }
        });

        Self {
            rx,
            broker,
            _task: task,
        }
    }

    pub async fn next(&mut self) -> Option<TuiEvent> {
        self.rx.recv().await
    }

    pub fn pause_events(&self) {
        self.broker.pause_events();
    }

    pub fn resume_events(&self) {
        self.broker.resume_events();
    }
}

async fn next_crossterm_event(
    event_stream: &mut Option<EventStream>,
) -> Option<std::io::Result<CrosstermEvent>> {
    let Some(stream) = event_stream.as_mut() else {
        future::pending().await
    };
    stream.next().await
}
