//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/format-code-as-indented.js

use crate::state::State;
use markdown::mdast::Code;
use regex::Regex;

pub fn format_code_as_indented(code: &Code, state: &State) -> bool {
    let non_whitespace = code.value.chars().any(|c| !c.is_whitespace());
    let blank = Regex::new(r"^[\t ]*(?:[\r\n]|$)|(?:^|[\r\n])[\t ]*$").unwrap();

    !state.options.fences
        && !code.value.is_empty()
        && code.lang.is_none()
        && non_whitespace
        && !blank.is_match(&code.value)
}
