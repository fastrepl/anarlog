#![recursion_limit = "256"]

mod from_ast;
mod from_md;
mod to_ast;
mod validate;

pub use from_ast::mdast_to_markdown;
pub use from_md::md_to_tiptap_json;
pub use to_ast::tiptap_json_to_mdast;
pub use validate::validate_tiptap_json;

pub fn tiptap_json_to_md(json: &serde_json::Value) -> Result<String, String> {
    let mdast = tiptap_json_to_mdast(json);
    mdast_to_markdown(&mdast)
}

#[cfg(test)]
mod tests;
