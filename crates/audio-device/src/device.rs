use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum AudioDirection {
    Input,
    Output,
}

/// Transport type for audio devices.
///
/// Used to determine device type and priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum TransportType {
    BuiltIn,
    Usb,
    Bluetooth,
    Hdmi,
    Pci,
    Virtual,
    Unknown,
}

/// Bluetooth outputs default to "headphone" because most Bluetooth audio on a laptop is, and
/// popular headphones (Sony WH-*, Bose QC*) carry no headphone keyword in their name. This
/// denylist catches the common Bluetooth speakers so they keep echo cancellation.
pub(crate) fn name_suggests_speaker(name: &str) -> bool {
    const SPEAKER_MARKERS: &[&str] = &[
        "speaker",
        "soundbar",
        "sound bar",
        "soundlink",
        "boombox",
        "homepod",
        "sonos",
        "megaboom",
        "wonderboom",
        "jbl flip",
        "jbl charge",
        "jbl clip",
        "jbl go",
        "jbl xtreme",
    ];

    let name_lower = name.to_lowercase();
    SPEAKER_MARKERS
        .iter()
        .any(|marker| name_lower.contains(marker))
}

/// Capture endpoints that are not microphones: loopbacks of the output mix and analog or digital
/// jacks. Opening one instead of a mic records the far end or nothing at all. Generic USB mics
/// enumerate as "USB Digital Audio" and the like, so a name that says "mic" always wins.
pub(crate) fn name_suggests_non_microphone(name: &str) -> bool {
    const NON_MIC_MARKERS: &[&str] = &[
        "line in",
        "line-in",
        "stereo mix",
        "wave out mix",
        "what u hear",
        "loopback",
        "monitor of",
        "s/pdif",
        "spdif",
    ];

    let name_lower = name.to_lowercase();
    !name_lower.contains("mic")
        && NON_MIC_MARKERS
            .iter()
            .any(|marker| name_lower.contains(marker))
}

pub(crate) fn name_suggests_microphone(name: &str) -> bool {
    name.to_lowercase().contains("mic")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct AudioDevice {
    pub id: DeviceId,
    pub name: String,
    pub direction: AudioDirection,
    pub transport_type: TransportType,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_muted: Option<bool>,
}

impl AudioDevice {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: AudioDirection,
        transport_type: TransportType,
    ) -> Self {
        Self {
            id: DeviceId::new(id),
            name: name.into(),
            direction,
            transport_type,
            is_default: false,
            volume: None,
            is_muted: None,
        }
    }

    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = Some(volume);
        self
    }

    pub fn with_muted(mut self, is_muted: bool) -> Self {
        self.is_muted = Some(is_muted);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{name_suggests_microphone, name_suggests_non_microphone, name_suggests_speaker};

    #[test]
    fn loopbacks_and_jacks_are_not_microphones() {
        for name in [
            "Stereo Mix (Realtek(R) Audio)",
            "Line In (Realtek(R) Audio)",
            "What U Hear (Sound Blaster)",
            "Monitor of Built-in Audio Analog Stereo",
            "Digital Audio (S/PDIF) (Realtek(R) Audio)",
        ] {
            assert!(name_suggests_non_microphone(name), "{name}");
        }
        for name in [
            "Microphone Array (Realtek(R) Audio)",
            "MacBook Pro Microphone",
            "External Microphone",
            "Microphone (USB Digital Audio)",
            "Microphone (USB PnP Audio Device)",
            "Blue Yeti",
        ] {
            assert!(!name_suggests_non_microphone(name), "{name}");
        }
    }

    #[test]
    fn microphone_names_are_recognized() {
        assert!(name_suggests_microphone("MacBook Pro Microphone"));
        assert!(name_suggests_microphone("Mic Array (Intel SST)"));
        assert!(!name_suggests_microphone("Built-in Input"));
        assert!(!name_suggests_microphone("Blue Yeti"));
    }

    #[test]
    fn bluetooth_speakers_are_recognized_by_name() {
        for name in [
            "Bose SoundLink Flex",
            "JBL Flip 6",
            "Sonos Roam",
            "HomePod mini",
            "Living Room Speaker",
        ] {
            assert!(name_suggests_speaker(name), "{name}");
        }
    }

    #[test]
    fn bluetooth_headphones_without_keywords_are_not_speakers() {
        for name in ["WH-1000XM5", "Bose QC45", "AirPods Pro", "Jabra Elite 85t"] {
            assert!(!name_suggests_speaker(name), "{name}");
        }
    }
}
