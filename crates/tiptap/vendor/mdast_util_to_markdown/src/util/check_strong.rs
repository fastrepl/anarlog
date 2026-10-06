//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-strong.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_strong(state: &State) -> Result<char, Message> {
    let marker = state.options.strong;

    if marker != '*' && marker != '_' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize strong with `{}` for `options.strong`, expected `*`, or `_`",
                marker
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(marker)
}
