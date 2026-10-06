//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-bullet-other.js

use super::check_bullet::check_bullet;
use crate::state::State;
use alloc::{boxed::Box, format};
use markdown::message::Message;

pub fn check_bullet_other(state: &mut State) -> Result<char, Message> {
    let bullet = check_bullet(state)?;
    let mut bullet_other = state.options.bullet_other;

    if bullet != '*' {
        bullet_other = '*';
    }

    if bullet_other != '*' && bullet_other != '+' && bullet_other != '-' {
        return Err(Message {
            place: None,
            reason: format!(
                "Cannot serialize items with `{}` for `options.bullet_other`, expected `*`, `+`, or `-`",
                bullet_other
            ),
            rule_id: Box::new("unexpected-marker".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    if bullet_other == bullet {
        return Err(Message {
            place: None,
            reason: format!(
                "Expected `bullet` (`{}`) and `bullet_other` (`{}`) to be different",
                bullet, bullet_other
            ),
            rule_id: Box::new("bullet-match-bullet_other".into()),
            source: Box::new("mdast-util-to-markdown".into()),
        });
    }

    Ok(bullet_other)
}
