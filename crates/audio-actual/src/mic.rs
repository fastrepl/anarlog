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
    is_unusable_input_device_with_id(None, name)
}

/// Like `is_unusable_input_device`, but also rejects the ALSA `null` pcm by its stable device id
/// (`cpal::Device::id()`) when the caller has one. `null` accepts writes and produces silence
/// with no real clock, so a capture stream opened on it free-spins the audio callback instead of
/// blocking on hardware timing (see `new_locked`). The id check is the primary signal; the
/// description check below is a defensive fallback for callers that only have a name, such as a
/// device_name string loaded from settings.
pub(crate) fn is_unusable_input_device_with_id(id: Option<&str>, name: &str) -> bool {
    if is_tap_device(name) {
        return true;
    }

    if id.is_some_and(|id| id.eq_ignore_ascii_case("null")) {
        return true;
    }

    let lower = name.to_ascii_lowercase();
    lower.contains(".monitor")
        || lower.contains("monitor of ")
        || lower.starts_with("discard all samples")
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

/// Finds the index in `listed_names` that a ranked candidate should open.
///
/// Candidates are normally matched by description (`name`). The default device is a special
/// case on ALSA: `host.default_input_device()` returns a synthetic device whose description
/// (e.g. "Default Audio Device") never matches the real ALSA hint description carried by its
/// entry in the listed devices, so a plain name match misses it even though `rank_input_devices`
/// ranked it correctly. When `is_default` is set, fall back to matching by stable device id
/// (`cpal::Device::id()`) against `listed_ids` — on ALSA the "default" pcm id also shows up as
/// its own listed hint, with the real description, whenever the system defines one.
fn resolve_ranked_candidate(
    name: &str,
    is_default: bool,
    default_id: Option<&str>,
    listed_names: &[String],
    listed_ids: &[Option<String>],
) -> Option<usize> {
    listed_names
        .iter()
        .position(|listed_name| listed_name == name)
        .or_else(|| {
            if !is_default {
                return None;
            }
            let id = default_id?;
            listed_ids
                .iter()
                .position(|listed_id| listed_id.as_deref() == Some(id))
        })
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

static MIC_STARTING: AtomicBool = AtomicBool::new(false);

pub(crate) fn startup_pending() -> bool {
    MIC_STARTING.load(Ordering::Acquire)
}

struct StartupPermit;

impl StartupPermit {
    fn acquire() -> Result<Self, crate::Error> {
        MIC_STARTING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| {
                crate::Error::MicStreamInitializationFailed(
                    "previous microphone startup is still pending".into(),
                )
            })
    }
}

impl Drop for StartupPermit {
    fn drop(&mut self) {
        MIC_STARTING.store(false, Ordering::Release);
    }
}

pub struct MicInput {
    handles: CpalHandles,
    config: cpal::SupportedStreamConfig,
    bluetooth: Option<Arc<anlg_audio_device::BluetoothInputActivation>>,
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
                    let id = d.id().ok().map(|id| id.1);
                    let name = d
                        .description()
                        .map(|desc| desc.name().to_string())
                        .unwrap_or("Unknown Microphone".to_string());
                    let keep = !is_unusable_input_device_with_id(id.as_deref(), &name);
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
                    let id = device.id().ok().map(|id| id.1);
                    let name = device
                        .description()
                        .map(|d| d.name().to_string())
                        .unwrap_or_default();
                    drop_quietly(device);
                    (!name.is_empty() && !is_unusable_input_device_with_id(id.as_deref(), &name))
                        .then_some(name)
                })
                .unwrap_or_else(|| "Unknown Microphone".to_string());
            drop_quietly(host);
            name
        })
    }

    pub fn new(device_name: Option<String>) -> Result<Self, crate::Error> {
        Self::new_excluding(device_name, &[])
    }

    pub(crate) fn new_excluding(
        device_name: Option<String>,
        excluded: &[String],
    ) -> Result<Self, crate::Error> {
        with_cpal_host_lock(|| Self::new_locked(device_name, excluded))
    }

    fn new_locked(device_name: Option<String>, excluded: &[String]) -> Result<Self, crate::Error> {
        let bluetooth = device_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .or_else(anlg_audio_device::default_input_device_name)
            .and_then(|name| anlg_audio_device::prepare_bluetooth_input_for_capture(&name))
            .map(Arc::new);
        let preferred = match bluetooth.as_ref() {
            Some(activation) => activation.name.clone(),
            None => device_name,
        };

        let host = cpal::default_host();

        let get_device_name = |d: &cpal::Device| {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_default()
        };
        let get_device_id = |d: &cpal::Device| d.id().ok().map(|id| id.1);

        let listed: Vec<cpal::Device> = host
            .input_devices()
            .map(|devices| devices.collect())
            .unwrap_or_else(|_| Vec::new());
        let listed_names: Vec<String> = listed.iter().map(get_device_name).collect();
        let listed_ids: Vec<Option<String>> = listed.iter().map(get_device_id).collect();
        let default_device = host.default_input_device();
        let default_id = default_device.as_ref().and_then(get_device_id);
        let default_name = default_device.map(|d| {
            let name = get_device_name(&d);
            drop_quietly(d);
            name
        });
        let ranked =
            rank_input_devices(preferred.as_deref(), default_name.as_deref(), &listed_names);

        let opened = if ranked.is_empty() {
            None
        } else {
            let mut opened = None;
            for name in ranked {
                if excluded.contains(&name) {
                    continue;
                }
                let is_default = default_name.as_deref() == Some(name.as_str());

                let device = resolve_ranked_candidate(
                    &name,
                    is_default,
                    default_id.as_deref(),
                    &listed_names,
                    &listed_ids,
                )
                .map(|index| listed[index].clone())
                .or_else(|| {
                    host.input_devices()
                        .ok()
                        .and_then(|devices| take_named_device(devices, &name, get_device_name))
                })
                .or_else(|| {
                    // Nothing in `listed` matches this candidate by name or id. For the default
                    // candidate, open the live default device directly rather than silently
                    // skipping the user's actual default input device — see
                    // `resolve_ranked_candidate` for why name matching alone misses it.
                    is_default.then(|| host.default_input_device()).flatten()
                });

                let Some(device) = device else {
                    tracing::warn!(device_name = name, "mic_candidate_device_unresolved");
                    continue;
                };

                if is_unusable_input_device_with_id(
                    get_device_id(&device).as_deref(),
                    &get_device_name(&device),
                ) {
                    drop_quietly(device);
                    continue;
                }

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
                let bluetooth =
                    bluetooth.filter(|activation| activation.name.as_ref() == Some(&name));
                tracing::info!(
                    anarlog.audio.sample_rate_hz = ?config.sample_rate(),
                    device_name = name,
                    "mic_input_initialized"
                );
                Ok(Self {
                    handles: CpalHandles::new(host, device),
                    config,
                    bluetooth,
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
        let startup_permit = StartupPermit::acquire()?;
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

            let result = with_cpal_host_lock(start_stream);
            drop(startup_permit);
            let stream = match result {
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
            _bluetooth: self.bluetooth.clone(),
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
    _bluetooth: Option<Arc<anlg_audio_device::BluetoothInputActivation>>,
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

    type RankInputDevicesCase<'a> = (
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
        &'a [&'a str],
        &'a [&'a str],
    );
    type ResolveRankedCandidateCase<'a> = (
        &'a str,
        &'a str,
        bool,
        Option<&'a str>,
        &'a [&'a str],
        &'a [Option<&'a str>],
        Option<usize>,
    );

    #[test]
    fn unusable_input_devices_are_detected_by_id_or_description() {
        for (id, description, expected) in [
            (None, "alsa_output.pci.analog-stereo.monitor", true),
            (None, "Monitor of Built-in Audio", true),
            (None, "Built-in Audio Analog Stereo", false),
            (None, "USB Microphone", false),
            (Some("null"), "Some Innocuous Description", true),
            (Some("NULL"), "Some Innocuous Description", true),
            (
                None,
                "Discard all samples (playback) or generate zero samples (capture)",
                true,
            ),
            (Some("hw:CARD=0,DEV=0"), "USB Microphone", false),
            (
                Some("default"),
                "Default ALSA Output (currently PipeWire Media Server)",
                false,
            ),
        ] {
            assert_eq!(
                is_unusable_input_device_with_id(id, description),
                expected,
                "id={id:?}, description={description:?}"
            );
        }

        assert!(is_unusable_input_device("Monitor of Built-in Audio"));
        assert!(is_unusable_input_device(
            "Discard all samples (playback) or generate zero samples (capture)"
        ));
    }

    #[test]
    fn rank_input_devices_cases() {
        // ALSA's synthetic default description is absent from the listed hint descriptions.
        let cases: [RankInputDevicesCase<'_>; 5] = [
            (
                "prefers named then default and skips monitors",
                Some("USB Microphone"),
                Some("Built-in Audio"),
                &[
                    "alsa_output.pci.analog-stereo.monitor",
                    "Built-in Audio",
                    "USB Microphone",
                    "Headset",
                ],
                &["USB Microphone", "Built-in Audio", "Headset"],
            ),
            (
                "skips empty names",
                Some(""),
                Some(""),
                &["", "USB Microphone", ""],
                &["USB Microphone"],
            ),
            (
                "falls back when preferred is a monitor",
                Some("Monitor of Built-in Audio"),
                Some("Built-in Audio"),
                &["Monitor of Built-in Audio", "Built-in Audio"],
                &["Built-in Audio"],
            ),
            (
                "excludes the null device like a monitor",
                None,
                Some("Built-in Audio"),
                &[
                    "Discard all samples (playback) or generate zero samples (capture)",
                    "Built-in Audio",
                    "USB Microphone",
                ],
                &["Built-in Audio", "USB Microphone"],
            ),
            (
                "ranks default even when its description is not listed",
                None,
                Some("Default Audio Device"),
                &[
                    "Default ALSA Output (currently PipeWire Media Server)",
                    "USB Microphone",
                ],
                &[
                    "Default Audio Device",
                    "Default ALSA Output (currently PipeWire Media Server)",
                    "USB Microphone",
                ],
            ),
        ];

        for (label, preferred, default, listed, expected) in cases {
            let listed = listed
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>();
            let ranked = rank_input_devices(preferred, default, &listed);
            let expected = expected
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>();
            assert_eq!(ranked, expected, "{label}");
        }
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
    fn resolve_ranked_candidate_cases() {
        let cases: [ResolveRankedCandidateCase<'_>; 4] = [
            (
                "matches default by id when description differs",
                "Default Audio Device",
                true,
                Some("default"),
                &[
                    "Discard all samples (playback) or generate zero samples (capture)",
                    "Default ALSA Output (currently PipeWire Media Server)",
                    "USB Microphone",
                ],
                &[Some("null"), Some("default"), Some("hw:CARD=1,DEV=0")],
                Some(1),
            ),
            (
                "prefers name match over id match",
                "USB Microphone",
                false,
                Some("default"),
                &["USB Microphone", "Built-in Audio"],
                &[Some("hw:CARD=1,DEV=0"), Some("default")],
                Some(0),
            ),
            (
                "does not id-match non-default candidates",
                "Some Other Name",
                false,
                Some("default"),
                &["Default ALSA Output"],
                &[Some("default")],
                None,
            ),
            (
                "returns none when default id is unavailable",
                "Default Audio Device",
                true,
                None,
                &["USB Microphone"],
                &[Some("hw:CARD=1,DEV=0")],
                None,
            ),
        ];

        for (label, name, is_default, default_id, names, ids, expected) in cases {
            let names = names
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>();
            let ids = ids
                .iter()
                .map(|id| id.map(str::to_string))
                .collect::<Vec<_>>();
            assert_eq!(
                resolve_ranked_candidate(name, is_default, default_id, &names, &ids),
                expected,
                "{label}"
            );
        }
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
}
