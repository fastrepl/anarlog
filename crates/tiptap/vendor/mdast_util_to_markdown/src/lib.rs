//! API.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/index.js.

#![no_std]

use alloc::string::String;
pub use configure::{IndentOptions, Options};
use markdown::{mdast::Node, message::Message};
use state::{Info, State};

extern crate alloc;
mod association;
mod configure;
mod construct_name;
mod handle;
mod state;
mod r#unsafe;
mod util;

/// Turn an mdast syntax tree into markdown.
pub fn to_markdown(tree: &Node) -> Result<String, Message> {
    to_markdown_with_options(tree, &Options::default())
}

/// Turn an mdast syntax tree, with options, into markdown.
pub fn to_markdown_with_options(tree: &Node, options: &Options) -> Result<String, Message> {
    let mut state = State::new(options);
    let mut result = state.handle(tree, &Info::new("\n", "\n"), None)?;

    if !result.is_empty() {
        let last_char = result.chars().last().unwrap();
        if last_char != '\n' && last_char != '\r' {
            result.push('\n');
        }
    }

    Ok(result)
}
