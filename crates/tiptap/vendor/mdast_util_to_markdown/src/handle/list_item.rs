//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/list-item.js

use super::Handle;
use crate::{
    configure::IndentOptions,
    construct_name::ConstructName,
    state::{Info, State},
    util::check_bullet::check_bullet,
};
use alloc::{
    format,
    string::{String, ToString},
};
use markdown::{
    mdast::{ListItem, Node},
    message::Message,
};

impl Handle for ListItem {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let list_item_indent = state.options.list_item_indent;
        let mut bullet = state
            .bullet_current
            .unwrap_or(check_bullet(state)?)
            .to_string();

        if let Some(Node::List(list)) = parent {
            if list.ordered {
                let bullet_number = if let Some(start) = list.start {
                    start as usize
                } else {
                    1
                };

                if state.options.increment_list_marker {
                    if let Some(position_node) = list.children.iter().position(|x| *x == *node) {
                        bullet = format!("{}{}", bullet_number + position_node, bullet);
                    }
                } else {
                    bullet = format!("{}{}", bullet_number, bullet);
                }
            }
        }

        let mut size = bullet.len() + 1;

        let should_compute_size = match list_item_indent {
            IndentOptions::Mixed => {
                if let Some(Node::List(list)) = parent {
                    list.spread || self.spread
                } else {
                    self.spread
                }
            }
            IndentOptions::Tab => true,
            _ => false,
        };

        if should_compute_size {
            size = compute_size(size);
        }

        state.enter(ConstructName::ListItem);

        let value = state.container_flow(node)?;
        let value = state.indent_lines(&value, |line, index, blank| {
            if index > 0 {
                if blank {
                    String::from(line)
                } else {
                    let blank = " ".repeat(size);
                    let mut result = String::with_capacity(blank.len() + line.len());
                    result.push_str(&blank);
                    result.push_str(line);
                    result
                }
            } else if blank {
                let mut result = String::with_capacity(bullet.len() + line.len());
                result.push_str(&bullet);
                result.push_str(line);
                result
            } else {
                // size - bullet.len() will never panic because size > bullet.len() always.
                let blank = " ".repeat(size - bullet.len());
                let mut result = String::with_capacity(blank.len() + line.len() + bullet.len());
                result.push_str(&bullet);
                result.push_str(&blank);
                result.push_str(line);
                result
            }
        });
        state.exit();

        Ok(value)
    }
}

fn compute_size(a: usize) -> usize {
    // `a.div_ceil(4)` is `((a + 4 - 1) / 4)`
    a.div_ceil(4) * 4
}
