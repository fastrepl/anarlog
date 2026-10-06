//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/paragraph.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
};
use markdown::{
    mdast::{Node, Paragraph},
    message::Message,
};

impl Handle for Paragraph {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        state.enter(ConstructName::Paragraph);
        state.enter(ConstructName::Phrasing);
        let value = state.container_phrasing(node, info)?;
        state.exit();
        state.exit();
        Ok(value)
    }
}
