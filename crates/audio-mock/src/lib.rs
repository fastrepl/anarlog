use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use futures_util::Stream;
use rodio::Source;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, MissedTickBehavior};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub use hypr_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream, Error};

const MOCK_MIC_DEVICE_NAME: &str = "mock-mic";
const MOCK_MIC_AUDIO_ENV: &str = "HYPR_MOCK_MIC_AUDIO";
const MOCK_SPK_AUDIO_ENV: &str = "HYPR_MOCK_SPK_AUDIO";
const MOCK_PLAYBACK_ENV: &str = "MOCK_PLAYBACK";
const DEFAULT_MIC_AUDIO_PATH: &str = hypr_data::english_10::AUDIO_MIC_MP3_PATH;
const DEFAULT_SPEAKER_AUDIO_PATH: &str = hypr_data::english_10::AUDIO_SPK_MP3_PATH;

pub struct MockAudio {
    mic_cache: OnceLock<Result<MockAudioData, Error>>,
    spk_cache: OnceLock<Result<MockAudioData, Error>>,
}

impl MockAudio {
    pub fn new() -> Self {
        Self {
            mic_cache: OnceLock::new(),
            spk_cache: OnceLock::new(),
        }
    }

    fn mic_audio(&self) -> Result<MockAudioData, Error> {
        self.mic_cache
            .get_or_init(|| load_mock_audio(MOCK_MIC_AUDIO_ENV, DEFAULT_MIC_AUDIO_PATH))
            .clone()
    }

    fn spk_audio(&self) -> Result<MockAudioData, Error> {
        self.spk_cache
            .get_or_init(|| load_mock_audio(MOCK_SPK_AUDIO_ENV, DEFAULT_SPEAKER_AUDIO_PATH))
            .clone()
    }
}

impl AudioProvider for MockAudio {
    fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
        let mic = self.mic_audio()?;
        let speaker = self.spk_audio()?;
        Ok(open_capture_stream(mic, speaker, config))
    }

    fn open_speaker_capture(
        &self,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        let speaker = self.spk_audio()?;
        Ok(open_capture_stream(
            MockAudioData::silence(),
            speaker,
            CaptureConfig {
                sample_rate,
                chunk_size,
                mic_device: None,
                enable_aec: false,
            },
        ))
    }

    fn open_mic_capture(
        &self,
        _device: Option<String>,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        let mic = self.mic_audio()?;
        Ok(open_capture_stream(
            mic,
            MockAudioData::silence(),
            CaptureConfig {
                sample_rate,
                chunk_size,
                mic_device: None,
                enable_aec: false,
            },
        ))
    }

    fn default_device_name(&self) -> String {
        MOCK_MIC_DEVICE_NAME.to_string()
    }

    fn list_mic_devices(&self) -> Vec<String> {
        vec![MOCK_MIC_DEVICE_NAME.to_string()]
    }

    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        let (tx, _rx) = std::sync::mpsc::channel();
        tx
    }

    fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        let (tx, _rx) = std::sync::mpsc::channel();
        tx
    }

    fn probe_mic(&self, _device: Option<String>) -> Result<(), Error> {
        self.mic_audio()?;
        Ok(())
    }

    fn probe_speaker(&self) -> Result<(), Error> {
        Ok(())
    }
}

#[derive(Clone)]
struct MockAudioData {
    samples: Arc<[f32]>,
    sample_rate: u32,
}

impl MockAudioData {
    fn silence() -> Self {
        Self {
            samples: Arc::from([]),
            sample_rate: 0,
        }
    }

    fn has_data(&self) -> bool {
        !self.samples.is_empty()
    }
}

struct CaptureStreamInner {
    inner: ReceiverStream<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    task: JoinHandle<()>,
}

impl Stream for CaptureStreamInner {
    type Item = Result<CaptureFrame, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for CaptureStreamInner {
    fn drop(&mut self) {
        self.cancel_token.cancel();
        self.task.abort();
    }
}

struct StereoPlaybackSource {
    mic: Arc<[f32]>,
    spk: Arc<[f32]>,
    position: usize,
    sample_rate: u32,
}

impl Iterator for StereoPlaybackSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let channel = self.position % 2;
        let sample_idx = self.position / 2;
        let max_len = self.mic.len().max(self.spk.len());
        if sample_idx >= max_len {
            return None;
        }
        self.position += 1;
        Some(if channel == 0 {
            self.mic.get(sample_idx).copied().unwrap_or(0.0)
        } else {
            self.spk.get(sample_idx).copied().unwrap_or(0.0)
        })
    }
}

impl Source for StereoPlaybackSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

fn start_stereo_playback(mic: &MockAudioData, spk: &MockAudioData) {
    let sample_rate = mic.sample_rate.max(spk.sample_rate).max(16000);
    let mic_samples = Arc::clone(&mic.samples);
    let spk_samples = Arc::clone(&spk.samples);

    std::thread::spawn(move || {
        use rodio::{OutputStreamBuilder, Sink};

        match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => {
                let sink = Sink::connect_new(stream.mixer());
                sink.append(StereoPlaybackSource {
                    mic: mic_samples,
                    spk: spk_samples,
                    position: 0,
                    sample_rate,
                });
                tracing::info!(
                    sample_rate,
                    "mock playback started (mic=left, speaker=right)"
                );
                sink.sleep_until_end();
            }
            Err(e) => {
                tracing::warn!(error = ?e, "failed to open audio output for mock playback");
            }
        }
    });
}

fn open_capture_stream(
    mic: MockAudioData,
    speaker: MockAudioData,
    config: CaptureConfig,
) -> CaptureStream {
    if std::env::var(MOCK_PLAYBACK_ENV).ok().as_deref() != Some("0") {
        start_stereo_playback(&mic, &speaker);
    }

    let cancel_token = CancellationToken::new();
    let (tx, rx) = mpsc::channel(32);
    let task = tokio::spawn(run_capture_loop(
        tx,
        cancel_token.clone(),
        mic,
        speaker,
        config,
    ));

    CaptureStream::new(CaptureStreamInner {
        inner: ReceiverStream::new(rx),
        cancel_token,
        task,
    })
}

async fn run_capture_loop(
    tx: mpsc::Sender<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    mic: MockAudioData,
    speaker: MockAudioData,
    config: CaptureConfig,
) {
    let chunk_size = config.chunk_size.max(1);
    let mut interval = tokio::time::interval(duration_for_tick(config.sample_rate, chunk_size));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut mic_position = 0;
    let mut speaker_position = 0;

    loop {
        if is_all_exhausted(&mic, mic_position, &speaker, speaker_position) {
            return;
        }

        tokio::select! {
            _ = cancel_token.cancelled() => return,
            _ = interval.tick() => {
                let raw_mic = next_chunk(&mic.samples, &mut mic_position, chunk_size);
                let raw_speaker = next_chunk(&speaker.samples, &mut speaker_position, chunk_size);
                let raw_mic = Arc::<[f32]>::from(raw_mic);
                let raw_speaker = Arc::<[f32]>::from(raw_speaker);
                let aec_mic = config.enable_aec.then(|| Arc::clone(&raw_mic));

                if tx.send(Ok(CaptureFrame { raw_mic, raw_speaker, aec_mic })).await.is_err() {
                    return;
                }
            }
        }
    }
}

fn is_all_exhausted(
    mic: &MockAudioData,
    mic_pos: usize,
    speaker: &MockAudioData,
    speaker_pos: usize,
) -> bool {
    let mic_done = !mic.has_data() || mic_pos >= mic.samples.len();
    let spk_done = !speaker.has_data() || speaker_pos >= speaker.samples.len();
    mic_done && spk_done && (mic.has_data() || speaker.has_data())
}

fn next_chunk(samples: &[f32], position: &mut usize, chunk_size: usize) -> Vec<f32> {
    let mut chunk = vec![0.0; chunk_size];
    if *position >= samples.len() {
        return chunk;
    }

    let available = (samples.len() - *position).min(chunk_size);
    chunk[..available].copy_from_slice(&samples[*position..*position + available]);
    *position += available;
    chunk
}

fn duration_for_tick(sample_rate: u32, samples_per_tick: usize) -> Duration {
    Duration::from_secs_f64(samples_per_tick as f64 / sample_rate.max(1) as f64)
}

fn load_mock_audio(env_key: &str, default_path: &'static str) -> Result<MockAudioData, Error> {
    if let Some(path) = std::env::var_os(env_key) {
        return load_audio(Path::new(&path), env_key == MOCK_SPK_AUDIO_ENV).or_else(|error| {
            tracing::warn!(env = env_key, path = ?path, error = ?error, "failed_to_load_mock_audio");
            fallback_mock_audio(env_key)
        });
    }

    load_audio(Path::new(default_path), env_key == MOCK_SPK_AUDIO_ENV).or_else(|error| {
        tracing::warn!(env = env_key, path = default_path, error = ?error, "failed_to_load_default_mock_audio");
        fallback_mock_audio(env_key)
    })
}

fn fallback_mock_audio(env_key: &str) -> Result<MockAudioData, Error> {
    if env_key == MOCK_MIC_AUDIO_ENV {
        Err(Error::NoInputDevice)
    } else {
        Ok(MockAudioData::silence())
    }
}

fn load_audio(path: &Path, is_speaker: bool) -> Result<MockAudioData, Error> {
    let file = std::fs::File::open(path).map_err(|_| map_audio_error(is_speaker))?;
    let decoder = rodio::Decoder::try_from(file).map_err(|_| map_audio_error(is_speaker))?;
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels().max(1) as usize;
    let samples = decoder
        .enumerate()
        .filter_map(|(idx, sample)| (idx % channels == 0).then_some(sample.clamp(-1.0, 1.0)))
        .collect::<Vec<_>>();

    Ok(MockAudioData {
        samples: Arc::from(samples.into_boxed_slice()),
        sample_rate,
    })
}

fn map_audio_error(is_speaker: bool) -> Error {
    if is_speaker {
        Error::SpeakerStreamSetupFailed
    } else {
        Error::MicStreamSetupFailed
    }
}
