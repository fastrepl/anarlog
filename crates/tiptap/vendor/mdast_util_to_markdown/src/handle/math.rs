//! JS equivalent: https://github.com/syntax-tree/mdast-util-math/blob/main/lib/index.js#L204

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::{longest_char_streak::longest_char_streak, safe::SafeConfig},
};
use alloc::string::String;
use markdown::{
    mdast::{Math, Node},
    message::Message,
};

impl Handle for Math {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let sequence = "$".repeat((longest_char_streak(&self.value, '$') + 1).max(2));
        state.enter(ConstructName::MathFlow);

        let mut value = String::new();
        value.push_str(&sequence);

        if let Some(meta) = &self.meta {
            state.enter(ConstructName::MathFlowMeta);
            value.push_str(&state.safe(meta, &SafeConfig::new(&value, "\n", Some('$'))));
            state.exit();
        }

        value.push('\n');

        if !self.value.is_empty() {
            value.push_str(&self.value);
            value.push('\n');
        }

        value.push_str(&sequence);
        state.exit();
        Ok(value)
    }
}
