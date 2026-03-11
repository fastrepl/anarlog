use std::{borrow::Cow, fmt::Write};

use specta::{
    TypeCollection,
    datatype::{Field, StructFields, StructType},
};

use crate::Error;

use super::{Zod, datatype, utils};

pub(super) fn struct_type(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    st: &StructType,
) -> Result<(), Error> {
    match st.fields() {
        StructFields::Unit => {
            s.push_str("z.null()");
        }
        StructFields::Unnamed(unnamed) => {
            let fields: Vec<_> = unnamed
                .fields()
                .iter()
                .filter(|f| f.ty().is_some())
                .collect();
            if fields.len() == 1 {
                if let Some(ty) = fields[0].ty() {
                    datatype(s, cfg, types, ty, false)?;
                }
            } else {
                s.push_str("z.tuple([");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    if let Some(ty) = field.ty() {
                        datatype(s, cfg, types, ty, false)?;
                    }
                }
                s.push_str("])");
            }
        }
        StructFields::Named(named) => {
            let fields: Vec<_> = named
                .fields()
                .iter()
                .filter(|(_, f)| f.ty().is_some())
                .collect();

            let (flattened, non_flattened): (Vec<_>, Vec<_>) =
                fields.into_iter().partition(|(_, f)| f.flatten());

            if flattened.is_empty() {
                write_object_fields(s, cfg, types, &non_flattened)?;
            } else {
                let mut sections: Vec<String> = Vec::new();

                for (_, field) in &flattened {
                    let mut buf = String::new();
                    datatype(&mut buf, cfg, types, field.ty().unwrap(), false)?;
                    sections.push(buf);
                }

                if !non_flattened.is_empty() {
                    let mut buf = String::new();
                    write_object_fields(&mut buf, cfg, types, &non_flattened)?;
                    sections.push(buf);
                }

                if let Some((first, rest)) = sections.split_first() {
                    s.push_str(first);
                    for section in rest {
                        write!(s, ".and({})", section)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn write_object_fields(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    fields: &[&(Cow<'static, str>, Field)],
) -> Result<(), Error> {
    s.push_str("z.object({\n");
    for (i, (name, field)) in fields.iter().enumerate() {
        if i > 0 {
            s.push_str(",\n");
        }
        write!(s, "  {}: ", utils::sanitise_key(name))?;
        field_type(s, cfg, types, field)?;
    }
    if !fields.is_empty() {
        s.push(',');
    }
    s.push_str("\n})");
    Ok(())
}

pub(super) fn field_type(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    field: &Field,
) -> Result<(), Error> {
    let Some(ty) = field.ty() else {
        return Ok(());
    };

    if field.optional() {
        s.push_str("z.preprocess((val) => val ?? undefined, ");
        datatype(s, cfg, types, ty, false)?;
        s.push_str(".optional())");
    } else {
        datatype(s, cfg, types, ty, false)?;
    }

    Ok(())
}
