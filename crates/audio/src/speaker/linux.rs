use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;

use anyhow::{Context, Result};
use futures_util::Stream;
use libpulse_binding as pulse;
use pin_project::pin_project;
use pulse::context::{Context as PaContext, FlagSet as ContextFlagSet};
use pulse::mainloop::threaded::Mainloop;
use pulse::sample::{Format, Spec};
use pulse::stream::{FlagSet as StreamFlagSet, Stream as PaStream};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Producer, Split},
};

use super::{BUFFER_SIZE, CHUNK_SIZE};

const SAMPLE_RATE: u32 = 48000;

pub struct SpeakerInput {
    sample_rate: u32,
}

struct WakerState {
    waker: Option<Waker>,
    has_data: bool,
}

#[pin_project(PinnedDrop)]
pub struct SpeakerStream {
    consumer: HeapCons<f32>,
    waker_state: Arc<Mutex<WakerState>>,
    current_sample_rate: Arc<AtomicU32>,
    read_buffer: Vec<f32>,
    stop_signal: Arc<AtomicBool>,
    _capture_thread: Option<thread::JoinHandle<()>>,
}

impl SpeakerInput {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sample_rate: SAMPLE_RATE,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn stream(self) -> SpeakerStream {
        let rb = HeapRb::<f32>::new(BUFFER_SIZE);
        let (producer, consumer) = rb.split();

        let waker_state = Arc::new(Mutex::new(WakerState {
            waker: None,
            has_data: false,
        }));

        let current_sample_rate = Arc::new(AtomicU32::new(self.sample_rate));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let capture_thread = {
            let waker_state = waker_state.clone();
            let current_sample_rate = current_sample_rate.clone();
            let stop_signal = stop_signal.clone();

            thread::spawn(move || {
                if let Err(e) =
                    capture_loop(producer, waker_state, current_sample_rate, stop_signal)
                {
                    tracing::error!(error.message = ?e, "pulseaudio_capture_thread_failed");
                }
            })
        };

        SpeakerStream {
            consumer,
            waker_state,
            current_sample_rate,
            read_buffer: vec![0.0f32; CHUNK_SIZE],
            stop_signal,
            _capture_thread: Some(capture_thread),
        }
    }
}

fn capture_loop(
    mut producer: HeapProd<f32>,
    waker_state: Arc<Mutex<WakerState>>,
    current_sample_rate: Arc<AtomicU32>,
    stop_signal: Arc<AtomicBool>,
) -> Result<()> {
    let mut mainloop = Mainloop::new().context("Failed to create PulseAudio mainloop")?;
    let mut context = PaContext::new(&mainloop, "hyprnote-speaker-capture")
        .context("Failed to create PulseAudio context")?;

    context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .map_err(|_| anyhow::anyhow!("Failed to connect to PulseAudio"))?;

    mainloop
        .start()
        .map_err(|_| anyhow::anyhow!("Failed to start mainloop"))?;

    wait_for_context_ready(&mut mainloop, &context)?;

    let spec = Spec {
        format: Format::F32le,
        channels: 1,
        rate: SAMPLE_RATE,
    };

    if !spec.is_valid() {
        anyhow::bail!("Invalid sample spec");
    }

    let monitor_device = get_default_monitor_device(&mut mainloop, &context);
    tracing::info!(
        hyprnote.audio.device = ?monitor_device,
        "connecting_to_monitor_device"
    );

    mainloop.lock();

    let mut stream = PaStream::new(&mut context, "hyprnote-capture", &spec, None)
        .context("Failed to create PulseAudio stream")?;

    stream
        .connect_record(
            monitor_device.as_deref(),
            None,
            StreamFlagSet::ADJUST_LATENCY | StreamFlagSet::AUTO_TIMING_UPDATE,
        )
        .map_err(|_| anyhow::anyhow!("Failed to connect stream for recording"))?;

    mainloop.unlock();

    wait_for_stream_ready(&mut mainloop, &stream)?;

    mainloop.lock();
    let actual_rate = stream
        .get_sample_spec()
        .map(|s| s.rate)
        .unwrap_or(SAMPLE_RATE);
    mainloop.unlock();

    current_sample_rate.store(actual_rate, Ordering::Release);
    tracing::info!(
        hyprnote.audio.sample_rate_hz = actual_rate,
        "pulseaudio_capture_initialized"
    );

    let mut buffer = vec![0u8; CHUNK_SIZE * 4];

    while !stop_signal.load(Ordering::Acquire) {
        mainloop.lock();

        let readable = stream.readable_size().unwrap_or(0);
        if readable == 0 {
            mainloop.unlock();
            thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        match stream.peek() {
            Ok(pulse::stream::PeekResult::Data(data)) => {
                let bytes_to_copy = data.len().min(buffer.len());
                buffer[..bytes_to_copy].copy_from_slice(&data[..bytes_to_copy]);

                let _ = stream.discard();
                mainloop.unlock();

                let samples: Vec<f32> = buffer[..bytes_to_copy]
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                if !samples.is_empty() {
                    let pushed = producer.push_slice(&samples);

                    if pushed < samples.len() {
                        tracing::warn!(
                            hyprnote.audio.dropped_samples = samples.len() - pushed,
                            "samples_dropped"
                        );
                    }

                    if pushed > 0 {
                        wake_consumer(&waker_state);
                    }
                }
            }
            Ok(pulse::stream::PeekResult::Hole(_)) => {
                let _ = stream.discard();
                mainloop.unlock();
            }
            Ok(pulse::stream::PeekResult::Empty) => {
                mainloop.unlock();
                thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => {
                mainloop.unlock();
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    mainloop.lock();
    let _ = stream.disconnect();
    mainloop.unlock();

    mainloop.stop();

    Ok(())
}

fn wait_for_context_ready(mainloop: &mut Mainloop, context: &PaContext) -> Result<()> {
    let timeout = std::time::Duration::from_secs(5);
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
                anyhow::bail!("PulseAudio context failed");
            }
            _ => thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
}

fn wait_for_stream_ready(mainloop: &mut Mainloop, stream: &PaStream) -> Result<()> {
    let timeout = std::time::Duration::from_secs(5);
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
                anyhow::bail!("PulseAudio stream failed");
            }
            _ => thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
}

fn get_default_monitor_device(mainloop: &mut Mainloop, context: &PaContext) -> Option<String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();

    mainloop.lock();
    let introspector = context.introspect();
    introspector.get_server_info(move |info| {
        if let Some(sink_name) = &info.default_sink_name {
            let monitor_name = format!("{}.monitor", sink_name);
            let _ = tx.send(Some(monitor_name));
        } else {
            let _ = tx.send(None);
        }
        done_clone.store(true, Ordering::Release);
    });
    mainloop.unlock();

    let timeout = std::time::Duration::from_secs(2);
    let start = std::time::Instant::now();

    while !done.load(Ordering::Acquire) && start.elapsed() < timeout {
        thread::sleep(std::time::Duration::from_millis(10));
    }

    rx.recv_timeout(std::time::Duration::from_millis(100))
        .ok()
        .flatten()
}

fn wake_consumer(waker_state: &Arc<Mutex<WakerState>>) {
    let should_wake = {
        let mut state = waker_state.lock().unwrap();
        if !state.has_data {
            state.has_data = true;
            state.waker.take()
        } else {
            None
        }
    };

    if let Some(waker) = should_wake {
        waker.wake();
    }
}

impl SpeakerStream {
    pub fn sample_rate(&self) -> u32 {
        self.current_sample_rate.load(Ordering::Acquire)
    }
}

impl Stream for SpeakerStream {
    type Item = Vec<f32>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let popped = this.consumer.pop_slice(this.read_buffer);

        if popped > 0 {
            return Poll::Ready(Some(this.read_buffer[..popped].to_vec()));
        }

        {
            let mut state = this.waker_state.lock().unwrap();
            state.has_data = false;
            state.waker = Some(cx.waker().clone());
        }

        Poll::Pending
    }
}

#[pin_project::pinned_drop]
impl PinnedDrop for SpeakerStream {
    fn drop(self: std::pin::Pin<&mut Self>) {
        let this = self.project();
        this.stop_signal.store(true, Ordering::Release);
        if let Ok(mut state) = this.waker_state.lock()
            && let Some(waker) = state.waker.take()
        {
            waker.wake();
        }
    }
}
