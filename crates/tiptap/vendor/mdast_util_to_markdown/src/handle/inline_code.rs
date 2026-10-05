//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/inline-code.js

use super::Handle;
use crate::state::{Info, State};
use alloc::{format, string::String};
use markdown::{
    mdast::{InlineCode, Node},
    message::Message,
};
use regex::Regex;

impl Handle for InlineCode {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let mut value = self.value.clone();
        let mut sequence = String::from('`');
        let mut grave_accent_match = Regex::new(&format!(r"(^|[^`]){}([^`]|$)", sequence)).unwrap();
        while grave_accent_match.is_match(&value) {
            sequence.push('`');
            grave_accent_match = Regex::new(&format!(r"(^|[^`]){}([^`]|$)", sequence)).unwrap();
        }

        let no_whitespaces = !value.chars().all(char::is_whitespace);
        let starts_with_whitespace = value.starts_with(char::is_whitespace);
        let ends_with_whitespace = value.ends_with(char::is_whitespace);
        let starts_with_tick = value.starts_with('`');
        let ends_with_tick = value.ends_with('`');

        if no_whitespaces
            && ((starts_with_whitespace && ends_with_whitespace)
                || starts_with_tick
                || ends_with_tick)
        {
            value = format!("{}{}{}", ' ', value, ' ');
        }

        for pattern in &mut state.r#unsafe {
            if !pattern.at_break {
                continue;
            }

            State::compile_pattern(pattern);

            if let Some(regex) = &pattern.compiled {
                while let Some(m) = regex.find(&value) {
                    let position = m.start();

                    let position = if position > 0
                        && &value[position..m.len()] == "\n"
                        && &value[position - 1..position] == "\r"
                    {
                        position - 1
                    } else {
                        position
                    };

                    value.replace_range(position..m.start() + 1, " ");
                }
            }
        }

        Ok(format!("{}{}{}", sequence, value, sequence))
    }
}

pub fn peek_inline_code() -> char {
    '`'
}
