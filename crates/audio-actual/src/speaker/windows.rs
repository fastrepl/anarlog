use anlg_audio_utils::{pcm_i16_to_f32, pcm_i32_to_f32};
use anyhow::{Context, Result};
use futures_util::Stream;
use futures_util::task::AtomicWaker;
use pin_project::pin_project;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Observer, Producer, Split},
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tracing::error;
use wasapi::{
    AudioClient, DeviceEnumerator, Direction, SampleType, SessionState, ShareMode, StreamMode,
    WaveFormat, initialize_mta,
};

use crate::async_ring::RingbufAsyncReader;
use crate::rt_ring::{PushStats, push_f32le_bytes_first_channel_to_ringbuf};

use super::{BUFFER_SIZE, CHUNK_SIZE};

const DEFAULT_SAMPLE_RATE: u32 = 44_100;

pub struct SpeakerInput;

impl SpeakerInput {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn sample_rate(&self) -> u32 {
        DEFAULT_SAMPLE_RATE
    }

    pub fn stream(self) -> Result<SpeakerStream> {
        let rb = HeapRb::<f32>::new(BUFFER_SIZE);
        let (producer, consumer) = rb.split();

        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let running = Arc::new(AtomicBool::new(true));
        let current_sample_rate = Arc::new(AtomicU32::new(DEFAULT_SAMPLE_RATE));
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
                let result = capture_audio_loop(
                    producer,
                    waker.clone(),
                    wake_pending.clone(),
                    alive.clone(),
                    running,
                    current_sample_rate,
                    dropped_samples,
                    init_tx,
                );

                if let Err(err) = result {
                    error!("Audio capture loop failed: {}", err);
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
                anyhow::bail!("Timed out initializing WASAPI loopback stream");
            }
        }

        Ok(SpeakerStream {
            reader: RingbufAsyncReader::new(consumer, waker, wake_pending, vec![0.0; CHUNK_SIZE])
                .with_alive(alive)
                .with_dropped_samples(dropped_samples, "samples_dropped"),
            current_sample_rate,
            running,
            capture_thread: Some(capture_thread),
        })
    }
}

#[pin_project(PinnedDrop)]
pub struct SpeakerStream {
    reader: RingbufAsyncReader<HeapCons<f32>>,
    current_sample_rate: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
    capture_thread: Option<thread::JoinHandle<()>>,
}

impl SpeakerStream {
    pub fn sample_rate(&self) -> u32 {
        self.current_sample_rate.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy)]
struct WasapiCaptureFormat {
    sample_rate: u32,
    channels: usize,
    sample_type: SampleType,
    bits_per_sample: u16,
}

fn capture_audio_loop(
    mut producer: HeapProd<f32>,
    waker: Arc<AtomicWaker>,
    wake_pending: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    current_sample_rate: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicUsize>,
    init_tx: std::sync::mpsc::Sender<Result<()>>,
) -> Result<()> {
    let setup_result = (|| -> Result<_> {
        initialize_mta()
            .ok()
            .context("Failed to initialize WASAPI COM apartment")?;

        let (mut audio_client, accepted_format, buffer_duration_hns, source) =
            match open_process_loopback_client() {
                Ok((client, format)) => (client, format, 0, "process"),
                Err(error) => {
                    tracing::info!(
                        error = %error,
                        "wasapi_process_loopback_unavailable_using_endpoint"
                    );
                    let (client, format, period) = open_endpoint_loopback_client()?;
                    (client, format, period, "endpoint")
                }
            };

        let capture_format = WasapiCaptureFormat {
            sample_rate: accepted_format.get_samplespersec(),
            channels: accepted_format.get_nchannels() as usize,
            sample_type: accepted_format
                .get_subformat()
                .context("Unsupported WASAPI sample type")?,
            bits_per_sample: accepted_format.get_bitspersample(),
        };

        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns,
        };

        audio_client
            .initialize_client(&accepted_format, &Direction::Capture, &mode)
            .context("Failed to initialize WASAPI loopback client")?;

        let event = audio_client
            .set_get_eventhandle()
            .context("Failed to create WASAPI event handle")?;
        let capture_client = audio_client
            .get_audiocaptureclient()
            .context("Failed to get WASAPI capture client")?;

        audio_client
            .start_stream()
            .context("Failed to start WASAPI loopback stream")?;

        Ok((audio_client, event, capture_client, capture_format, source))
    })();

    let (audio_client, event, capture_client, capture_format, source) = match setup_result {
        Ok(values) => values,
        Err(err) => {
            let _ = init_tx.send(Err(anyhow::anyhow!(err.to_string())));
            return Err(err);
        }
    };

    current_sample_rate.store(capture_format.sample_rate, Ordering::Release);
    tracing::info!(
        anarlog.audio.sample_rate_hz = capture_format.sample_rate,
        source,
        "wasapi_loopback_initialized"
    );
    let _ = init_tx.send(Ok(()));

    let mut temp_queue = VecDeque::new();
    let mut scratch = vec![0.0f32; crate::rt_ring::DEFAULT_SCRATCH_LEN];

    while running.load(Ordering::Acquire) {
        if event.wait_for_event(250).is_err() {
            continue;
        }

        temp_queue.clear();
        drain_packets(&capture_client, &mut temp_queue);

        if temp_queue.is_empty() {
            continue;
        }

        let stats = push_wasapi_bytes(
            temp_queue.make_contiguous(),
            capture_format,
            &mut scratch,
            &mut producer,
        )?;
        if stats.dropped > 0 {
            dropped_samples.fetch_add(stats.dropped, Ordering::Relaxed);
        }

        if stats.pushed > 0 && wake_pending.load(Ordering::Acquire) {
            wake_pending.store(false, Ordering::Release);
            waker.wake();
        }
    }

    alive.store(false, Ordering::Release);
    waker.wake();
    let _ = audio_client.stop_stream();

    Ok(())
}

// Process loopback captures every other process's render streams no matter which endpoint each one
// plays through, so the user never has to tell us which speakers the meeting app uses. Requires
// Windows 11 (build 20348+); older builds fail activation and we fall back to endpoint loopback.
//
// `include_tree = false` selects PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE: everything
// except our own process tree. The wasapi crate's doc comment describes this flag backwards.
fn open_process_loopback_client() -> Result<(AudioClient, WaveFormat)> {
    let client = AudioClient::new_application_loopback_client(std::process::id(), false)
        .context("Failed to activate WASAPI process loopback")?;
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        DEFAULT_SAMPLE_RATE as usize,
        2,
        None,
    );
    Ok((client, format))
}

fn open_endpoint_loopback_client() -> Result<(AudioClient, WaveFormat, i64)> {
    let enumerator =
        DeviceEnumerator::new().context("Failed to create WASAPI device enumerator")?;
    let device = open_render_device(&enumerator)?;
    let audio_client = device
        .get_iaudioclient()
        .context("Failed to get IAudioClient")?;

    let mix_format = audio_client
        .get_mixformat()
        .context("Failed to get WASAPI mix format")?;
    let desired_format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        mix_format.get_samplespersec() as usize,
        mix_format.get_nchannels() as usize,
        Some(mix_format.get_dwchannelmask()),
    );
    let accepted_format = audio_client
        .is_supported(&desired_format, &ShareMode::Shared)
        .context("Failed to query WASAPI shared-mode support")?
        .unwrap_or(desired_format);

    let (_default_period, min_period) = audio_client
        .get_device_period()
        .context("Failed to get WASAPI device period")?;

    Ok((audio_client, accepted_format, min_period))
}

fn drain_packets(capture_client: &wasapi::AudioCaptureClient, queue: &mut VecDeque<u8>) {
    loop {
        match capture_client.get_next_packet_size() {
            Ok(Some(frames)) if frames > 0 => {
                if let Err(err) = capture_client.read_from_device_to_deque(queue) {
                    error!("Failed to read audio data: {}", err);
                    return;
                }
            }
            Ok(_) => return,
            Err(err) => {
                error!("Failed to query WASAPI packet size: {}", err);
                return;
            }
        }
    }
}

fn open_render_device(enumerator: &DeviceEnumerator) -> Result<wasapi::Device> {
    let default = enumerator
        .get_default_device(&Direction::Render)
        .context("Failed to get default render device")?;

    Ok(render_device_in_use(enumerator, &default).unwrap_or(default))
}

// Meeting apps often play through an endpoint that is not the system default. When another process
// is actively rendering somewhere, follow it; the default wins ties so unrelated playback on a
// secondary device does not pull us away from a meeting on the default one.
fn render_device_in_use(
    enumerator: &DeviceEnumerator,
    default: &wasapi::Device,
) -> Option<wasapi::Device> {
    if has_foreign_active_session(default) {
        return None;
    }

    let collection = enumerator.get_device_collection(&Direction::Render).ok()?;
    let device = collection
        .into_iter()
        .filter_map(|device| device.ok())
        .find(has_foreign_active_session)?;

    tracing::info!(
        device = ?device.get_friendlyname().ok(),
        "wasapi_loopback_following_active_render_endpoint"
    );
    Some(device)
}

fn has_foreign_active_session(device: &wasapi::Device) -> bool {
    let self_pid = std::process::id();
    let Ok(manager) = device.get_iaudiosessionmanager() else {
        return false;
    };
    let Ok(sessions) = manager.get_audiosessionenumerator() else {
        return false;
    };
    let Ok(count) = sessions.get_count() else {
        return false;
    };

    (0..count).any(|index| {
        sessions.get_session(index).ok().is_some_and(|session| {
            session.get_state().ok() == Some(SessionState::Active)
                && session
                    .get_process_id()
                    .ok()
                    .is_some_and(|pid| pid != 0 && pid != self_pid)
        })
    })
}

fn push_wasapi_bytes(
    data: &[u8],
    format: WasapiCaptureFormat,
    scratch: &mut [f32],
    producer: &mut HeapProd<f32>,
) -> Result<PushStats> {
    match (format.sample_type, format.bits_per_sample) {
        (SampleType::Float, 32) => Ok(push_f32le_bytes_first_channel_to_ringbuf(
            data,
            format.channels,
            scratch,
            producer,
        )),
        (SampleType::Int, 16) => Ok(push_pcm_bytes_first_channel_to_ringbuf(
            data,
            format.channels,
            2,
            scratch,
            producer,
            |bytes| pcm_i16_to_f32(i16::from_le_bytes([bytes[0], bytes[1]])),
        )),
        (SampleType::Int, 32) => Ok(push_pcm_bytes_first_channel_to_ringbuf(
            data,
            format.channels,
            4,
            scratch,
            producer,
            |bytes| pcm_i32_to_f32(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        )),
        (sample_type, bits_per_sample) => anyhow::bail!(
            "Unsupported WASAPI capture format: {:?} {}-bit",
            sample_type,
            bits_per_sample
        ),
    }
}

fn push_pcm_bytes_first_channel_to_ringbuf(
    data: &[u8],
    channels: usize,
    sample_bytes: usize,
    scratch: &mut [f32],
    producer: &mut HeapProd<f32>,
    mut convert: impl FnMut(&[u8]) -> f32,
) -> PushStats {
    if scratch.is_empty() || channels == 0 || sample_bytes == 0 {
        return PushStats::default();
    }

    let frame_size = channels.saturating_mul(sample_bytes);
    if frame_size == 0 {
        return PushStats::default();
    }

    let frame_count = data.len() / frame_size;
    if frame_count == 0 {
        return PushStats::default();
    }

    let mut offset = 0usize;
    let mut pushed_total = 0usize;
    let mut dropped_total = 0usize;

    while offset < frame_count {
        let count = (frame_count - offset).min(scratch.len());

        let vacant = producer.vacant_len();
        if vacant == 0 {
            dropped_total += frame_count - offset;
            break;
        }

        let convert_count = count.min(vacant);

        for i in 0..convert_count {
            let byte_offset = (offset + i) * frame_size;
            scratch[i] = convert(&data[byte_offset..byte_offset + sample_bytes]);
        }

        let pushed = producer.push_slice(&scratch[..convert_count]);
        pushed_total += pushed;
        dropped_total += count - pushed;

        offset += count;
    }

    PushStats {
        pushed: pushed_total,
        dropped: dropped_total,
    }
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
        this.running.store(false, Ordering::Release);

        if let Some(thread) = this.capture_thread.take()
            && let Err(err) = thread.join()
        {
            error!("Failed to join capture thread: {:?}", err);
        }
    }
}
