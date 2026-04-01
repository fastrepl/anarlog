pub use ::earshot;
use ::earshot::VoiceActivityProfile;

pub const FRAME_10MS: usize = 160;
pub const FRAME_20MS: usize = 320;
pub const FRAME_30MS: usize = 480;

#[derive(Debug, thiserror::Error)]
#[error("voice activity detection failed")]
pub struct VadError;

pub struct VoiceActivityDetector {
    inner: ::earshot::VoiceActivityDetector,
}

impl VoiceActivityDetector {
    pub fn new() -> Self {
        Self {
            inner: ::earshot::VoiceActivityDetector::new(VoiceActivityProfile::QUALITY),
        }
    }

    pub fn predict_16khz(&mut self, samples: &[i16]) -> Result<bool, VadError> {
        self.inner.predict_16khz(samples).map_err(|_| VadError)
    }
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new()
    }
}

pub fn choose_optimal_frame_size(len: usize) -> usize {
    if len >= FRAME_30MS && len.is_multiple_of(FRAME_30MS) {
        FRAME_30MS
    } else if len >= FRAME_20MS && len.is_multiple_of(FRAME_20MS) {
        FRAME_20MS
    } else if len >= FRAME_10MS && len.is_multiple_of(FRAME_10MS) {
        FRAME_10MS
    } else {
        let padding_30 = (FRAME_30MS - (len % FRAME_30MS)) % FRAME_30MS;
        let padding_20 = (FRAME_20MS - (len % FRAME_20MS)) % FRAME_20MS;
        let padding_10 = (FRAME_10MS - (len % FRAME_10MS)) % FRAME_10MS;

        if padding_30 <= padding_20 && padding_30 <= padding_10 {
            FRAME_30MS
        } else if padding_20 <= padding_10 {
            FRAME_20MS
        } else {
            FRAME_10MS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_size_selection() {
        assert_eq!(choose_optimal_frame_size(160), 160);
        assert_eq!(choose_optimal_frame_size(320), 320);
        assert_eq!(choose_optimal_frame_size(480), 480);
        assert_eq!(choose_optimal_frame_size(960), 480);
        assert_eq!(choose_optimal_frame_size(640), 320);
        assert_eq!(choose_optimal_frame_size(512), 320);
    }

    #[test]
    fn test_frame_size_for_small_inputs() {
        assert_eq!(choose_optimal_frame_size(100), 320);
        assert_eq!(choose_optimal_frame_size(50), 320);
    }
}
