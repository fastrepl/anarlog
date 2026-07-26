pub mod convert;
pub mod embedded;
pub mod extract;
pub mod parser;
pub mod proto;
pub mod table;
pub mod utils;

pub use convert::*;
pub use embedded::*;
pub use extract::*;
pub use parser::*;
pub use proto::*;
pub use table::*;
pub use utils::*;

pub const STYLE_TYPE_DEFAULT: i32 = -1;
pub const STYLE_TYPE_TITLE: i32 = 0;
pub const STYLE_TYPE_HEADING: i32 = 1;
pub const STYLE_TYPE_SUBHEADING: i32 = 2;
pub const STYLE_TYPE_MONOSPACED: i32 = 4;
pub const STYLE_TYPE_DOTTED_LIST: i32 = 100;
pub const STYLE_TYPE_DASHED_LIST: i32 = 101;
pub const STYLE_TYPE_NUMBERED_LIST: i32 = 102;
pub const STYLE_TYPE_CHECKBOX: i32 = 103;

pub const STYLE_TYPE_BLOCK_QUOTE: i32 = 1;

pub const STYLE_ALIGNMENT_LEFT: i32 = 0;
pub const STYLE_ALIGNMENT_CENTER: i32 = 1;
pub const STYLE_ALIGNMENT_RIGHT: i32 = 2;
pub const STYLE_ALIGNMENT_JUSTIFY: i32 = 3;

pub const FONT_TYPE_DEFAULT: i32 = 0;
pub const FONT_TYPE_BOLD: i32 = 1;
pub const FONT_TYPE_ITALIC: i32 = 2;
pub const FONT_TYPE_BOLD_ITALIC: i32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to decode protobuf: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("Failed to decompress data: {0}")]
    Decompression(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
