//! JS equivalent: https://github.com/syntax-tree/mdast-util-math/blob/main/lib/index.js#L241

use super::Handle;
use crate::state::{Info, State};
use alloc::format;
use markdown::{
    mdast::{InlineMath, Node},
    message::Message,
};
use regex::Regex;

impl Handle for InlineMath {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let mut size: usize = if !state.options.single_dollar_text_math {
            2
        } else {
            1
        };

        let pattern = format!("(^|[^$]){}([^$]|$)", "\\$".repeat(size));
        let mut dollar_sign_match = Regex::new(&pattern).unwrap();
        while dollar_sign_match.is_match(&self.value) {
            size += 1;
            let pattern = format!("(^|[^$]){}([^$]|$)", "\\$".repeat(size));
            dollar_sign_match = Regex::new(&pattern).unwrap();
        }

        let sequence = "$".repeat(size);

        let no_whitespaces = !self.value.chars().all(char::is_whitespace);
        let starts_with_whitespace = self.value.starts_with(char::is_whitespace);
        let ends_with_whitespace = self.value.ends_with(char::is_whitespace);
        let starts_with_dollar = self.value.starts_with('$');
        let ends_with_dollar = self.value.ends_with('$');

        let mut value = self.value.clone();
        if no_whitespaces
            && ((starts_with_whitespace && ends_with_whitespace)
                || starts_with_dollar
                || ends_with_dollar)
        {
            value = format!(" {} ", value);
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

pub fn peek_inline_math() -> char {
    '$'
}
