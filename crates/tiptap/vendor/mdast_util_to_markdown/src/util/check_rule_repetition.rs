//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-rule-repetition.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_rule_repetition(state: &State) -> Result<u32, Message> {
    let repetition = state.options.rule_repetition;

    if repetition < 3 {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize rules with repetition `{}` for `options.rule_repetition`, expected `3` or more",
                repetition
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(repetition)
}
