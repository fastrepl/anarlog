//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/handle/heading.js

use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::format_heading_as_setext::format_heading_as_setext,
};
use alloc::format;
use markdown::{
    mdast::{Heading, Node},
    message::Message,
};

impl Handle for Heading {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<alloc::string::String, Message> {
        let rank = self.depth.clamp(1, 6);

        if format_heading_as_setext(self, state) {
            state.enter(ConstructName::HeadingSetext);
            state.enter(ConstructName::Phrasing);
            let mut value = state.container_phrasing(node, &Info::new("\n", "\n"))?;

            state.exit();
            state.exit();

            let underline_char = if rank == 1 { "=" } else { "-" };
            let last_line_rank = value
                .rfind('\n')
                .unwrap_or(0)
                .max(value.rfind('\r').unwrap_or(0));

            let last_line_rank = if last_line_rank > 0 {
                last_line_rank + 1
            } else {
                0
            };

            let setext_underline = underline_char.repeat(value.len() - last_line_rank);
            value.push('\n');
            value.push_str(&setext_underline);

            return Ok(value);
        }

        let sequence = "#".repeat(rank as usize);
        state.enter(ConstructName::HeadingAtx);
        state.enter(ConstructName::Phrasing);

        let mut value = state.container_phrasing(node, &Info::new("# ", "\n"))?;

        if let Some(first_char) = value.chars().nth(0) {
            if first_char == ' ' || first_char == '\t' {
                let hex_code = u32::from(first_char);
                value = format!("&#x{:X};{}", hex_code, &value[1..])
            }
        }

        if value.is_empty() {
            value.push_str(&sequence);
        } else {
            value = format!("{} {}", &sequence, value);
        }

        if state.options.close_atx {
            value.push(' ');
            value.push_str(&sequence);
        }

        state.exit();
        state.exit();

        Ok(value)
    }
}
