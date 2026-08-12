use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anlg_audio_utils::Source;

use crate::encoder::f32_to_i16;
use crate::{Error, MonoStreamEncoder};

const ENCODE_FRAMES: usize = 4096;

/// Decodes any supported audio file, downmixes it to mono, and writes it back out
/// as MP3 segments of at most `segment_duration`. Providers that cap upload size
/// or time out on long audio get one request per segment.
pub fn encode_mono_segments(
    source_path: &Path,
    output_dir: &Path,
    segment_duration: Duration,
) -> Result<Vec<PathBuf>, Error> {
    let source = anlg_audio_utils::source_from_path(source_path)?;
    let sample_rate: u32 = source.sample_rate().into();
    let channels = usize::from(u16::from(source.channels())).max(1);
    let frames_per_segment = frames_per_segment(sample_rate, segment_duration)?;

    let mut paths: Vec<PathBuf> = Vec::new();
    let mut current: Option<SegmentEncoder> = None;

    for sample in anlg_audio_utils::mono_frames(source, channels) {
        if current
            .as_ref()
            .is_some_and(|segment| segment.frames >= frames_per_segment)
        {
            current
                .take()
                .expect("segment was just observed")
                .finish()?;
        }

        if current.is_none() {
            let path = output_dir.join(format!("segment-{:04}.mp3", paths.len()));
            current = Some(SegmentEncoder::create(&path, sample_rate)?);
            paths.push(path);
        }

        current
            .as_mut()
            .expect("segment was just created")
            .push(sample)?;
    }

    if let Some(segment) = current {
        segment.finish()?;
    }

    Ok(paths)
}

fn frames_per_segment(sample_rate: u32, segment_duration: Duration) -> Result<usize, Error> {
    let frames = (u128::from(sample_rate) * segment_duration.as_millis()) / 1000;
    usize::try_from(frames)
        .ok()
        .filter(|frames| *frames > 0)
        .ok_or(Error::InvalidSegmentDuration(segment_duration))
}

struct SegmentEncoder {
    encoder: MonoStreamEncoder,
    writer: BufWriter<File>,
    pcm: Vec<i16>,
    encoded: Vec<u8>,
    frames: usize,
}

impl SegmentEncoder {
    fn create(path: &Path, sample_rate: u32) -> Result<Self, Error> {
        Ok(Self {
            encoder: MonoStreamEncoder::new(sample_rate)?,
            writer: BufWriter::new(File::create(path)?),
            pcm: Vec::with_capacity(ENCODE_FRAMES),
            encoded: Vec::new(),
            frames: 0,
        })
    }

    fn push(&mut self, sample: f32) -> Result<(), Error> {
        self.pcm.push(f32_to_i16(sample));
        self.frames += 1;

        if self.pcm.len() >= ENCODE_FRAMES {
            self.drain()?;
        }
        Ok(())
    }

    fn drain(&mut self) -> Result<(), Error> {
        self.encoded.clear();
        self.encoder.encode_i16(&self.pcm, &mut self.encoded)?;
        self.writer.write_all(&self.encoded)?;
        self.pcm.clear();
        Ok(())
    }

    fn finish(mut self) -> Result<(), Error> {
        self.drain()?;
        self.encoded.clear();
        self.encoder.flush(&mut self.encoded)?;
        self.writer.write_all(&self.encoded)?;
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_per_segment_scales_with_sample_rate() {
        assert_eq!(
            frames_per_segment(16_000, Duration::from_secs(10)).unwrap(),
            160_000
        );
        assert!(frames_per_segment(16_000, Duration::ZERO).is_err());
    }
}
