use std::fs::File;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::time::Duration;

use anlg_audio_utils::Source;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_OPUS, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia_adapter_libopus::OpusDecoder;

const OPUS_SAMPLE_RATE_HZ: u32 = 48_000;

// Symphonia (and therefore rodio) ships no Opus decoder, so Ogg/WebM Opus files such as
// WhatsApp voice notes are demuxed here and decoded with libopus instead.
pub(crate) struct OpusSource {
    format: Box<dyn FormatReader>,
    decoder: OpusDecoder,
    track_id: u32,
    channels: NonZeroU16,
    sample_rate: NonZeroU32,
    total_duration: Option<Duration>,
    buffer: Vec<f32>,
    position: usize,
}

impl OpusSource {
    pub(crate) fn open(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }

        let format = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                // Gapless mode makes the Ogg demuxer mark the encoder's end padding for trimming.
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .ok()?
            .format;
        let track = format
            .tracks()
            .iter()
            .find(|track| track.codec_params.codec == CODEC_TYPE_OPUS)?;
        let mut params = track.codec_params.clone();
        // The WebM demuxer only reports a channel layout, and may report the encoder's input
        // rate. Opus always decodes at 48 kHz, which is also the unit of its pre-skip.
        if params.channels.is_none() {
            params.channels = params.channel_layout.map(|layout| layout.into_channels());
        }
        params.sample_rate = Some(OPUS_SAMPLE_RATE_HZ);

        let channels = u16::try_from(params.channels?.count()).ok()?;
        let total_duration = params
            .time_base
            .zip(params.n_frames)
            .map(|(time_base, frames)| {
                let time = time_base.calc_time(frames);
                Duration::from_secs(time.seconds) + Duration::from_secs_f64(time.frac)
            });

        Some(Self {
            decoder: OpusDecoder::try_new(&params, &DecoderOptions::default()).ok()?,
            track_id: track.id,
            channels: NonZeroU16::new(channels)?,
            sample_rate: NonZeroU32::new(OPUS_SAMPLE_RATE_HZ)?,
            total_duration,
            format,
            buffer: Vec::new(),
            position: 0,
        })
    }

    fn decode_next_packet(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(_) => return false,
            };
            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let mut samples =
                        SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                    samples.copy_interleaved_ref(decoded);
                    self.buffer.clear();
                    self.buffer.extend_from_slice(samples.samples());
                    self.position = 0;
                    if !self.buffer.is_empty() {
                        return true;
                    }
                }
                // A corrupt packet should not end the whole import, matching rodio's decoders.
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => return false,
            }
        }
    }
}

impl Iterator for OpusSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.position >= self.buffer.len() && !self.decode_next_packet() {
            return None;
        }

        let sample = self.buffer[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl Source for OpusSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }
}

#[cfg(test)]
mod tests {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::formats::Packet;

    use super::*;

    #[test]
    fn ogg_opus_decodes_without_pre_skip_or_end_padding() {
        let source = OpusSource::open(Path::new(anlg_data::english_1::AUDIO_OPUS_PATH)).unwrap();

        // ffmpeg decodes this mono fixture to 8,375,313 samples at 48 kHz.
        assert_eq!(source.count(), 8_375_313);
    }

    #[test]
    fn decoder_accepts_120_ms_packets() {
        let mut params = CodecParameters::new();
        params
            .for_codec(CODEC_TYPE_OPUS)
            .with_sample_rate(OPUS_SAMPLE_RATE_HZ)
            .with_channels(symphonia::core::audio::Channels::FRONT_LEFT);
        let mut decoder = OpusDecoder::try_new(&params, &DecoderOptions::default()).unwrap();

        // TOC 0xFB: CELT fullband 20 ms, mono, code 3. Frame count byte 0x06: six empty CBR
        // frames, which libopus conceals, so the packet decodes to 6 x 20 ms = 120 ms.
        let packet = Packet::new_from_slice(0, 0, 5_760, &[0xFB, 0x06]);
        let decoded = decoder.decode(&packet).unwrap();

        assert_eq!(decoded.frames(), 5_760);
    }
}
