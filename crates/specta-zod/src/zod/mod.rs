mod collections;
mod enums;
mod primitives;
mod structs;
mod utils;

#[cfg(test)]
mod tests;

use std::{borrow::Cow, fmt::Write, path::Path};

use specta::{
    TypeCollection,
    datatype::{DataType, NamedDataType},
};

use crate::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BigIntExportBehavior {
    String,
    #[default]
    Number,
    BigInt,
    Fail,
}

pub struct Zod {
    pub header: Cow<'static, str>,
    pub bigint: BigIntExportBehavior,
}

impl Default for Zod {
    fn default() -> Self {
        Self {
            header: r#"import { z } from "zod";
import { jsonObject } from "./shared";

"#
            .into(),
            bigint: BigIntExportBehavior::default(),
        }
    }
}

impl Zod {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn header(mut self, header: impl Into<Cow<'static, str>>) -> Self {
        self.header = header.into();
        self
    }

    pub fn bigint(mut self, bigint: BigIntExportBehavior) -> Self {
        self.bigint = bigint;
        self
    }

    pub fn export(&self, types: &TypeCollection) -> Result<String, Error> {
        let mut output = self.header.to_string();

        let mut sorted: Vec<_> = types.into_iter().collect();
        sorted.sort_by_key(|(_, ndt)| ndt.name().clone());

        for (_, ndt) in &sorted {
            export_named_datatype(&mut output, self, types, ndt)?;
            output.push('\n');
        }

        Ok(output)
    }

    pub fn export_to(&self, path: impl AsRef<Path>, types: &TypeCollection) -> Result<(), Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.export(types)?)?;
        Ok(())
    }
}

fn export_named_datatype(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    ndt: &NamedDataType,
) -> Result<(), Error> {
    let name = ndt.name();
    utils::sanitise_type_name(name)?;
    write!(s, "export const {}Schema = ", utils::to_camel_case(name))?;
    datatype(s, cfg, types, &ndt.inner, false)?;
    s.push_str(";\n");

    writeln!(
        s,
        "export type {} = z.infer<typeof {}Schema>;",
        name,
        utils::to_camel_case(name)
    )?;

    Ok(())
}

pub(super) fn datatype(
    s: &mut String,
    cfg: &Zod,
    types: &TypeCollection,
    dt: &DataType,
    is_json: bool,
) -> Result<(), Error> {
    match dt {
        DataType::Any => s.push_str("z.any()"),
        DataType::Unknown => s.push_str("z.unknown()"),
        DataType::Primitive(p) => primitives::primitive(s, cfg, p)?,
        DataType::Literal(l) => primitives::literal(s, l)?,
        DataType::List(l) => collections::list(s, cfg, types, l, is_json)?,
        DataType::Map(m) => collections::map(s, cfg, types, m, is_json)?,
        DataType::Nullable(inner) => collections::nullable(s, cfg, types, inner)?,
        DataType::Struct(st) => structs::struct_type(s, cfg, types, st)?,
        DataType::Enum(e) => enums::enum_type(s, cfg, types, e)?,
        DataType::Tuple(t) => collections::tuple(s, cfg, types, t, is_json)?,
        DataType::Reference(r) => collections::reference(s, types, r)?,
        DataType::Generic(g) => s.push_str(&g.to_string()),
    }
    Ok(())
}
