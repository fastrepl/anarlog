use crate::{state::Info, State};
use alloc::string::String;
use markdown::{mdast::Node, message::Message};

mod blockquote;
mod r#break;
mod code;
mod definition;
pub mod emphasis;
mod heading;
pub mod html;
pub mod image;
pub mod image_reference;
pub mod inline_code;
pub mod inline_math;
pub mod link;
pub mod link_reference;
mod list;
mod list_item;
mod math;
mod paragraph;
mod root;
pub mod strong;
mod text;
mod thematic_break;

pub trait Handle {
    fn handle(
        &self,
        state: &mut State,
        info: &Info,
        parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message>;
}
