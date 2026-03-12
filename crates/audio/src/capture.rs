use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};
use hypr_aec::AEC;
use hypr_resampler::ResampleExtDynamicNew;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::Error;
use crate::mic::MicInput;
use crate::speaker::SpeakerInput;

type ChunkStream = Pin<Box<dyn Stream<Item = Result<Vec<f32>, hypr_resampler::Error>> + Send>>;

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub sample_rate: u32,
    pub chunk_size: usize,
    pub mic_device: Option<String>,
    pub include_mic: bool,
    pub include_speaker: bool,
    pub enable_aec: bool,
}

#[derive(Debug, Clone)]
pub struct CaptureFrame {
    pub raw_mic: Arc<[f32]>,
    pub raw_speaker: Arc<[f32]>,
    pub aec_mic: Option<Arc<[f32]>>,
}

impl CaptureFrame {
    pub fn preferred_mic(&self) -> Arc<[f32]> {
        self.aec_mic
            .as_ref()
            .map(Arc::clone)
            .unwrap_or_else(|| Arc::clone(&self.raw_mic))
    }

    pub fn raw_dual(&self) -> (Arc<[f32]>, Arc<[f32]>) {
        (Arc::clone(&self.raw_mic), Arc::clone(&self.raw_speaker))
    }

    pub fn aec_dual(&self) -> (Arc<[f32]>, Arc<[f32]>) {
        (self.preferred_mic(), Arc::clone(&self.raw_speaker))
    }
}

pub struct CaptureStream {
    inner: ReceiverStream<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    task: JoinHandle<()>,
}

impl Stream for CaptureStream {
    type Item = Result<CaptureFrame, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        self.cancel_token.cancel();
        self.task.abort();
    }
}

pub(crate) fn open_capture(config: CaptureConfig) -> Result<CaptureStream, Error> {
    let mic_stream = if config.include_mic {
        Some(setup_mic_stream(
            config.sample_rate,
            config.chunk_size,
            config.mic_device.clone(),
        )?)
    } else {
        None
    };

    if config.include_mic && config.include_speaker {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let speaker_stream = if config.include_speaker {
        Some(setup_speaker_stream(config.sample_rate, config.chunk_size)?)
    } else {
        None
    };

    let cancel_token = CancellationToken::new();
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    let task = tokio::spawn(run_capture_loop(
        tx,
        cancel_token.clone(),
        config.enable_aec,
        config.include_mic,
        config.include_speaker,
        mic_stream,
        speaker_stream,
    ));

    Ok(CaptureStream {
        inner: ReceiverStream::new(rx),
        cancel_token,
        task,
    })
}

fn setup_mic_stream(
    sample_rate: u32,
    chunk_size: usize,
    mic_device: Option<String>,
) -> Result<ChunkStream, Error> {
    let mic = MicInput::new(mic_device).map_err(|_| Error::MicOpenFailed)?;
    mic.stream()
        .resampled_chunks(sample_rate, chunk_size)
        .map(|stream| Box::pin(stream) as ChunkStream)
        .map_err(|_| Error::MicStreamSetupFailed)
}

fn setup_speaker_stream(sample_rate: u32, chunk_size: usize) -> Result<ChunkStream, Error> {
    let speaker = SpeakerInput::new().map_err(|_| Error::SpeakerStreamSetupFailed)?;
    speaker
        .stream()
        .map_err(|_| Error::SpeakerStreamSetupFailed)?
        .resampled_chunks(sample_rate, chunk_size)
        .map(|stream| Box::pin(stream) as ChunkStream)
        .map_err(|_| Error::SpeakerStreamSetupFailed)
}

async fn run_capture_loop(
    tx: tokio::sync::mpsc::Sender<Result<CaptureFrame, Error>>,
    cancel_token: CancellationToken,
    enable_aec: bool,
    include_mic: bool,
    include_speaker: bool,
    mut mic_stream: Option<ChunkStream>,
    mut speaker_stream: Option<ChunkStream>,
) {
    let mut joiner = Joiner::new();
    let mut aec = if enable_aec { build_aec() } else { None };

    if !include_mic && !include_speaker {
        return;
    }

    loop {
        let result = tokio::select! {
            _ = cancel_token.cancelled() => StreamResult::Stop,
            item = async { mic_stream.as_mut()?.next().await }, if mic_stream.is_some() => {
                handle_stream_item(item, CaptureSide::Mic, &mut joiner)
            }
            item = async { speaker_stream.as_mut()?.next().await }, if speaker_stream.is_some() => {
                handle_stream_item(item, CaptureSide::Speaker, &mut joiner)
            }
        };

        match result {
            StreamResult::Continue => {
                while let Some((raw_mic, raw_speaker)) = joiner.pop_pair() {
                    let raw_mic = Arc::<[f32]>::from(raw_mic);
                    let raw_speaker = Arc::<[f32]>::from(raw_speaker);
                    let aec_mic = process_aec(&mut aec, &raw_mic, &raw_speaker);
                    if tx
                        .send(Ok(CaptureFrame {
                            raw_mic,
                            raw_speaker,
                            aec_mic,
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            StreamResult::Stop => return,
            StreamResult::Failed(err) => {
                let _ = tx.send(Err(err)).await;
                return;
            }
        }
    }
}

fn build_aec() -> Option<AEC> {
    AEC::new()
        .map_err(|error| tracing::warn!(error.message = ?error, "aec_init_failed"))
        .ok()
}

fn process_aec(aec: &mut Option<AEC>, mic: &[f32], speaker: &[f32]) -> Option<Arc<[f32]>> {
    let processor = aec.as_mut()?;
    match processor.process_streaming(mic, speaker) {
        Ok(processed) => Some(Arc::<[f32]>::from(processed)),
        Err(error) => {
            tracing::warn!(error.message = ?error, "aec_failed");
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureSide {
    Mic,
    Speaker,
}

enum StreamResult {
    Continue,
    Stop,
    Failed(Error),
}

fn handle_stream_item(
    item: Option<Result<Vec<f32>, hypr_resampler::Error>>,
    side: CaptureSide,
    joiner: &mut Joiner,
) -> StreamResult {
    match item {
        Some(Ok(data)) => {
            match side {
                CaptureSide::Mic => joiner.push_mic(data),
                CaptureSide::Speaker => joiner.push_speaker(data),
            }
            StreamResult::Continue
        }
        Some(Err(_)) => StreamResult::Failed(match side {
            CaptureSide::Mic => Error::MicResampleFailed,
            CaptureSide::Speaker => Error::SpeakerResampleFailed,
        }),
        None => StreamResult::Failed(match side {
            CaptureSide::Mic => Error::MicStreamEnded,
            CaptureSide::Speaker => Error::SpeakerStreamEnded,
        }),
    }
}

type AudioPair = (Vec<f32>, Vec<f32>);

struct Joiner {
    mic: VecDeque<Vec<f32>>,
    speaker: VecDeque<Vec<f32>>,
}

impl Joiner {
    const MAX_LAG: usize = 4;
    const MAX_QUEUE_SIZE: usize = 30;

    fn new() -> Self {
        Self {
            mic: VecDeque::new(),
            speaker: VecDeque::new(),
        }
    }

    fn push_mic(&mut self, data: Vec<f32>) {
        self.mic.push_back(data);
        if self.mic.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("mic_queue_overflow");
            self.mic.pop_front();
        }
    }

    fn push_speaker(&mut self, data: Vec<f32>) {
        self.speaker.push_back(data);
        if self.speaker.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("speaker_queue_overflow");
            self.speaker.pop_front();
        }
    }

    fn pop_pair(&mut self) -> Option<AudioPair> {
        if self.mic.front().is_some() && self.speaker.front().is_some() {
            return Some((self.mic.pop_front()?, self.speaker.pop_front()?));
        }

        if self.mic.front().is_some() && self.speaker.is_empty() && self.mic.len() > Self::MAX_LAG {
            let mic = self.mic.pop_front()?;
            let silence = vec![0.0; mic.len()];
            return Some((mic, silence));
        }
        if self.speaker.front().is_some()
            && self.mic.is_empty()
            && self.speaker.len() > Self::MAX_LAG
        {
            let speaker = self.speaker.pop_front()?;
            let silence = vec![0.0; speaker.len()];
            return Some((silence, speaker));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joiner_emits_silence_filled_pair_after_max_lag() {
        let mut joiner = Joiner::new();
        for _ in 0..=Joiner::MAX_LAG {
            joiner.push_mic(vec![0.25, -0.25]);
        }

        let (mic, speaker) = joiner.pop_pair().unwrap();
        assert_eq!(mic, vec![0.25, -0.25]);
        assert_eq!(speaker, vec![0.0, 0.0]);
    }

    #[test]
    fn capture_frame_exposes_raw_and_aec_views() {
        let frame = CaptureFrame {
            raw_mic: Arc::from([0.1_f32, 0.2]),
            raw_speaker: Arc::from([0.3_f32, 0.4]),
            aec_mic: Some(Arc::from([0.9_f32, 1.0])),
        };

        let (raw_mic, raw_speaker) = frame.raw_dual();
        assert_eq!(&*raw_mic, &[0.1, 0.2]);
        assert_eq!(&*raw_speaker, &[0.3, 0.4]);

        let (aec_mic, aec_speaker) = frame.aec_dual();
        assert_eq!(&*aec_mic, &[0.9, 1.0]);
        assert_eq!(&*aec_speaker, &[0.3, 0.4]);
    }

    #[test]
    fn build_aec_returns_instance() {
        let aec = build_aec();
        assert!(aec.is_some());
    }

    #[test]
    fn process_aec_returns_output_when_enabled() {
        let mut aec = build_aec();
        let mic = Arc::<[f32]>::from(vec![0.1_f32; 160]);
        let speaker = Arc::<[f32]>::from(vec![0.2_f32; 160]);

        let processed = process_aec(&mut aec, &mic, &speaker);
        assert_eq!(processed.as_ref().map(|data| data.len()), Some(160));
    }
}
