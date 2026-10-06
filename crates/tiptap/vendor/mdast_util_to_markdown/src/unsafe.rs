//! Unsafe patterns.
//!
//! JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/unsafe.js>.
//! Also: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L287-L305>.

use crate::{construct_name::ConstructName, Options};
use alloc::{vec, vec::Vec};
use regex::Regex;

#[derive(Default)]
pub struct Unsafe<'a> {
    pub after: Option<&'a str>,
    pub at_break: bool,
    pub before: Option<&'a str>,
    pub character: char,
    pub(crate) compiled: Option<Regex>,
    pub in_construct: Vec<ConstructName>,
    pub not_in_construct: Vec<ConstructName>,
}

impl<'a> Unsafe<'a> {
    pub fn new(
        character: char,
        before: Option<&'a str>,
        after: Option<&'a str>,
        in_construct: Vec<ConstructName>,
        not_in_construct: Vec<ConstructName>,
        at_break: bool,
    ) -> Self {
        Unsafe {
            after,
            at_break,
            before,
            character,
            compiled: None,
            in_construct,
            not_in_construct,
        }
    }

    pub fn get_default_unsafe(options: &Options) -> Vec<Self> {
        let full_phrasing_spans = vec![
            ConstructName::Autolink,
            ConstructName::DestinationLiteral,
            ConstructName::DestinationRaw,
            ConstructName::Reference,
            ConstructName::TitleApostrophe,
            ConstructName::TitleQuote,
        ];

        vec![
            Self::new(
                '\t',
                None,
                "[\\r\\n]".into(),
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                '\t',
                "[\\r\\n]".into(),
                None,
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                '\t',
                None,
                None,
                vec![
                    ConstructName::CodeFencedLangGraveAccent,
                    ConstructName::CodeFencedLangTilde,
                ],
                vec![],
                false,
            ),
            Self::new(
                '\r',
                None,
                None,
                vec![
                    ConstructName::CodeFencedLangGraveAccent,
                    ConstructName::CodeFencedLangTilde,
                    ConstructName::CodeFencedMetaGraveAccent,
                    ConstructName::CodeFencedMetaTilde,
                    ConstructName::DestinationLiteral,
                    ConstructName::HeadingAtx,
                    ConstructName::MathFlowMeta,
                ],
                vec![],
                false,
            ),
            Self::new(
                '\n',
                None,
                None,
                vec![
                    ConstructName::CodeFencedLangGraveAccent,
                    ConstructName::CodeFencedLangTilde,
                    ConstructName::CodeFencedMetaGraveAccent,
                    ConstructName::CodeFencedMetaTilde,
                    ConstructName::DestinationLiteral,
                    ConstructName::HeadingAtx,
                    ConstructName::MathFlowMeta,
                ],
                vec![],
                false,
            ),
            Self::new(
                ' ',
                None,
                "[\\r\\n]".into(),
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                ' ',
                "[\\r\\n]".into(),
                None,
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                ' ',
                None,
                None,
                vec![
                    ConstructName::CodeFencedLangGraveAccent,
                    ConstructName::CodeFencedLangTilde,
                ],
                vec![],
                false,
            ),
            Self::new(
                '!',
                None,
                "\\[".into(),
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new(
                '\"',
                None,
                None,
                vec![ConstructName::TitleQuote],
                vec![],
                false,
            ),
            Self::new('#', None, None, vec![], vec![], true),
            Self::new(
                '#',
                None,
                "(?:[\r\n]|$)".into(),
                vec![ConstructName::HeadingAtx],
                vec![],
                false,
            ),
            Self::new(
                '&',
                None,
                "[#A-Za-z]".into(),
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                '\'',
                None,
                None,
                vec![ConstructName::TitleApostrophe],
                vec![],
                false,
            ),
            Self::new(
                '(',
                None,
                None,
                vec![ConstructName::DestinationRaw],
                vec![],
                false,
            ),
            Self::new(
                '(',
                "\\]".into(),
                None,
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new(')', "\\d+".into(), None, vec![], vec![], true),
            Self::new(
                ')',
                None,
                None,
                vec![ConstructName::DestinationRaw],
                vec![],
                false,
            ),
            Self::new('*', None, "(?:[ \t\r\n*])".into(), vec![], vec![], true),
            Self::new(
                '*',
                None,
                None,
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new('+', None, "(?:[ \t\r\n])".into(), vec![], vec![], true),
            Self::new('-', None, "(?:[ \t\r\n-])".into(), vec![], vec![], true),
            Self::new(
                '.',
                "\\d+".into(),
                "(?:[ \t\r\n]|$)".into(),
                vec![],
                vec![],
                true,
            ),
            Self::new('<', None, "[!/?A-Za-z]".into(), vec![], vec![], true),
            Self::new(
                '<',
                None,
                "[!/?A-Za-z]".into(),
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new(
                '<',
                None,
                None,
                vec![ConstructName::DestinationLiteral],
                vec![],
                false,
            ),
            Self::new('=', None, None, vec![], vec![], true),
            Self::new('>', None, None, vec![], vec![], true),
            Self::new(
                '>',
                None,
                None,
                vec![ConstructName::DestinationLiteral],
                vec![],
                false,
            ),
            Self::new('[', None, None, vec![], vec![], true),
            Self::new(
                '[',
                None,
                None,
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new(
                '[',
                None,
                None,
                vec![ConstructName::Label, ConstructName::Reference],
                vec![],
                false,
            ),
            Self::new(
                '\\',
                None,
                "[\\r\\n]".into(),
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                ']',
                None,
                None,
                vec![ConstructName::Label, ConstructName::Reference],
                vec![],
                false,
            ),
            Self::new('_', None, None, vec![], vec![], true),
            Self::new(
                '_',
                None,
                None,
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new('`', None, None, vec![], vec![], true),
            Self::new(
                '`',
                None,
                None,
                vec![
                    ConstructName::CodeFencedLangGraveAccent,
                    ConstructName::CodeFencedMetaGraveAccent,
                ],
                vec![],
                false,
            ),
            Self::new(
                '`',
                None,
                None,
                vec![ConstructName::Phrasing],
                full_phrasing_spans.clone(),
                false,
            ),
            Self::new('~', None, None, vec![], vec![], true),
            Self::new(
                '$',
                None,
                if options.single_dollar_text_math {
                    None
                } else {
                    "\\$".into()
                },
                vec![ConstructName::Phrasing],
                vec![],
                false,
            ),
            Self::new(
                '$',
                None,
                None,
                vec![ConstructName::MathFlowMeta],
                vec![],
                false,
            ),
            Self::new('$', None, "\\$".into(), vec![], vec![], true),
        ]
    }

    pub(crate) fn set_compiled(&mut self, regex_pattern: Regex) {
        self.compiled = Some(regex_pattern);
    }
}
