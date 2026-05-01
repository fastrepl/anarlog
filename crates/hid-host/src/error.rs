use hypr_hid_interface::{PacketError, PacketType};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hid(#[from] hidapi::HidError),
    #[error(transparent)]
    Packet(#[from] PacketError),
    #[error("no HID device matched filter {filter:?}")]
    DeviceNotFound {
        filter: crate::info::HidDeviceFilter,
    },
    #[error("multiple HID devices matched filter {filter:?}: {count}")]
    MultipleDevicesMatched {
        filter: crate::info::HidDeviceFilter,
        count: usize,
    },
    #[error("report payload is {actual} bytes, which exceeds configured capacity {max}")]
    ReportPayloadTooLarge { actual: usize, max: usize },
    #[error("feature report support is not configured")]
    FeatureReportsNotConfigured,
    #[error("read timed out")]
    ReadTimedOut,
    #[error("unexpected report id: expected {expected:#04x}, got {actual:#04x}")]
    UnexpectedReportId { expected: u8, actual: u8 },
    #[error("timeout of {millis}ms exceeds hidapi i32 limit")]
    TimeoutOverflow { millis: u128 },
    #[error("expected a {expected:?} packet, got {actual:?}")]
    UnexpectedPacketType {
        expected: PacketType,
        actual: PacketType,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
