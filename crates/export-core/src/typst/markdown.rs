use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::utils::{escape_typst_literal, escape_typst_string};

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

fn alignment_to_typst(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Left | Alignment::None => "left",
    }
}

pub fn markdown_to_typst(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, options);
    let mut result = String::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut table_alignments: Vec<Alignment> = Vec::new();
    let mut table_column: usize = 0;
    let mut in_table_header = false;
    let mut in_code_block = false;
    let mut code_block_close = String::from("\", block: true)");

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
                result.push_str(&escape_typst_literal(&dest_url));
                result.push_str("\")[");
            }
            Event::End(TagEnd::Link) => result.push(']'),
            Event::Start(Tag::List(start_num)) => {
                list_stack.push(start_num);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                if list_stack.is_empty() {
                    result.push('\n');
                }
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
            Event::TaskListMarker(checked) => {
                result.push_str(if checked {
                    "#sym.ballot-check "
                } else {
                    "#sym.ballot "
                });
            }
            Event::Start(Tag::BlockQuote(_)) => {
                result.push_str("#quote(block: true)[\n");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                result.push_str("]\n\n");
            }
            Event::Code(text) => {
                result.push_str("#raw(\"");
                result.push_str(&escape_typst_literal(&text));
                result.push_str("\")");
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_close = match kind {
                    CodeBlockKind::Fenced(lang) => match lang.split_whitespace().next() {
                        Some(lang) if !lang.is_empty() => {
                            format!("\", lang: \"{}\", block: true)", escape_typst_literal(lang))
                        }
                        _ => "\", block: true)".to_string(),
                    },
                    _ => "\", block: true)".to_string(),
                };
                result.push_str("\n#raw(\"");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                result.push_str(&code_block_close);
                result.push_str("\n\n");
            }
            Event::Start(Tag::Table(alignments)) => {
                table_alignments = alignments;
                table_column = 0;
                let aligns = table_alignments
                    .iter()
                    .map(|a| format!("{},", alignment_to_typst(*a)))
                    .collect::<Vec<_>>()
                    .join(" ");
                result.push_str(&format!(
                    "\n#table(\n  columns: {},\n  align: (col, row) => ({}).at(col),\n  inset: (x: 8pt, y: 5.5pt),\n  stroke: none,\n",
                    table_alignments.len(),
                    aligns
                ));
            }
            Event::End(TagEnd::Table) => {
                result.push_str("  table.hline(stroke: 0.5pt + hairline),\n)\n\n");
                table_alignments.clear();
            }
            Event::Start(Tag::TableHead) => {
                in_table_header = true;
                table_column = 0;
                result.push_str("  table.header(\n");
            }
            Event::End(TagEnd::TableHead) => {
                in_table_header = false;
                result.push_str("  ),\n  table.hline(stroke: 0.5pt + hairline),\n");
            }
            Event::Start(Tag::TableRow) => {
                table_column = 0;
            }
            Event::End(TagEnd::TableRow) => {}
            Event::Start(Tag::TableCell) => {
                let align = table_alignments
                    .get(table_column)
                    .map(|a| alignment_to_typst(*a))
                    .unwrap_or("left");
                table_column += 1;
                if in_table_header {
                    result.push_str(&format!(
                        "    table.cell(fill: surface, align: {})[*",
                        align
                    ));
                } else {
                    result.push_str(&format!("  table.cell(align: {})[", align));
                }
            }
            Event::End(TagEnd::TableCell) => {
                if in_table_header {
                    result.push_str("*],\n");
                } else {
                    result.push_str("],\n");
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    result.push_str(&escape_typst_literal(&text));
                } else {
                    result.push_str(&escape_typst_string(&text));
                }
            }
            Event::SoftBreak => result.push('\n'),
            Event::HardBreak => result.push_str("\\\n"),
            Event::Rule => {
                result.push_str("\n#line(length: 100%, stroke: 0.5pt + hairline)\n\n");
            }
            _ => {}
        }
    }

    result.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::markdown_to_typst;

    #[test]
    fn single_column_table_uses_array() {
        let out = markdown_to_typst("| Status |\n| --- |\n| Done |");
        assert!(out.contains("(left,).at(col)"), "{out}");
    }

    #[test]
    fn image_alt_text_is_kept() {
        let out = markdown_to_typst("See ![the diagram](https://x/img.png \"t\") now");
        assert!(out.contains("the diagram"), "{out}");
    }

    #[test]
    fn hash_in_link_url_is_escaped() {
        let out = markdown_to_typst("[t](https://x/p#frag)");
        assert!(out.contains("\\u{23}frag"), "{out}");
    }

    #[test]
    fn fence_metadata_uses_first_token() {
        let out = markdown_to_typst("```rust title=main.rs\nfn main() {}\n```");
        assert!(out.contains("lang: \"rust\""), "{out}");
    }
}
