use std::fmt::Write;

use specta::{
    TypeCollection,
    datatype::{DataType, DataTypeReference, List, Map, TupleType},
};

use crate::Error;

use super::{Zod, datatype, utils};

pub(super) fn list(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    l: &List,
    is_json: bool,
) -> Result<(), Error> {
    if let Some(length) = l.length() {
        s.push_str("z.tuple([");
        for i in 0..length {
            if i > 0 {
                s.push_str(", ");
            }
            datatype(s, cfg, types, l.ty(), true)?;
        }
        s.push_str("])");
    } else {
        if !is_json {
            s.push_str("jsonObject(");
        }
        s.push_str("z.array(");
        datatype(s, cfg, types, l.ty(), true)?;
        s.push(')');
        if !is_json {
            s.push(')');
        }
    }
    Ok(())
}

pub(super) fn map(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    m: &Map,
    is_json: bool,
) -> Result<(), Error> {
    if !is_json {
        s.push_str("jsonObject(");
    }
    s.push_str("z.record(");
    datatype(s, cfg, types, m.key_ty(), true)?;
    s.push_str(", ");
    datatype(s, cfg, types, m.value_ty(), true)?;
    s.push(')');
    if !is_json {
        s.push(')');
    }
    Ok(())
}

pub(super) fn nullable(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    inner: &DataType,
) -> Result<(), Error> {
    s.push_str("z.preprocess((val) => val ?? undefined, ");
    datatype(s, cfg, types, inner, false)?;
    s.push_str(".optional())");
    Ok(())
}

pub(super) fn tuple(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    t: &TupleType,
    is_json: bool,
) -> Result<(), Error> {
    let elements = t.elements();
    if elements.is_empty() {
        s.push_str("z.null()");
        return Ok(());
    }

    if !is_json && elements.len() > 1 {
        s.push_str("jsonObject(");
    }
    s.push_str("z.tuple([");
    for (i, elem) in elements.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        datatype(s, cfg, types, elem, true)?;
    }
    s.push_str("])");
    if !is_json && elements.len() > 1 {
        s.push(')');
    }

    Ok(())
}

pub(super) fn reference(
    s: &mut String,
    types: &TypeCollection,
    r: &DataTypeReference,
) -> Result<(), Error> {
    if let Some(ndt) = types.get(r.sid()) {
        write!(s, "{}Schema", utils::to_camel_case(ndt.name()))?;
    }

    Ok(())
}
