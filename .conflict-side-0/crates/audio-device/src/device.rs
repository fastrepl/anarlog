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
