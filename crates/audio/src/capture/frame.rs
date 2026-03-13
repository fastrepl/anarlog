use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CaptureFrame {
    pub raw_mic: Arc<[f32]>,
    pub raw_speaker: Arc<[f32]>,
    pub aec_mic: Option<Arc<[f32]>>,
}

impl CaptureFrame {
    pub fn preferred_mic(&self) -> Arc<[f32]> {
        self.aec_mic
            .as_ref()
            .map(Arc::clone)
            .unwrap_or_else(|| Arc::clone(&self.raw_mic))
    }

    pub fn raw_dual(&self) -> (Arc<[f32]>, Arc<[f32]>) {
        (Arc::clone(&self.raw_mic), Arc::clone(&self.raw_speaker))
    }

    pub fn aec_dual(&self) -> (Arc<[f32]>, Arc<[f32]>) {
        (self.preferred_mic(), Arc::clone(&self.raw_speaker))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_frame_exposes_raw_and_aec_views() {
        let frame = CaptureFrame {
            raw_mic: Arc::from([0.1_f32, 0.2]),
            raw_speaker: Arc::from([0.3_f32, 0.4]),
            aec_mic: Some(Arc::from([0.9_f32, 1.0])),
        };

        let (raw_mic, raw_speaker) = frame.raw_dual();
        assert_eq!(&*raw_mic, &[0.1, 0.2]);
        assert_eq!(&*raw_speaker, &[0.3, 0.4]);

        let (aec_mic, aec_speaker) = frame.aec_dual();
        assert_eq!(&*aec_mic, &[0.9, 1.0]);
        assert_eq!(&*aec_speaker, &[0.3, 0.4]);
    }
}
