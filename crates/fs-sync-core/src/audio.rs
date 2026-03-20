use std::fs::{File, copy, remove_file, rename};
use std::io::{BufWriter, ErrorKind, Write};
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::time::Duration;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use hypr_audio_utils::{Source, f32_to_i16, mono_frames};
use hypr_resampler::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::error::{AudioImportError, AudioProcessingError};
use crate::runtime::{AudioImportEvent, AudioImportRuntime};

const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
const AUDIO_FORMATS: [&str; 3] = ["audio.mp3", "audio.wav", "audio.ogg"];
const RESAMPLE_CHUNK_SIZE: usize = 1024;
const MONO_ENCODE_CHUNK_SIZE: usize = 4096;
const TARGET_MP3_BYTES_PER_SECOND: usize = 64_000 / 8;
const MP3_BUFFER_OVERHEAD_BYTES: usize = 4096;

pub fn exists(session_dir: &Path) -> std::io::Result<bool> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .try_fold(false, |acc, path| {
            std::fs::exists(&path).map(|exists| acc || exists)
        })
}

pub fn delete(session_dir: &Path) -> std::io::Result<()> {
    for format in AUDIO_FORMATS {
        let path = session_dir.join(format);
        if std::fs::exists(&path).unwrap_or(false) {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn path(session_dir: &Path) -> Option<PathBuf> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .find(|path| path.exists())
}

pub fn import_to_session(
    runtime: &dyn AudioImportRuntime,
    session_id: &str,
    session_dir: &Path,
    source_path: &Path,
) -> Result<PathBuf, AudioImportError> {
    runtime.emit(AudioImportEvent::Started {
        session_id: session_id.to_string(),
    });

    std::fs::create_dir_all(session_dir)?;

    let target_path = session_dir.join("audio.mp3");
    let tmp_path = session_dir.join("audio.mp3.tmp");

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    let on_progress = {
        let session_id = session_id.to_string();
        let mut last_emitted: f64 = 0.0;
        let mut last_time = std::time::Instant::now();
        move |percentage: f64| {
            let now = std::time::Instant::now();
            if (percentage - last_emitted) >= 0.01
                || now.duration_since(last_time).as_millis() >= 100
            {
                runtime.emit(AudioImportEvent::Progress {
                    session_id: session_id.clone(),
                    percentage,
                });
                last_emitted = percentage;
                last_time = now;
            }
        }
    };

    match import_audio_with_progress(source_path, &tmp_path, &target_path, on_progress) {
        Ok(final_path) => {
            runtime.emit(AudioImportEvent::Completed {
                session_id: session_id.to_string(),
            });
            Ok(final_path)
        }
        Err(error) => {
            if tmp_path.exists() {
                let _ = std::fs::remove_file(&tmp_path);
            }
            runtime.emit(AudioImportEvent::Failed {
                session_id: session_id.to_string(),
                error: error.to_string(),
            });
            Err(error.into())
        }
    }
}

fn import_audio_with_progress(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
    on_progress: impl FnMut(f64),
) -> Result<PathBuf, AudioProcessingError> {
    decode_to_mp3_file_with_progress(source_path, tmp_path, None, on_progress)?;
    atomic_move(tmp_path, target_path)?;
    Ok(target_path.to_path_buf())
}

pub fn import_audio(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, AudioProcessingError> {
    import_audio_with_max_duration(source_path, tmp_path, target_path, None)
}

pub fn import_audio_with_max_duration(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
    max_duration: Option<Duration>,
) -> Result<PathBuf, AudioProcessingError> {
    decode_to_mp3_file(source_path, tmp_path, max_duration)?;
    atomic_move(tmp_path, target_path)?;
    Ok(target_path.to_path_buf())
}

pub fn decode_to_mp3(source_path: &Path) -> Result<Vec<u8>, AudioProcessingError> {
    decode_to_mp3_with_max_duration(source_path, None)
}

pub fn decode_to_mp3_with_max_duration(
    source_path: &Path,
    max_duration: Option<Duration>,
) -> Result<Vec<u8>, AudioProcessingError> {
    with_afconvert_fallback(source_path, |path| {
        let mut mp3_bytes = Vec::new();
        let bytes_written = decode_with_rodio(path, max_duration, &mut mp3_bytes)?;
        if bytes_written == 0 {
            return Err(AudioProcessingError::EmptyInput);
        }
        Ok(mp3_bytes)
    })
}

fn decode_to_mp3_file(
    path: &Path,
    tmp_path: &Path,
    max_duration: Option<Duration>,
) -> Result<(), AudioProcessingError> {
    with_afconvert_fallback(path, |path| {
        let file = File::create(tmp_path)?;
        let writer = BufWriter::new(file);
        let bytes_written = decode_with_rodio(path, max_duration, writer)?;
        if bytes_written == 0 {
            let _ = std::fs::remove_file(tmp_path);
            return Err(AudioProcessingError::EmptyInput);
        }
        Ok(())
    })
}

fn decode_to_mp3_file_with_progress(
    path: &Path,
    tmp_path: &Path,
    max_duration: Option<Duration>,
    on_progress: impl FnMut(f64),
) -> Result<(), AudioProcessingError> {
    with_afconvert_fallback_mut(path, on_progress, |path, on_progress| {
        let file = File::create(tmp_path)?;
        let writer = BufWriter::new(file);
        let bytes_written = decode_with_rodio_progress(path, max_duration, writer, on_progress)?;
        if bytes_written == 0 {
            let _ = std::fs::remove_file(tmp_path);
            return Err(AudioProcessingError::EmptyInput);
        }
        Ok(())
    })
}

fn with_afconvert_fallback_mut<F, T>(
    source_path: &Path,
    mut on_progress: impl FnMut(f64),
    mut try_fn: F,
) -> Result<T, AudioProcessingError>
where
    F: FnMut(&Path, &mut dyn FnMut(f64)) -> Result<T, AudioProcessingError>,
{
    match try_fn(source_path, &mut on_progress) {
        Ok(val) => Ok(val),
        Err(_first_err) => {
            #[cfg(target_os = "macos")]
            {
                let wav_path = hypr_afconvert::to_wav(source_path)
                    .map_err(|e| AudioProcessingError::AfconvertFailed(e.to_string()))?;
                let result = try_fn(&wav_path, &mut on_progress);
                let _ = std::fs::remove_file(&wav_path);
                result
            }
            #[cfg(not(target_os = "macos"))]
            Err(_first_err)
        }
    }
}

fn decode_with_rodio_progress<W: Write>(
    path: &Path,
    max_duration: Option<Duration>,
    output: W,
    on_progress: &mut dyn FnMut(f64),
) -> Result<usize, AudioProcessingError> {
    let file = File::open(path)?;
    let decoder = rodio::Decoder::try_from(file)?;
    encode_source_to_mp3_with_progress(decoder, max_duration, output, on_progress)
}

fn encode_source_to_mp3_with_progress<S, W>(
    source: S,
    max_duration: Option<Duration>,
    output: W,
    on_progress: &mut dyn FnMut(f64),
) -> Result<usize, AudioProcessingError>
where
    S: Source<Item = f32>,
    W: Write,
{
    let source_rate: u32 = source.sample_rate().into();
    let channel_count_raw: u16 = source.channels().into();
    let input_duration = source.total_duration();
    let channel_count_raw = channel_count_raw.max(1);
    let channel_count_u8 = u8::try_from(channel_count_raw).map_err(|_| {
        AudioProcessingError::UnsupportedChannelCount {
            count: channel_count_raw,
        }
    })?;
    let channel_count =
        NonZeroU8::new(channel_count_u8).ok_or(AudioProcessingError::InvalidChannelCount)?;

    let mut encoder = hypr_mp3::MonoStreamEncoder::new(TARGET_SAMPLE_RATE_HZ).map_err(mp3_err)?;
    let effective_duration = max_duration
        .map(|max| input_duration.map_or(max, |inp| inp.min(max)))
        .or(input_duration);
    let mut output = Mp3Output::new(output, estimated_mp3_capacity(effective_duration));
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

            if let Some(total) = total_frames {
                on_progress(processed_frames as f64 / total as f64);
            }
        }

        if !saw_input {
            return Ok(0);
        }

        let expected_output_frames = (expected_output_frames as f64 * TARGET_SAMPLE_RATE_HZ as f64
            / source_rate as f64)
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

            if let Some(total) = total_frames {
                on_progress(processed_frames as f64 / total as f64);
            }
        }

        if !saw_input {
            return Ok(0);
        }

        if !mono_chunk.is_empty() {
            encode_mono_chunk(&mut encoder, &mono_chunk, &mut mono_pcm, &mut output)?;
        }
    }

    on_progress(1.0);

    encoder.flush(output.buffer()).map_err(mp3_err)?;
    output.flush()?;

    Ok(output.bytes_written())
}

fn with_afconvert_fallback<F, T>(source_path: &Path, try_fn: F) -> Result<T, AudioProcessingError>
where
    F: Fn(&Path) -> Result<T, AudioProcessingError>,
{
    match try_fn(source_path) {
        Ok(val) => Ok(val),
        Err(_first_err) => {
            #[cfg(target_os = "macos")]
            {
                let wav_path = hypr_afconvert::to_wav(source_path)
                    .map_err(|e| AudioProcessingError::AfconvertFailed(e.to_string()))?;
                let result = try_fn(&wav_path);
                let _ = std::fs::remove_file(&wav_path);
                result
            }
            #[cfg(not(target_os = "macos"))]
            Err(_first_err)
        }
    }
}

fn decode_with_rodio<W: Write>(
    path: &Path,
    max_duration: Option<Duration>,
    output: W,
) -> Result<usize, AudioProcessingError> {
    let file = File::open(path)?;
    let decoder = rodio::Decoder::try_from(file)?;
    encode_source_to_mp3(decoder, max_duration, output)
}

fn encode_source_to_mp3<S, W>(
    source: S,
    max_duration: Option<Duration>,
    output: W,
) -> Result<usize, AudioProcessingError>
where
    S: Source<Item = f32>,
    W: Write,
{
    let source_rate: u32 = source.sample_rate().into();
    let channel_count_raw: u16 = source.channels().into();
    let input_duration = source.total_duration();
    let channel_count_raw = channel_count_raw.max(1);
    let channel_count_u8 = u8::try_from(channel_count_raw).map_err(|_| {
        AudioProcessingError::UnsupportedChannelCount {
            count: channel_count_raw,
        }
    })?;
    let channel_count =
        NonZeroU8::new(channel_count_u8).ok_or(AudioProcessingError::InvalidChannelCount)?;

    let mut encoder = hypr_mp3::MonoStreamEncoder::new(TARGET_SAMPLE_RATE_HZ).map_err(mp3_err)?;
    let effective_duration = max_duration
        .map(|max| input_duration.map_or(max, |inp| inp.min(max)))
        .or(input_duration);
    let mut output = Mp3Output::new(output, estimated_mp3_capacity(effective_duration));
    let channel_count = usize::from(channel_count.get());
    let needs_resample = source_rate != TARGET_SAMPLE_RATE_HZ;
    let mut saw_input = false;
    let mut remaining_frames = max_duration
        .map(|duration| max_frames_for_duration(source_rate, duration))
        .unwrap_or(usize::MAX);

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
            state.input_buf[0].push(mono_frame);

            if state.input_buf[0].len() < state.resampler.input_frames_next() {
                continue;
            }

            state.encode_chunk(&mut encoder, &mut output, None)?;
        }

        if !saw_input {
            return Ok(0);
        }

        let expected_output_frames = (expected_output_frames as f64 * TARGET_SAMPLE_RATE_HZ as f64
            / source_rate as f64)
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
            mono_chunk.push(mono_frame);

            if mono_chunk.len() < MONO_ENCODE_CHUNK_SIZE {
                continue;
            }

            encode_mono_chunk(&mut encoder, &mono_chunk, &mut mono_pcm, &mut output)?;
            mono_chunk.clear();
        }

        if !saw_input {
            return Ok(0);
        }

        if !mono_chunk.is_empty() {
            encode_mono_chunk(&mut encoder, &mono_chunk, &mut mono_pcm, &mut output)?;
        }
    }

    encoder.flush(output.buffer()).map_err(mp3_err)?;
    output.flush()?;

    Ok(output.bytes_written())
}

fn mp3_err(e: hypr_mp3::Error) -> AudioProcessingError {
    AudioProcessingError::Mp3Encode(e.to_string())
}

fn estimated_mp3_capacity(duration: Option<Duration>) -> usize {
    let Some(duration) = duration else {
        return 0;
    };

    let bytes_from_seconds = duration
        .as_secs()
        .saturating_mul(TARGET_MP3_BYTES_PER_SECOND as u64);
    let bytes_from_nanos = (u64::from(duration.subsec_nanos())
        .saturating_mul(TARGET_MP3_BYTES_PER_SECOND as u64))
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

fn create_mono_resampler(source_rate: u32) -> Result<Async<f32>, AudioProcessingError> {
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
    fn new(source_rate: u32) -> Result<Self, AudioProcessingError> {
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

    fn encode_chunk(
        &mut self,
        encoder: &mut hypr_mp3::MonoStreamEncoder,
        output: &mut Mp3Output<impl Write>,
        partial_len: Option<usize>,
    ) -> Result<(), AudioProcessingError> {
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
            return Ok(());
        }

        let trim = self.frames_to_trim.min(produced_frames);
        self.frames_to_trim -= trim;

        let encoded_frames = &self.output_buf[0][trim..produced_frames];
        if !encoded_frames.is_empty() {
            encode_mono_chunk(encoder, encoded_frames, &mut self.mono_pcm, output)?;
            self.written_frames += encoded_frames.len();
        }

        Ok(())
    }
}

fn encode_mono_chunk<W: Write>(
    encoder: &mut hypr_mp3::MonoStreamEncoder,
    samples: &[f32],
    mono_pcm: &mut Vec<i16>,
    output: &mut Mp3Output<W>,
) -> Result<(), AudioProcessingError> {
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

    fn flush(&mut self) -> Result<(), AudioProcessingError> {
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

fn is_cross_device(_err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        _err.raw_os_error() == Some(18)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn rename_or_copy(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device(&err) => {
            copy(from, to)?;
            remove_file(from)?;
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn atomic_move(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    match rename_or_copy(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            remove_file(to)?;
            rename_or_copy(from, to)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    const MIN_MP3_BYTES: u64 = 1024;

    macro_rules! test_import_audio {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let source_path = std::path::Path::new($path);
                    let temp = TempDir::new().unwrap();
                    let tmp_path = temp.path().join("tmp.mp3");
                    let target_path = temp.path().join("target.mp3");

                    let result = import_audio(source_path, &tmp_path, &target_path);
                    assert!(result.is_ok(), "import_audio failed: {:?}", result.err());
                    assert!(target_path.exists());

                    let size = std::fs::metadata(&target_path).unwrap().len();
                    assert!(
                        size > MIN_MP3_BYTES,
                        "Output too small ({size} bytes), likely empty audio"
                    );
                }
            )*
        };
    }

    test_import_audio! {
        test_import_wav: hypr_data::english_1::AUDIO_PATH,
        test_import_mp3: hypr_data::english_1::AUDIO_MP3_PATH,
        test_import_mp4: hypr_data::english_1::AUDIO_MP4_PATH,
        test_import_m4a: hypr_data::english_1::AUDIO_M4A_PATH,
        test_import_ogg: hypr_data::english_1::AUDIO_OGG_PATH,
        test_import_flac: hypr_data::english_1::AUDIO_FLAC_PATH,
        test_import_aac: hypr_data::english_1::AUDIO_AAC_PATH,
        test_import_aiff: hypr_data::english_1::AUDIO_AIFF_PATH,
        test_import_caf: hypr_data::english_1::AUDIO_CAF_PATH,
    }

    #[test]
    fn test_decode_to_mp3_problem_m4a() {
        let path = match std::env::var("PROBLEM_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let bytes = decode_to_mp3(&path).unwrap();
        assert!(
            bytes.len() > MIN_MP3_BYTES as usize,
            "Output too small ({} bytes)",
            bytes.len()
        );
    }

    #[test]
    fn test_decode_to_mp3_problem2_m4a() {
        let path = match std::env::var("PROBLEM2_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let bytes = decode_to_mp3(&path).unwrap();
        assert!(
            bytes.len() > MIN_MP3_BYTES as usize,
            "Output too small ({} bytes)",
            bytes.len()
        );
    }

    #[test]
    fn test_encode_source_to_mp3_preserves_duration_for_stereo_resample() {
        let channels = std::num::NonZeroU16::new(2).unwrap();
        let sample_rate = std::num::NonZeroU32::new(44_100).unwrap();
        let source = rodio::buffer::SamplesBuffer::new(
            channels,
            sample_rate,
            vec![0.5f32; 44_100 * 5 * usize::from(channels.get())],
        );

        let mut bytes = Vec::new();
        let bytes_written = encode_source_to_mp3(source, None, &mut bytes).unwrap();
        assert_eq!(bytes_written, bytes.len());
        assert!(bytes.len() > MIN_MP3_BYTES as usize);

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("encoded.mp3");
        std::fs::write(&path, &bytes).unwrap();

        let decoder = rodio::Decoder::try_from(File::open(&path).unwrap()).unwrap();
        let output_rate: u32 = decoder.sample_rate().into();
        let output_channels: u16 = decoder.channels().into();
        let output_samples: Vec<f32> = decoder.collect();

        assert_eq!(output_rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(output_channels, 1);

        let actual_frames = output_samples.len();
        let expected_frames = TARGET_SAMPLE_RATE_HZ as usize * 5;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})",
        );
    }
}
