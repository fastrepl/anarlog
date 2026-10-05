//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js

use alloc::{format, string::String, vec::Vec};
use regex::Regex;

pub struct EscapeInfos {
    pub after: bool,
    pub before: bool,
}

pub struct SafeConfig<'a> {
    pub after: &'a str,
    pub before: &'a str,
    pub encode: Option<char>,
}

impl<'a> SafeConfig<'a> {
    pub(crate) fn new(before: &'a str, after: &'a str, encode: Option<char>) -> Self {
        SafeConfig {
            after,
            before,
            encode,
        }
    }
}

/// JS: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js#L148>.
pub fn escape_backslashes(value: &str, after: &str) -> String {
    let expression = Regex::new(r"\\[!-/:-@\[-`{-~]").unwrap();
    let mut results: String = String::new();
    let whole = format!("{}{}", value, after);

    let positions: Vec<usize> = expression.find_iter(&whole).map(|m| m.start()).collect();
    let mut start = 0;

    for position in &positions {
        if start != *position {
            results.push_str(&value[start..*position]);
        }

        results.push('\\');

        start = *position;
    }

    results.push_str(&value[start..]);

    results
}
