use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

use crate::{Error, value::GGUFMetadataValueType};

pub fn read_versioned_size<R: Read + Seek>(
    reader: &mut R,
    version: u32,
    is_little_endian: bool,
) -> Result<u64, Error> {
    if version < 2 {
        if is_little_endian {
            Ok(reader.read_u32::<LittleEndian>()? as u64)
        } else {
            Ok(reader.read_u32::<BigEndian>()? as u64)
        }
    } else if is_little_endian {
        Ok(reader.read_u64::<LittleEndian>()?)
    } else {
        Ok(reader.read_u64::<BigEndian>()?)
    }
}

pub fn read_string<R: Read + Seek>(
    reader: &mut R,
    version: u32,
    is_little_endian: bool,
) -> Result<String, Error> {
    let len = read_versioned_size(reader, version, is_little_endian)?;
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|_| Error::InvalidUtf8)
}

pub fn skip_value<R: Read + Seek>(
    reader: &mut R,
    value_type: GGUFMetadataValueType,
    version: u32,
    is_little_endian: bool,
) -> Result<(), Error> {
    match value_type {
        GGUFMetadataValueType::Uint8
        | GGUFMetadataValueType::Int8
        | GGUFMetadataValueType::Bool => {
            reader.seek(SeekFrom::Current(1))?;
        }
        GGUFMetadataValueType::Uint16 | GGUFMetadataValueType::Int16 => {
            reader.seek(SeekFrom::Current(2))?;
        }
        GGUFMetadataValueType::Uint32
        | GGUFMetadataValueType::Int32
        | GGUFMetadataValueType::Float32 => {
            reader.seek(SeekFrom::Current(4))?;
        }
        GGUFMetadataValueType::Uint64
        | GGUFMetadataValueType::Int64
        | GGUFMetadataValueType::Float64 => {
            reader.seek(SeekFrom::Current(8))?;
        }
        GGUFMetadataValueType::String => {
            let len = read_versioned_size(reader, version, is_little_endian)?;
            reader.seek(SeekFrom::Current(len as i64))?;
        }
        GGUFMetadataValueType::Array => {
            let item_type_raw = if is_little_endian {
                reader.read_u32::<LittleEndian>()?
            } else {
                reader.read_u32::<BigEndian>()?
            };
            let item_type = GGUFMetadataValueType::try_from(item_type_raw)?;

            let item_count = read_versioned_size(reader, version, is_little_endian)?;

            for _ in 0..item_count {
                skip_value(reader, item_type, version, is_little_endian)?;
            }
        }
    }

    Ok(())
}
