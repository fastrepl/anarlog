//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/image-reference.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::safe::SafeConfig,
};
use alloc::string::String;
use core::mem;
use markdown::{
    mdast::{ImageReference, Node, ReferenceKind},
    message::Message,
};

impl Handle for ImageReference {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        state.enter(ConstructName::ImageReference);
        state.enter(ConstructName::Label);

        let mut value = String::from("![");
        let alt = state.safe(&self.alt, &SafeConfig::new(&value, "]", None));

        value.push_str(&alt);
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

        if matches!(self.reference_kind, ReferenceKind::Full) || alt.is_empty() || alt != reference
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

pub fn peek_image_reference() -> char {
    '!'
}
