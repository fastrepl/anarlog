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

pub(crate) fn is_unusable_input_device(name: &str) -> bool {
    if is_tap_device(name) {
        return true;
    }

    let lower = name.to_ascii_lowercase();
    lower.contains(".monitor") || lower.contains("monitor of ")
}

fn rank_input_devices(
    preferred: Option<&str>,
    default_name: Option<&str>,
    names: &[String],
) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut push = |name: &str| {
        if name.is_empty() || is_unusable_input_device(name) {
            return;
        }
        if ordered.iter().any(|existing| existing == name) {
            return;
        }
        ordered.push(name.to_string());
    };

    if let Some(name) = preferred {
        push(name);
    }
    if let Some(name) = default_name {
        push(name);
    }
    for name in names {
        push(name);
    }
    ordered
}

fn with_cpal_host_lock<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(target_os = "linux")]
    {
        use std::cell::Cell;

        thread_local! {
            static LOCK_HELD: Cell<bool> = const { Cell::new(false) };
        }

        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

        LOCK_HELD.with(|held| {
            if held.get() {
                return f();
            }

            let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            struct Reset<'a>(&'a Cell<bool>);
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.0.set(false);
                }
            }

            held.set(true);
            let _reset = Reset(held);
            f()
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        f()
    }
}

fn drop_quietly<T>(value: T) {
    if std::thread::panicking() {
        std::mem::forget(value);
        return;
    }

    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value))).is_err() {
        tracing::error!("audio_backend_drop_panicked");
    }
}

fn take_first_matching<T>(
    items: impl IntoIterator<Item = T>,
    mut matches: impl FnMut(&T) -> bool,
    mut drop_unused: impl FnMut(T),
) -> Option<T> {
    let mut found = None;
    for item in items {
        if found.is_none() && matches(&item) {
            found = Some(item);
        } else {
            drop_unused(item);
        }
    }
    found
}

fn take_named_device(
    devices: impl IntoIterator<Item = cpal::Device>,
    name: &str,
    get_device_name: impl Fn(&cpal::Device) -> String,
) -> Option<cpal::Device> {
    take_first_matching(
        devices,
        |device| get_device_name(device) == name,
        drop_quietly,
    )
}

fn drop_quietly_pair<A, B>(first: Option<A>, second: Option<B>) {
    with_cpal_host_lock(|| {
        if let Some(first) = first {
            drop_quietly(first);
        }
        if let Some(second) = second {
            drop_quietly(second);
        }
    });
}

struct CpalHandles {
    host: Option<cpal::Host>,
    device: Option<cpal::Device>,
}

impl CpalHandles {
    fn new(host: cpal::Host, device: cpal::Device) -> Self {
        Self {
            host: Some(host),
            device: Some(device),
        }
    }

    fn device(&self) -> &cpal::Device {
        self.device.as_ref().expect("mic device already dropped")
    }
}

impl Drop for CpalHandles {
    fn drop(&mut self) {
        drop_quietly_pair(self.device.take(), self.host.take());
    }
}

pub struct MicInput {
    handles: CpalHandles,
    config: cpal::SupportedStreamConfig,
}

const MIC_READ_CHUNK_SIZE: usize = 256;
const MIC_BUFFER_SIZE: usize = MIC_READ_CHUNK_SIZE * 256;

fn default_stream_config_error_type(error: &cpal::DefaultStreamConfigError) -> &'static str {
    match error {
        cpal::DefaultStreamConfigError::DeviceNotAvailable => "device_not_available",
        cpal::DefaultStreamConfigError::StreamTypeNotSupported => "stream_type_not_supported",
        cpal::DefaultStreamConfigError::BackendSpecific { .. } => "backend_specific",
    }
}

fn build_stream_error_type(error: &cpal::BuildStreamError) -> &'static str {
    match error {
        cpal::BuildStreamError::DeviceNotAvailable => "device_not_available",
        cpal::BuildStreamError::StreamConfigNotSupported => "stream_config_not_supported",
        cpal::BuildStreamError::InvalidArgument => "invalid_argument",
        cpal::BuildStreamError::StreamIdOverflow => "stream_id_overflow",
        cpal::BuildStreamError::BackendSpecific { .. } => "backend_specific",
    }
}

impl MicInput {
    pub fn device_name(&self) -> String {
        self.handles
            .device()
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or("Unknown Microphone".to_string())
    }

    pub fn list_devices() -> Vec<String> {
        with_cpal_host_lock(|| {
            let host = cpal::default_host();
            let names = host
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
                    let keep = !is_unusable_input_device(&name);
                    drop_quietly(d);
                    keep.then_some(name)
                })
                .collect();
            drop_quietly(host);
            names
        })
    }

    pub fn default_device_name() -> String {
        with_cpal_host_lock(|| {
            let host = cpal::default_host();
            let name = host
                .default_input_device()
                .and_then(|device| {
                    let name = device
                        .description()
                        .map(|d| d.name().to_string())
                        .unwrap_or_default();
                    drop_quietly(device);
                    (!name.is_empty() && !is_unusable_input_device(&name)).then_some(name)
                })
                .unwrap_or_else(|| "Unknown Microphone".to_string());
            drop_quietly(host);
            name
        })
    }

    pub fn new(device_name: Option<String>) -> Result<Self, crate::Error> {
        with_cpal_host_lock(|| Self::new_locked(device_name))
    }

    fn new_locked(device_name: Option<String>) -> Result<Self, crate::Error> {
        let host = cpal::default_host();

        let get_device_name = |d: &cpal::Device| {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_default()
        };

        let listed: Vec<cpal::Device> = host
            .input_devices()
            .map(|devices| devices.collect())
            .unwrap_or_else(|_| Vec::new());
        let listed_names: Vec<String> = listed.iter().map(get_device_name).collect();
        let default_name = host.default_input_device().map(|d| {
            let name = get_device_name(&d);
            drop_quietly(d);
            name
        });
        let ranked = rank_input_devices(
            device_name.as_deref(),
            default_name.as_deref(),
            &listed_names,
        );

        let opened = if ranked.is_empty() {
            None
        } else {
            let mut opened = None;
            for name in ranked {
                let Some(device) = listed
                    .iter()
                    .find(|d| get_device_name(d) == name)
                    .cloned()
                    .or_else(|| {
                        host.input_devices()
                            .ok()
                            .and_then(|devices| take_named_device(devices, &name, get_device_name))
                    })
                else {
                    continue;
                };

                match device.default_input_config() {
                    Ok(config) => {
                        opened = Some((device, config, name));
                        break;
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            error.type = default_stream_config_error_type(&err),
                            device_name = name,
                            "mic_default_input_config_failed"
                        );
                        drop_quietly(device);
                    }
                }
            }
            opened
        };

        for leftover in listed {
            drop_quietly(leftover);
        }

        match opened {
            Some((device, config, name)) => {
                tracing::info!(
                    anarlog.audio.sample_rate_hz = ?config.sample_rate(),
                    device_name = name,
                    "mic_input_initialized"
                );
                Ok(Self {
                    handles: CpalHandles::new(host, device),
                    config,
                })
            }
            None => {
                drop_quietly(host);
                Err(if default_name.is_none() && listed_names.is_empty() {
                    crate::Error::NoInputDevice
                } else {
                    crate::Error::MicOpenFailed
                })
            }
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate()
    }
}

impl MicInput {
    pub fn stream(&self) -> Result<MicStream, crate::Error> {
        let config = self.config.clone();
        let device = self.handles.device().clone();
        let (drop_tx, drop_rx) = std::sync::mpsc::channel();
        let (init_tx, init_rx) = std::sync::mpsc::channel();

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

        let capture_thread = std::thread::spawn(move || {
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

            let start_stream = || -> Result<cpal::Stream, String> {
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
                        return Err(format!(
                            "unsupported microphone sample format: {sample_format:?}"
                        ));
                    }
                };

                let stream = stream.map_err(|err| {
                    tracing::error!(
                        error = %err,
                        error.type = build_stream_error_type(&err),
                        "mic_stream_build_failed"
                    );
                    format!("failed to build microphone stream: {err}")
                })?;

                stream.play().map_err(|err| {
                    tracing::error!(error = %err, "mic_stream_start_failed");
                    format!("failed to start microphone stream: {err}")
                })?;

                Ok(stream)
            };

            let stream = match with_cpal_host_lock(start_stream) {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = init_tx.send(Err(error));
                    alive_for_thread.store(false, Ordering::Release);
                    waker_for_thread.wake();
                    with_cpal_host_lock(|| drop_quietly(device));
                    return;
                }
            };

            let _ = init_tx.send(Ok(()));
            let _ = drop_rx.recv();

            alive_for_thread.store(false, Ordering::Release);
            waker_for_thread.wake();
            with_cpal_host_lock(|| {
                drop_quietly(stream);
                drop_quietly(device);
            });
        });

        match init_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            // The thread returns right after reporting a failure, so joining here is
            // immediate and guarantees the cpal stream is dropped before we return.
            Ok(Err(error)) => {
                let _ = capture_thread.join();
                return Err(crate::Error::MicStreamInitializationFailed(error));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Deliberately not joined: a timeout means the thread is still inside
                // build_input_stream, and waiting on it would reintroduce exactly the
                // block this timeout exists to avoid. The drop signal is queued, so it
                // tears down as soon as the driver returns.
                let _ = drop_tx.send(());
                return Err(crate::Error::MicStreamInitializationFailed(
                    "timed out while starting the microphone stream".to_string(),
                ));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let _ = capture_thread.join();
                return Err(crate::Error::MicStreamInitializationFailed(
                    "microphone capture stopped during initialization".to_string(),
                ));
            }
        }

        Ok(MicStream {
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
        })
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

    #[test]
    fn default_stream_config_errors_have_stable_types() {
        assert_eq!(
            default_stream_config_error_type(&cpal::DefaultStreamConfigError::DeviceNotAvailable),
            "device_not_available"
        );
        assert_eq!(
            default_stream_config_error_type(
                &cpal::DefaultStreamConfigError::StreamTypeNotSupported
            ),
            "stream_type_not_supported"
        );
        assert_eq!(
            default_stream_config_error_type(&cpal::DefaultStreamConfigError::BackendSpecific {
                err: cpal::BackendSpecificError {
                    description: "private driver detail".to_string(),
                },
            }),
            "backend_specific"
        );
    }

    #[test]
    fn monitor_sources_are_unusable_input_devices() {
        assert!(is_unusable_input_device(
            "alsa_output.pci.analog-stereo.monitor"
        ));
        assert!(is_unusable_input_device("Monitor of Built-in Audio"));
        assert!(!is_unusable_input_device("Built-in Audio Analog Stereo"));
        assert!(!is_unusable_input_device("USB Microphone"));
    }

    #[test]
    fn rank_input_devices_prefers_named_then_default_and_skips_monitors() {
        let ranked = rank_input_devices(
            Some("USB Microphone"),
            Some("Built-in Audio"),
            &[
                "alsa_output.pci.analog-stereo.monitor".to_string(),
                "Built-in Audio".to_string(),
                "USB Microphone".to_string(),
                "Headset".to_string(),
            ],
        );

        assert_eq!(
            ranked,
            vec![
                "USB Microphone".to_string(),
                "Built-in Audio".to_string(),
                "Headset".to_string(),
            ]
        );
    }

    #[test]
    fn rank_input_devices_skips_empty_names() {
        let ranked = rank_input_devices(
            Some(""),
            Some(""),
            &[String::new(), "USB Microphone".to_string(), String::new()],
        );

        assert_eq!(ranked, vec!["USB Microphone".to_string()]);
    }

    #[test]
    fn take_first_matching_quiet_drops_skipped_and_leftover_items() {
        struct Boom {
            keep: bool,
        }
        impl Drop for Boom {
            fn drop(&mut self) {
                if !self.keep {
                    panic!("drop boom");
                }
            }
        }

        let kept = take_first_matching(
            vec![
                Boom { keep: false },
                Boom { keep: true },
                Boom { keep: false },
            ],
            |item| item.keep,
            drop_quietly,
        );

        assert!(kept.is_some());
    }

    #[test]
    fn rank_input_devices_falls_back_when_preferred_is_a_monitor() {
        let ranked = rank_input_devices(
            Some("Monitor of Built-in Audio"),
            Some("Built-in Audio"),
            &[
                "Monitor of Built-in Audio".to_string(),
                "Built-in Audio".to_string(),
            ],
        );

        assert_eq!(ranked, vec!["Built-in Audio".to_string()]);
    }

    #[test]
    fn drop_quietly_swallows_destructor_panics() {
        struct Boom;
        impl Drop for Boom {
            fn drop(&mut self) {
                panic!("drop boom");
            }
        }

        drop_quietly(Boom);
    }

    #[test]
    fn drop_quietly_pair_drops_second_after_first_panics() {
        struct Boom;
        impl Drop for Boom {
            fn drop(&mut self) {
                panic!("drop boom");
            }
        }

        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        struct Mark(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Mark {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        drop_quietly_pair(Some(Boom), Some(Mark(dropped.clone())));
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn drop_quietly_forgets_values_during_unwind() {
        struct Boom;
        impl Drop for Boom {
            fn drop(&mut self) {
                panic!("drop boom");
            }
        }

        let result = std::panic::catch_unwind(|| {
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    drop_quietly(Boom);
                }
            }

            let _guard = Guard;
            panic!("first");
        });

        assert!(result.is_err());
    }

    #[test]
    fn build_stream_errors_have_stable_types() {
        assert_eq!(
            build_stream_error_type(&cpal::BuildStreamError::DeviceNotAvailable),
            "device_not_available"
        );
        assert_eq!(
            build_stream_error_type(&cpal::BuildStreamError::StreamConfigNotSupported),
            "stream_config_not_supported"
        );
        assert_eq!(
            build_stream_error_type(&cpal::BuildStreamError::InvalidArgument),
            "invalid_argument"
        );
        assert_eq!(
            build_stream_error_type(&cpal::BuildStreamError::StreamIdOverflow),
            "stream_id_overflow"
        );
        assert_eq!(
            build_stream_error_type(&cpal::BuildStreamError::BackendSpecific {
                err: cpal::BackendSpecificError {
                    description: "private driver detail".to_string(),
                },
            }),
            "backend_specific"
        );
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[tokio::test]
    #[ignore = "requires audio hardware and speech near the microphone"]
    async fn test_mic() {
        let mic = MicInput::new(None).unwrap();
        let mut stream = mic.stream().unwrap();

        let mut buffer = Vec::new();
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => break,
                sample = stream.next() => {
                    match sample {
                        Some(sample) => buffer.push(sample),
                        None => panic!("microphone stream ended unexpectedly"),
                    }
                    if buffer.len() >= mic.sample_rate() as usize {
                        break;
                    }
                }
            }
        }

        assert!(!buffer.is_empty(), "microphone produced no samples");
        assert!(
            rms(&buffer) > 1e-4,
            "microphone capture was silent; speak while running this test"
        );
    }

    #[tokio::test]
    #[ignore = "requires audio hardware"]
    async fn test_mic_stream_with_resampling() {
        use anlg_audio_utils::chunk_size_for_stt;
        use anlg_resampler::ResampleExtDynamicNew;

        let mic = MicInput::new(None).unwrap();
        println!("mic device: {}", mic.device_name());
        println!("mic sample_rate: {}", mic.sample_rate());

        let target_rate = 16000;
        let chunk_size = chunk_size_for_stt(target_rate);
        println!("target_rate: {}, chunk_size: {}", target_rate, chunk_size);

        let stream = mic.stream().unwrap();
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
