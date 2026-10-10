use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anlg_audio::{CaptureChannel, Error};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::stream::ChunkStream;

static MIC_OPENING: AtomicBool = AtomicBool::new(false);
static SPEAKER_OPENING: AtomicBool = AtomicBool::new(false);

struct OpeningPermit(&'static AtomicBool);

impl Drop for OpeningPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTHY_RESET: Duration = Duration::from_secs(30);
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

pub(super) struct OpenedChannel {
    pub stream: ChunkStream,
    pub device: Option<String>,
}

pub(super) enum ChannelItem {
    Ready(CaptureChannel, Option<String>),
    Failed(CaptureChannel, Error),
    Chunk(CaptureChannel, Vec<f32>),
}

pub(super) type OpenChannel =
    Arc<dyn Fn(Option<String>, &[String]) -> Result<OpenedChannel, Error> + Send + Sync>;

pub(super) async fn run_channel(
    channel: CaptureChannel,
    open: OpenChannel,
    mut device: Option<String>,
    mut retry_rx: mpsc::UnboundedReceiver<(Option<String>, bool)>,
    tx: mpsc::Sender<ChannelItem>,
    cancel: CancellationToken,
) {
    let mut failures = 0;
    let mut excluded = Vec::new();
    loop {
        let open2 = open.clone();
        let device2 = device.clone();
        let excluded2 = excluded.clone();
        let (opened_tx, mut opening) = tokio::sync::oneshot::channel();
        let cancel_for_open = cancel.clone();
        // Native calls can outlive cancellation; keep them off Tokio's shutdown path.
        let spawned = std::thread::Builder::new()
            .name("audio-capture-startup".into())
            .spawn(move || {
                let result = (|| {
                    if cancel_for_open.is_cancelled() {
                        return Err(initialization_error(channel, "capture cancelled".into()));
                    }
                    let starting = match channel {
                        CaptureChannel::Mic => &MIC_OPENING,
                        CaptureChannel::Speaker => &SPEAKER_OPENING,
                    };
                    starting
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .map_err(|_| {
                            initialization_error(
                                channel,
                                "previous audio startup is still pending".into(),
                            )
                        })?;
                    let _permit = OpeningPermit(starting);
                    open2(device2, &excluded2)
                })();
                let _ = opened_tx.send(result);
            });
        if let Err(error) = spawned {
            tracing::error!(?channel, %error, "capture_worker_start_failed");
        }
        let mut startup_reported = false;
        let opened = tokio::select! {
            _ = cancel.cancelled() => return,
            result = &mut opening => result.unwrap_or_else(|error| Err(initialization_error(channel, error.to_string()))),
            _ = tokio::time::sleep(FRAME_TIMEOUT) => {
                if tx.send(ChannelItem::Failed(channel, initialization_error(channel, "audio startup timed out".into()))).await.is_err() { return; }
                startup_reported = true;
                // A native driver call cannot be cancelled. Keep this attempt owned until it
                // returns so a timeout or manual retry cannot pile up more calls.
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    result = opening => { drop(result); }
                }
                Err(initialization_error(channel, "audio startup timed out".into()))
            }
        };
        let mut manual_retry = false;
        let error = match opened {
            Ok(mut opened) => {
                let mut healthy_since = None;
                let mut last_frame = tokio::time::Instant::now();
                loop {
                    let item = tokio::select! {
                        _ = cancel.cancelled() => return,
                        request = retry_rx.recv() => {
                            let Some((request, force_restart)) = request else { return; };
                            if healthy_since.is_some() && !force_restart && (channel == CaptureChannel::Speaker || request == device) { continue; }
                            device = request;
                            failures = 0;
                            excluded.clear();
                            manual_retry = true;
                            break initialization_error(channel, "retry requested".into());
                        }
                        item = tokio::time::timeout_at(last_frame + FRAME_TIMEOUT, opened.stream.next()) => item,
                    };
                    match item {
                        Ok(Some(Ok(data))) if !data.is_empty() => {
                            last_frame = tokio::time::Instant::now();
                            let started = match healthy_since {
                                Some(started) => started,
                                None => {
                                    let started = tokio::time::Instant::now();
                                    healthy_since = Some(started);
                                    if tx
                                        .send(ChannelItem::Ready(channel, opened.device.clone()))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                    started
                                }
                            };
                            if started.elapsed() >= HEALTHY_RESET {
                                failures = 0;
                                excluded.clear();
                            }
                            if tx.send(ChannelItem::Chunk(channel, data)).await.is_err() {
                                return;
                            }
                        }
                        Ok(Some(Ok(_))) => {}
                        item => {
                            if let Some(name) = opened.device {
                                excluded.push(name);
                            }
                            let reason = match item {
                                Ok(Some(Err(error))) => error.to_string(),
                                Ok(None) => "audio stream ended".into(),
                                _ => "audio stream stopped delivering frames".into(),
                            };
                            break initialization_error(channel, reason);
                        }
                    }
                }
            }
            Err(error) => error,
        };
        if !startup_reported && tx.send(ChannelItem::Failed(channel, error)).await.is_err() {
            return;
        }
        if manual_retry {
            continue;
        }
        let delay = RETRY_DELAYS.get(failures).copied();
        failures += 1;
        if let Some(delay) = delay {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(delay) => {},
                request = retry_rx.recv() => {
                    let Some((request, _)) = request else { return; };
                    device = request;
                    failures = 0;
                    excluded.clear();
                }
            }
        } else {
            tokio::select! {
                _ = cancel.cancelled() => return,
                request = retry_rx.recv() => {
                    let Some((request, _)) = request else { return; };
                    device = request;
                    failures = 0;
                    excluded.clear();
                }
            }
        }
    }
}

fn initialization_error(channel: CaptureChannel, message: String) -> Error {
    match channel {
        CaptureChannel::Mic => Error::MicStreamInitializationFailed(message),
        CaptureChannel::Speaker => Error::SpeakerStreamInitializationFailed(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    #[serial_test::serial]
    async fn failed_capture_exhausts_automatic_retries_and_manual_retry_recovers_silent_frames() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let open: OpenChannel = Arc::new(move |_, _| {
            if counter.fetch_add(1, Ordering::Relaxed) < 4 {
                return Err(Error::MicOpenFailed);
            }
            Ok(OpenedChannel {
                stream: Box::pin(stream::iter([Ok(vec![0.0; 32])]).chain(stream::pending())),
                device: Some("working mic".into()),
            })
        });
        let (retry, retry_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run_channel(
            CaptureChannel::Mic,
            open,
            None,
            retry_rx,
            tx,
            cancel.clone(),
        ));
        for _ in 0..4 {
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(6), rx.recv())
                    .await
                    .unwrap(),
                Some(ChannelItem::Failed(CaptureChannel::Mic, _))
            ));
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .is_err()
        );
        retry.send((None, false)).unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .unwrap(),
            Some(ChannelItem::Ready(CaptureChannel::Mic, _))
        ));
        match rx.recv().await.unwrap() {
            ChannelItem::Chunk(CaptureChannel::Mic, samples) => assert_eq!(samples, vec![0.0; 32]),
            _ => panic!("expected silent but live capture"),
        }
        retry.send((None, false)).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .is_err()
        );
        retry.send((None, true)).unwrap();
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Failed(CaptureChannel::Mic, _))
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Ready(CaptureChannel::Mic, _))
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Chunk(CaptureChannel::Mic, _))
        ));
        cancel.cancel();
        task.await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn healthy_speaker_ignores_user_retry_but_reopens_for_an_output_change() {
        let open: OpenChannel = Arc::new(|_, _| {
            Ok(OpenedChannel {
                stream: Box::pin(stream::iter([Ok(vec![0.25; 32])]).chain(stream::pending())),
                device: None,
            })
        });
        let (retry, retry_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run_channel(
            CaptureChannel::Speaker,
            open,
            None,
            retry_rx,
            tx,
            cancel.clone(),
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Ready(CaptureChannel::Speaker, _))
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Chunk(CaptureChannel::Speaker, _))
        ));
        retry.send((Some("new microphone".into()), false)).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .is_err()
        );
        retry.send((None, true)).unwrap();
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Failed(CaptureChannel::Speaker, _))
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Ready(CaptureChannel::Speaker, _))
        ));
        assert!(matches!(
            rx.recv().await,
            Some(ChannelItem::Chunk(CaptureChannel::Speaker, _))
        ));
        cancel.cancel();
        task.await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn a_timed_out_native_startup_stays_owned_until_it_returns() {
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();
        let open: OpenChannel = Arc::new(move |_, _| {
            started_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            Err(Error::MicOpenFailed)
        });
        let (retry, retry_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run_channel(
            CaptureChannel::Mic,
            open,
            None,
            retry_rx,
            tx,
            cancel.clone(),
        ));
        started_rx.recv().await.unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(6), rx.recv())
                .await
                .unwrap(),
            Some(ChannelItem::Failed(CaptureChannel::Mic, _))
        ));
        retry.send((None, false)).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), started_rx.recv())
                .await
                .is_err()
        );
        cancel.cancel();
        task.await.unwrap();
        // Start a new capture before the old native callback returns.
        let open: OpenChannel = Arc::new(|_, _| {
            Ok(OpenedChannel {
                stream: Box::pin(stream::iter([Ok(vec![0.5; 32])]).chain(stream::pending())),
                device: Some("new microphone".into()),
            })
        });
        let (next_retry, next_retry_rx) = mpsc::unbounded_channel();
        let (next_tx, mut next_rx) = mpsc::channel(32);
        let next_cancel = CancellationToken::new();
        let next_task = tokio::spawn(run_channel(
            CaptureChannel::Mic,
            open,
            None,
            next_retry_rx,
            next_tx,
            next_cancel.clone(),
        ));
        assert!(matches!(
            next_rx.recv().await,
            Some(ChannelItem::Failed(CaptureChannel::Mic, _))
        ));
        let open: OpenChannel = Arc::new(|_, _| {
            Ok(OpenedChannel {
                stream: Box::pin(stream::iter([Ok(vec![0.25; 32])]).chain(stream::pending())),
                device: None,
            })
        });
        let (_speaker_retry, speaker_retry_rx) = mpsc::unbounded_channel();
        let (speaker_tx, mut speaker_rx) = mpsc::channel(32);
        let speaker_task = tokio::spawn(run_channel(
            CaptureChannel::Speaker,
            open,
            None,
            speaker_retry_rx,
            speaker_tx,
            next_cancel.clone(),
        ));
        assert!(matches!(
            speaker_rx.recv().await,
            Some(ChannelItem::Ready(CaptureChannel::Speaker, _))
        ));
        match speaker_rx.recv().await.unwrap() {
            ChannelItem::Chunk(CaptureChannel::Speaker, samples) => {
                assert_eq!(samples, vec![0.25; 32])
            }
            _ => panic!("next recording must retain healthy system audio"),
        }
        release_tx.send(()).unwrap();
        // The callback remains protected even after its session is cancelled.
        tokio::time::timeout(Duration::from_secs(1), async {
            while MIC_OPENING.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        next_retry.send((None, true)).unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), next_rx.recv())
                .await
                .unwrap(),
            Some(ChannelItem::Ready(CaptureChannel::Mic, _))
        ));
        match next_rx.recv().await.unwrap() {
            ChannelItem::Chunk(CaptureChannel::Mic, samples) => assert_eq!(samples, vec![0.5; 32]),
            _ => panic!("microphone must recover after the old startup returns"),
        }
        next_cancel.cancel();
        next_task.await.unwrap();
        speaker_task.await.unwrap();
    }
}
