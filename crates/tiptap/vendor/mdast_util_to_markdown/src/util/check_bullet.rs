//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-bullet.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_bullet(state: &mut State) -> Result<char, Message> {
    let marker = state.options.bullet;

    if marker != '*' && marker != '+' && marker != '-' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize items with `{}` for `options.bullet`, expected `*`, `+`, or `-`",
                marker
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(marker)
}
