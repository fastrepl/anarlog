use crate::{StreamingVad, VadConfig};

pub struct VadMask {
    vad: Option<StreamingVad>,
    vad_cfg: VadConfig,
}

impl VadMask {
    pub fn new() -> Self {
        Self {
            vad: None,
            vad_cfg: VadConfig::default(),
        }
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

        vad.process_in_place(samples, |frame, is_speech| {
            if !is_speech {
                frame.fill(0.0);
            }
        });
    }
}

impl Default for VadMask {
    fn default() -> Self {
        Self::new()
    }
}
