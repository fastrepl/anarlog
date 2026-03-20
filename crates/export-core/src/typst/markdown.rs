use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

use super::utils::escape_typst_string;

fn heading_level_to_equals(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "=",
        HeadingLevel::H2 => "==",
        HeadingLevel::H3 => "===",
        HeadingLevel::H4 => "====",
        HeadingLevel::H5 => "=====",
        HeadingLevel::H6 => "======",
    }
}

pub fn markdown_to_typst(md: &str) -> String {
    let parser = Parser::new(md);
    let mut result = String::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                result.push_str(heading_level_to_equals(level));
                result.push(' ');
            }
            Event::End(TagEnd::Heading(_)) => {
                result.push_str("\n\n");
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                result.push_str("\n\n");
            }
            Event::Start(Tag::Strong) => result.push('*'),
            Event::End(TagEnd::Strong) => result.push('*'),
            Event::Start(Tag::Emphasis) => result.push('_'),
            Event::End(TagEnd::Emphasis) => result.push('_'),
            Event::Start(Tag::Strikethrough) => result.push_str("#strike["),
            Event::End(TagEnd::Strikethrough) => result.push(']'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                result.push_str("#link(\"");
                result.push_str(&dest_url);
                result.push_str("\")[");
            }
            Event::End(TagEnd::Link) => result.push(']'),
            Event::Start(Tag::List(start_num)) => {
                list_stack.push(start_num);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                if let Some(Some(num)) = list_stack.last_mut() {
                    result.push_str(&format!("{}{}. ", indent, num));
                    *num += 1;
                } else {
                    result.push_str(&format!("{}- ", indent));
                }
            }
            Event::End(TagEnd::Item) => {
                result.push('\n');
            }
            Event::Start(Tag::BlockQuote(_)) => {
                result.push_str("#quote(block: true)[\n");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                result.push_str("]\n\n");
            }
            Event::Code(text) => {
                result.push('`');
                result.push_str(&text);
                result.push('`');
            }
            Event::Text(text) => {
                result.push_str(&escape_typst_string(&text));
            }
            Event::SoftBreak => result.push('\n'),
            Event::HardBreak => result.push_str("\\\n"),
            _ => {}
        }
    }

    result.trim_end().to_string()
}
