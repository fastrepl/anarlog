use crate::error::{PacketError, Result};

pub const DEFAULT_REPORT_ID: u8 = 0;
pub const DEFAULT_REPORT_LEN: usize = 64;
pub const PACKET_HEADER_LEN: usize = 7;
pub const PROTOCOL_VERSION_1: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Command = 1,
    Response = 2,
    Event = 3,
}

impl TryFrom<u8> for PacketType {
    type Error = PacketError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Command),
            2 => Ok(Self::Response),
            3 => Ok(Self::Event),
            _ => Err(PacketError::InvalidPacketType { value }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub version: u8,
    pub packet_type: PacketType,
    pub opcode: u8,
    pub flags: u8,
    pub seq: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(
        version: u8,
        packet_type: PacketType,
        opcode: u8,
        flags: u8,
        seq: u8,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            version,
            packet_type,
            opcode,
            flags,
            seq,
            payload: payload.into(),
        }
    }

    pub fn command(opcode: u8, seq: u8, payload: impl Into<Vec<u8>>) -> Self {
        Self::new(
            PROTOCOL_VERSION_1,
            PacketType::Command,
            opcode,
            0,
            seq,
            payload,
        )
    }

    pub fn response(opcode: u8, seq: u8, payload: impl Into<Vec<u8>>) -> Self {
        Self::new(
            PROTOCOL_VERSION_1,
            PacketType::Response,
            opcode,
            0,
            seq,
            payload,
        )
    }

    pub fn event(opcode: u8, seq: u8, payload: impl Into<Vec<u8>>) -> Self {
        Self::new(
            PROTOCOL_VERSION_1,
            PacketType::Event,
            opcode,
            0,
            seq,
            payload,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let payload_len =
            u16::try_from(self.payload.len()).map_err(|_| PacketError::PacketPayloadOverflow {
                payload_len: self.payload.len(),
            })?;

        let mut bytes = Vec::with_capacity(PACKET_HEADER_LEN + self.payload.len());
        bytes.push(self.version);
        bytes.push(self.packet_type as u8);
        bytes.push(self.opcode);
        bytes.push(self.flags);
        bytes.push(self.seq);
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend_from_slice(&self.payload);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < PACKET_HEADER_LEN {
            return Err(PacketError::PacketTooShort {
                expected: PACKET_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        let payload_len = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let total_len = PACKET_HEADER_LEN + payload_len;
        if bytes.len() < total_len {
            return Err(PacketError::PacketPayloadTooLarge {
                declared: payload_len,
                available: bytes.len().saturating_sub(PACKET_HEADER_LEN),
            });
        }

        Ok(Self {
            version: bytes[0],
            packet_type: PacketType::try_from(bytes[1])?,
            opcode: bytes[2],
            flags: bytes[3],
            seq: bytes[4],
            payload: bytes[PACKET_HEADER_LEN..total_len].to_vec(),
        })
    }

    pub fn expect_type(self, expected: PacketType) -> Result<Self> {
        if self.packet_type == expected {
            return Ok(self);
        }

        Err(PacketError::UnexpectedPacketType {
            expected,
            actual: self.packet_type,
        })
    }

    pub fn is_reply_to(&self, request: &Packet) -> bool {
        self.packet_type == PacketType::Response
            && request.packet_type == PacketType::Command
            && self.opcode == request.opcode
            && self.seq == request.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_round_trips() {
        let packet = Packet::new(
            PROTOCOL_VERSION_1,
            PacketType::Command,
            0x10,
            0x04,
            7,
            [1, 2, 3],
        );
        let encoded = packet.encode().unwrap();
        let decoded = Packet::decode(&encoded).unwrap();

        assert_eq!(decoded, packet);
    }

    #[test]
    fn packet_decode_ignores_trailing_padding() {
        let packet = Packet::response(0x20, 9, [0xaa, 0xbb]);
        let mut encoded = packet.encode().unwrap();
        encoded.extend_from_slice(&[0, 0, 0, 0]);

        let decoded = Packet::decode(&encoded).unwrap();

        assert_eq!(decoded, packet);
    }
}
