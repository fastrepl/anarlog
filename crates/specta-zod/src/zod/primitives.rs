use std::fmt::Write;

use specta::datatype::PrimitiveType;

use crate::Error;

use super::{BigIntExportBehavior, Zod, utils};

pub(super) fn primitive(s: &mut String, cfg: &Zod, p: &PrimitiveType) -> Result<(), Error> {
    use PrimitiveType::*;

    let zod_type = match p {
        i8 | i16 | i32 | u8 | u16 | u32 | f32 | f64 => "z.number()",
        usize | isize | i64 | u64 | i128 | u128 => match cfg.bigint {
            BigIntExportBehavior::String => "z.string()",
            BigIntExportBehavior::Number => "z.number()",
            BigIntExportBehavior::BigInt => "z.bigint()",
            BigIntExportBehavior::Fail => return Err(Error::BigIntForbidden),
        },
        bool => "z.boolean()",
        String | char => "z.string()",
    };

    s.push_str(zod_type);
    Ok(())
}

pub(super) fn literal(s: &mut String, l: &specta::datatype::LiteralType) -> Result<(), Error> {
    use specta::datatype::LiteralType;

    match l {
        LiteralType::i8(v) => write!(s, "z.literal({})", v)?,
        LiteralType::i16(v) => write!(s, "z.literal({})", v)?,
        LiteralType::i32(v) => write!(s, "z.literal({})", v)?,
        LiteralType::u8(v) => write!(s, "z.literal({})", v)?,
        LiteralType::u16(v) => write!(s, "z.literal({})", v)?,
        LiteralType::u32(v) => write!(s, "z.literal({})", v)?,
        LiteralType::f32(v) => write!(s, "z.literal({})", v)?,
        LiteralType::f64(v) => write!(s, "z.literal({})", v)?,
        LiteralType::bool(v) => write!(s, "z.literal({})", v)?,
        LiteralType::String(v) => write!(s, "z.literal(\"{}\")", utils::escape_string(v))?,
        LiteralType::char(v) => write!(s, "z.literal(\"{}\")", utils::escape_char(*v))?,
        LiteralType::None => s.push_str("z.null()"),
        _ => s.push_str("z.any()"),
    }
    Ok(())
}
