pub mod listener;
pub mod recorder;
pub mod root;
pub mod session;
pub mod source;

pub use listener::*;
pub use recorder::*;
pub use root::*;
pub use session::*;
pub use source::*;

#[cfg(target_os = "macos")]
pub const SAMPLE_RATE: u32 = 16 * 1000;
#[cfg(not(target_os = "macos"))]
pub const SAMPLE_RATE: u32 = 16 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    #[allow(dead_code)]
    MicOnly,
    SpeakerOnly,
    MicAndSpeaker,
}

impl ChannelMode {
    pub fn determine(onboarding: bool) -> Self {
        if onboarding {
            return ChannelMode::SpeakerOnly;
        }

        ChannelMode::MicAndSpeaker
    }

    pub fn uses_mic(self) -> bool {
        matches!(self, ChannelMode::MicOnly | ChannelMode::MicAndSpeaker)
    }

    pub fn uses_speaker(self) -> bool {
        matches!(self, ChannelMode::SpeakerOnly | ChannelMode::MicAndSpeaker)
    }
}

#[derive(Clone)]
pub struct AudioChunk {
    pub data: Vec<f32>,
}
