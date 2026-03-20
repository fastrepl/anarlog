use std::ops::Range;
use std::sync::LazyLock;

use ansi_to_tui::IntoText;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::Style;
use ratatui::text::Text;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};

use crate::StyleSheet;

pub(crate) struct LinkInfo {
    pub range: Range<usize>,
    pub url: String,
}

pub(crate) struct HighlightResult {
    pub styles: Vec<Vec<(Range<usize>, Style)>>,
    pub links: Vec<Vec<LinkInfo>>,
}

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

pub(crate) struct Highlighter<S: StyleSheet> {
    styles: S,
}

impl<S: StyleSheet> Highlighter<S> {
    pub fn new(styles: S) -> Self {
        Self { styles }
    }

    pub fn set_styles(&mut self, styles: S) {
        self.styles = styles;
    }

    pub fn highlight(&self, lines: &[String]) -> HighlightResult {
        let source = lines.join("\n");
        let mut styles: Vec<Vec<(Range<usize>, Style)>> =
            lines.iter().map(|_| Vec::new()).collect();
        let mut links: Vec<Vec<LinkInfo>> = lines.iter().map(|_| Vec::new()).collect();

        // Build line start offsets for byte→line mapping
        let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
        let mut off = 0;
        for line in lines {
            line_starts.push(off);
            off += line.len() + 1;
        }

        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(&source, opts).into_offset_iter();

        let mut inline_style_stack: Vec<Style> = Vec::new();
        let mut in_code_block = false;
        let mut code_lang: Option<String> = None;
        let mut code_block_text = String::new();
        let mut code_block_byte_start: usize = 0;
        let mut link_url_stack: Vec<String> = Vec::new();

        for (event, range) in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    let lvl = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    let style = self.styles.heading(lvl);
                    inline_style_stack.push(style);
                    // Style the entire heading source line (including the # prefix)
                    let line_idx = byte_to_line(&line_starts, range.start);
                    if line_idx < styles.len() {
                        let line_len = lines[line_idx].len();
                        styles[line_idx].push((0..line_len, style));
                    }
                }
                Event::End(TagEnd::Heading(_)) => {
                    inline_style_stack.pop();
                }
                Event::Start(Tag::Strong) => {
                    inline_style_stack.push(self.styles.strong());
                }
                Event::End(TagEnd::Strong) => {
                    inline_style_stack.pop();
                }
                Event::Start(Tag::Emphasis) => {
                    inline_style_stack.push(self.styles.emphasis());
                }
                Event::End(TagEnd::Emphasis) => {
                    inline_style_stack.pop();
                }
                Event::Start(Tag::Strikethrough) => {
                    inline_style_stack.push(self.styles.strikethrough());
                }
                Event::End(TagEnd::Strikethrough) => {
                    inline_style_stack.pop();
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    inline_style_stack.push(self.styles.link());
                    link_url_stack.push(dest_url.to_string());
                }
                Event::End(TagEnd::Link) => {
                    inline_style_stack.pop();
                    link_url_stack.pop();
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    inline_style_stack.push(self.styles.blockquote());
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    inline_style_stack.pop();
                }
                Event::Start(Tag::List(_)) => {
                    // list markers styled via Item events
                }
                Event::Start(Tag::Item) => {
                    let line_idx = byte_to_line(&line_starts, range.start);
                    if line_idx < styles.len() {
                        let line = &lines[line_idx];
                        // Style the leading marker (e.g. "- " or "1. ")
                        let trimmed = line.trim_start();
                        let indent = line.len() - trimmed.len();
                        let marker_len = if trimmed.starts_with("- ")
                            || trimmed.starts_with("* ")
                            || trimmed.starts_with("+ ")
                        {
                            indent + 2
                        } else {
                            // ordered list: digits + ". "
                            let num_len =
                                trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
                            if num_len > 0 { indent + num_len + 2 } else { 0 }
                        };
                        if marker_len > 0 {
                            styles[line_idx].push((0..marker_len, self.styles.list_marker()));
                        }
                    }
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    in_code_block = true;
                    code_lang = match kind {
                        CodeBlockKind::Fenced(lang) => {
                            let l = lang.trim().to_string();
                            if l.is_empty() { None } else { Some(l) }
                        }
                        CodeBlockKind::Indented => None,
                    };
                    code_block_text.clear();
                    // Style the fence line itself
                    let fence_line = byte_to_line(&line_starts, range.start);
                    if fence_line < styles.len() {
                        let line_len = lines[fence_line].len();
                        styles[fence_line].push((0..line_len, self.styles.code_fence()));
                    }
                    code_block_byte_start = 0;
                }
                Event::End(TagEnd::CodeBlock) => {
                    if !code_block_text.is_empty() {
                        let start_line = byte_to_line(&line_starts, code_block_byte_start);
                        self.highlight_code_block(
                            &code_block_text,
                            code_lang.as_deref(),
                            start_line,
                            &mut styles,
                            lines,
                        );
                    }
                    // Style the closing fence
                    let end_line = byte_to_line(&line_starts, range.start);
                    if end_line < styles.len() {
                        let line_len = lines[end_line].len();
                        styles[end_line].push((0..line_len, self.styles.code_fence()));
                    }
                    in_code_block = false;
                    code_lang = None;
                    code_block_text.clear();
                }
                Event::Text(text) if in_code_block => {
                    if code_block_text.is_empty() {
                        code_block_byte_start = range.start;
                    }
                    code_block_text.push_str(&text);
                }
                Event::Code(_) => {
                    let style = self.styles.code_inline();
                    let line_idx = byte_to_line(&line_starts, range.start);
                    if line_idx < styles.len() {
                        let line_start = line_starts[line_idx];
                        let local_start = range.start - line_start;
                        let local_end = (range.end - line_start).min(lines[line_idx].len());
                        styles[line_idx].push((local_start..local_end, style));
                    }
                }
                Event::Text(text) => {
                    if let Some(style) = inline_style_stack.last().copied() {
                        // For headings, we already styled the whole line
                        // For other inline elements, style the text range
                        let line_idx = byte_to_line(&line_starts, range.start);
                        if line_idx < styles.len() {
                            let line_start = line_starts[line_idx];
                            let local_start = range.start - line_start;
                            let local_end = (local_start + text.len()).min(lines[line_idx].len());
                            // Don't add if heading already covers this
                            let already = styles[line_idx]
                                .iter()
                                .any(|(r, _)| r.start == 0 && r.end == lines[line_idx].len());
                            if !already {
                                styles[line_idx].push((local_start..local_end, style));
                            }
                            // Record link info if inside a link
                            if let Some(url) = link_url_stack.last() {
                                if line_idx < links.len() {
                                    links[line_idx].push(LinkInfo {
                                        range: local_start..local_end,
                                        url: url.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        HighlightResult { styles, links }
    }

    fn highlight_code_block(
        &self,
        code: &str,
        lang: Option<&str>,
        start_line: usize,
        result: &mut [Vec<(Range<usize>, Style)>],
        lines: &[String],
    ) {
        if let Some(lang) = lang {
            if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang) {
                let theme = &THEME_SET.themes["base16-ocean.dark"];
                let mut h = HighlightLines::new(syntax, theme);
                for (i, code_line) in LinesWithEndings::from(code).enumerate() {
                    let line_idx = start_line + i;
                    if line_idx >= result.len() {
                        break;
                    }
                    if let Ok(ranges) = h.highlight_line(code_line, &SYNTAX_SET) {
                        let escaped = as_24_bit_terminal_escaped(&ranges, false);
                        if let Ok(text) = escaped.into_text() {
                            apply_syntect_line(&text, line_idx, result, lines);
                        }
                    }
                }
                return;
            }
        }
        // Fallback: style code block content with code_fence style
        let style = self.styles.code_fence();
        for (i, code_line) in code.lines().enumerate() {
            let line_idx = start_line + i;
            if line_idx >= result.len() || code_line.is_empty() {
                continue;
            }
            let source_line = &lines[line_idx];
            if let Some(offset) = source_line.find(code_line) {
                result[line_idx].push((offset..offset + code_line.len(), style));
            }
        }
    }
}

fn byte_to_line(line_starts: &[usize], byte: usize) -> usize {
    match line_starts.binary_search(&byte) {
        Ok(idx) => idx,
        Err(idx) => idx.saturating_sub(1),
    }
}

// Maps syntect spans to byte ranges via substring search; may mis-align if the same
// token text appears multiple times on a line.
fn apply_syntect_line(
    text: &Text<'_>,
    line_idx: usize,
    result: &mut [Vec<(Range<usize>, Style)>],
    lines: &[String],
) {
    let source_line = &lines[line_idx];
    let mut col = 0usize;
    for syntect_line in &text.lines {
        for span in &syntect_line.spans {
            let content = span.content.as_ref();
            let content_trimmed = content.trim_end_matches('\n');
            if content_trimmed.is_empty() {
                continue;
            }
            if let Some(pos) = source_line[col..].find(content_trimmed) {
                let abs_start = col + pos;
                let abs_end = abs_start + content_trimmed.len();
                if span.style != Style::default() {
                    result[line_idx].push((abs_start..abs_end, span.style));
                }
                col = abs_end;
            }
        }
    }
}
