use crate::PacketType;

#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error("packet is too short: expected at least {expected} bytes, got {actual}")]
    PacketTooShort { expected: usize, actual: usize },
    #[error("packet payload length {declared} exceeds available bytes {available}")]
    PacketPayloadTooLarge { declared: usize, available: usize },
    #[error("packet type byte {value:#04x} is invalid")]
    InvalidPacketType { value: u8 },
    #[error("packet payload length {payload_len} exceeds u16 capacity")]
    PacketPayloadOverflow { payload_len: usize },
    #[error("expected a {expected:?} packet, got {actual:?}")]
    UnexpectedPacketType {
        expected: PacketType,
        actual: PacketType,
    },
}

pub type Result<T> = std::result::Result<T, PacketError>;
