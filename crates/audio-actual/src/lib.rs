mod async_ring;
mod capture;
mod mic;
mod norm;
mod rt_ring;
mod speaker;

pub use mic::*;
pub use norm::*;
pub use speaker::*;

pub use cpal;
use cpal::traits::{DeviceTrait, HostTrait};

use futures_util::Stream;
pub use hypr_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream, Error};
pub use hypr_audio_interface::AsyncSource;

pub const TAP_DEVICE_NAME: &str = "hypr-audio-tap";

pub struct AudioOutput {}

impl AudioOutput {
    pub fn to_speaker(bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        use rodio::{Decoder, Player, stream::DeviceSinkBuilder};
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            if let Ok(stream) = DeviceSinkBuilder::open_default_sink() {
                let file = std::io::Cursor::new(bytes);
                if let Ok(source) = Decoder::try_from(file) {
                    let player = Player::connect_new(stream.mixer());
                    player.append(source);

                    let _ = rx.recv_timeout(std::time::Duration::from_secs(3600));
                    player.stop();
                }
            }
        });

        tx
    }

    pub fn silence() -> std::sync::mpsc::Sender<()> {
        use rodio::{
            Player, nz,
            source::{Source, Zero},
            stream::DeviceSinkBuilder,
        };

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            if let Ok(stream) = DeviceSinkBuilder::open_default_sink() {
                let silence = Zero::new(nz!(2u16), nz!(48_000u32))
                    .take_duration(std::time::Duration::from_secs(1))
                    .repeat_infinite();

                let player = Player::connect_new(stream.mixer());
                player.append(silence);

                let _ = rx.recv();
                player.stop();
            }
        });

        tx
    }
}

pub enum AudioSource {
    RealtimeMic,
    RealtimeSpeaker,
    Recorded,
}

pub struct AudioInput {
    source: AudioSource,
    mic: Option<MicInput>,
    speaker: Option<SpeakerInput>,
    data: Option<Vec<u8>>,
}

impl AudioInput {
    pub fn get_default_device_name() -> String {
        let host = cpal::default_host();
        let device = host.default_input_device().unwrap();
        device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or("Unknown Microphone".to_string())
    }

    pub fn sample_rate(&self) -> u32 {
        match &self.source {
            AudioSource::RealtimeMic => self.mic.as_ref().unwrap().sample_rate(),
            AudioSource::RealtimeSpeaker => self.speaker.as_ref().unwrap().sample_rate(),
            AudioSource::Recorded => 16000,
        }
    }

    pub fn list_mic_devices() -> Vec<String> {
        let host = cpal::default_host();

        let devices: Vec<cpal::Device> = host
            .input_devices()
            .map(|devices| devices.collect())
            .unwrap_or_else(|_| Vec::new());

        devices
            .into_iter()
            .filter_map(|d| d.description().map(|desc| desc.name().to_string()).ok())
            .filter(|d| d != TAP_DEVICE_NAME)
            .collect()
    }

    pub fn from_mic_and_speaker(config: CaptureConfig) -> Result<CaptureStream, Error> {
        capture::open_capture(config)
    }

    pub fn from_speaker_capture(
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        capture::open_speaker_capture(sample_rate, chunk_size)
    }

    pub fn from_mic_capture(
        device: Option<String>,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        capture::open_mic_capture(device, sample_rate, chunk_size)
    }

    pub fn from_mic(device_name: Option<String>) -> Result<Self, Error> {
        let mic = MicInput::new(device_name)?;

        Ok(Self {
            source: AudioSource::RealtimeMic,
            mic: Some(mic),
            speaker: None,
            data: None,
        })
    }

    pub fn from_speaker() -> Self {
        Self {
            source: AudioSource::RealtimeSpeaker,
            mic: None,
            speaker: Some(SpeakerInput::new().unwrap()),
            data: None,
        }
    }

    pub fn device_name(&self) -> String {
        match &self.source {
            AudioSource::RealtimeMic => self.mic.as_ref().unwrap().device_name(),
            AudioSource::RealtimeSpeaker => "RealtimeSpeaker".to_string(),
            AudioSource::Recorded => "Recorded".to_string(),
        }
    }

    pub fn stream(&mut self) -> AudioStream {
        match &self.source {
            AudioSource::RealtimeMic => AudioStream::RealtimeMic {
                mic: self.mic.as_ref().unwrap().stream(),
            },
            AudioSource::RealtimeSpeaker => AudioStream::RealtimeSpeaker {
                speaker: self.speaker.take().unwrap().stream().unwrap(),
            },
            AudioSource::Recorded => AudioStream::Recorded {
                data: self.data.as_ref().unwrap().clone(),
                position: 0,
            },
        }
    }
}

pub enum AudioStream {
    RealtimeMic { mic: MicStream },
    RealtimeSpeaker { speaker: SpeakerStream },
    Recorded { data: Vec<u8>, position: usize },
}

impl Stream for AudioStream {
    type Item = f32;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use futures_util::StreamExt;
        use std::task::Poll;

        match &mut *self {
            AudioStream::RealtimeMic { mic } => mic.poll_next_unpin(cx),
            AudioStream::RealtimeSpeaker { speaker } => speaker.poll_next_unpin(cx),
            AudioStream::Recorded { data, position } => {
                if *position + 2 <= data.len() {
                    let bytes = [data[*position], data[*position + 1]];
                    let sample = i16::from_le_bytes(bytes) as f32 / 32768.0;
                    *position += 2;

                    std::thread::sleep(std::time::Duration::from_secs_f64(1.0 / 16000.0));
                    Poll::Ready(Some(sample))
                } else {
                    Poll::Ready(None)
                }
            }
        }
    }
}

impl AsyncSource for AudioStream {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        self
    }

    fn sample_rate(&self) -> u32 {
        match self {
            AudioStream::RealtimeMic { mic } => mic.sample_rate(),
            AudioStream::RealtimeSpeaker { speaker } => speaker.sample_rate(),
            AudioStream::Recorded { .. } => 16000,
        }
    }
}

pub struct ActualAudio;

impl AudioProvider for ActualAudio {
    fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
        capture::open_capture(config)
    }

    fn open_speaker_capture(
        &self,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        capture::open_speaker_capture(sample_rate, chunk_size)
    }

    fn open_mic_capture(
        &self,
        device: Option<String>,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        capture::open_mic_capture(device, sample_rate, chunk_size)
    }

    fn default_device_name(&self) -> String {
        AudioInput::get_default_device_name()
    }

    fn list_mic_devices(&self) -> Vec<String> {
        AudioInput::list_mic_devices()
    }

    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        AudioOutput::silence()
    }

    fn play_bytes(&self, bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        AudioOutput::to_speaker(bytes)
    }

    fn probe_mic(&self, device: Option<String>) -> Result<(), Error> {
        let mut input = AudioInput::from_mic(device)?;
        let _stream = input.stream();
        Ok(())
    }

    fn probe_speaker(&self) -> Result<(), Error> {
        let speaker = SpeakerInput::new().map_err(|_| Error::SpeakerStreamSetupFailed)?;
        let _stream = speaker
            .stream()
            .map_err(|_| Error::SpeakerStreamSetupFailed)?;
        Ok(())
    }
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn play_sine_for_sec(seconds: u64) -> std::thread::JoinHandle<()> {
    use rodio::{
        Player, nz,
        source::{Function::Sine, SignalGenerator, Source},
        stream::DeviceSinkBuilder,
    };
    use std::{
        thread::{sleep, spawn},
        time::Duration,
    };

    spawn(move || {
        let stream = DeviceSinkBuilder::open_default_sink().unwrap();
        let source = SignalGenerator::new(nz!(44100u32), 440.0, Sine);

        let source = source
            .take_duration(Duration::from_secs(seconds))
            .amplify(0.01);

        let player = Player::connect_new(stream.mixer());
        player.append(source);
        sleep(Duration::from_secs(seconds));
    })
}
