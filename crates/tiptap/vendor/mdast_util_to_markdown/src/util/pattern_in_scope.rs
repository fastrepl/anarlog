//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/pattern-in-scope.js

use crate::{construct_name::ConstructName, r#unsafe::Unsafe};

/// JS: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/pattern-in-scope.js#L24>.
fn list_in_scope(stack: &[ConstructName], list: &[ConstructName], none: bool) -> bool {
    if list.is_empty() {
        return none;
    }

    for construct_name in list {
        if stack.contains(construct_name) {
            return true;
        }
    }

    false
}

/// JS: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/pattern-in-scope.js#L11>.
pub fn pattern_in_scope(stack: &[ConstructName], pattern: &Unsafe) -> bool {
    list_in_scope(stack, &pattern.in_construct, true)
        && !list_in_scope(stack, &pattern.not_in_construct, false)
}
