mod error;
pub use error::Error;

pub use imp::snapshot;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported {
    use crate::{Error, Snapshot};

    pub fn snapshot() -> Result<Snapshot, Error> {
        Err(Error::UnsupportedPlatform)
    }
}

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as imp;
#[cfg(target_os = "windows")]
use windows as imp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    Ac,
    Battery,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub has_battery: bool,
    pub power_source: PowerSource,
    pub is_charging: Option<bool>,
    pub battery_percent: Option<u8>,
    pub low_power_mode: bool,
    pub thermal_state: ThermalState,
}

impl Snapshot {
    pub fn on_battery(&self) -> bool {
        matches!(self.power_source, PowerSource::Battery)
    }

    pub fn on_ac_power(&self) -> bool {
        matches!(self.power_source, PowerSource::Ac)
    }
}
