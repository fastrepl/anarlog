#![cfg(target_os = "macos")]

use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context, Poll},
    thread,
};

use block2::RcBlock;
use futures_core::Stream;
use hypr_activity_capture_interface::{
    CaptureError, CaptureStream, EventCoalescer, Transition, WatchOptions,
};
use objc2_app_kit::{NSWorkspace, NSWorkspaceDidActivateApplicationNotification};
use objc2_foundation::NSNotification;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::platform::MacosCapture;

pub(crate) fn spawn_watch_stream(
    capture: MacosCapture,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError> {
    let (transition_tx, transition_rx) = unbounded_channel();
    let (wake_tx, wake_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread_name = "activity-capture-macos".to_string();
    let thread_wake_tx = wake_tx.clone();

    let handle = thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            watch_loop(
                capture,
                options,
                thread_wake_tx,
                wake_rx,
                thread_stop,
                transition_tx,
            )
        })
        .map_err(|error| CaptureError::platform(error.to_string()))?;

    Ok(Box::pin(WatchStream {
        inner: UnboundedReceiverStream::new(transition_rx),
        stop,
        wake_tx: Some(wake_tx),
        handle: Some(handle),
    }))
}

fn watch_loop(
    capture: MacosCapture,
    options: WatchOptions,
    wake_tx: mpsc::Sender<()>,
    wake_rx: mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
    transition_tx: UnboundedSender<Result<Transition, CaptureError>>,
) {
    let workspace = NSWorkspace::sharedWorkspace();
    let center = workspace.notificationCenter();

    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &RcBlock::new(move |_notification: std::ptr::NonNull<NSNotification>| {
                let _ = wake_tx.send(());
            }),
        )
    };

    let mut coalescer = EventCoalescer::default();
    let mut first_transition_suppressed = !options.emit_initial;

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        match wake_rx.recv_timeout(options.poll_interval) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if stop.load(Ordering::SeqCst) {
            break;
        }

        match capture.capture_snapshot() {
            Ok(snapshot) => {
                let Some(transition) = coalescer.push(snapshot) else {
                    continue;
                };
                if first_transition_suppressed
                    && transition.previous.is_none()
                    && transition.current.is_some()
                {
                    first_transition_suppressed = false;
                    continue;
                }
                first_transition_suppressed = false;

                if transition_tx.send(Ok(transition)).is_err() {
                    break;
                }
            }
            Err(error) => {
                let _ = transition_tx.send(Err(error));
                break;
            }
        }
    }

    unsafe {
        center.removeObserver(observer.as_ref());
    }
}

struct WatchStream {
    inner: UnboundedReceiverStream<Result<Transition, CaptureError>>,
    stop: Arc<AtomicBool>,
    wake_tx: Option<mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Stream for WatchStream {
    type Item = Result<Transition, CaptureError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for WatchStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(wake_tx) = self.wake_tx.take() {
            let _ = wake_tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
