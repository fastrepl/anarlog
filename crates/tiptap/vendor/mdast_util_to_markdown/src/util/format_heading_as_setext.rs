//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-heading-as-setext.js

use alloc::string::{String, ToString};
use markdown::mdast::{Heading, Node};
use regex::Regex;

use crate::state::State;

pub fn format_heading_as_setext(heading: &Heading, state: &State) -> bool {
    let line_break = Regex::new(r"\r?\n|\r").unwrap();
    let mut literal_with_line_break = false;

    for child in &heading.children {
        if include_literal_with_line_break(child, &line_break) {
            literal_with_line_break = true;
            break;
        }
    }

    heading.depth < 3
        && !to_string(&heading.children).is_empty()
        && (state.options.setext || literal_with_line_break)
}

/// See: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-heading-as-setext.js>.
fn include_literal_with_line_break(node: &Node, regex: &Regex) -> bool {
    match node {
        Node::Break(_) => true,
        // Literals.
        Node::Code(x) => regex.is_match(&x.value),
        Node::Html(x) => regex.is_match(&x.value),
        Node::InlineCode(x) => regex.is_match(&x.value),
        Node::InlineMath(x) => regex.is_match(&x.value),
        Node::Math(x) => regex.is_match(&x.value),
        Node::MdxFlowExpression(x) => regex.is_match(&x.value),
        Node::MdxTextExpression(x) => regex.is_match(&x.value),
        Node::MdxjsEsm(x) => regex.is_match(&x.value),
        Node::Text(x) => regex.is_match(&x.value),
        Node::Toml(x) => regex.is_match(&x.value),
        Node::Yaml(x) => regex.is_match(&x.value),
        // Anything else.
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    if include_literal_with_line_break(child, regex) {
                        return true;
                    }
                }
            }

            false
        }
    }
}

/// Tiny version of `mdast-util-to-string`.
fn to_string(children: &[Node]) -> String {
    children.iter().map(ToString::to_string).collect()
}
