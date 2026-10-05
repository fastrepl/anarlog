//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/link-reference.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::safe::SafeConfig,
};
use alloc::string::String;
use core::mem;
use markdown::{
    mdast::{LinkReference, Node, ReferenceKind},
    message::Message,
};

impl Handle for LinkReference {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        state.enter(ConstructName::LinkReference);
        state.enter(ConstructName::Label);

        let mut value = String::from("[");
        let text = state.container_phrasing(node, &Info::new(&value, "]"))?;

        value.push_str(&text);
        value.push_str("][");

        state.exit();

        let old_stack = mem::take(&mut state.stack);
        state.enter(ConstructName::Reference);

        let reference = state.safe(
            &state.association(self),
            &SafeConfig::new(&value, "]", None),
        );

        state.exit();
        state.stack = old_stack;
        state.exit();

        if matches!(self.reference_kind, ReferenceKind::Full)
            || text.is_empty()
            || text != reference
        {
            value.push_str(&reference);
            value.push(']');
        } else if matches!(self.reference_kind, ReferenceKind::Shortcut) {
            value.pop();
        } else {
            value.push(']');
        }

        Ok(value)
    }
}

pub fn peek_link_reference() -> char {
    '['
}
