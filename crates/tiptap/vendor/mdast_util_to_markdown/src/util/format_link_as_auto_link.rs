//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-link-as-autolink.js

use crate::state::State;
use alloc::{format, string::ToString};
use markdown::mdast::{Link, Node};
use regex::RegexBuilder;

pub fn format_link_as_auto_link(link: &Link, node: &Node, state: &State) -> bool {
    let raw = node.to_string();

    if let Some(children) = node.children() {
        if children.len() != 1 {
            return false;
        }

        let mailto = format!("mailto:{}", raw);
        let start_with_protocol = RegexBuilder::new("^[a-z][a-z+.-]+:")
            .case_insensitive(true)
            .build()
            .unwrap();

        return !state.options.resource_link
            && !link.url.is_empty()
            && link.title.is_none()
            && matches!(children[0], Node::Text(_))
            && (raw == link.url || mailto == link.url)
            && start_with_protocol.is_match(&link.url)
            && is_valid_url(&link.url);
    }

    false
}

fn is_valid_url(url: &str) -> bool {
    !url.chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '<' || c == '>')
}
