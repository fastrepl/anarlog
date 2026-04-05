mod error;
mod packet;

pub use error::{PacketError, Result};
pub use packet::{
    DEFAULT_REPORT_ID, DEFAULT_REPORT_LEN, PACKET_HEADER_LEN, PROTOCOL_VERSION_1, Packet,
    PacketType,
};
