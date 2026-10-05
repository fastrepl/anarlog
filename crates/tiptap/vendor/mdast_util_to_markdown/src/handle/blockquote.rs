//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/blockquote.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
};
use alloc::string::String;
use markdown::{
    mdast::{Blockquote, Node},
    message::Message,
};

impl Handle for Blockquote {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        state.enter(ConstructName::Blockquote);
        let value = state.container_flow(node)?;
        let value = state.indent_lines(&value, map);
        state.exit();
        Ok(value)
    }
}

fn map(line: &str, _index: usize, blank: bool) -> String {
    let mut result = String::with_capacity(2 + line.len());
    let marker = ">";
    result.push_str(marker);
    if !blank {
        result.push(' ');
    }
    result.push_str(line);
    result
}
