pub const GGUF_MAGIC: u32 = 0x46554747;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum GGUFMetadataValueType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl TryFrom<u32> for GGUFMetadataValueType {
    type Error = crate::Error;

    fn try_from(value: u32) -> Result<Self, crate::Error> {
        match value {
            0 => Ok(GGUFMetadataValueType::Uint8),
            1 => Ok(GGUFMetadataValueType::Int8),
            2 => Ok(GGUFMetadataValueType::Uint16),
            3 => Ok(GGUFMetadataValueType::Int16),
            4 => Ok(GGUFMetadataValueType::Uint32),
            5 => Ok(GGUFMetadataValueType::Int32),
            6 => Ok(GGUFMetadataValueType::Float32),
            7 => Ok(GGUFMetadataValueType::Bool),
            8 => Ok(GGUFMetadataValueType::String),
            9 => Ok(GGUFMetadataValueType::Array),
            10 => Ok(GGUFMetadataValueType::Uint64),
            11 => Ok(GGUFMetadataValueType::Int64),
            12 => Ok(GGUFMetadataValueType::Float64),
            _ => Err(crate::Error::UnsupportedValueType(value)),
        }
    }
}
