//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-rule.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_rule(state: &State) -> Result<char, Message> {
    let marker = state.options.rule;

    if marker != '*' && marker != '-' && marker != '_' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize rules with `{}` for `options.rule`, expected `*`, `-`, or `_`",
                marker
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(marker)
}
