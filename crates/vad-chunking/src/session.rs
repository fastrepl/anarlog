use std::time::Duration;

use hypr_onnx::ndarray::ArrayView1;
use hypr_vad::silero_onnx::{CHUNK_SIZE_16KHZ, SileroVad};

const SAMPLE_RATE: usize = 16000;

#[derive(Debug, Clone)]
pub struct AdaptiveVadConfig {
    pub positive_speech_threshold: f32,
    pub negative_speech_threshold: f32,
    pub redemption_time: Duration,
    pub pre_speech_pad: Duration,
    pub min_speech_time: Duration,
    pub min_chunk_duration: Duration,
    pub target_chunk_duration: Duration,
    pub max_negative_threshold: f32,
}

impl Default for AdaptiveVadConfig {
    fn default() -> Self {
        Self {
            positive_speech_threshold: 0.5,
            negative_speech_threshold: 0.35,
            redemption_time: Duration::from_millis(600),
            pre_speech_pad: Duration::from_millis(600),
            min_speech_time: Duration::from_millis(90),
            min_chunk_duration: Duration::from_secs(3),
            target_chunk_duration: Duration::from_secs(20),
            max_negative_threshold: 0.80,
        }
    }
}

impl AdaptiveVadConfig {
    pub fn validate(&self) -> Result<(), crate::Error> {
        validate_threshold("positive_speech_threshold", self.positive_speech_threshold)?;
        validate_threshold("negative_speech_threshold", self.negative_speech_threshold)?;
        validate_threshold("max_negative_threshold", self.max_negative_threshold)?;

        if self.negative_speech_threshold > self.positive_speech_threshold {
            return Err(crate::Error::InvalidConfig(
                "negative_speech_threshold must be <= positive_speech_threshold".into(),
            ));
        }

        if self.max_negative_threshold < self.negative_speech_threshold {
            return Err(crate::Error::InvalidConfig(
                "max_negative_threshold must be >= negative_speech_threshold".into(),
            ));
        }

        if self.redemption_time.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "redemption_time must be greater than zero".into(),
            ));
        }

        if self.min_speech_time.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "min_speech_time must be greater than zero".into(),
            ));
        }

        if self.min_chunk_duration.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "min_chunk_duration must be greater than zero".into(),
            ));
        }

        if self.target_chunk_duration <= self.min_chunk_duration {
            return Err(crate::Error::InvalidConfig(
                "target_chunk_duration must be greater than min_chunk_duration".into(),
            ));
        }

        Ok(())
    }
}

fn validate_threshold(name: &str, value: f32) -> Result<(), crate::Error> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(crate::Error::InvalidConfig(format!(
            "{name} must be between 0.0 and 1.0"
        )))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum VadTransition {
    SpeechStart {
        timestamp_ms: usize,
    },
    SpeechEnd {
        start_timestamp_ms: usize,
        end_timestamp_ms: usize,
        samples: Vec<f32>,
    },
}

#[derive(Clone, Copy)]
enum VadState {
    Silence,
    Speech {
        start_sample: usize,
        confirmed: bool,
        speech_samples: usize,
    },
}

pub struct AdaptiveVadSession {
    silero: SileroVad,
    config: AdaptiveVadConfig,
    state: VadState,
    retained_audio: Vec<f32>,
    retained_start_sample: usize,
    cursor_sample: usize,
    silent_samples: usize,
    last_prob: f32,
}

impl AdaptiveVadSession {
    pub fn new(config: AdaptiveVadConfig) -> Result<Self, crate::Error> {
        config.validate()?;

        let silero = SileroVad::new_embedded()
            .map_err(|e| crate::Error::VadSessionCreationFailed(e.to_string()))?;
        Ok(Self {
            silero,
            config,
            state: VadState::Silence,
            retained_audio: Vec::new(),
            retained_start_sample: 0,
            cursor_sample: 0,
            silent_samples: 0,
            last_prob: 0.0,
        })
    }

    pub fn last_probability(&self) -> f32 {
        self.last_prob
    }

    pub fn is_speaking(&self) -> bool {
        matches!(
            self.state,
            VadState::Speech {
                confirmed: true,
                ..
            }
        )
    }

    pub fn speech_duration(&self) -> Duration {
        match &self.state {
            VadState::Speech { speech_samples, .. } => {
                Duration::from_millis((*speech_samples * 1000 / SAMPLE_RATE) as u64)
            }
            VadState::Silence => Duration::ZERO,
        }
    }

    fn duration_to_samples(duration: Duration) -> usize {
        ((duration.as_millis() * SAMPLE_RATE as u128) / 1000) as usize
    }

    fn samples_to_ms(samples: usize) -> usize {
        samples * 1000 / SAMPLE_RATE
    }

    fn session_end_sample(&self) -> usize {
        self.retained_start_sample + self.retained_audio.len()
    }

    fn absolute_to_index(&self, sample: usize) -> usize {
        debug_assert!(sample >= self.retained_start_sample);
        debug_assert!(sample <= self.session_end_sample());
        sample - self.retained_start_sample
    }

    fn speech_end_transition(&self, start_sample: usize, end_sample: usize) -> VadTransition {
        let start_idx = self.absolute_to_index(start_sample);
        let end_idx = self.absolute_to_index(end_sample);

        VadTransition::SpeechEnd {
            start_timestamp_ms: Self::samples_to_ms(start_sample),
            end_timestamp_ms: Self::samples_to_ms(end_sample),
            samples: self.retained_audio[start_idx..end_idx].to_vec(),
        }
    }

    fn reset_to_silence(&mut self) {
        self.state = VadState::Silence;
        self.silent_samples = 0;
    }

    fn trim_buffer(&mut self) {
        let min_keep_sample = match self.state {
            VadState::Silence => self
                .session_end_sample()
                .saturating_sub(Self::duration_to_samples(self.config.pre_speech_pad)),
            VadState::Speech { start_sample, .. } => start_sample,
        };
        let keep_from = min_keep_sample.min(self.cursor_sample);

        if keep_from <= self.retained_start_sample {
            return;
        }

        let drop_count = keep_from - self.retained_start_sample;
        self.retained_audio.drain(..drop_count);
        self.retained_start_sample = keep_from;
    }

    pub(crate) fn process(
        &mut self,
        audio_frame: &[f32],
    ) -> Result<Vec<VadTransition>, crate::Error> {
        self.retained_audio.extend_from_slice(audio_frame);

        let mut transitions = Vec::new();

        while self.session_end_sample().saturating_sub(self.cursor_sample) >= CHUNK_SIZE_16KHZ {
            let chunk_start = self.absolute_to_index(self.cursor_sample);
            let chunk =
                ArrayView1::from(&self.retained_audio[chunk_start..chunk_start + CHUNK_SIZE_16KHZ]);

            let prob = self
                .silero
                .process_chunk(&chunk, 16000)
                .map_err(|e| crate::Error::VadProcessingFailed(e.to_string()))?;
            self.last_prob = prob;
            self.cursor_sample += CHUNK_SIZE_16KHZ;

            if let Some(t) = self.advance(prob) {
                transitions.push(t);
            }
        }

        self.trim_buffer();
        Ok(transitions)
    }

    pub(crate) fn finish(
        &mut self,
        trailing_audio: &[f32],
    ) -> Result<Vec<VadTransition>, crate::Error> {
        self.retained_audio.extend_from_slice(trailing_audio);

        let mut transitions = Vec::new();
        if let VadState::Speech {
            start_sample,
            confirmed: true,
            ..
        } = self.state
        {
            let end_sample = self.session_end_sample();
            transitions.push(self.speech_end_transition(start_sample, end_sample));
        }

        self.reset_to_silence();
        self.trim_buffer();
        Ok(transitions)
    }

    fn neg_threshold_for_speech_samples(&self, speech_samples: usize) -> f32 {
        let speech_secs = (speech_samples as f64) / SAMPLE_RATE as f64;
        let min_secs = self.config.min_chunk_duration.as_secs_f64();
        let target_secs = self.config.target_chunk_duration.as_secs_f64();
        let max_thresh = self.config.max_negative_threshold;
        let base_thresh = self.config.negative_speech_threshold;

        if speech_secs < min_secs {
            max_thresh
        } else if speech_secs >= target_secs {
            base_thresh
        } else {
            let t = (speech_secs - min_secs) / (target_secs - min_secs);
            max_thresh - t as f32 * (max_thresh - base_thresh)
        }
    }

    fn advance(&mut self, prob: f32) -> Option<VadTransition> {
        match self.state {
            VadState::Silence => {
                if prob > self.config.positive_speech_threshold {
                    let pad_samples = Self::duration_to_samples(self.config.pre_speech_pad);
                    let start_sample = self.cursor_sample.saturating_sub(pad_samples);
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed: false,
                        speech_samples: CHUNK_SIZE_16KHZ,
                    };
                    self.silent_samples = 0;
                }
                None
            }
            VadState::Speech {
                start_sample,
                confirmed,
                speech_samples,
            } => {
                let speech_samples = speech_samples + CHUNK_SIZE_16KHZ;

                let neg_thresh = self.neg_threshold_for_speech_samples(speech_samples);
                if prob < neg_thresh {
                    self.silent_samples += CHUNK_SIZE_16KHZ;
                } else {
                    self.silent_samples = 0;
                }

                let min_speech_samples = Self::duration_to_samples(self.config.min_speech_time);
                let redemption_samples = Self::duration_to_samples(self.config.redemption_time);

                if !confirmed && speech_samples >= min_speech_samples {
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed: true,
                        speech_samples,
                    };
                    return Some(VadTransition::SpeechStart {
                        timestamp_ms: Self::samples_to_ms(start_sample),
                    });
                }

                if confirmed && self.silent_samples >= redemption_samples {
                    let speech_end_sample = self.cursor_sample.saturating_sub(self.silent_samples);
                    let transition = self.speech_end_transition(start_sample, speech_end_sample);
                    self.reset_to_silence();
                    self.trim_buffer();
                    return Some(transition);
                }

                if !confirmed && self.silent_samples >= redemption_samples {
                    self.reset_to_silence();
                    self.trim_buffer();
                } else {
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed,
                        speech_samples,
                    };
                }

                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::*;

    fn decode_audio() -> Vec<f32> {
        rodio::Decoder::new(BufReader::new(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        ))
        .unwrap()
        .collect()
    }

    #[test]
    fn test_invalid_config_rejected() {
        let config = AdaptiveVadConfig {
            target_chunk_duration: Duration::from_secs(3),
            min_chunk_duration: Duration::from_secs(3),
            ..Default::default()
        };

        assert!(matches!(
            AdaptiveVadSession::new(config),
            Err(crate::Error::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_finish_emits_confirmed_speech_with_partial_tail() {
        let audio = decode_audio();
        let mut session = AdaptiveVadSession::new(AdaptiveVadConfig::default()).unwrap();
        let mut processed = 0usize;

        while processed + CHUNK_SIZE_16KHZ + 100 <= audio.len() {
            let chunk = &audio[processed..processed + CHUNK_SIZE_16KHZ];
            let transitions = session.process(chunk).unwrap();
            processed += CHUNK_SIZE_16KHZ;

            if transitions
                .iter()
                .any(|transition| matches!(transition, VadTransition::SpeechStart { .. }))
            {
                let tail = audio[processed..processed + 100].to_vec();
                let transitions = session.finish(&tail).unwrap();

                assert_eq!(transitions.len(), 1);
                let VadTransition::SpeechEnd {
                    start_timestamp_ms,
                    end_timestamp_ms,
                    samples,
                } = &transitions[0]
                else {
                    panic!("expected speech end transition");
                };

                assert!(*end_timestamp_ms > *start_timestamp_ms);
                assert_eq!(&samples[samples.len() - tail.len()..], tail.as_slice());

                return;
            }
        }

        panic!("did not observe speech start in fixture audio");
    }

    #[test]
    fn test_retained_buffer_is_bounded_for_long_silence() {
        let mut session = AdaptiveVadSession::new(AdaptiveVadConfig::default()).unwrap();
        let silence = vec![0.0; CHUNK_SIZE_16KHZ];

        for _ in 0..5000 {
            session.process(&silence).unwrap();
        }

        let max_expected = AdaptiveVadSession::duration_to_samples(session.config.pre_speech_pad)
            + CHUNK_SIZE_16KHZ;
        assert!(session.retained_audio.len() <= max_expected);
    }
}
