//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-quote.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_quote(state: &State) -> Result<char, Message> {
    let marker = state.options.quote;

    if marker != '"' && marker != '\'' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize title with `{}`  for `options.quote`, expected `\"`, or `'`",
                marker
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(marker)
}
