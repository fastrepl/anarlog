// Phrase rules follow Char's `DictationPhrases` (char/chat-context-react/src/composer/dictation-audio.ts,
// d4608723), translated to native sample offsets.
use super::wav::SAMPLE_RATE;

const SPEECH_RMS: f64 = 0.008;
const MAX_PHRASE_SAMPLES: usize = SAMPLE_RATE as usize * 6;
const MIN_PAUSED_PHRASE_SAMPLES: usize = SAMPLE_RATE as usize * 6 / 5;
const TRAILING_SILENCE_SAMPLES: usize = SAMPLE_RATE as usize / 2;
pub const MAX_RETAINED_BYTES: usize = 10 * 1024 * 1024;

struct Phrase {
    start: u64,
    samples: Vec<i16>,
}

/// Splits the recording into phrases and keeps the voiced ones, so a failed stream can retry only
/// what it never committed. Silent phrases are dropped. Retained audio is bounded; past the bound
/// no suffix can be cut and the caller must fall back to the full recording.
#[derive(Default)]
pub struct Phrases {
    current: Vec<i16>,
    current_start: u64,
    last_voice: usize,
    retained: Vec<Phrase>,
    retained_bytes: usize,
    overflowed: bool,
    voiced: bool,
}

impl Phrases {
    /// Returns the frame's RMS level.
    pub fn push(&mut self, frame: &[f32], pcm: &[i16]) -> f64 {
        let rms = (frame.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
            / frame.len().max(1) as f64)
            .sqrt();
        self.current.extend_from_slice(pcm);
        if rms >= SPEECH_RMS {
            self.last_voice = self.current.len();
        }
        let length = self.current.len();
        if length >= MAX_PHRASE_SAMPLES
            || (self.last_voice > 0
                && length >= MIN_PAUSED_PHRASE_SAMPLES
                && length - self.last_voice >= TRAILING_SILENCE_SAMPLES)
        {
            self.flush();
        }
        rms
    }

    pub fn flush(&mut self) {
        let samples = std::mem::take(&mut self.current);
        let start = self.current_start;
        self.current_start += samples.len() as u64;
        let voiced = self.last_voice > 0;
        self.last_voice = 0;
        if !voiced {
            return;
        }
        self.voiced = true;
        if self.overflowed {
            return;
        }
        self.retained_bytes += samples.len() * 2;
        if self.retained_bytes > MAX_RETAINED_BYTES {
            self.overflowed = true;
            self.retained.clear();
            return;
        }
        self.retained.push(Phrase { start, samples });
    }

    pub fn voiced(&self) -> bool {
        self.voiced || self.last_voice > 0
    }

    /// Voiced audio after `boundary` (samples from the recording start), or None when retention
    /// overflowed. Phrases that end at or before the boundary are skipped; a phrase that straddles
    /// it keeps only its tail.
    pub fn suffix(&self, boundary: u64) -> Option<Vec<i16>> {
        if self.overflowed {
            return None;
        }
        let mut suffix = Vec::new();
        for phrase in &self.retained {
            let end = phrase.start + phrase.samples.len() as u64;
            if end <= boundary {
                continue;
            }
            let from = boundary.saturating_sub(phrase.start) as usize;
            suffix.extend_from_slice(&phrase.samples[from..]);
        }
        Some(suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: usize = 1_600;

    fn push(phrases: &mut Phrases, level: f32, frames: usize) {
        for _ in 0..frames {
            let frame = [level; FRAME];
            phrases.push(&frame, &super::super::wav::to_pcm16(&frame));
        }
    }

    #[test]
    fn a_pause_closes_a_phrase_and_silence_is_never_retained() {
        let mut phrases = Phrases::default();
        push(&mut phrases, 0.0, 70);
        assert!(!phrases.voiced());
        // 1.2 s of speech followed by 0.5 s of silence closes the phrase.
        push(&mut phrases, 0.1, 12);
        push(&mut phrases, 0.0, 5);
        assert_eq!(phrases.retained.len(), 1);
        assert_eq!(
            phrases.retained[0].start,
            70 * FRAME as u64 - 10 * FRAME as u64
        );
        push(&mut phrases, 0.0, 30);
        phrases.flush();
        assert_eq!(phrases.retained.len(), 1);
        assert!(phrases.voiced());
    }

    #[test]
    fn a_phrase_never_grows_past_six_seconds() {
        let mut phrases = Phrases::default();
        push(&mut phrases, 0.1, 61);
        assert_eq!(phrases.retained.len(), 1);
        assert_eq!(phrases.retained[0].samples.len(), MAX_PHRASE_SAMPLES);
        assert_eq!(phrases.current.len(), FRAME);
    }

    #[test]
    fn the_suffix_starts_exactly_at_the_committed_boundary() {
        let mut phrases = Phrases::default();
        push(&mut phrases, 0.1, 20);
        push(&mut phrases, 0.0, 5);
        push(&mut phrases, 0.2, 20);
        phrases.flush();
        let boundary = 15 * FRAME as u64;
        let suffix = phrases.suffix(boundary).unwrap();
        assert_eq!(suffix.len(), (25 - 15) * FRAME + 20 * FRAME);
        assert_eq!(phrases.suffix(45 * FRAME as u64).unwrap().len(), 0);
    }
}
