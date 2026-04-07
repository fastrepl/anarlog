use anyhow::{Context, Result};
use futures_util::Stream;
use futures_util::task::AtomicWaker;
use hypr_audio_utils::{pcm_i16_to_f32, pcm_i32_to_f32};
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
    DeviceEnumerator, Direction, SampleType, ShareMode, StreamMode, WaveFormat, initialize_mta,
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

        let enumerator =
            DeviceEnumerator::new().context("Failed to create WASAPI device enumerator")?;
        let device = enumerator
            .get_default_device(&Direction::Render)
            .context("Failed to get default render device")?;
        let mut audio_client = device
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

        let capture_format = WasapiCaptureFormat {
            sample_rate: accepted_format.get_samplespersec(),
            channels: accepted_format.get_nchannels() as usize,
            sample_type: accepted_format
                .get_subformat()
                .context("Unsupported WASAPI sample type")?,
            bits_per_sample: accepted_format.get_bitspersample(),
        };

        let (_default_period, min_period) = audio_client
            .get_device_period()
            .context("Failed to get WASAPI device period")?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: min_period,
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

        Ok((audio_client, event, capture_client, capture_format))
    })();

    let (audio_client, event, capture_client, capture_format) = match setup_result {
        Ok(values) => values,
        Err(err) => {
            let _ = init_tx.send(Err(anyhow::anyhow!(err.to_string())));
            return Err(err);
        }
    };

    current_sample_rate.store(capture_format.sample_rate, Ordering::Release);
    tracing::info!(
        hyprnote.audio.sample_rate_hz = capture_format.sample_rate,
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
        if let Err(err) = capture_client.read_from_device_to_deque(&mut temp_queue) {
            error!("Failed to read audio data: {}", err);
            continue;
        }

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
