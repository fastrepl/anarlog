//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/break.js

use super::Handle;
use crate::{
    state::{Info, State},
    util::pattern_in_scope::pattern_in_scope,
};
use alloc::string::ToString;
use markdown::{
    mdast::{Break, Node},
    message::Message,
};

impl Handle for Break {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        for pattern in state.r#unsafe.iter() {
            // If we can’t put eols in this construct (setext headings, tables), use a
            // space instead.
            if pattern.character == '\n' && pattern_in_scope(&state.stack, pattern) {
                let space_or_tab = info.before.chars().any(|c| c == '\t' || c == ' ');

                if space_or_tab {
                    return Ok("".to_string());
                }

                return Ok(" ".to_string());
            }
        }

        Ok("\\\n".to_string())
    }
}
