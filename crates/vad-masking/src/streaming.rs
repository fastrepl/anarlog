use hypr_audio_utils::f32_to_i16_samples;
use hypr_vad::earshot::VoiceActivityDetector;

#[derive(Clone, Debug)]
pub struct VadConfig {
    pub hangover_frames: usize,
    pub amplitude_floor: f32,
    pub start_in_speech: bool,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            hangover_frames: 6,
            amplitude_floor: 0.0005,
            start_in_speech: true,
        }
    }
}

pub struct StreamingVad {
    vad: VoiceActivityDetector,
    cfg: VadConfig,
    frame_size: usize,
    in_speech: bool,
    trailing_non_speech: usize,
    scratch_frame: Vec<f32>,
}

impl StreamingVad {
    pub fn new(frame_hint: usize) -> Self {
        Self::with_config(frame_hint, VadConfig::default())
    }

    pub fn with_config(frame_hint: usize, cfg: VadConfig) -> Self {
        let frame_size = hypr_vad::earshot::choose_optimal_frame_size(frame_hint);
        debug_assert!(frame_size > 0, "VAD frame size must be > 0");

        Self {
            vad: VoiceActivityDetector::new(),
            frame_size,
            in_speech: cfg.start_in_speech,
            trailing_non_speech: 0,
            scratch_frame: Vec::new(),
            cfg,
        }
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    fn smooth_decision(&mut self, raw_is_speech: bool) -> bool {
        if raw_is_speech {
            self.in_speech = true;
            self.trailing_non_speech = 0;
            true
        } else if self.in_speech && self.trailing_non_speech < self.cfg.hangover_frames {
            self.trailing_non_speech += 1;
            true
        } else {
            self.in_speech = false;
            self.trailing_non_speech = 0;
            false
        }
    }

    fn classify_frame(&mut self, frame: &[f32]) -> bool {
        if frame.is_empty() {
            return false;
        }

        let rms = Self::calculate_rms(frame);
        if rms < self.cfg.amplitude_floor {
            return self.smooth_decision(false);
        }

        let raw_is_speech = if frame.len() == self.frame_size {
            let i16_samples = f32_to_i16_samples(frame);
            self.vad.predict_16khz(&i16_samples).unwrap_or(true)
        } else {
            self.scratch_frame.clear();
            self.scratch_frame.extend_from_slice(frame);
            self.scratch_frame.resize(self.frame_size, 0.0);
            let i16_samples = f32_to_i16_samples(&self.scratch_frame);
            self.vad.predict_16khz(&i16_samples).unwrap_or(true)
        };

        self.smooth_decision(raw_is_speech)
    }

    pub fn process_in_place<F>(&mut self, samples: &mut [f32], mut f: F)
    where
        F: FnMut(&mut [f32], bool),
    {
        if samples.is_empty() {
            return;
        }

        for frame in samples.chunks_mut(self.frame_size) {
            let is_speech = self.classify_frame(frame);
            f(frame, is_speech);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hangover_logic() {
        let mut vad = StreamingVad::with_config(
            320,
            VadConfig {
                hangover_frames: 3,
                ..Default::default()
            },
        );

        assert!(vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 0);

        assert!(vad.smooth_decision(true));
        assert!(vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 0);

        assert!(vad.smooth_decision(false));
        assert!(vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 1);

        assert!(vad.smooth_decision(false));
        assert!(vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 2);

        assert!(vad.smooth_decision(false));
        assert!(vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 3);

        assert!(!vad.smooth_decision(false));
        assert!(!vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 0);

        assert!(!vad.smooth_decision(false));
        assert!(!vad.in_speech);
        assert_eq!(vad.trailing_non_speech, 0);
    }

    #[test]
    fn test_frame_size_selection() {
        assert_eq!(StreamingVad::new(160).frame_size(), 160);
        assert_eq!(StreamingVad::new(320).frame_size(), 320);
        assert_eq!(StreamingVad::new(480).frame_size(), 480);
        assert_eq!(StreamingVad::new(512).frame_size(), 320);
        assert_eq!(StreamingVad::new(640).frame_size(), 320);
        assert_eq!(StreamingVad::new(960).frame_size(), 480);
    }
}
