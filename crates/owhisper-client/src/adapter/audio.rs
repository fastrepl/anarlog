use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use hypr_resampler::{
    Async, FixedAsync, RubatoChunkResampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use hypr_ws_client::client::Message;

use crate::Error;

const I16_SCALE: f32 = 32768.0;
const RESAMPLER_CHUNK_SIZE: usize = 1024;

#[derive(Debug)]
pub enum RealtimeAudioInput {
    Mono(Bytes),
    Stereo { mic: Bytes, speaker: Bytes },
}

pub trait RealtimeAudioEncoder: Send {
    fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, Error>;
    fn flush(&mut self) -> Result<Vec<Message>, Error>;
}

type Serializer = Arc<dyn Fn(Bytes) -> Result<Message, Error> + Send + Sync>;

pub struct MessageAudioEncoder {
    serialize: Serializer,
}

impl MessageAudioEncoder {
    pub fn new<F>(serialize: F) -> Self
    where
        F: Fn(Bytes) -> Result<Message, Error> + Send + Sync + 'static,
    {
        Self {
            serialize: Arc::new(serialize),
        }
    }

    pub fn binary() -> Self {
        Self::new(|audio| Ok(Message::Binary(audio)))
    }
}

impl RealtimeAudioEncoder for MessageAudioEncoder {
    fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, Error> {
        let audio = require_mono(input)?;
        Ok(vec![(self.serialize)(audio)?])
    }

    fn flush(&mut self) -> Result<Vec<Message>, Error> {
        Ok(Vec::new())
    }
}

pub struct InterleavedStereoEncoder {
    serialize: Serializer,
}

impl InterleavedStereoEncoder {
    pub fn new<F>(serialize: F) -> Self
    where
        F: Fn(Bytes) -> Result<Message, Error> + Send + Sync + 'static,
    {
        Self {
            serialize: Arc::new(serialize),
        }
    }

    pub fn binary() -> Self {
        Self::new(|audio| Ok(Message::Binary(audio)))
    }
}

impl RealtimeAudioEncoder for InterleavedStereoEncoder {
    fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, Error> {
        let (mic, speaker) = require_stereo(input)?;
        let interleaved = interleave_audio(&mic, &speaker);
        Ok(vec![(self.serialize)(interleaved)?])
    }

    fn flush(&mut self) -> Result<Vec<Message>, Error> {
        Ok(Vec::new())
    }
}

pub struct ResamplingMonoEncoder {
    serialize: Serializer,
    mode: ResamplingMode,
}

impl ResamplingMonoEncoder {
    pub fn new<F>(
        input_sample_rate: u32,
        output_sample_rate: u32,
        serialize: F,
    ) -> Result<Self, Error>
    where
        F: Fn(Bytes) -> Result<Message, Error> + Send + Sync + 'static,
    {
        let mode = if input_sample_rate == output_sample_rate {
            ResamplingMode::Passthrough
        } else {
            ResamplingMode::Resampler(MonoPcm16Resampler::new(
                input_sample_rate,
                output_sample_rate,
            )?)
        };
        Ok(Self {
            serialize: Arc::new(serialize),
            mode,
        })
    }
}

impl RealtimeAudioEncoder for ResamplingMonoEncoder {
    fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, Error> {
        let audio = require_mono(input)?;
        match &mut self.mode {
            ResamplingMode::Passthrough => Ok(vec![(self.serialize)(audio)?]),
            ResamplingMode::Resampler(resampler) => {
                let Some(audio) = resampler.push_bytes(audio)? else {
                    return Ok(Vec::new());
                };
                Ok(vec![(self.serialize)(audio)?])
            }
        }
    }

    fn flush(&mut self) -> Result<Vec<Message>, Error> {
        match &mut self.mode {
            ResamplingMode::Passthrough => Ok(Vec::new()),
            ResamplingMode::Resampler(resampler) => {
                let Some(audio) = resampler.flush()? else {
                    return Ok(Vec::new());
                };
                Ok(vec![(self.serialize)(audio)?])
            }
        }
    }
}

enum ResamplingMode {
    Passthrough,
    Resampler(MonoPcm16Resampler),
}

fn require_mono(input: RealtimeAudioInput) -> Result<Bytes, Error> {
    match input {
        RealtimeAudioInput::Mono(audio) => Ok(audio),
        RealtimeAudioInput::Stereo { .. } => Err(Error::AudioProcessing(
            "expected mono audio input".to_string(),
        )),
    }
}

fn require_stereo(input: RealtimeAudioInput) -> Result<(Bytes, Bytes), Error> {
    match input {
        RealtimeAudioInput::Stereo { mic, speaker } => Ok((mic, speaker)),
        RealtimeAudioInput::Mono(_) => Err(Error::AudioProcessing(
            "expected stereo audio input".to_string(),
        )),
    }
}

fn interleave_audio(mic: &[u8], speaker: &[u8]) -> Bytes {
    let mic_samples: Vec<i16> = mic
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let speaker_samples: Vec<i16> = speaker
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let max_len = mic_samples.len().max(speaker_samples.len());
    let mut interleaved = Vec::with_capacity(max_len * 2 * 2);

    for i in 0..max_len {
        let mic_sample = mic_samples.get(i).copied().unwrap_or(0);
        let speaker_sample = speaker_samples.get(i).copied().unwrap_or(0);
        interleaved.extend_from_slice(&mic_sample.to_le_bytes());
        interleaved.extend_from_slice(&speaker_sample.to_le_bytes());
    }

    interleaved.into()
}

struct MonoPcm16Resampler {
    inner: RubatoChunkResampler<Async<f32>, 1>,
}

impl MonoPcm16Resampler {
    fn new(input_sample_rate: u32, output_sample_rate: u32) -> Result<Self, Error> {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = output_sample_rate as f64 / input_sample_rate as f64;
        let resampler = Async::<f32>::new_sinc(
            ratio,
            2.0,
            &params,
            RESAMPLER_CHUNK_SIZE,
            1,
            FixedAsync::Input,
        )
        .map_err(map_resampler_error)?;

        Ok(Self {
            inner: RubatoChunkResampler::new(resampler, RESAMPLER_CHUNK_SIZE, RESAMPLER_CHUNK_SIZE),
        })
    }

    fn push_bytes(&mut self, audio: Bytes) -> Result<Option<Bytes>, Error> {
        for chunk in audio.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / I16_SCALE;
            self.inner.push_sample(sample);
        }

        self.inner
            .process_all_ready_blocks()
            .map_err(map_resampler_error)?;
        Ok(Self::encode_output(self.inner.take_all_output()))
    }

    fn flush(&mut self) -> Result<Option<Bytes>, Error> {
        if self.inner.has_input() {
            self.inner
                .process_partial_block(true)
                .map_err(map_resampler_error)?;
        }

        Ok(Self::encode_output(self.inner.take_all_output()))
    }

    fn encode_output(samples: Option<Vec<f32>>) -> Option<Bytes> {
        let samples = samples?;
        if samples.is_empty() {
            return None;
        }

        let mut buf = BytesMut::with_capacity(samples.len() * std::mem::size_of::<i16>());
        for sample in samples {
            let scaled = (sample * I16_SCALE).clamp(-I16_SCALE, I16_SCALE);
            buf.put_i16_le(scaled as i16);
        }

        Some(buf.freeze())
    }
}

fn map_resampler_error(error: impl std::fmt::Display) -> Error {
    Error::AudioProcessing(format!("realtime resampler error: {error}"))
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use hypr_ws_client::client::Message;

    use super::{
        InterleavedStereoEncoder, MessageAudioEncoder, RealtimeAudioEncoder, RealtimeAudioInput,
        ResamplingMonoEncoder,
    };

    #[test]
    fn message_audio_encoder_preserves_mono_bytes() {
        let mut encoder = MessageAudioEncoder::binary();
        let output = encoder
            .push(RealtimeAudioInput::Mono(Bytes::from_static(b"mono")))
            .unwrap();

        assert_eq!(output.len(), 1);
        let Message::Binary(audio) = &output[0] else {
            panic!("expected binary message");
        };
        assert_eq!(audio.as_ref(), b"mono");
    }

    #[test]
    fn interleaved_stereo_encoder_interleaves_pcm16() {
        let mut encoder = InterleavedStereoEncoder::binary();
        let output = encoder
            .push(RealtimeAudioInput::Stereo {
                mic: Bytes::from_static(&[1, 0, 2, 0]),
                speaker: Bytes::from_static(&[3, 0, 4, 0]),
            })
            .unwrap();

        assert_eq!(output.len(), 1);
        let Message::Binary(audio) = &output[0] else {
            panic!("expected binary message");
        };
        assert_eq!(audio.as_ref(), &[1, 0, 3, 0, 2, 0, 4, 0]);
    }

    #[test]
    fn resampling_mono_encoder_passthroughs_same_rate_audio() {
        let mut encoder =
            ResamplingMonoEncoder::new(24_000, 24_000, |audio| Ok(Message::Binary(audio))).unwrap();

        let output = encoder
            .push(RealtimeAudioInput::Mono(Bytes::from_static(&[1, 0, 2, 0])))
            .unwrap();

        assert_eq!(output.len(), 1);
        let Message::Binary(audio) = &output[0] else {
            panic!("expected binary message");
        };
        assert_eq!(audio.as_ref(), &[1, 0, 2, 0]);
        assert!(encoder.flush().unwrap().is_empty());
    }
}
