use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use hypr_docs::{
    Field, JsDocExtractor, Module, TsType, TsUnionOrIntersectionType, TypeDoc, collect_type_docs,
    extract_fields, is_false, parse_module, property_by_name, type_lit_from, type_name_from,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepLinkInfo {
    pub path: String,
    pub description: Option<String>,
    pub params: Vec<ParamField>,
}

impl DeepLinkInfo {
    pub fn doc_render(&self) -> String {
        let yaml = serde_yaml::to_string(self).unwrap_or_default();
        format!("---\n{}---\n", yaml)
    }

    pub fn doc_path(&self) -> String {
        let name = self.path.trim_start_matches('/').replace('/', "-");
        format!("{}.mdx", name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamField {
    pub name: String,
    pub description: Option<String>,
    pub type_name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
}

impl From<Field> for ParamField {
    fn from(field: Field) -> Self {
        Self {
            name: field.name,
            description: field.description,
            type_name: field.type_name,
            optional: field.optional,
        }
    }
}

pub fn parse_deeplinks(source_code: &str) -> Result<Vec<DeepLinkInfo>, String> {
    let (module, fm) = parse_module(source_code)?;
    let jsdoc = JsDocExtractor::new(source_code, &fm);
    let type_docs = collect_type_docs(&module, &jsdoc);
    Ok(extract_deeplink_variants(&module, &jsdoc, &type_docs))
}

fn extract_deeplink_variants(
    module: &Module,
    jsdoc: &JsDocExtractor<'_>,
    type_docs: &HashMap<String, TypeDoc>,
) -> Vec<DeepLinkInfo> {
    deeplink_union(module)
        .map(|ty| deeplink_variants(ty, jsdoc, type_docs))
        .unwrap_or_default()
}

fn deeplink_variants(
    type_ann: &TsType,
    jsdoc: &JsDocExtractor<'_>,
    type_docs: &HashMap<String, TypeDoc>,
) -> Vec<DeepLinkInfo> {
    if let TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) =
        type_ann
    {
        union
            .types
            .iter()
            .filter_map(|variant| deeplink_from_variant(variant.as_ref(), jsdoc, type_docs))
            .collect()
    } else {
        deeplink_from_variant(type_ann, jsdoc, type_docs)
            .map(|info| vec![info])
            .unwrap_or_default()
    }
}

fn deeplink_from_variant(
    type_ann: &TsType,
    jsdoc: &JsDocExtractor<'_>,
    type_docs: &HashMap<String, TypeDoc>,
) -> Option<DeepLinkInfo> {
    let type_lit = type_lit_from(type_ann)?;

    let to_prop = property_by_name(&type_lit.members, "to")?;
    let path = extract_literal_string_value(to_prop.type_ann.as_ref()?.type_ann.as_ref())?;

    let search_prop = property_by_name(&type_lit.members, "search")?;
    let search_type_name = search_prop
        .type_ann
        .as_ref()
        .and_then(|ta| type_name_from(&ta.type_ann))?;

    let (description, params) = type_docs
        .get(&search_type_name)
        .map(|doc| {
            (
                doc.description.clone(),
                doc.args.iter().cloned().map(ParamField::from).collect(),
            )
        })
        .unwrap_or_else(|| {
            let params = search_prop
                .type_ann
                .as_ref()
                .map(|ta| extract_fields(&ta.type_ann, jsdoc))
                .unwrap_or_default()
                .into_iter()
                .map(ParamField::from)
                .collect();
            (None, params)
        });

    Some(DeepLinkInfo {
        path,
        description,
        params,
    })
}

fn deeplink_union(module: &Module) -> Option<&TsType> {
    hypr_docs::exported_type_aliases(module)
        .find(|(alias, _)| alias.id.sym.as_ref() == "DeepLink")
        .map(|(alias, _)| alias.type_ann.as_ref())
}

fn extract_literal_string_value(type_ann: &TsType) -> Option<String> {
    if let TsType::TsLitType(lit_type) = type_ann {
        if let hypr_docs::TsLit::Str(s) = &lit_type.lit {
            return s.value.as_str().map(|s| s.to_string());
        }
    }
    None
}
