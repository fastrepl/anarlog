use dagc::MonoAgc;
use hypr_vad_masking::{StreamingVad, VadConfig};

pub struct VadAgc {
    agc: MonoAgc,
    vad: Option<StreamingVad>,
    vad_cfg: VadConfig,
    mask_non_speech: bool,
}

impl VadAgc {
    pub fn new(desired_output_rms: f32, distortion_factor: f32) -> Self {
        Self {
            agc: MonoAgc::new(desired_output_rms, distortion_factor).expect("failed_to_create_agc"),
            vad: None,
            vad_cfg: VadConfig::default(),
            mask_non_speech: false,
        }
    }

    pub fn with_masking(mut self, mask_non_speech: bool) -> Self {
        self.mask_non_speech = mask_non_speech;
        self
    }

    pub fn with_vad_config(mut self, cfg: VadConfig) -> Self {
        self.vad_cfg = cfg;
        self
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        let vad = self
            .vad
            .get_or_insert_with(|| StreamingVad::with_config(samples.len(), self.vad_cfg.clone()));

        let agc = &mut self.agc;
        let mask_non_speech = self.mask_non_speech;

        vad.process_in_place(samples, |frame, is_speech| {
            agc.freeze_gain(!is_speech);
            if !is_speech && mask_non_speech {
                frame.fill(0.0);
            }
            agc.process(frame);
        });
    }

    pub fn gain(&self) -> f32 {
        self.agc.gain()
    }
}

impl Default for VadAgc {
    fn default() -> Self {
        Self::new(0.03, 0.0001)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agc() {
        let input_audio = rodio::Decoder::try_from(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        )
        .unwrap();
        let original_samples: Vec<f32> = input_audio.collect();

        let mut agc = VadAgc::default();

        let mut processed_samples = Vec::new();
        let chunks = original_samples.chunks(512);

        for chunk in chunks {
            let mut target = chunk.to_vec();
            agc.process(&mut target);

            for &sample in &target {
                processed_samples.push(sample);
            }
        }

        let wav = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create("./test.wav", wav).unwrap();
        for sample in processed_samples {
            writer.write_sample(sample).unwrap();
        }
    }

    #[test]
    fn test_cross_call_framing() {
        let input_audio = rodio::Decoder::try_from(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        )
        .unwrap();
        let original_samples: Vec<f32> = input_audio.collect();

        let mut agc = VadAgc::default();
        let mut processed: Vec<f32> = Vec::new();
        for chunk in original_samples.chunks(200) {
            let mut target = chunk.to_vec();
            agc.process(&mut target);
            processed.extend_from_slice(&target);
        }

        assert_eq!(processed.len(), original_samples.len());

        for &sample in &processed {
            assert!(sample.is_finite(), "Sample is not finite");
        }

        let rms: f32 = processed.iter().map(|&s| s * s).sum::<f32>() / processed.len() as f32;
        let rms = rms.sqrt();
        assert!(
            rms > 0.0 && rms < 1.0,
            "RMS {} is out of expected range",
            rms
        );
    }
}
