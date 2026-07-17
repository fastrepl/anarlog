use std::collections::HashMap;

use hypr_hooks::cli_flag;
use serde::{Deserialize, Serialize};

use hypr_docs::{
    Field, JsDocExtractor, Module, TsType, TsUnionOrIntersectionType, TypeDoc, collect_type_docs,
    first_property, is_false, parse_module, prop_name, property_by_name, type_lit_from,
    type_name_from,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInfo {
    pub name: String,
    pub description: Option<String>,
    pub args: Vec<ArgField>,
}

impl HookInfo {
    pub fn doc_render(&self) -> String {
        let yaml = serde_yaml::to_string(self).unwrap_or_default();
        format!("---\n{}---\n", yaml)
    }

    pub fn doc_path(&self) -> String {
        format!("{}.mdx", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgField {
    pub name: String,
    pub description: Option<String>,
    pub type_name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
}

impl From<Field> for ArgField {
    fn from(field: Field) -> Self {
        Self {
            name: cli_flag(&field.name),
            description: field.description,
            type_name: field.type_name,
            optional: field.optional,
        }
    }
}

pub fn parse_hooks(source_code: &str) -> Result<Vec<HookInfo>, String> {
    let (module, fm) = parse_module(source_code)?;
    let jsdoc = JsDocExtractor::new(source_code, &fm);
    let type_docs = collect_type_docs(&module, &jsdoc);
    Ok(extract_hook_events(&module, &type_docs))
}

fn extract_hook_events(module: &Module, type_docs: &HashMap<String, TypeDoc>) -> Vec<HookInfo> {
    hook_union(module)
        .map(|ty| hook_variants(ty, type_docs))
        .unwrap_or_default()
}

fn hook_variants(type_ann: &TsType, type_docs: &HashMap<String, TypeDoc>) -> Vec<HookInfo> {
    if let TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) =
        type_ann
    {
        union
            .types
            .iter()
            .filter_map(|variant| hook_from_variant(variant.as_ref(), type_docs))
            .collect()
    } else {
        Vec::new()
    }
}

fn hook_from_variant(type_ann: &TsType, type_docs: &HashMap<String, TypeDoc>) -> Option<HookInfo> {
    let type_lit = type_lit_from(type_ann)?;
    let prop = first_property(type_lit)?;
    let hook_name = prop_name(prop)?;
    let args_type = prop
        .type_ann
        .as_ref()
        .and_then(|ty| args_type_name(&ty.type_ann))?;

    let (description, args) = type_docs
        .get(&args_type)
        .map(|doc| {
            (
                doc.description.clone(),
                doc.args.iter().cloned().map(ArgField::from).collect(),
            )
        })
        .unwrap_or((None, Vec::new()));

    Some(HookInfo {
        name: hook_name,
        description,
        args,
    })
}

fn hook_union(module: &Module) -> Option<&TsType> {
    hypr_docs::exported_type_aliases(module)
        .find(|(alias, _)| alias.id.sym.as_ref() == "HookEvent")
        .map(|(alias, _)| alias.type_ann.as_ref())
}

fn args_type_name(type_ann: &TsType) -> Option<String> {
    let type_lit = type_lit_from(type_ann)?;
    let prop = property_by_name(&type_lit.members, "args")?;
    prop.type_ann
        .as_ref()
        .and_then(|ta| type_name_from(&ta.type_ann))
}
