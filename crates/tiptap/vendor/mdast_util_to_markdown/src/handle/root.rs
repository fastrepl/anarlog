//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/root.js

use super::Handle;
use crate::state::{Info, State};
use alloc::string::String;
use markdown::{
    mdast::{Node, Root},
    message::Message,
};

impl Handle for Root {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message> {
        let has_phrasing = self.children.iter().any(phrasing);

        if has_phrasing {
            state.container_phrasing(node, info)
        } else {
            state.container_flow(node)
        }
    }
}

// JS: <https://github.com/syntax-tree/mdast-util-phrasing>.
fn phrasing(child: &Node) -> bool {
    // Note: `html` nodes are ambiguous.
    matches!(
        *child,
        Node::Break(_)
            | Node::Emphasis(_)
            | Node::Image(_)
            | Node::ImageReference(_)
            | Node::InlineCode(_)
            | Node::InlineMath(_)
            | Node::Link(_)
            | Node::LinkReference(_)
            | Node::Strong(_)
            | Node::Text(_)
    )
}
