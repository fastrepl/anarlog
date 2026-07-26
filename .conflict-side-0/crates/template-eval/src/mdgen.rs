#[derive(Clone, serde::Deserialize, serde::Serialize, askama::Template)]
#[template(path = "mdgen.system.md.jinja")]
pub struct MdgenSystem {
    pub topic: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::tpl_snapshot;

    tpl_snapshot!(
        test_mdgen_system,
        MdgenSystem {
            topic: "Go tests for LLM evaluation".to_string(),
        },
        @r#"
    You are a careful technical writer.

    Write a short Markdown document about "Go tests for LLM evaluation".

    Requirements:

    - Must start with a level-1 heading (# ...)
    - Must include a bullet list (at least 3 items)

    Only output Markdown. No surrounding explanations.
    "#
    );
}
