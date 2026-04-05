use std::marker::PhantomData;

use mp3lame_encoder::{Builder as LameBuilder, DualPcm, FlushNoGap, MonoPcm};

use crate::Error;

pub struct Mono;
pub struct Stereo;

pub type MonoStreamEncoder = StreamEncoder<Mono>;
pub type StereoStreamEncoder = StreamEncoder<Stereo>;

pub struct StreamEncoder<M> {
    core: EncoderCore,
    _marker: PhantomData<M>,
}

struct EncoderCore {
    encoder: mp3lame_encoder::Encoder,
}

impl<M> StreamEncoder<M> {
    fn with_channels(sample_rate: u32, channels: u8) -> Result<Self, Error> {
        Ok(Self {
            core: EncoderCore::new(sample_rate, channels)?,
            _marker: PhantomData,
        })
    }

    pub fn flush(&mut self, output: &mut Vec<u8>) -> Result<(), Error> {
        self.core.flush(output)
    }
}

impl StreamEncoder<Mono> {
    pub fn new(sample_rate: u32) -> Result<Self, Error> {
        Self::with_channels(sample_rate, 1)
    }

    pub fn encode_f32(&mut self, samples: &[f32], output: &mut Vec<u8>) -> Result<(), Error> {
        if samples.is_empty() {
            return Ok(());
        }

        let mut pcm = Vec::with_capacity(samples.len());
        pcm.extend(samples.iter().copied().map(f32_to_i16));
        self.encode_i16(&pcm, output)
    }

    pub fn encode_i16(&mut self, samples: &[i16], output: &mut Vec<u8>) -> Result<(), Error> {
        if samples.is_empty() {
            return Ok(());
        }

        output.reserve(mp3lame_encoder::max_required_buffer_size(samples.len()));
        self.core
            .encoder
            .encode_to_vec(MonoPcm(samples), output)
            .map_err(|e| Error::LameEncode(format!("{:?}", e)))?;
        Ok(())
    }
}

impl StreamEncoder<Stereo> {
    pub fn new(sample_rate: u32) -> Result<Self, Error> {
        Self::with_channels(sample_rate, 2)
    }

    pub fn encode_f32(
        &mut self,
        left: &[f32],
        right: &[f32],
        output: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let frames = left.len().max(right.len());
        if frames == 0 {
            return Ok(());
        }

        let mut left_pcm = Vec::with_capacity(frames);
        let mut right_pcm = Vec::with_capacity(frames);

        for i in 0..frames {
            left_pcm.push(f32_to_i16(left.get(i).copied().unwrap_or(0.0)));
            right_pcm.push(f32_to_i16(right.get(i).copied().unwrap_or(0.0)));
        }

        self.encode_i16(&left_pcm, &right_pcm, output)
    }

    pub fn encode_i16(
        &mut self,
        left: &[i16],
        right: &[i16],
        output: &mut Vec<u8>,
    ) -> Result<(), Error> {
        if left.is_empty() && right.is_empty() {
            return Ok(());
        }

        let frames = left.len().max(right.len());
        let mut left_pcm = Vec::with_capacity(frames);
        let mut right_pcm = Vec::with_capacity(frames);

        for i in 0..frames {
            left_pcm.push(left.get(i).copied().unwrap_or(0));
            right_pcm.push(right.get(i).copied().unwrap_or(0));
        }

        output.reserve(mp3lame_encoder::max_required_buffer_size(frames));
        self.core
            .encoder
            .encode_to_vec(
                DualPcm {
                    left: &left_pcm,
                    right: &right_pcm,
                },
                output,
            )
            .map_err(|e| Error::LameEncode(format!("{:?}", e)))?;
        Ok(())
    }
}

impl EncoderCore {
    fn new(sample_rate: u32, channels: u8) -> Result<Self, Error> {
        let bitrate = bitrate_for_channels(channels)?;
        let mut mp3_builder = LameBuilder::new().ok_or(Error::LameInit)?;
        mp3_builder
            .set_num_channels(channels)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;
        mp3_builder
            .set_sample_rate(sample_rate)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;
        mp3_builder
            .set_brate(bitrate)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;
        mp3_builder
            .set_quality(mp3lame_encoder::Quality::NearBest)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;

        Ok(Self {
            encoder: mp3_builder
                .build()
                .map_err(|e| Error::LameBuild(format!("{:?}", e)))?,
        })
    }

    fn flush(&mut self, output: &mut Vec<u8>) -> Result<(), Error> {
        output.reserve(mp3lame_encoder::max_required_buffer_size(0));
        self.encoder
            .flush_to_vec::<FlushNoGap>(output)
            .map_err(|e| Error::LameFlush(format!("{:?}", e)))?;
        Ok(())
    }
}

pub(crate) use hypr_audio_utils::f32_to_i16;

fn bitrate_for_channels(channels: u8) -> Result<mp3lame_encoder::Bitrate, Error> {
    match channels {
        1 => Ok(mp3lame_encoder::Bitrate::Kbps64),
        2 => Ok(mp3lame_encoder::Bitrate::Kbps128),
        count => Err(Error::UnsupportedChannelCount(count.into())),
    }
}

pub(crate) fn int_to_i16(sample: i32, bits_per_sample: u16) -> i16 {
    let max_amplitude = match bits_per_sample {
        0 | 1 => return 0,
        32.. => i32::MAX as f32,
        bits => ((1i64 << (bits - 1)) - 1) as f32,
    };
    f32_to_i16(sample as f32 / max_amplitude)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_to_i16_clamps_out_of_range_values() {
        assert_eq!(f32_to_i16(-2.0), -i16::MAX);
        assert_eq!(f32_to_i16(2.0), i16::MAX);
    }

    #[test]
    fn int_to_i16_scales_32_bit_extremes() {
        assert_eq!(int_to_i16(i32::MAX, 32), i16::MAX);
        assert_eq!(int_to_i16(i32::MIN, 32), -i16::MAX);
    }

    #[test]
    fn int_to_i16_handles_single_bit_depth() {
        assert_eq!(int_to_i16(1, 1), 0);
    }

    #[test]
    fn mono_stream_encoder_encodes_memory_buffer() -> Result<(), Error> {
        let mut encoder = MonoStreamEncoder::new(16_000)?;
        let mut output = Vec::new();

        encoder.encode_f32(&[0.1; 16_000], &mut output)?;
        encoder.flush(&mut output)?;

        assert!(!output.is_empty());
        Ok(())
    }

    #[test]
    fn stereo_stream_encoder_encodes_memory_buffer() -> Result<(), Error> {
        let mut encoder = StereoStreamEncoder::new(48_000)?;
        let mut output = Vec::new();

        encoder.encode_f32(&[0.1; 16_000], &[0.2; 16_000], &mut output)?;
        encoder.flush(&mut output)?;

        assert!(!output.is_empty());
        Ok(())
    }
}
