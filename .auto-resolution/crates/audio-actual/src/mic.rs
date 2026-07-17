use cpal::{
    SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use dasp::sample::ToSample;
use futures_util::Stream;
use futures_util::task::AtomicWaker;
use ringbuf::{HeapCons, HeapProd, HeapRb, traits::Split};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::AsyncSource;
use crate::async_ring::RingbufAsyncReader;

fn is_tap_device(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        name.contains(crate::TAP_DEVICE_NAME)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        false
    }
}

pub struct MicInput {
    _host: cpal::Host,
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
}

const MIC_READ_CHUNK_SIZE: usize = 256;
const MIC_BUFFER_SIZE: usize = MIC_READ_CHUNK_SIZE * 256;

impl MicInput {
    pub fn device_name(&self) -> String {
        self.device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or("Unknown Microphone".to_string())
    }

    pub fn list_devices() -> Vec<String> {
        cpal::default_host()
            .input_devices()
            .map_err(|err| {
                tracing::error!(error = %err, "mic_list_devices_failed");
                err
            })
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                let name = d
                    .description()
                    .map(|desc| desc.name().to_string())
                    .unwrap_or("Unknown Microphone".to_string());
                if is_tap_device(&name) {
                    None
                } else {
                    Some(name)
                }
            })
            .collect()
    }

    pub fn new(device_name: Option<String>) -> Result<Self, crate::Error> {
        let host = cpal::default_host();

        let get_device_name = |d: &cpal::Device| {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_default()
        };

        let default_input_device = host
            .default_input_device()
            .filter(|d| !is_tap_device(&get_device_name(d)));

        let input_devices: Vec<cpal::Device> = host
            .input_devices()
            .map(|devices| {
                devices
                    .filter(|d| !is_tap_device(&get_device_name(d)))
                    .collect()
            })
            .unwrap_or_else(|_| Vec::new());

        let device = match device_name {
            None => default_input_device
                .or_else(|| input_devices.into_iter().next())
                .ok_or(crate::Error::NoInputDevice)?,
            Some(name) => input_devices
                .into_iter()
                .find(|d| get_device_name(d) == name)
                .or(default_input_device)
                .or_else(|| {
                    host.input_devices().ok().and_then(|mut devices| {
                        devices.find(|d| !is_tap_device(&get_device_name(d)))
                    })
                })
                .ok_or(crate::Error::NoInputDevice)?,
        };

        let device_name = get_device_name(&device);
        let config = device.default_input_config().map_err(|err| {
            tracing::error!(error = %err, device_name, "mic_default_input_config_failed");
            crate::Error::MicOpenFailed
        })?;
        tracing::info!(
            hyprnote.audio.sample_rate_hz = ?config.sample_rate(),
            device_name,
            "mic_input_initialized"
        );

        Ok(Self {
            _host: host,
            device,
            config,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate()
    }
}

impl MicInput {
    pub fn stream(&self) -> MicStream {
        let config = self.config.clone();
        let device = self.device.clone();
        let (drop_tx, drop_rx) = std::sync::mpsc::channel();

        let rb = HeapRb::<f32>::new(MIC_BUFFER_SIZE);
        let (producer, consumer) = rb.split();

        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let dropped_samples = Arc::new(AtomicUsize::new(0));

        let waker_for_thread = waker.clone();
        let wake_pending_for_thread = wake_pending.clone();
        let alive_for_thread = alive.clone();
        let dropped_for_thread = dropped_samples.clone();

        std::thread::spawn(move || {
            fn build_stream<S: ToSample<f32> + SizedSample>(
                device: &cpal::Device,
                config: &cpal::SupportedStreamConfig,
                mut producer: HeapProd<f32>,
                waker: Arc<AtomicWaker>,
                wake_pending: Arc<AtomicBool>,
                dropped_samples: Arc<AtomicUsize>,
                alive: Arc<AtomicBool>,
            ) -> Result<cpal::Stream, cpal::BuildStreamError> {
                let channels = config.channels() as usize;
                let mut scratch = vec![0.0f32; crate::rt_ring::DEFAULT_SCRATCH_LEN];
                let waker_for_err = waker.clone();
                let alive_for_err = alive.clone();
                device.build_input_stream::<S, _, _>(
                    &config.config(),
                    move |data: &[S], _input_callback_info: &_| {
                        let stats = crate::rt_ring::push_interleaved_downmix_to_mono_ringbuf(
                            data,
                            channels,
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
                    },
                    move |err| {
                        tracing::error!(error = %err, "mic_stream_error");
                        alive_for_err.store(false, Ordering::Release);
                        waker_for_err.wake();
                    },
                    None,
                )
            }

            let start_stream = || {
                let stream = match config.sample_format() {
                    cpal::SampleFormat::I8 => build_stream::<i8>(
                        &device,
                        &config,
                        producer,
                        waker_for_thread.clone(),
                        wake_pending_for_thread.clone(),
                        dropped_for_thread.clone(),
                        alive_for_thread.clone(),
                    ),
                    cpal::SampleFormat::I16 => build_stream::<i16>(
                        &device,
                        &config,
                        producer,
                        waker_for_thread.clone(),
                        wake_pending_for_thread.clone(),
                        dropped_for_thread.clone(),
                        alive_for_thread.clone(),
                    ),
                    cpal::SampleFormat::I32 => build_stream::<i32>(
                        &device,
                        &config,
                        producer,
                        waker_for_thread.clone(),
                        wake_pending_for_thread.clone(),
                        dropped_for_thread.clone(),
                        alive_for_thread.clone(),
                    ),
                    cpal::SampleFormat::F32 => build_stream::<f32>(
                        &device,
                        &config,
                        producer,
                        waker_for_thread.clone(),
                        wake_pending_for_thread.clone(),
                        dropped_for_thread.clone(),
                        alive_for_thread.clone(),
                    ),
                    sample_format => {
                        tracing::error!(sample_format = ?sample_format, "unsupported");
                        return None;
                    }
                };

                let stream = match stream {
                    Ok(stream) => stream,
                    Err(err) => {
                        tracing::error!("Error starting stream: {}", err);
                        return None;
                    }
                };

                if let Err(err) = stream.play() {
                    tracing::error!("Error playing stream: {}", err);
                    return None;
                }

                Some(stream)
            };

            let stream = match start_stream() {
                Some(stream) => stream,
                None => {
                    alive_for_thread.store(false, Ordering::Release);
                    waker_for_thread.wake();
                    return;
                }
            };

            // Wait for the stream to be dropped
            let _ = drop_rx.recv();

            // Then drop the stream
            alive_for_thread.store(false, Ordering::Release);
            waker_for_thread.wake();
            drop(stream);
        });

        MicStream {
            drop_tx,
            config: self.config.clone(),
            reader: RingbufAsyncReader::new(
                consumer,
                waker,
                wake_pending,
                vec![0.0f32; MIC_READ_CHUNK_SIZE],
            )
            .with_alive(alive)
            .with_dropped_samples(dropped_samples, "mic_samples_dropped"),
        }
    }
}

pub struct MicStream {
    drop_tx: std::sync::mpsc::Sender<()>,
    config: cpal::SupportedStreamConfig,
    reader: RingbufAsyncReader<HeapCons<f32>>,
}

impl Drop for MicStream {
    fn drop(&mut self) {
        let _ = self.drop_tx.send(());
    }
}

impl Stream for MicStream {
    type Item = f32;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        this.reader.poll_next_sample(cx).poll
    }
}

impl AsyncSource for MicStream {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        self
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn test_mic() {
        let mic = MicInput::new(None).unwrap();
        let mut stream = mic.stream();

        let mut buffer = Vec::new();
        while let Some(sample) = stream.next().await {
            buffer.push(sample);
            if buffer.len() > 6000 {
                break;
            }
        }

        assert!(buffer.iter().any(|x| *x != 0.0));
    }

    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn test_mic_stream_with_resampling() {
        use hypr_audio_utils::chunk_size_for_stt;
        use hypr_resampler::ResampleExtDynamicNew;

        let mic = MicInput::new(None).unwrap();
        println!("mic device: {}", mic.device_name());
        println!("mic sample_rate: {}", mic.sample_rate());

        let target_rate = 16000;
        let chunk_size = chunk_size_for_stt(target_rate);
        println!("target_rate: {}, chunk_size: {}", target_rate, chunk_size);

        let stream = mic.stream();
        let mut resampled = stream.resampled_chunks(target_rate, chunk_size).unwrap();

        let mut chunks_received = 0;
        let mut total_samples = 0;

        let timeout = tokio::time::Duration::from_secs(3);
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            tokio::select! {
                chunk = resampled.next() => {
                    match chunk {
                        Some(Ok(data)) => {
                            chunks_received += 1;
                            total_samples += data.len();
                            let has_nonzero = data.iter().any(|&x| x != 0.0);
                            println!(
                                "chunk {}: {} samples, has_nonzero={}",
                                chunks_received, data.len(), has_nonzero
                            );
                            if chunks_received >= 10 {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            panic!("resampling error: {:?}", e);
                        }
                        None => {
                            panic!("stream ended unexpectedly");
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                    println!("timeout waiting for chunk, chunks_received={}", chunks_received);
                }
            }
        }

        println!(
            "total: {} chunks, {} samples in {:?}",
            chunks_received,
            total_samples,
            start.elapsed()
        );
        assert!(chunks_received > 0, "should receive at least one chunk");
        assert!(total_samples > 0, "should receive samples");
    }
}
