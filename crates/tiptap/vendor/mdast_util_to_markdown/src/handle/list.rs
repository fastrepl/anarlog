//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/list.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::{
        check_bullet::check_bullet, check_bullet_ordered::check_bullet_ordered,
        check_bullet_other::check_bullet_other, check_rule::check_rule,
    },
};
use markdown::{
    mdast::{List, Node},
    message::Message,
};

impl Handle for List {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        state.enter(ConstructName::List);
        let bullet_current = state.bullet_current;

        let mut bullet = if self.ordered {
            check_bullet_ordered(state)?
        } else {
            check_bullet(state)?
        };

        let bullet_other = if self.ordered {
            if bullet == '.' {
                ')'
            } else {
                '.'
            }
        } else {
            check_bullet_other(state)?
        };

        let mut use_different_marker = false;
        if let Some(bullet_last_used) = state.bullet_last_used {
            use_different_marker = bullet == bullet_last_used;
        }

        if !self.ordered {
            let is_valid_bullet = bullet == '*' || bullet == '-';
            let is_within_bounds = state.stack.len() >= 4 && state.index_stack.len() >= 3;

            let first_list_item_has_no_children = !self.children.is_empty()
                && self.children[0]
                    .children()
                    .map(|inner| inner.is_empty())
                    .expect("There's at least one list item.");

            if is_valid_bullet
                && is_within_bounds
                && first_list_item_has_no_children
                && state.stack[state.stack.len() - 1] == ConstructName::List
                && state.stack[state.stack.len() - 2] == ConstructName::ListItem
                && state.stack[state.stack.len() - 3] == ConstructName::List
                && state.stack[state.stack.len() - 4] == ConstructName::ListItem
                && state.index_stack[state.index_stack.len() - 1] == 0
                && state.index_stack[state.index_stack.len() - 2] == 0
                && state.index_stack[state.index_stack.len() - 3] == 0
            {
                use_different_marker = true;
            }

            if check_rule(state)? == bullet {
                for child in self.children.iter() {
                    if let Some(child_children) = child.children() {
                        if !child_children.is_empty()
                            && matches!(child, Node::ListItem(_))
                            && matches!(child_children[0], Node::ThematicBreak(_))
                        {
                            use_different_marker = true;
                            break;
                        }
                    }
                }
            }
        }

        if use_different_marker {
            bullet = bullet_other;
        }

        state.bullet_current = Some(bullet);
        let value = state.container_flow(node)?;
        state.bullet_last_used = Some(bullet);
        state.bullet_current = bullet_current;
        state.exit();
        Ok(value)
    }
}
