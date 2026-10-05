//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-fence.js

use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_fence(state: &mut State) -> Result<char, Message> {
    let marker = state.options.fence;

    if marker != '`' && marker != '~' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize code with `{}` for `options.fence`, expected `` ` `` or `~`",
                marker
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(marker)
}
