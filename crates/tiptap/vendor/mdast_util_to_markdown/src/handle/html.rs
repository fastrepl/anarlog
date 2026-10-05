//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/html.js

use super::Handle;
use crate::state::{Info, State};
use markdown::{
    mdast::{Html, Node},
    message::Message,
};

impl Handle for Html {
    fn handle(
        &self,
        _state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        Ok(self.value.clone())
    }
}

pub fn peek_html() -> char {
    '<'
}
