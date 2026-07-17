use crate::{
    STYLE_TYPE_CHECKBOX, STYLE_TYPE_DASHED_LIST, STYLE_TYPE_DOTTED_LIST, STYLE_TYPE_HEADING,
    STYLE_TYPE_MONOSPACED, STYLE_TYPE_NUMBERED_LIST, STYLE_TYPE_SUBHEADING, STYLE_TYPE_TITLE,
    extract::extract_text_spans, proto::Note,
};

pub fn note_to_markdown(note: &Note) -> String {
    let spans = extract_text_spans(note);
    let mut markdown = String::new();
    let mut in_code_block = false;
    let mut list_counters: Vec<usize> = Vec::new();

    for span in spans {
        let lines: Vec<&str> = span.text.split('\n').collect();

        for (line_idx, line) in lines.iter().enumerate() {
            if line_idx > 0 && !in_code_block {
                markdown.push('\n');
            }

            let is_block_quote = span
                .paragraph_style
                .as_ref()
                .and_then(|ps| ps.block_quote)
                .map(|bq| bq == 1)
                .unwrap_or(false);

            let indent_amount = span
                .paragraph_style
                .as_ref()
                .and_then(|ps| ps.indent_amount)
                .unwrap_or(0) as usize;

            let mut prefix = String::new();

            if is_block_quote {
                prefix.push_str("> ");
            }

            for _ in 0..indent_amount {
                prefix.push_str("  ");
            }

            if let Some(style_type) = span.style_type {
                match style_type {
                    STYLE_TYPE_TITLE => {
                        if !in_code_block {
                            markdown.push_str(&prefix);
                            markdown.push_str("# ");
                        }
                    }
                    STYLE_TYPE_HEADING => {
                        if !in_code_block {
                            markdown.push_str(&prefix);
                            markdown.push_str("## ");
                        }
                    }
                    STYLE_TYPE_SUBHEADING => {
                        if !in_code_block {
                            markdown.push_str(&prefix);
                            markdown.push_str("### ");
                        }
                    }
                    STYLE_TYPE_MONOSPACED => {
                        if !in_code_block && line_idx == 0 {
                            markdown.push_str("```\n");
                            in_code_block = true;
                        }
                    }
                    STYLE_TYPE_DOTTED_LIST => {
                        markdown.push_str(&prefix);
                        markdown.push_str("- ");
                    }
                    STYLE_TYPE_DASHED_LIST => {
                        markdown.push_str(&prefix);
                        markdown.push_str("- ");
                    }
                    STYLE_TYPE_NUMBERED_LIST => {
                        while list_counters.len() <= indent_amount {
                            list_counters.push(1);
                        }
                        markdown.push_str(&prefix);
                        markdown.push_str(&format!("{}. ", list_counters[indent_amount]));
                        list_counters[indent_amount] += 1;
                    }
                    STYLE_TYPE_CHECKBOX => {
                        let is_checked = span
                            .paragraph_style
                            .as_ref()
                            .and_then(|ps| ps.checklist.as_ref())
                            .map(|cl| cl.done == 1)
                            .unwrap_or(false);
                        markdown.push_str(&prefix);
                        if is_checked {
                            markdown.push_str("- [x] ");
                        } else {
                            markdown.push_str("- [ ] ");
                        }
                    }
                    _ => {
                        if !in_code_block {
                            markdown.push_str(&prefix);
                        }
                    }
                }
            } else if !in_code_block {
                markdown.push_str(&prefix);
            }

            let mut formatted_text = String::new();

            if in_code_block {
                formatted_text.push_str(line);
            } else {
                if span.bold && span.italic {
                    formatted_text.push_str("***");
                } else if span.bold {
                    formatted_text.push_str("**");
                } else if span.italic {
                    formatted_text.push('*');
                }

                if span.strikethrough {
                    formatted_text.push_str("~~");
                }

                if let Some(ref link) = span.link {
                    formatted_text.push('[');
                    formatted_text.push_str(line);
                    formatted_text.push_str("](");
                    formatted_text.push_str(link);
                    formatted_text.push(')');
                } else {
                    formatted_text.push_str(line);
                }

                if span.strikethrough {
                    formatted_text.push_str("~~");
                }

                if span.bold && span.italic {
                    formatted_text.push_str("***");
                } else if span.bold {
                    formatted_text.push_str("**");
                } else if span.italic {
                    formatted_text.push('*');
                }
            }

            markdown.push_str(&formatted_text);

            if in_code_block && line_idx == lines.len() - 1 {
                let next_is_code = false;
                if !next_is_code {
                    markdown.push_str("\n```");
                    in_code_block = false;
                }
            }
        }
    }

    if in_code_block {
        markdown.push_str("\n```");
    }

    markdown
}
