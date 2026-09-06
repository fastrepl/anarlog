//! `buildInputRules` from `packages/editor/src/note/keymap.ts`: shortcuts that
//! fire on the character being typed, before it lands in the document. The
//! first matching rule wins, as in prosemirror-inputrules.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::model::{Caret, Doc};

/// `MAX_MATCH` in prosemirror-inputrules.
const MAX_MATCH: usize = 500;

enum Rule {
    Heading,
    Blockquote,
    BulletList,
    TaskList,
    OrderedList,
    CodeBlock,
    Divider,
    /// `SYMBOL_REPLACEMENTS`
    Symbol,
    /// `--` → em dash, unless preceded by another dash.
    Dash,
    Abbreviation,
    Fraction,
    /// Smart quote: the replacement goes where the captured quote is.
    Quote(&'static str),
    /// `...` → `…`
    Ellipsis,
    /// `**text**` and friends: `(mark, delimiter length)`.
    Mark(&'static str, usize),
}

static RULES: LazyLock<Vec<(Regex, Rule)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"^(#{1,6})\s$").unwrap(), Rule::Heading),
        (Regex::new(r"^\s*>\s$").unwrap(), Rule::Blockquote),
        (Regex::new(r"^\s*([-+*])\s$").unwrap(), Rule::BulletList),
        (Regex::new(r"^\s*(\d+)\.\s$").unwrap(), Rule::OrderedList),
        (Regex::new(r"^```$").unwrap(), Rule::CodeBlock),
        (Regex::new(r"^(?:---|___|\*\*\*)\s$").unwrap(), Rule::Divider),
        (Regex::new(r"^\s*\[([ x]?)\]\s$").unwrap(), Rule::TaskList),
        (
            Regex::new(r"(?:<->|==>|<==|<=>|->|<-|\+/-|\+-|=/=)$").unwrap(),
            Rule::Symbol,
        ),
        (Regex::new(r"([^-])--$").unwrap(), Rule::Dash),
        (
            Regex::new(r"(?i)(^|[\s(\[{])(\((?:c|r|tm)\))$").unwrap(),
            Rule::Abbreviation,
        ),
        (
            Regex::new(r"(?i)(^|[\s(\[{])((?:c/o|1/2|1/3|1/4|1/5|1/6|1/8|2/3|2/5|3/4|3/5|3/8|4/5|5/6|5/8|7/8))([\s.,;:!?])$").unwrap(),
            Rule::Fraction,
        ),
        (Regex::new("(?:^|[\\s{\\[(<'\"‘“])(\")$").unwrap(), Rule::Quote("“")),
        (Regex::new("\"$").unwrap(), Rule::Quote("”")),
        (Regex::new("(?:^|[\\s{\\[(<'\"‘“])(')$").unwrap(), Rule::Quote("‘")),
        (Regex::new("'$").unwrap(), Rule::Quote("’")),
        (Regex::new(r"\.\.\.$").unwrap(), Rule::Ellipsis),
        (Regex::new(r"(^|[^*])\*\*([^*]+)\*\*$").unwrap(), Rule::Mark("bold", 2)),
        (Regex::new(r"(^|[^~])~~([^~]+)~~$").unwrap(), Rule::Mark("strike", 2)),
        (Regex::new(r"(^|[^=])==([^=]+)==$").unwrap(), Rule::Mark("highlight", 2)),
        (Regex::new(r"(^|[^*])\*([^*]+)\*$").unwrap(), Rule::Mark("italic", 1)),
        (Regex::new(r"(^|[^_])_([^_]+)_$").unwrap(), Rule::Mark("italic", 1)),
        (Regex::new(r"(^|[^~])~([^~]+)~$").unwrap(), Rule::Mark("strike", 1)),
    ]
});

fn symbol(text: &str) -> Option<&'static str> {
    Some(match text.to_lowercase().as_str() {
        "->" => "→",
        "<-" => "←",
        "<->" => "↔",
        "==>" => "⇒",
        "<==" => "⇐",
        "<=>" => "⇔",
        "+-" | "+/-" => "±",
        "=/=" => "≠",
        _ => return None,
    })
}

fn abbreviation(text: &str) -> Option<&'static str> {
    Some(match text.to_lowercase().as_str() {
        "(c)" => "©",
        "(r)" => "®",
        "(tm)" => "™",
        _ => return None,
    })
}

fn fraction(text: &str) -> Option<&'static str> {
    Some(match text.to_lowercase().as_str() {
        "c/o" => "℅",
        "1/2" => "½",
        "1/3" => "⅓",
        "1/4" => "¼",
        "1/5" => "⅕",
        "1/6" => "⅙",
        "1/8" => "⅛",
        "2/3" => "⅔",
        "2/5" => "⅖",
        "3/4" => "¾",
        "3/5" => "⅗",
        "3/8" => "⅜",
        "4/5" => "⅘",
        "5/6" => "⅚",
        "5/8" => "⅝",
        "7/8" => "⅞",
        _ => return None,
    })
}

pub struct Outcome {
    pub caret: Caret,
    /// `tr.removeStoredMark(markType)` after a mark rule.
    pub clear_stored_mark: Option<&'static str>,
}

/// Runs the rules for `typed` at `caret`. Returns `None` when no rule fired,
/// in which case the caller inserts the text as usual.
pub fn apply(doc: &mut Doc, caret: Caret, typed: &str, in_code_context: bool) -> Option<Outcome> {
    let text = doc.text(caret.block);
    let mut from = caret.offset.saturating_sub(MAX_MATCH);
    while !text.is_char_boundary(from) {
        from += 1;
    }
    let before = &text[from..caret.offset];
    let combined = format!("{before}{typed}");
    let block_type = doc.block_type(caret.block)?;
    let parent_type = doc.parent_type(caret.block)?;
    let is_paragraph = block_type == "paragraph";
    // Block-level rules only apply where the schema allows the new node
    // (`canReplaceWith` / `findWrapping`): in the document, a blockquote, or
    // a list item's `block*` tail, since an item must start with a paragraph.
    let block_allowed = is_paragraph
        && (matches!(parent_type.as_str(), "doc" | "blockquote")
            || (matches!(parent_type.as_str(), "listItem" | "taskItem")
                && !doc.is_first_child(caret.block)));

    for (regex, rule) in RULES.iter() {
        let Some(captures) = regex.captures(&combined) else {
            continue;
        };
        let whole = captures.get(0).unwrap();
        // `start = from - (match[0].length - text.length)`, in block offsets.
        let start = caret.offset + typed.len() - whole.len();
        let end = caret.offset;
        let group = |index: usize| captures.get(index).map(|m| m.as_str()).unwrap_or("");
        match rule {
            Rule::Heading => {
                if !block_allowed {
                    continue;
                }
                let level = group(1).len() as u64;
                doc.delete_range(caret.block, start..end);
                doc.set_block_type(caret.block, "heading", Some(json!({ "level": level })));
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: start,
                    },
                    clear_stored_mark: None,
                });
            }
            Rule::Blockquote => {
                if !block_allowed {
                    continue;
                }
                doc.delete_range(caret.block, start..end);
                doc.wrap_block(caret.block, "blockquote", None, false);
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: start,
                    },
                    clear_stored_mark: None,
                });
            }
            Rule::BulletList => {
                if !block_allowed {
                    continue;
                }
                doc.delete_range(caret.block, start..end);
                doc.wrap_block(caret.block, "bulletList", None, true);
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: start,
                    },
                    clear_stored_mark: None,
                });
            }
            Rule::TaskList => {
                // Unlike the wrapping rules this one `replaceWith`s, which the
                // fitter also accepts in an item's first paragraph (keeping the
                // paragraph and nesting the list after it).
                let in_item_head = is_paragraph
                    && matches!(parent_type.as_str(), "listItem" | "taskItem")
                    && doc.is_first_child(caret.block);
                if !block_allowed && !in_item_head {
                    continue;
                }
                let checked = group(1) == "x";
                doc.delete_range(caret.block, start..end);
                let caret = if block_allowed {
                    doc.replace_with_task_list(caret.block, checked)
                } else {
                    doc.insert_task_list_after(caret.block, checked)
                };
                return Some(Outcome {
                    caret,
                    clear_stored_mark: None,
                });
            }
            Rule::OrderedList => {
                if !block_allowed {
                    continue;
                }
                let number: u64 = group(1).parse().unwrap_or(1);
                doc.delete_range(caret.block, start..end);
                // `joinPredicate`: continue the previous list only when the
                // number follows on from it.
                let join =
                    doc.previous_sibling_list_continues_at(caret.block, "orderedList", number);
                doc.wrap_block(
                    caret.block,
                    "orderedList",
                    Some(json!({ "start": number })),
                    join,
                );
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: start,
                    },
                    clear_stored_mark: None,
                });
            }
            Rule::CodeBlock => {
                if !block_allowed {
                    continue;
                }
                doc.delete_range(caret.block, start..end);
                doc.set_block_type(caret.block, "codeBlock", None);
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: start,
                    },
                    clear_stored_mark: None,
                });
            }
            Rule::Divider => {
                if !block_allowed || start != 0 {
                    continue;
                }
                let caret = doc.replace_block_with_rule(caret.block);
                return Some(Outcome {
                    caret,
                    clear_stored_mark: None,
                });
            }
            Rule::Symbol
            | Rule::Dash
            | Rule::Abbreviation
            | Rule::Fraction
            | Rule::Quote(_)
            | Rule::Ellipsis => {
                if in_code_context {
                    continue;
                }
                let (replace_from, replacement) = match rule {
                    Rule::Symbol => {
                        let Some(replacement) = symbol(whole.as_str()) else {
                            continue;
                        };
                        (start, replacement.to_string())
                    }
                    Rule::Dash => (start + group(1).len(), "—".to_string()),
                    Rule::Abbreviation => {
                        let Some(replacement) = abbreviation(group(2)) else {
                            continue;
                        };
                        (start + group(1).len(), replacement.to_string())
                    }
                    Rule::Fraction => {
                        let Some(replacement) = fraction(group(2)) else {
                            continue;
                        };
                        (start + group(1).len(), format!("{replacement}{}", group(3)))
                    }
                    Rule::Quote(replacement) => {
                        let quote = group(1);
                        let offset = if quote.is_empty() {
                            0
                        } else {
                            whole.as_str().rfind(quote).unwrap_or(0)
                        };
                        (start + offset, replacement.to_string())
                    }
                    Rule::Ellipsis => (start, "…".to_string()),
                    _ => unreachable!(),
                };
                let replace_from = replace_from.min(end);
                doc.delete_range(caret.block, replace_from..end);
                let caret = doc.insert_text(
                    Caret {
                        block: caret.block,
                        offset: replace_from,
                    },
                    &replacement,
                );
                return Some(Outcome {
                    caret,
                    clear_stored_mark: None,
                });
            }
            Rule::Mark(mark, delimiter) => {
                if block_type == "codeBlock" {
                    continue;
                }
                let prefix = group(1);
                let content = group(2);
                let open_start = start + prefix.len();
                // The typed character is the last closing delimiter and is
                // not in the document; only `delimiter - 1` chars remain.
                let close_in_doc = delimiter - 1;
                if close_in_doc > 0 {
                    doc.delete_range(caret.block, end - close_in_doc..end);
                }
                doc.delete_range(caret.block, open_start..open_start + delimiter);
                let content_end = open_start + content.len();
                doc.toggle_mark(
                    Caret {
                        block: caret.block,
                        offset: open_start,
                    },
                    Caret {
                        block: caret.block,
                        offset: content_end,
                    },
                    mark,
                );
                return Some(Outcome {
                    caret: Caret {
                        block: caret.block,
                        offset: content_end,
                    },
                    clear_stored_mark: Some(mark),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Doc {
        Doc::parse(&format!(
            r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{text}"}}]}}]}}"#
        ))
    }

    fn type_char(doc: &mut Doc, caret: Caret, typed: &str) -> Caret {
        match apply(doc, caret, typed, false) {
            Some(outcome) => outcome.caret,
            None => doc.insert_text(caret, typed),
        }
    }

    #[test]
    fn markdown_shortcuts_change_block_types() {
        let mut d = doc("##");
        let c = type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 2,
            },
            " ",
        );
        assert_eq!(
            c,
            Caret {
                block: 0,
                offset: 0
            }
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2}}]}"#
        );

        let mut d = doc("-");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 1,
            },
            " ",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph"}]}]}]}"#
        );
        // An item's first paragraph cannot become a heading, but a later
        // paragraph in the item can hold a nested list.
        let c = type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 0,
            },
            "#",
        );
        let c = type_char(&mut d, c, " ");
        assert_eq!(d.text(0), "# ");
        assert_eq!(c.offset, 2);
        let mut d = Doc::parse(
            r##"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"paragraph","content":[{"type":"text","text":"1."}]}]}]}]}"##,
        );
        type_char(
            &mut d,
            Caret {
                block: 1,
                offset: 2,
            },
            " ",
        );
        assert_eq!(
            d.to_json(),
            r##"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"orderedList","attrs":{"start":1},"content":[{"type":"listItem","content":[{"type":"paragraph"}]}]}]}]}]}"##
        );

        let mut d = doc("1.");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 2,
            },
            " ",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"orderedList","attrs":{"start":1},"content":[{"type":"listItem","content":[{"type":"paragraph"}]}]}]}"#
        );

        let mut d = doc(">");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 1,
            },
            " ",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph"}]}]}"#
        );

        let mut d = doc("``");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 2,
            },
            "`",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"codeBlock"}]}"#
        );

        let mut d = doc("---");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 3,
            },
            " ",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"horizontalRule"},{"type":"paragraph"}]}"#
        );
    }

    #[test]
    fn text_replacements_and_smart_quotes() {
        let mut d = doc("a -");
        let c = type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 3,
            },
            ">",
        );
        assert_eq!(d.text(0), "a →");
        assert_eq!(c.offset, "a →".len());

        let mut d = doc("wait -");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 6,
            },
            "-",
        );
        assert_eq!(d.text(0), "wait —");
        // A line-start `--` survives for the horizontal rule.
        let mut d = doc("-");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 1,
            },
            "-",
        );
        assert_eq!(d.text(0), "--");

        let mut d = doc("(c");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 2,
            },
            ")",
        );
        assert_eq!(d.text(0), "©");
        let mut d = doc("1/2");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 3,
            },
            " ",
        );
        assert_eq!(d.text(0), "½ ");

        let mut d = doc("say ");
        let c = type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 4,
            },
            "\"",
        );
        let c = type_char(&mut d, c, "h");
        let c = type_char(&mut d, c, "i");
        type_char(&mut d, c, "\"");
        assert_eq!(d.text(0), "say “hi”");

        let mut d = doc("..");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 2,
            },
            ".",
        );
        assert_eq!(d.text(0), "…");

        // No replacements inside code.
        let mut d = doc("a -");
        assert!(
            apply(
                &mut d,
                Caret {
                    block: 0,
                    offset: 3
                },
                ">",
                true
            )
            .is_none()
        );
    }

    #[test]
    fn mark_shortcuts_wrap_the_content() {
        let mut d = doc("make **this*");
        let outcome = apply(
            &mut d,
            Caret {
                block: 0,
                offset: 12,
            },
            "*",
            false,
        )
        .unwrap();
        assert_eq!(outcome.clear_stored_mark, Some("bold"));
        assert_eq!(
            outcome.caret,
            Caret {
                block: 0,
                offset: 9
            }
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"make "},{"type":"text","marks":[{"type":"bold"}],"text":"this"}]}]}"#
        );

        let mut d = doc("_it");
        type_char(
            &mut d,
            Caret {
                block: 0,
                offset: 3,
            },
            "_",
        );
        assert_eq!(
            d.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","marks":[{"type":"italic"}],"text":"it"}]}]}"#
        );
    }
}
