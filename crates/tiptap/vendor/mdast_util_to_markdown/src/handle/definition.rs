//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/definition.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::{
        check_quote::check_quote, contains_control_or_whitespace::contains_control_or_whitespace,
        safe::SafeConfig,
    },
};
use alloc::string::String;
use markdown::{
    mdast::{Definition, Node},
    message::Message,
};

impl Handle for Definition {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let quote = check_quote(state)?;

        state.enter(ConstructName::Definition);
        state.enter(ConstructName::Label);

        let mut value = String::from('[');

        value.push_str(&state.safe(
            &state.association(self),
            &SafeConfig::new(&value, "]", None),
        ));

        value.push_str("]: ");

        state.exit();

        if self.url.is_empty() || contains_control_or_whitespace(&self.url) {
            state.enter(ConstructName::DestinationLiteral);
            value.push('<');
            value.push_str(&state.safe(&self.url, &SafeConfig::new(&value, ">", None)));
            value.push('>');
        } else {
            state.enter(ConstructName::DestinationRaw);
            let after = if self.title.is_some() { " " } else { ")" };
            value.push_str(&state.safe(&self.url, &SafeConfig::new(&value, after, None)));
        }

        state.exit();

        if let Some(title) = &self.title {
            let title_construct_name = if quote == '"' {
                ConstructName::TitleQuote
            } else {
                ConstructName::TitleApostrophe
            };

            state.enter(title_construct_name);
            value.push(' ');
            value.push(quote);

            let mut before_buffer = [0u8; 4];
            let before = quote.encode_utf8(&mut before_buffer);
            value.push_str(&state.safe(title, &SafeConfig::new(&self.url, before, None)));

            value.push(quote);
            state.exit();
        }

        state.exit();

        Ok(value)
    }
}
