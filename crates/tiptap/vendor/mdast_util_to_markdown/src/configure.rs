//! Configuration.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L307.

#[derive(Clone, Copy)]
/// Configuration for indent of lists.
pub enum IndentOptions {
    /// Depends on the item and its parent list: uses `IndentOptions::One` if
    /// the item and list are tight and `IndentOptions::Tab` otherwise.
    Mixed,
    /// The size of the bullet plus one space.
    One,
    /// Tab stop.
    Tab,
}

/// Configuration.
pub struct Options {
    /// Marker to use for bullets of items in unordered lists (`'*'`, `'+'`, or
    /// `'-'`, default: `'*'`).
    pub bullet: char,
    /// Marker to use for bullets of items in ordered lists (`'.'` or `')'`,
    /// default: `'.'`).
    pub bullet_ordered: char,
    /// Marker to use in certain cases where the primary bullet doesn’t work
    /// (`'*'`, `'+'`, or `'-'`, default: `'-'` when bullet is `'*'`, `'*'`
    /// otherwise).
    pub bullet_other: char,
    /// Whether to add the same number of number signs (`#`) at the end of an
    /// ATX heading as the opening sequence (`bool`, default: `false`).
    pub close_atx: bool,
    /// Marker to use for emphasis (`'*'` or `'_'`, default: `'*'`).
    pub emphasis: char,
    /// Marker to use for fenced code (``'`'`` or `'~'`, default: ``'`'``).
    pub fence: char,
    /// Whether to use fenced code always (`bool`, default: `true`).
    /// The default is to use fenced code if there is a language defined,
    /// if the code is empty,
    /// or if it starts or ends in blank lines.
    pub fences: bool,
    /// Whether to increment the counter of ordered lists items (`bool`,
    /// default: `true`).
    pub increment_list_marker: bool,
    /// How to indent the content of list items (default: `IndentOptions::One`).
    pub list_item_indent: IndentOptions,
    /// Marker to use for titles (`'"'` or `"'"`, default: `'"'`).
    pub quote: char,
    /// Whether to always use resource links (`bool`, default: `false`).
    /// The default is to use autolinks (`<https://example.com>`) when possible
    /// and resource links (`[text](url)`) otherwise.
    pub resource_link: bool,
    /// Marker to use for thematic breaks (`'*'`, `'-'`, or `'_'`, default:
    /// `'*'`).
    pub rule: char,
    /// Number of markers to use for thematic breaks (`u32`, default: `3`, min:
    /// `3`).
    pub rule_repetition: u32,
    /// Whether to add spaces between markers in thematic breaks (`bool`,
    /// default: `false`).
    pub rule_spaces: bool,
    /// Whether to use setext headings when possible (`bool`, default:
    /// `false`).
    /// The default is to always use ATX headings (`# heading`) instead of
    /// setext headings (`heading\n=======`).
    /// Setext headings cannot be used for empty headings or headings with a
    /// rank of three or more.
    pub setext: bool,
    /// Whether to support math (text) with a single dollar (`bool`, default: `true`).
    /// Single dollars work in Pandoc and many other places, but often interfere with “normal”
    /// dollars in text.
    /// If you turn this off, you can still use two or more dollars for text math.
    pub single_dollar_text_math: bool,
    /// Marker to use for strong (`'*'` or `'_'`, default: `'*'`).
    pub strong: char,
    /// Whether to join definitions without a blank line (`bool`, default:
    /// `false`).
    pub tight_definitions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            bullet: '*',
            bullet_ordered: '.',
            bullet_other: '-',
            close_atx: false,
            emphasis: '*',
            fence: '`',
            fences: true,
            increment_list_marker: true,
            list_item_indent: IndentOptions::One,
            quote: '"',
            resource_link: false,
            rule: '*',
            rule_repetition: 3,
            rule_spaces: false,
            setext: false,
            single_dollar_text_math: true,
            strong: '*',
            tight_definitions: false,
        }
    }
}
