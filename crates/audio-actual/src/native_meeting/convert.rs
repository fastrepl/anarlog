//! Converts the helper's 48 kHz tracks to Anarlog's live rate exactly once.
//!
//! Both tracks are placed by the helper's frame index on one host-clock timeline, so output
//! sample `n` of either track is the same moment. A frame no PCM arrived for (a pause, a late
//! start, a silent call tap, a device gap) is silence. A chunk with no real sample on either track
//! is a `Gap`: it keeps transcript offsets capture-relative without claiming a device delivered it.

use std::collections::VecDeque;
use std::sync::Arc;

use anlg_audio::CaptureFrame;
use anlg_audio::meeting::NATIVE_SAMPLE_RATE;

use super::wire::Track;

/// Buffered input beyond what the watermark has released. Reached only if the helper stops
/// sending its clock, which ends the capture instead of growing without bound.
const MAX_BUFFERED_FRAMES: u64 = 10 * NATIVE_SAMPLE_RATE as u64;

#[derive(Debug)]
pub(crate) struct Chunk {
    pub frame: CaptureFrame,
    /// Whether either track had real samples in this chunk.
    pub captured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConvertError {
    UnsupportedRate(u32),
    ClockStalled,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedRate(rate) => write!(f, "cannot convert 48 kHz audio to {rate} Hz"),
            Self::ClockStalled => write!(f, "the native helper stopped advancing its clock"),
        }
    }
}

struct TrackBuffer {
    /// Input frame of `samples[0]`.
    start: u64,
    samples: VecDeque<f32>,
    real: VecDeque<bool>,
    /// End of the last PCM run; per track, the helper only moves forward.
    received_to: u64,
}

impl TrackBuffer {
    fn new() -> Self {
        Self {
            start: 0,
            samples: VecDeque::new(),
            real: VecDeque::new(),
            received_to: 0,
        }
    }

    fn end(&self) -> u64 {
        self.start + self.samples.len() as u64
    }

    fn extend_to(&mut self, end: u64) {
        while self.end() < end {
            self.samples.push_back(0.0);
            self.real.push_back(false);
        }
    }

    fn trim_to(&mut self, start: u64) {
        while self.start < start && !self.samples.is_empty() {
            self.samples.pop_front();
            self.real.pop_front();
            self.start += 1;
        }
        self.start = self.start.max(start);
    }

    fn sample(&self, frame: i64) -> f32 {
        if frame < self.start as i64 {
            return 0.0;
        }
        self.samples
            .get((frame - self.start as i64) as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn has_real(&self, from: u64, to: u64) -> bool {
        (from.max(self.start)..to.min(self.end()))
            .any(|frame| self.real[(frame - self.start) as usize])
    }
}

pub(crate) struct Converter {
    factor: u64,
    chunk: u64,
    half: u64,
    taps: Vec<f32>,
    tracks: [TrackBuffer; 2],
    watermark: u64,
    /// Output samples emitted so far; the next chunk starts here.
    emitted: u64,
    finished: bool,
    pub late_frames: u64,
}

impl Converter {
    pub(crate) fn new(sample_rate: u32, chunk_size: usize) -> Result<Self, ConvertError> {
        if sample_rate == 0 || !NATIVE_SAMPLE_RATE.is_multiple_of(sample_rate) || chunk_size == 0 {
            return Err(ConvertError::UnsupportedRate(sample_rate));
        }
        let factor = (NATIVE_SAMPLE_RATE / sample_rate) as u64;
        let (half, taps) = low_pass(factor);
        Ok(Self {
            factor,
            chunk: chunk_size as u64,
            half,
            taps,
            tracks: [TrackBuffer::new(), TrackBuffer::new()],
            watermark: 0,
            emitted: 0,
            finished: false,
            late_frames: 0,
        })
    }

    /// First input frame the next chunk can still change; anything earlier already left.
    fn released(&self) -> u64 {
        if self.emitted == 0 {
            0
        } else {
            (self.emitted - 1) * self.factor + self.half + 1
        }
    }

    pub(crate) fn push(&mut self, track: Track, frame: u64, samples: &[f32]) {
        if self.finished {
            return;
        }
        let released = self.released();
        let buffer = &mut self.tracks[index(track)];
        let mut first = frame.max(buffer.received_to);
        if first < released {
            first = released;
        }
        let end = frame + samples.len() as u64;
        self.late_frames += first.saturating_sub(frame).min(samples.len() as u64);
        if first >= end {
            return;
        }
        buffer.extend_to(end);
        for at in first..end {
            let slot = (at - buffer.start) as usize;
            buffer.samples[slot] = samples[(at - frame) as usize];
            buffer.real[slot] = true;
        }
        buffer.received_to = end;
    }

    /// The helper's clock: input below `frame` is final even on a track with no PCM.
    pub(crate) fn advance(&mut self, frame: u64) {
        self.watermark = self.watermark.max(frame);
    }

    pub(crate) fn drain(&mut self) -> Result<Vec<Chunk>, ConvertError> {
        let mut chunks = Vec::new();
        while !self.finished {
            let needed = (self.emitted + self.chunk - 1) * self.factor + self.half + 1;
            let covered = self
                .tracks
                .iter()
                .map(|track| track.received_to.max(self.watermark))
                .min()
                .unwrap_or(0);
            if covered < needed {
                break;
            }
            chunks.push(self.emit(self.chunk));
        }
        let buffered = self
            .tracks
            .iter()
            .map(|track| track.end().saturating_sub(track.start))
            .max()
            .unwrap_or(0);
        if buffered > MAX_BUFFERED_FRAMES {
            return Err(ConvertError::ClockStalled);
        }
        Ok(chunks)
    }

    /// Everything up to `frames`, the helper's final padded length, including a short last chunk.
    pub(crate) fn finish(&mut self, frames: u64) -> Vec<Chunk> {
        let total = frames.div_ceil(self.factor);
        self.watermark = u64::MAX;
        self.finished = true;
        let mut chunks = Vec::new();
        while self.emitted < total {
            let count = self.chunk.min(total - self.emitted);
            chunks.push(self.emit(count));
        }
        chunks
    }

    fn emit(&mut self, count: u64) -> Chunk {
        let first_in = self.emitted * self.factor;
        let last_in = (self.emitted + count) * self.factor;
        let reach = last_in + self.half;
        for track in &mut self.tracks {
            track.extend_to(reach);
        }
        let captured = self
            .tracks
            .iter()
            .any(|track| track.has_real(first_in, last_in));
        let [mic, remote] = [0, 1].map(|track| {
            let buffer = &self.tracks[track];
            (0..count)
                .map(|n| {
                    let center = ((self.emitted + n) * self.factor) as i64;
                    self.taps
                        .iter()
                        .enumerate()
                        .map(|(k, tap)| tap * buffer.sample(center + k as i64 - self.half as i64))
                        .sum::<f32>()
                })
                .collect::<Arc<[f32]>>()
        });
        self.emitted += count;
        let keep = (self.emitted * self.factor).saturating_sub(self.half);
        for track in &mut self.tracks {
            track.trim_to(keep);
        }
        Chunk {
            frame: CaptureFrame {
                raw_mic: mic,
                raw_speaker: remote,
                aec_mic: None,
            },
            captured,
        }
    }
}

fn index(track: Track) -> usize {
    match track {
        Track::Microphone => 0,
        Track::Remote => 1,
    }
}

/// A linear-phase Blackman-windowed sinc low-pass, centred so conversion adds no delay. The
/// passband ends below 7/8 of the output Nyquist, so speech keeps its band without aliasing.
fn low_pass(factor: u64) -> (u64, Vec<f32>) {
    if factor == 1 {
        return (0, vec![1.0]);
    }
    let half = 21 * factor + 1;
    let length = 2 * half + 1;
    let cutoff = 0.4375 / factor as f64;
    let mut taps = (0..length)
        .map(|k| {
            let x = k as f64 - half as f64;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let phase = 2.0 * std::f64::consts::PI * k as f64 / (length - 1) as f64;
            sinc * (0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos())
        })
        .collect::<Vec<_>>();
    let sum = taps.iter().sum::<f64>();
    taps.iter_mut().for_each(|tap| *tap /= sum);
    (half, taps.into_iter().map(|tap| tap as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;
    const CHUNK: usize = 1_920;

    fn tone(hz: f64, frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                (2.0 * std::f64::consts::PI * hz * i as f64 / NATIVE_SAMPLE_RATE as f64).sin()
                    as f32
                    * amplitude
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn run(converter: &mut Converter, frames: u64) -> Vec<Chunk> {
        let mut chunks = converter.drain().unwrap();
        chunks.extend(converter.finish(frames));
        chunks
    }

    fn joined(chunks: &[Chunk], remote: bool) -> Vec<f32> {
        chunks
            .iter()
            .flat_map(|chunk| {
                if remote {
                    chunk.frame.raw_speaker.to_vec()
                } else {
                    chunk.frame.raw_mic.to_vec()
                }
            })
            .collect()
    }

    #[test]
    fn speech_band_passes_and_content_above_the_new_nyquist_is_removed() {
        for (hz, low, high) in [
            (1_000.0, 0.69, 0.72),
            (6_000.0, 0.66, 0.72),
            (12_000.0, 0.0, 0.002),
        ] {
            let mut converter = Converter::new(RATE, CHUNK).unwrap();
            converter.push(Track::Microphone, 0, &tone(hz, 48_000, 1.0));
            converter.advance(48_000);
            let mic = joined(&run(&mut converter, 48_000), false);
            assert_eq!(mic.len(), 16_000);
            let level = rms(&mic[200..15_800]);
            assert!((low..=high).contains(&level), "{hz} Hz: rms {level}");
        }
    }

    /// The filter is centred: an impulse lands on the output sample of the same moment, so the
    /// conversion never shifts one track against the other or against capture time.
    #[test]
    fn conversion_adds_no_delay_and_tracks_share_output_indices() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        let mut mic = vec![0.0; 14_400];
        mic[3_000] = 1.0;
        converter.push(Track::Microphone, 0, &mic);
        // The call starts 0.1 s late, its impulse 0.125 s into the capture.
        let mut remote = vec![0.0; 9_600];
        remote[6_000 - 4_800] = 1.0;
        converter.push(Track::Remote, 4_800, &remote);
        let chunks = run(&mut converter, 14_400);
        let peak = |samples: &[f32]| {
            samples
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0
        };
        let mic = joined(&chunks, false);
        let remote = joined(&chunks, true);
        assert_eq!((mic.len(), remote.len()), (4_800, 4_800));
        assert_eq!(peak(&mic), 1_000);
        assert_eq!(peak(&remote), 2_000);
        assert!(remote[..1_500].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn pauses_and_missing_audio_become_gap_chunks_on_the_same_clock() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        // 0.5 s of speech, a 2 s pause with no PCM at all, then 0.5 s more.
        converter.push(Track::Microphone, 0, &vec![0.5; 24_000]);
        converter.advance(24_000 + 96_000);
        converter.push(Track::Microphone, 120_000, &vec![0.5; 24_000]);
        let chunks = run(&mut converter, 144_000);
        let mic = joined(&chunks, false);
        assert_eq!(mic.len(), 48_000);
        let gaps = chunks.iter().filter(|chunk| !chunk.captured).count();
        // The pause covers 32,000 output samples: every whole chunk inside it is a gap.
        assert_eq!(gaps, 32_000 / CHUNK - 1);
        // Paused frames are exactly silent away from the filter's reach at the edges.
        assert!(mic[8_100..39_900].iter().all(|s| *s == 0.0));
        assert!((mic[4_000] - 0.5).abs() < 0.001 && (mic[44_000] - 0.5).abs() < 0.001);
    }

    #[test]
    fn a_track_without_buffers_is_released_by_the_clock_not_held() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        // The call tap is silent and delivers nothing; the microphone keeps going.
        converter.push(Track::Microphone, 0, &vec![0.1; 48_000]);
        assert!(
            converter.drain().unwrap().is_empty(),
            "no chunk before the clock says so"
        );
        converter.advance(47_000);
        let chunks = converter.drain().unwrap();
        assert!(!chunks.is_empty() && chunks.iter().all(|chunk| chunk.captured));
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.frame.raw_speaker.iter().all(|s| *s == 0.0))
        );
    }

    #[test]
    fn no_audio_at_all_is_only_gaps_of_the_full_length() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        converter.advance(30_000);
        let chunks = run(&mut converter, 30_000);
        assert_eq!(joined(&chunks, false).len(), 10_000);
        assert!(chunks.iter().all(|chunk| !chunk.captured));
        assert_eq!(chunks.last().unwrap().frame.raw_mic.len(), 10_000 % CHUNK);
        // A recording stopped at its first frame has nothing to send.
        assert!(run(&mut Converter::new(RATE, CHUNK).unwrap(), 0).is_empty());
    }

    #[test]
    fn audio_arriving_after_the_clock_released_its_time_is_dropped_and_counted() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        converter.advance(48_000);
        let before = converter.drain().unwrap();
        let emitted = joined(&before, true);
        converter.push(Track::Remote, 0, &vec![0.9; 48_000]);
        assert!(converter.late_frames > 40_000);
        // What already left is unchanged; the late tail beyond it is kept.
        assert!(emitted.iter().all(|s| *s == 0.0));
        let after = run(&mut converter, 48_000);
        assert!(joined(&after, true).iter().any(|s| *s > 0.5));
    }

    #[test]
    fn a_helper_that_stops_its_clock_ends_the_capture_instead_of_buffering_forever() {
        let mut converter = Converter::new(RATE, CHUNK).unwrap();
        converter.push(Track::Microphone, 0, &vec![0.1; 11 * 48_000]);
        assert_eq!(converter.drain().unwrap_err(), ConvertError::ClockStalled);
    }

    #[test]
    fn only_integer_rate_factors_are_accepted() {
        assert!(Converter::new(16_000, CHUNK).is_ok());
        assert!(Converter::new(48_000, CHUNK).is_ok());
        assert_eq!(
            Converter::new(44_100, CHUNK).err(),
            Some(ConvertError::UnsupportedRate(44_100))
        );
        // At the native rate it is a pass-through.
        let mut converter = Converter::new(48_000, 4).unwrap();
        converter.push(Track::Remote, 2, &[0.25, 0.5]);
        let chunks = run(&mut converter, 4);
        assert_eq!(&*chunks[0].frame.raw_speaker, &[0.0, 0.0, 0.25, 0.5]);
    }
}
