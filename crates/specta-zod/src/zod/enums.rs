use std::fmt::Write;

use specta::{
    TypeCollection,
    datatype::{EnumRepr, EnumType, EnumVariants},
};

use crate::Error;

use super::{Zod, datatype, structs, utils};

pub(super) fn enum_type(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    e: &EnumType,
) -> Result<(), Error> {
    let variants: Vec<_> = e.variants().iter().filter(|(_, v)| !v.skip()).collect();

    if variants.is_empty() {
        s.push_str("z.never()");
        return Ok(());
    }

    let is_simple_string_enum = variants
        .iter()
        .all(|(_, v)| matches!(v.inner(), EnumVariants::Unit));

    match e.repr() {
        EnumRepr::External if is_simple_string_enum => {
            s.push_str("z.enum([");
            for (i, (name, _)) in variants.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                write!(s, "\"{}\"", utils::escape_string(name))?;
            }
            s.push_str("])");
        }
        repr => {
            if variants.len() == 1 {
                let (name, variant) = &variants[0];
                enum_variant(s, cfg, types, name, variant.inner(), repr)?;
            } else {
                s.push_str("z.union([");
                for (i, (name, variant)) in variants.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    enum_variant(s, cfg, types, name, variant.inner(), repr)?;
                }
                s.push_str("])");
            }
        }
    }

    Ok(())
}

fn enum_variant(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    name: &str,
    fields: &EnumVariants,
    repr: &EnumRepr,
) -> Result<(), Error> {
    match repr {
        EnumRepr::External => match fields {
            EnumVariants::Unit => {
                write!(s, "z.literal(\"{}\")", utils::escape_string(name))?;
            }
            EnumVariants::Unnamed(unnamed) => {
                let field_types: Vec<_> = unnamed
                    .fields()
                    .iter()
                    .filter(|f| f.ty().is_some())
                    .collect();
                write!(s, "z.object({{ {}: ", utils::sanitise_key(name))?;
                if field_types.len() == 1 {
                    if let Some(ty) = field_types[0].ty() {
                        datatype(s, cfg, types, ty, false)?;
                    }
                } else {
                    s.push_str("z.tuple([");
                    for (i, f) in field_types.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        if let Some(ty) = f.ty() {
                            datatype(s, cfg, types, ty, false)?;
                        }
                    }
                    s.push_str("])");
                }
                s.push_str(" })");
            }
            EnumVariants::Named(named) => {
                write!(s, "z.object({{ {}: z.object({{ ", utils::sanitise_key(name))?;
                let fields: Vec<_> = named
                    .fields()
                    .iter()
                    .filter(|(_, f)| f.ty().is_some())
                    .collect();
                for (i, (field_name, field)) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    write!(s, "{}: ", utils::sanitise_key(field_name))?;
                    structs::field_type(s, cfg, types, field)?;
                }
                s.push_str(" }) })");
            }
        },
        EnumRepr::Internal { tag } => match fields {
            EnumVariants::Unit => {
                write!(
                    s,
                    "z.object({{ {}: z.literal(\"{}\") }})",
                    utils::sanitise_key(tag),
                    utils::escape_string(name)
                )?;
            }
            EnumVariants::Named(named) => {
                write!(
                    s,
                    "z.object({{ {}: z.literal(\"{}\"), ",
                    utils::sanitise_key(tag),
                    utils::escape_string(name)
                )?;
                let fields: Vec<_> = named
                    .fields()
                    .iter()
                    .filter(|(_, f)| f.ty().is_some())
                    .collect();
                for (i, (field_name, field)) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    write!(s, "{}: ", utils::sanitise_key(field_name))?;
                    structs::field_type(s, cfg, types, field)?;
                }
                s.push_str(" })");
            }
            _ => {
                write!(
                    s,
                    "z.object({{ {}: z.literal(\"{}\") }})",
                    utils::sanitise_key(tag),
                    utils::escape_string(name)
                )?;
            }
        },
        EnumRepr::Adjacent { tag, content } => match fields {
            EnumVariants::Unit => {
                write!(
                    s,
                    "z.object({{ {}: z.literal(\"{}\") }})",
                    utils::sanitise_key(tag),
                    utils::escape_string(name)
                )?;
            }
            _ => {
                write!(
                    s,
                    "z.object({{ {}: z.literal(\"{}\"), {}: ",
                    utils::sanitise_key(tag),
                    utils::escape_string(name),
                    utils::sanitise_key(content)
                )?;
                match fields {
                    EnumVariants::Unnamed(unnamed) => {
                        let field_types: Vec<_> = unnamed
                            .fields()
                            .iter()
                            .filter(|f| f.ty().is_some())
                            .collect();
                        if field_types.len() == 1 {
                            if let Some(ty) = field_types[0].ty() {
                                datatype(s, cfg, types, ty, false)?;
                            }
                        } else {
                            s.push_str("z.tuple([");
                            for (i, f) in field_types.iter().enumerate() {
                                if i > 0 {
                                    s.push_str(", ");
                                }
                                if let Some(ty) = f.ty() {
                                    datatype(s, cfg, types, ty, false)?;
                                }
                            }
                            s.push_str("])");
                        }
                    }
                    EnumVariants::Named(named) => {
                        s.push_str("z.object({ ");
                        let fields: Vec<_> = named
                            .fields()
                            .iter()
                            .filter(|(_, f)| f.ty().is_some())
                            .collect();
                        for (i, (field_name, field)) in fields.iter().enumerate() {
                            if i > 0 {
                                s.push_str(", ");
                            }
                            write!(s, "{}: ", utils::sanitise_key(field_name))?;
                            structs::field_type(s, cfg, types, field)?;
                        }
                        s.push_str(" })");
                    }
                    EnumVariants::Unit => {}
                }
                s.push_str(" })");
            }
        },
        EnumRepr::Untagged => match fields {
            EnumVariants::Unit => {
                s.push_str("z.null()");
            }
            EnumVariants::Unnamed(unnamed) => {
                let field_types: Vec<_> = unnamed
                    .fields()
                    .iter()
                    .filter(|f| f.ty().is_some())
                    .collect();
                if field_types.len() == 1 {
                    if let Some(ty) = field_types[0].ty() {
                        datatype(s, cfg, types, ty, false)?;
                    }
                } else {
                    s.push_str("z.tuple([");
                    for (i, f) in field_types.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        if let Some(ty) = f.ty() {
                            datatype(s, cfg, types, ty, false)?;
                        }
                    }
                    s.push_str("])");
                }
            }
            EnumVariants::Named(named) => {
                s.push_str("z.object({ ");
                let fields: Vec<_> = named
                    .fields()
                    .iter()
                    .filter(|(_, f)| f.ty().is_some())
                    .collect();
                for (i, (field_name, field)) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    write!(s, "{}: ", utils::sanitise_key(field_name))?;
                    structs::field_type(s, cfg, types, field)?;
                }
                s.push_str(" })");
            }
        },
    }

    Ok(())
}
