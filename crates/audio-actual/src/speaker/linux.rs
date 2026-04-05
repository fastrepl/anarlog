use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::Stream;
use futures_util::task::AtomicWaker;
use libpulse_binding as pulse;
use pin_project::pin_project;
use pipewire as pw;
use pulse::context::{Context as PaContext, FlagSet as ContextFlagSet};
use pulse::mainloop::threaded::Mainloop;
use pulse::sample::{Format, Spec};
use pulse::stream::{FlagSet as StreamFlagSet, Stream as PaStream};
use pw::properties::properties;
use pw::spa::utils::Direction;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Producer, Split},
};

use crate::async_ring::RingbufAsyncReader;
use crate::rt_ring::push_f32le_bytes_first_channel_to_ringbuf;

use super::{BUFFER_SIZE, CHUNK_SIZE};

const DEFAULT_SAMPLE_RATE: u32 = 48_000;

pub struct SpeakerInput {
    sample_rate: u32,
}

#[pin_project(PinnedDrop)]
pub struct SpeakerStream {
    reader: RingbufAsyncReader<HeapCons<f32>>,
    current_sample_rate: Arc<AtomicU32>,
    backend_control: BackendControl,
}

enum BackendControl {
    PipeWire {
        shutdown: pw::channel::Sender<()>,
        capture_thread: Option<thread::JoinHandle<()>>,
    },
    PulseAudio {
        running: Arc<AtomicBool>,
        capture_thread: Option<thread::JoinHandle<()>>,
    },
}

struct PipeWireUserData {
    format: pw::spa::param::audio::AudioInfoRaw,
    producer: HeapProd<f32>,
    waker: Arc<AtomicWaker>,
    wake_pending: Arc<AtomicBool>,
    dropped_samples: Arc<AtomicUsize>,
    current_sample_rate: Arc<AtomicU32>,
    scratch: Vec<f32>,
}

impl SpeakerInput {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn stream(self) -> Result<SpeakerStream> {
        match SpeakerStream::try_pipewire(self.sample_rate) {
            Ok(stream) => Ok(stream),
            Err(pipewire_err) => {
                tracing::warn!(error = ?pipewire_err, "pipewire_capture_unavailable");
                SpeakerStream::try_pulseaudio(self.sample_rate).map_err(|pulse_err| {
                    anyhow::anyhow!(
                        "PipeWire speaker capture failed: {pipewire_err:#}; PulseAudio speaker capture failed: {pulse_err:#}"
                    )
                })
            }
        }
    }
}

impl SpeakerStream {
    fn try_pipewire(initial_rate: u32) -> Result<Self> {
        let rb = HeapRb::<f32>::new(BUFFER_SIZE);
        let (producer, consumer) = rb.split();

        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let current_sample_rate = Arc::new(AtomicU32::new(initial_rate));
        let dropped_samples = Arc::new(AtomicUsize::new(0));
        let (init_tx, init_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = pw::channel::channel::<()>();

        let capture_thread = {
            let waker = waker.clone();
            let wake_pending = wake_pending.clone();
            let alive = alive.clone();
            let current_sample_rate = current_sample_rate.clone();
            let dropped_samples = dropped_samples.clone();

            thread::spawn(move || {
                let result = pipewire_capture_loop(
                    producer,
                    Arc::clone(&waker),
                    wake_pending,
                    Arc::clone(&alive),
                    current_sample_rate,
                    dropped_samples,
                    shutdown_rx,
                    init_tx,
                );

                if let Err(err) = result {
                    tracing::error!(error = ?err, "pipewire_capture_thread_failed");
                }

                alive.store(false, Ordering::Release);
                waker.wake();
            })
        };

        match init_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = shutdown_tx.send(());
                let _ = capture_thread.join();
                return Err(err);
            }
            Err(_) => {
                let _ = shutdown_tx.send(());
                let _ = capture_thread.join();
                anyhow::bail!("Timed out initializing PipeWire speaker capture");
            }
        }

        Ok(Self {
            reader: RingbufAsyncReader::new(consumer, waker, wake_pending, vec![0.0; CHUNK_SIZE])
                .with_alive(alive)
                .with_dropped_samples(dropped_samples, "samples_dropped"),
            current_sample_rate,
            backend_control: BackendControl::PipeWire {
                shutdown: shutdown_tx,
                capture_thread: Some(capture_thread),
            },
        })
    }

    fn try_pulseaudio(initial_rate: u32) -> Result<Self> {
        let rb = HeapRb::<f32>::new(BUFFER_SIZE);
        let (producer, consumer) = rb.split();

        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let running = Arc::new(AtomicBool::new(true));
        let current_sample_rate = Arc::new(AtomicU32::new(initial_rate));
        let dropped_samples = Arc::new(AtomicUsize::new(0));
        let (init_tx, init_rx) = std::sync::mpsc::channel();

        let capture_thread = {
            let waker = waker.clone();
            let wake_pending = wake_pending.clone();
            let alive = alive.clone();
            let running = running.clone();
            let current_sample_rate = current_sample_rate.clone();
            let dropped_samples = dropped_samples.clone();

            thread::spawn(move || {
                let result = pulseaudio_capture_loop(
                    producer,
                    Arc::clone(&waker),
                    wake_pending,
                    Arc::clone(&alive),
                    running,
                    current_sample_rate,
                    dropped_samples,
                    init_tx,
                );

                if let Err(err) = result {
                    tracing::error!(error = ?err, "pulseaudio_capture_thread_failed");
                }

                alive.store(false, Ordering::Release);
                waker.wake();
            })
        };

        match init_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                running.store(false, Ordering::Release);
                let _ = capture_thread.join();
                return Err(err);
            }
            Err(_) => {
                running.store(false, Ordering::Release);
                let _ = capture_thread.join();
                anyhow::bail!("Timed out initializing PulseAudio speaker capture");
            }
        }

        Ok(Self {
            reader: RingbufAsyncReader::new(consumer, waker, wake_pending, vec![0.0; CHUNK_SIZE])
                .with_alive(alive)
                .with_dropped_samples(dropped_samples, "samples_dropped"),
            current_sample_rate,
            backend_control: BackendControl::PulseAudio {
                running,
                capture_thread: Some(capture_thread),
            },
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.current_sample_rate.load(Ordering::Acquire)
    }
}

fn pipewire_capture_loop(
    producer: HeapProd<f32>,
    waker: Arc<AtomicWaker>,
    wake_pending: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    current_sample_rate: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicUsize>,
    shutdown_rx: pw::channel::Receiver<()>,
    init_tx: std::sync::mpsc::Sender<Result<()>>,
) -> Result<()> {
    pw::init();
    let _deinit_guard = PipeWireDeinitGuard;

    let mainloop =
        pw::main_loop::MainLoopRc::new(None).context("Failed to create PipeWire main loop")?;
    let context = pw::context::ContextBox::new(&mainloop.loop_(), None)
        .context("Failed to create PipeWire context")?;
    let core = context
        .connect(None)
        .context("Failed to connect to PipeWire core")?;

    let stream = pw::stream::StreamBox::new(
        &core,
        "hyprnote-speaker-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::STREAM_CAPTURE_SINK => "true",
        },
    )
    .context("Failed to create PipeWire stream")?;

    let _shutdown = shutdown_rx.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |_| mainloop.quit()
    });

    let _listener = stream
        .add_local_listener_with_user_data(PipeWireUserData {
            format: Default::default(),
            producer,
            waker,
            wake_pending,
            dropped_samples,
            current_sample_rate,
            scratch: vec![0.0f32; crate::rt_ring::DEFAULT_SCRATCH_LEN],
        })
        .state_changed({
            let mainloop = mainloop.clone();
            move |_, _, old, new| {
                tracing::debug!(?old, ?new, "pipewire_stream_state_changed");
                if let pw::stream::StreamState::Error(error) = new {
                    tracing::error!(error = %error, "pipewire_stream_error");
                    mainloop.quit();
                }
            }
        })
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }

            let Ok((media_type, media_subtype)) = pw::spa::param::format_utils::parse_format(param)
            else {
                return;
            };
            if media_type != pw::spa::param::format::MediaType::Audio
                || media_subtype != pw::spa::param::format::MediaSubtype::Raw
            {
                return;
            }

            if user_data.format.parse(param).is_ok() {
                let rate = user_data.format.rate();
                if rate > 0 {
                    user_data.current_sample_rate.store(rate, Ordering::Release);
                    tracing::info!(
                        hyprnote.audio.sample_rate_hz = rate,
                        "pipewire_capture_initialized"
                    );
                }
            }
        })
        .process(|stream, user_data| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };

            let datas = buffer.datas_mut();
            if datas.is_empty() {
                return;
            }

            let data = &mut datas[0];
            let chunk_size = data.chunk().size() as usize;
            let Some(bytes) = data.data() else {
                return;
            };

            let chunk_len = chunk_size.min(bytes.len());
            if chunk_len == 0 {
                return;
            }

            let channels = user_data.format.channels().max(1) as usize;
            let stats = push_f32le_bytes_first_channel_to_ringbuf(
                &bytes[..chunk_len],
                channels,
                &mut user_data.scratch,
                &mut user_data.producer,
            );

            if stats.dropped > 0 {
                user_data
                    .dropped_samples
                    .fetch_add(stats.dropped, Ordering::Relaxed);
            }

            if stats.pushed > 0 && user_data.wake_pending.load(Ordering::Acquire) {
                user_data.wake_pending.store(false, Ordering::Release);
                user_data.waker.wake();
            }
        })
        .register()
        .context("Failed to register PipeWire stream listener")?;

    let mut audio_info = pw::spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(pw::spa::param::audio::AudioFormat::F32LE);
    let format_object = pw::spa::pod::Object {
        type_: pw::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: pw::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(format_object),
    )
    .context("Failed to serialize PipeWire format pod")?
    .0
    .into_inner();
    let mut params = [
        pw::spa::pod::Pod::from_bytes(&values).context("Failed to build PipeWire format pod")?
    ];

    stream
        .connect(
            Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut params,
        )
        .context("Failed to connect PipeWire capture stream")?;

    let _ = init_tx.send(Ok(()));
    mainloop.run();

    alive.store(false, Ordering::Release);
    Ok(())
}

struct PipeWireDeinitGuard;

impl Drop for PipeWireDeinitGuard {
    fn drop(&mut self) {
        unsafe { pw::deinit() };
    }
}

fn pulseaudio_capture_loop(
    mut producer: HeapProd<f32>,
    waker: Arc<AtomicWaker>,
    wake_pending: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    current_sample_rate: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicUsize>,
    init_tx: std::sync::mpsc::Sender<Result<()>>,
) -> Result<()> {
    let mut mainloop = Mainloop::new().context("Failed to create PulseAudio mainloop")?;
    let mut context = PaContext::new(&mainloop, "hyprnote-speaker-capture")
        .context("Failed to create PulseAudio context")?;

    context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .map_err(|_| anyhow::anyhow!("Failed to connect to PulseAudio"))?;

    mainloop
        .start()
        .map_err(|_| anyhow::anyhow!("Failed to start PulseAudio mainloop"))?;

    let setup_result = (|| -> Result<_> {
        wait_for_context_ready(&mut mainloop, &context)?;

        let spec = Spec {
            format: Format::F32le,
            channels: 1,
            rate: DEFAULT_SAMPLE_RATE,
        };
        if !spec.is_valid() {
            anyhow::bail!("Invalid PulseAudio sample spec");
        }

        let monitor_device = get_default_monitor_device(&mut mainloop, &context)
            .context("Failed to resolve PulseAudio monitor source")?;
        tracing::info!(hyprnote.audio.device = %monitor_device, "connecting_to_monitor_device");

        mainloop.lock();
        let stream_result = (|| -> Result<_> {
            let mut stream = PaStream::new(&mut context, "hyprnote-capture", &spec, None)
                .context("Failed to create PulseAudio stream")?;
            stream
                .connect_record(
                    Some(&monitor_device),
                    None,
                    StreamFlagSet::ADJUST_LATENCY | StreamFlagSet::AUTO_TIMING_UPDATE,
                )
                .map_err(|_| anyhow::anyhow!("Failed to connect PulseAudio record stream"))?;
            Ok(stream)
        })();
        mainloop.unlock();
        let mut stream = stream_result?;

        wait_for_stream_ready(&mut mainloop, &stream)?;

        mainloop.lock();
        let actual_rate = stream
            .get_sample_spec()
            .map(|sample_spec| sample_spec.rate)
            .unwrap_or(DEFAULT_SAMPLE_RATE);
        mainloop.unlock();

        Ok((stream, actual_rate))
    })();

    let (mut stream, actual_rate) = match setup_result {
        Ok(values) => values,
        Err(err) => {
            let _ = init_tx.send(Err(anyhow::anyhow!(err.to_string())));
            mainloop.stop();
            return Err(err);
        }
    };

    current_sample_rate.store(actual_rate, Ordering::Release);
    tracing::info!(
        hyprnote.audio.sample_rate_hz = actual_rate,
        "pulseaudio_capture_initialized"
    );
    let _ = init_tx.send(Ok(()));

    let mut scratch = vec![0.0f32; crate::rt_ring::DEFAULT_SCRATCH_LEN];

    while running.load(Ordering::Acquire) {
        mainloop.lock();

        let readable = stream.readable_size().unwrap_or(0);
        if readable == 0 {
            mainloop.unlock();
            thread::sleep(Duration::from_millis(5));
            continue;
        }

        match stream.peek() {
            Ok(pulse::stream::PeekResult::Data(data)) => {
                let bytes = data.to_vec();
                let _ = stream.discard();
                mainloop.unlock();

                let stats = push_f32le_bytes_first_channel_to_ringbuf(
                    &bytes,
                    1,
                    &mut scratch,
                    &mut producer,
                );
                if stats.dropped > 0 {
                    dropped_samples.fetch_add(stats.dropped, Ordering::Relaxed);
                }

                if stats.pushed > 0 && wake_pending.load(Ordering::Acquire) {
                    wake_pending.store(false, Ordering::Release);
                    waker.wake();
                }
            }
            Ok(pulse::stream::PeekResult::Hole(_)) => {
                let _ = stream.discard();
                mainloop.unlock();
            }
            Ok(pulse::stream::PeekResult::Empty) => {
                mainloop.unlock();
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => {
                mainloop.unlock();
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    mainloop.lock();
    let _ = stream.disconnect();
    mainloop.unlock();
    mainloop.stop();
    alive.store(false, Ordering::Release);

    Ok(())
}

fn wait_for_context_ready(mainloop: &mut Mainloop, context: &PaContext) -> Result<()> {
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("Timeout waiting for PulseAudio context");
        }

        mainloop.lock();
        let state = context.get_state();
        mainloop.unlock();

        match state {
            pulse::context::State::Ready => return Ok(()),
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                anyhow::bail!("PulseAudio context entered {:?} state", state);
            }
            _ => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn wait_for_stream_ready(mainloop: &mut Mainloop, stream: &PaStream) -> Result<()> {
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("Timeout waiting for PulseAudio stream");
        }

        mainloop.lock();
        let state = stream.get_state();
        mainloop.unlock();

        match state {
            pulse::stream::State::Ready => return Ok(()),
            pulse::stream::State::Failed | pulse::stream::State::Terminated => {
                anyhow::bail!("PulseAudio stream entered {:?} state", state);
            }
            _ => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn get_default_monitor_device(mainloop: &mut Mainloop, context: &PaContext) -> Option<String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    mainloop.lock();
    let introspector = context.introspect();
    introspector.get_server_info(move |info| {
        if let Some(sink_name) = &info.default_sink_name {
            let monitor_name = format!("{}.monitor", sink_name);
            let _ = tx.send(Some(monitor_name));
        } else {
            let _ = tx.send(None);
        }
    });
    mainloop.unlock();

    rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
}

impl Stream for SpeakerStream {
    type Item = Vec<f32>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.reader.poll_next_chunk(cx).poll
    }
}

#[pin_project::pinned_drop]
impl PinnedDrop for SpeakerStream {
    fn drop(self: std::pin::Pin<&mut Self>) {
        let this = self.project();

        match this.backend_control {
            BackendControl::PipeWire {
                shutdown,
                capture_thread,
            } => {
                let _ = shutdown.send(());
                if let Some(thread) = capture_thread.take()
                    && let Err(err) = thread.join()
                {
                    tracing::error!(error = ?err, "failed_to_join_pipewire_thread");
                }
            }
            BackendControl::PulseAudio {
                running,
                capture_thread,
            } => {
                running.store(false, Ordering::Release);
                if let Some(thread) = capture_thread.take()
                    && let Err(err) = thread.join()
                {
                    tracing::error!(error = ?err, "failed_to_join_pulseaudio_thread");
                }
            }
        }
    }
}
