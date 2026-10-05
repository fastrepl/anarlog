//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/code.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::{
        check_fence::check_fence, format_code_as_indented::format_code_as_indented,
        longest_char_streak::longest_char_streak, safe::SafeConfig,
    },
};
use alloc::{
    format,
    string::{String, ToString},
};
use markdown::{
    mdast::{Code, Node},
    message::Message,
};

impl Handle for Code {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let marker = check_fence(state)?;

        if format_code_as_indented(self, state) {
            state.enter(ConstructName::CodeIndented);
            let value = state.indent_lines(&self.value, map);
            state.exit();
            return Ok(value);
        }

        let sequence = marker
            .to_string()
            .repeat((longest_char_streak(&self.value, marker) + 1).max(3));

        state.enter(ConstructName::CodeFenced);
        let mut value = sequence.clone();

        if let Some(lang) = &self.lang {
            let code_fenced_lang_construct = if marker == '`' {
                ConstructName::CodeFencedLangGraveAccent
            } else {
                ConstructName::CodeFencedLangTilde
            };
            state.enter(code_fenced_lang_construct);

            value.push_str(&state.safe(lang, &SafeConfig::new(&value, " ", Some('`'))));

            state.exit();

            if let Some(meta) = &self.meta {
                let code_fenced_meta_construct = if marker == '`' {
                    ConstructName::CodeFencedMetaGraveAccent
                } else {
                    ConstructName::CodeFencedMetaTilde
                };

                state.enter(code_fenced_meta_construct);
                value.push(' ');

                value.push_str(&state.safe(meta, &SafeConfig::new(&value, "\n", Some('`'))));

                state.exit();
            }
        }

        value.push('\n');

        if !self.value.is_empty() {
            value.push_str(&self.value);
            value.push('\n');
        }

        value.push_str(&sequence);
        state.exit();

        Ok(value)
    }
}

fn map(line: &str, _index: usize, blank: bool) -> String {
    if blank {
        String::new()
    } else {
        format!("    {}", line)
    }
}
