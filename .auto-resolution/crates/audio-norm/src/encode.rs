use std::io::Write;
use std::num::NonZeroU8;
use std::time::Duration;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use hypr_audio_utils::{Source, f32_to_i16, mono_frames};
use hypr_resampler::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::Error;

pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
const RESAMPLE_CHUNK_SIZE: usize = 1024;
const MONO_ENCODE_CHUNK_SIZE: usize = 4096;
const TARGET_MP3_BYTES_PER_SECOND_MONO: usize = 64_000 / 8;
const TARGET_MP3_BYTES_PER_SECOND_STEREO: usize = 128_000 / 8;
const MP3_BUFFER_OVERHEAD_BYTES: usize = 4096;

pub(crate) fn encode_source_to_mp3<S, W>(
    source: S,
    max_duration: Option<Duration>,
    output: W,
    mut on_progress: Option<&mut dyn FnMut(f64)>,
) -> Result<usize, Error>
where
    S: Source<Item = f32>,
    W: Write,
{
    let source_rate: u32 = source.sample_rate().into();
    let channel_count_raw: u16 = source.channels().into();
    let input_duration = source.total_duration();
    let channel_count_raw = channel_count_raw.max(1);
    let channel_count_u8 =
        u8::try_from(channel_count_raw).map_err(|_| Error::UnsupportedChannelCount {
            count: channel_count_raw,
        })?;
    let channel_count = NonZeroU8::new(channel_count_u8).ok_or(Error::InvalidChannelCount)?;

    let effective_duration = max_duration
        .map(|max| input_duration.map_or(max, |inp| inp.min(max)))
        .or(input_duration);
    let channel_count = usize::from(channel_count.get());
    let needs_resample = source_rate != TARGET_SAMPLE_RATE_HZ;
    let mut saw_input = false;
    let mut remaining_frames = max_duration
        .map(|duration| max_frames_for_duration(source_rate, duration))
        .unwrap_or(usize::MAX);

    let total_frames = effective_duration.map(|d| {
        let frames = d.as_secs_f64() * source_rate as f64;
        frames.ceil() as usize
    });
    let mut processed_frames: usize = 0;

    if channel_count == 2 {
        let mut encoder =
            hypr_mp3::StereoStreamEncoder::new(TARGET_SAMPLE_RATE_HZ).map_err(mp3_err)?;
        let mut output = Mp3Output::new(
            output,
            estimated_mp3_capacity(effective_duration, TARGET_MP3_BYTES_PER_SECOND_STEREO),
        );
        let mut source_iter = source.into_iter();

        if needs_resample {
            let mut left_state = ResamplerState::new(source_rate)?;
            let mut right_state = ResamplerState::new(source_rate)?;
            let mut expected_output_frames = 0usize;
            let mut left_pcm = Vec::with_capacity(RESAMPLE_CHUNK_SIZE);
            let mut right_pcm = Vec::with_capacity(RESAMPLE_CHUNK_SIZE);

            loop {
                let Some(left) = source_iter.next() else {
                    break;
                };
                let right = source_iter.next().unwrap_or(0.0);
                if remaining_frames == 0 {
                    break;
                }
                remaining_frames -= 1;
                saw_input = true;
                expected_output_frames += 1;
                processed_frames += 1;
                left_state.input_buf[0].push(left);
                right_state.input_buf[0].push(right);

                if left_state.input_buf[0].len() < left_state.resampler.input_frames_next() {
                    continue;
                }

                let left_frames = left_state.process_resample(None)?;
                let right_frames = right_state.process_resample(None)?;
                if !left_frames.is_empty() || !right_frames.is_empty() {
                    encode_stereo_chunk(
                        &mut encoder,
                        &left_frames,
                        &right_frames,
                        &mut left_pcm,
                        &mut right_pcm,
                        &mut output,
                    )?;
                }

                if let Some(total) = total_frames
                    && let Some(ref mut cb) = on_progress
                {
                    cb(processed_frames as f64 / total as f64);
                }
            }

            if !saw_input {
                return Ok(0);
            }

            let expected_output_frames =
                (expected_output_frames as f64 * TARGET_SAMPLE_RATE_HZ as f64 / source_rate as f64)
                    .ceil() as usize;

            if !left_state.input_buf[0].is_empty() {
                let partial_len = left_state.input_buf[0].len();
                let left_frames = left_state.process_resample(Some(partial_len))?;
                let right_frames = right_state.process_resample(Some(partial_len))?;
                if !left_frames.is_empty() || !right_frames.is_empty() {
                    encode_stereo_chunk(
                        &mut encoder,
                        &left_frames,
                        &right_frames,
                        &mut left_pcm,
                        &mut right_pcm,
                        &mut output,
                    )?;
                }
            }

            while left_state.written_frames < expected_output_frames {
                let left_frames = left_state.process_resample(Some(0))?;
                let right_frames = right_state.process_resample(Some(0))?;
                if !left_frames.is_empty() || !right_frames.is_empty() {
                    encode_stereo_chunk(
                        &mut encoder,
                        &left_frames,
                        &right_frames,
                        &mut left_pcm,
                        &mut right_pcm,
                        &mut output,
                    )?;
                }
            }
        } else {
            let mut left_chunk = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);
            let mut right_chunk = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);
            let mut left_pcm = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);
            let mut right_pcm = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);

            loop {
                let Some(left) = source_iter.next() else {
                    break;
                };
                let right = source_iter.next().unwrap_or(0.0);
                if remaining_frames == 0 {
                    break;
                }
                remaining_frames -= 1;
                saw_input = true;
                processed_frames += 1;
                left_chunk.push(left);
                right_chunk.push(right);

                if left_chunk.len() < MONO_ENCODE_CHUNK_SIZE {
                    continue;
                }

                encode_stereo_chunk(
                    &mut encoder,
                    &left_chunk,
                    &right_chunk,
                    &mut left_pcm,
                    &mut right_pcm,
                    &mut output,
                )?;
                left_chunk.clear();
                right_chunk.clear();

                if let Some(total) = total_frames
                    && let Some(ref mut cb) = on_progress
                {
                    cb(processed_frames as f64 / total as f64);
                }
            }

            if !saw_input {
                return Ok(0);
            }

            if !left_chunk.is_empty() {
                encode_stereo_chunk(
                    &mut encoder,
                    &left_chunk,
                    &right_chunk,
                    &mut left_pcm,
                    &mut right_pcm,
                    &mut output,
                )?;
            }
        }

        if let Some(ref mut cb) = on_progress {
            cb(1.0);
        }
        encoder.flush(output.buffer()).map_err(mp3_err)?;
        output.flush()?;
        Ok(output.bytes_written())
    } else {
        let mut encoder =
            hypr_mp3::MonoStreamEncoder::new(TARGET_SAMPLE_RATE_HZ).map_err(mp3_err)?;
        let mut output = Mp3Output::new(
            output,
            estimated_mp3_capacity(effective_duration, TARGET_MP3_BYTES_PER_SECOND_MONO),
        );

        if needs_resample {
            let mut state = ResamplerState::new(source_rate)?;
            let mut expected_output_frames = 0usize;

            for mono_frame in mono_frames(source, channel_count) {
                if remaining_frames == 0 {
                    break;
                }
                remaining_frames -= 1;
                saw_input = true;
                expected_output_frames += 1;
                processed_frames += 1;
                state.input_buf[0].push(mono_frame);

                if state.input_buf[0].len() < state.resampler.input_frames_next() {
                    continue;
                }

                state.encode_chunk(&mut encoder, &mut output, None)?;

                if let Some(total) = total_frames
                    && let Some(ref mut cb) = on_progress
                {
                    cb(processed_frames as f64 / total as f64);
                }
            }

            if !saw_input {
                return Ok(0);
            }

            let expected_output_frames =
                (expected_output_frames as f64 * TARGET_SAMPLE_RATE_HZ as f64 / source_rate as f64)
                    .ceil() as usize;

            if !state.input_buf[0].is_empty() {
                let partial_len = state.input_buf[0].len();
                state.encode_chunk(&mut encoder, &mut output, Some(partial_len))?;
            }

            while state.written_frames < expected_output_frames {
                state.encode_chunk(&mut encoder, &mut output, Some(0))?;
            }
        } else {
            let mut mono_chunk = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);
            let mut mono_pcm = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);

            for mono_frame in mono_frames(source, channel_count) {
                if remaining_frames == 0 {
                    break;
                }
                remaining_frames -= 1;
                saw_input = true;
                processed_frames += 1;
                mono_chunk.push(mono_frame);

                if mono_chunk.len() < MONO_ENCODE_CHUNK_SIZE {
                    continue;
                }

                encode_mono_chunk(&mut encoder, &mono_chunk, &mut mono_pcm, &mut output)?;
                mono_chunk.clear();

                if let Some(total) = total_frames
                    && let Some(ref mut cb) = on_progress
                {
                    cb(processed_frames as f64 / total as f64);
                }
            }

            if !saw_input {
                return Ok(0);
            }

            if !mono_chunk.is_empty() {
                encode_mono_chunk(&mut encoder, &mono_chunk, &mut mono_pcm, &mut output)?;
            }
        }

        if let Some(ref mut cb) = on_progress {
            cb(1.0);
        }
        encoder.flush(output.buffer()).map_err(mp3_err)?;
        output.flush()?;
        Ok(output.bytes_written())
    }
}

fn mp3_err(e: hypr_mp3::Error) -> Error {
    Error::Mp3Encode(e.to_string())
}

fn estimated_mp3_capacity(duration: Option<Duration>, bytes_per_second: usize) -> usize {
    let Some(duration) = duration else {
        return 0;
    };

    let bytes_from_seconds = duration.as_secs().saturating_mul(bytes_per_second as u64);
    let bytes_from_nanos = (u64::from(duration.subsec_nanos())
        .saturating_mul(bytes_per_second as u64))
        / 1_000_000_000u64;
    let total_bytes = bytes_from_seconds
        .saturating_add(bytes_from_nanos)
        .saturating_add(MP3_BUFFER_OVERHEAD_BYTES as u64);

    total_bytes.min(usize::MAX as u64) as usize
}

fn max_frames_for_duration(source_rate: u32, duration: Duration) -> usize {
    let frames_from_seconds = u128::from(duration.as_secs()) * u128::from(source_rate);
    let frames_from_nanos =
        u128::from(duration.subsec_nanos()) * u128::from(source_rate) / 1_000_000_000u128;
    let total_frames = frames_from_seconds.saturating_add(frames_from_nanos);
    total_frames.min(usize::MAX as u128) as usize
}

fn create_mono_resampler(source_rate: u32) -> Result<Async<f32>, Error> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    Ok(Async::<f32>::new_sinc(
        TARGET_SAMPLE_RATE_HZ as f64 / source_rate as f64,
        2.0,
        &params,
        RESAMPLE_CHUNK_SIZE,
        1,
        FixedAsync::Input,
    )
    .map_err(hypr_resampler::Error::from)?)
}

struct ResamplerState {
    resampler: Async<f32>,
    input_buf: Vec<Vec<f32>>,
    output_buf: Vec<Vec<f32>>,
    mono_pcm: Vec<i16>,
    frames_to_trim: usize,
    written_frames: usize,
}

impl ResamplerState {
    fn new(source_rate: u32) -> Result<Self, Error> {
        let resampler = create_mono_resampler(source_rate)?;
        let output_max = resampler.output_frames_max();
        Ok(Self {
            input_buf: vec![Vec::with_capacity(RESAMPLE_CHUNK_SIZE)],
            output_buf: vec![vec![0.0; output_max]],
            mono_pcm: Vec::with_capacity(output_max),
            frames_to_trim: resampler.output_delay(),
            written_frames: 0,
            resampler,
        })
    }

    fn process_resample(&mut self, partial_len: Option<usize>) -> Result<Vec<f32>, Error> {
        let frames_needed = self.resampler.input_frames_next();
        if self.input_buf[0].len() < frames_needed {
            self.input_buf[0].resize(frames_needed, 0.0);
        }

        let frames_in = self.input_buf[0].len();
        let input_adapter =
            SequentialSliceOfVecs::new(&self.input_buf, 1, frames_in).expect("input adapter");
        let frames_out = self.output_buf[0].len();
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut self.output_buf, 1, frames_out)
                .expect("output adapter");
        let indexing = partial_len.map(|partial_len| Indexing {
            input_offset: 0,
            output_offset: 0,
            partial_len: Some(partial_len),
            active_channels_mask: None,
        });
        let (_, produced_frames) = self
            .resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, indexing.as_ref())
            .map_err(hypr_resampler::Error::from)?;
        self.input_buf[0].clear();

        if produced_frames == 0 {
            return Ok(Vec::new());
        }

        let trim = self.frames_to_trim.min(produced_frames);
        self.frames_to_trim -= trim;

        let frames = &self.output_buf[0][trim..produced_frames];
        self.written_frames += frames.len();
        Ok(frames.to_vec())
    }

    fn encode_chunk(
        &mut self,
        encoder: &mut hypr_mp3::MonoStreamEncoder,
        output: &mut Mp3Output<impl Write>,
        partial_len: Option<usize>,
    ) -> Result<(), Error> {
        let frames = self.process_resample(partial_len)?;
        if !frames.is_empty() {
            encode_mono_chunk(encoder, &frames, &mut self.mono_pcm, output)?;
        }
        Ok(())
    }
}

fn encode_mono_chunk<W: Write>(
    encoder: &mut hypr_mp3::MonoStreamEncoder,
    samples: &[f32],
    mono_pcm: &mut Vec<i16>,
    output: &mut Mp3Output<W>,
) -> Result<(), Error> {
    if samples.is_empty() {
        return Ok(());
    }

    mono_pcm.clear();
    mono_pcm.extend(samples.iter().copied().map(f32_to_i16));
    encoder
        .encode_i16(mono_pcm, output.buffer())
        .map_err(mp3_err)?;
    output.flush()
}

fn encode_stereo_chunk<W: Write>(
    encoder: &mut hypr_mp3::StereoStreamEncoder,
    left: &[f32],
    right: &[f32],
    left_pcm: &mut Vec<i16>,
    right_pcm: &mut Vec<i16>,
    output: &mut Mp3Output<W>,
) -> Result<(), Error> {
    if left.is_empty() && right.is_empty() {
        return Ok(());
    }

    left_pcm.clear();
    left_pcm.extend(left.iter().copied().map(f32_to_i16));
    right_pcm.clear();
    right_pcm.extend(right.iter().copied().map(f32_to_i16));
    encoder
        .encode_i16(left_pcm, right_pcm, output.buffer())
        .map_err(mp3_err)?;
    output.flush()
}

struct Mp3Output<W> {
    writer: W,
    buffer: Vec<u8>,
    bytes_written: usize,
}

impl<W: Write> Mp3Output<W> {
    fn new(writer: W, estimated_total_bytes: usize) -> Self {
        Self {
            writer,
            buffer: Vec::with_capacity(estimated_total_bytes.min(MP3_BUFFER_OVERHEAD_BYTES)),
            bytes_written: 0,
        }
    }

    fn buffer(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    fn flush(&mut self) -> Result<(), Error> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        self.writer.write_all(&self.buffer)?;
        self.bytes_written += self.buffer.len();
        self.buffer.clear();
        Ok(())
    }

    fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_MP3_BYTES: usize = 1024;

    fn make_source(
        channels: u16,
        sample_rate: u32,
        duration_secs: usize,
    ) -> rodio::buffer::SamplesBuffer {
        let channels_nz = std::num::NonZeroU16::new(channels).unwrap();
        let rate_nz = std::num::NonZeroU32::new(sample_rate).unwrap();
        let total_samples = sample_rate as usize * duration_secs * channels as usize;
        rodio::buffer::SamplesBuffer::new(channels_nz, rate_nz, vec![0.5f32; total_samples])
    }

    fn decode_mp3_bytes(bytes: &[u8]) -> (u32, u16, Vec<f32>) {
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.path().join("test.mp3");
        std::fs::write(&path, bytes).unwrap();
        let decoder = rodio::Decoder::try_from(std::fs::File::open(&path).unwrap()).unwrap();
        let rate: u32 = decoder.sample_rate().into();
        let ch: u16 = decoder.channels().into();
        let samples: Vec<f32> = decoder.collect();
        (rate, ch, samples)
    }

    #[test]
    fn test_encode_mono_no_resample() {
        let source = make_source(1, TARGET_SAMPLE_RATE_HZ, 2);
        let mut bytes = Vec::new();
        let written = encode_source_to_mp3(source, None, &mut bytes, None).unwrap();
        assert_eq!(written, bytes.len());
        assert!(bytes.len() > MIN_MP3_BYTES);

        let (rate, ch, _) = decode_mp3_bytes(&bytes);
        assert_eq!(rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(ch, 1);
    }

    #[test]
    fn test_encode_mono_with_resample() {
        let source = make_source(1, 44_100, 3);
        let mut bytes = Vec::new();
        let written = encode_source_to_mp3(source, None, &mut bytes, None).unwrap();
        assert_eq!(written, bytes.len());
        assert!(bytes.len() > MIN_MP3_BYTES);

        let (rate, ch, samples) = decode_mp3_bytes(&bytes);
        assert_eq!(rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(ch, 1);

        let actual_frames = samples.len();
        let expected_frames = TARGET_SAMPLE_RATE_HZ as usize * 3;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})",
        );
    }

    #[test]
    fn test_encode_stereo_no_resample() {
        let source = make_source(2, TARGET_SAMPLE_RATE_HZ, 2);
        let mut bytes = Vec::new();
        let written = encode_source_to_mp3(source, None, &mut bytes, None).unwrap();
        assert_eq!(written, bytes.len());
        assert!(bytes.len() > MIN_MP3_BYTES);

        let (rate, ch, _) = decode_mp3_bytes(&bytes);
        assert_eq!(rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(ch, 2);
    }

    #[test]
    fn test_encode_stereo_with_resample() {
        let source = make_source(2, 44_100, 5);
        let mut bytes = Vec::new();
        let written = encode_source_to_mp3(source, None, &mut bytes, None).unwrap();
        assert_eq!(written, bytes.len());
        assert!(bytes.len() > MIN_MP3_BYTES);

        let (rate, ch, samples) = decode_mp3_bytes(&bytes);
        assert_eq!(rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(ch, 2);

        let actual_frames = samples.len() / 2;
        let expected_frames = TARGET_SAMPLE_RATE_HZ as usize * 5;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})",
        );
    }

    #[test]
    fn test_encode_empty_source_returns_zero() {
        let source = make_source(1, TARGET_SAMPLE_RATE_HZ, 0);
        let mut bytes = Vec::new();
        let written = encode_source_to_mp3(source, None, &mut bytes, None).unwrap();
        assert_eq!(written, 0);
    }

    #[test]
    fn test_encode_with_max_duration_truncates() {
        let source = make_source(1, 44_100, 10);
        let max = Some(Duration::from_secs(2));
        let mut bytes = Vec::new();
        encode_source_to_mp3(source, max, &mut bytes, None).unwrap();
        assert!(bytes.len() > MIN_MP3_BYTES);

        let (_, _, samples) = decode_mp3_bytes(&bytes);
        let actual_frames = samples.len();
        let expected_frames = TARGET_SAMPLE_RATE_HZ as usize * 2;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.10,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})",
        );
    }

    #[test]
    fn test_progress_callback_fires() {
        let source = make_source(1, 44_100, 3);
        let mut last_value = 0.0f64;
        let mut call_count = 0usize;
        let mut bytes = Vec::new();

        {
            let mut cb = |p: f64| {
                last_value = p;
                call_count += 1;
            };
            encode_source_to_mp3(source, None, &mut bytes, Some(&mut cb)).unwrap();
        }

        assert!(call_count > 0, "progress callback was never called");
        assert!(
            (last_value - 1.0).abs() < f64::EPSILON,
            "final progress should be 1.0, got {last_value}",
        );
    }
}
