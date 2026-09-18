//! `@pierre/diffs` `MultiFileDiff` (`diffStyle: "unified"`) as data: the rows
//! of a unified diff as jsdiff's `structuredPatch` builds them (its Myers
//! variant, four lines of context), word-level emphasis for paired changed
//! lines (`diffWordsWithSpace`), and the `pierre-light` / `pierre-dark`
//! markdown token colours the highlighter would apply.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Delete,
    Insert,
}

/// One rendered row of the diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffRow {
    /// `data-separator='line-info'`: skipped context before a hunk.
    Separator(String),
    Line {
        kind: LineKind,
        /// The old number for context and deletions, the new one for insertions.
        number: usize,
        text: String,
        /// `data-diff-span`: the changed words of a paired deletion / insertion.
        emphasis: Vec<Range<usize>>,
    },
    /// `data-no-newline`: "No newline at end of file" under the line.
    NoNewline(LineKind),
}

/// The emphasised byte ranges of a deleted and an inserted line.
type EmphasisPair = (Vec<Range<usize>>, Vec<Range<usize>>);

/// `additions` / `deletions` of the file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub additions: usize,
    pub deletions: usize,
}

pub struct Diff {
    pub rows: Vec<DiffRow>,
    pub stats: Stats,
    /// Digits of the widest line number, for the gutter width.
    pub number_digits: usize,
}

pub fn diff(old: &str, new: &str) -> Diff {
    let old_lines = line_tokens(old);
    let new_lines = line_tokens(new);
    let hunks = hunks(&old_lines, &new_lines);
    let mut rows = Vec::new();
    let mut stats = Stats::default();
    let mut max_number = 1;
    // 1-based old line the next hunk may start at without skipping context.
    let mut next_old = 1;
    for hunk in &hunks {
        if hunk.old_start > next_old {
            rows.push(DiffRow::Separator(hunk.header()));
        }
        next_old = hunk.old_start + hunk.old_lines;
        let (mut old_number, mut new_number) = (hunk.old_start, hunk.new_start);
        let mut index = 0;
        while index < hunk.lines.len() {
            match hunk.lines[index].kind {
                LineKind::Context => {
                    max_number = max_number.max(old_number);
                    push_line(&mut rows, &hunk.lines[index], old_number, Vec::new());
                    old_number += 1;
                    new_number += 1;
                    index += 1;
                }
                LineKind::Insert => {
                    max_number = max_number.max(new_number);
                    stats.additions += 1;
                    push_line(&mut rows, &hunk.lines[index], new_number, Vec::new());
                    new_number += 1;
                    index += 1;
                }
                LineKind::Delete => {
                    // A run of deletions followed by a run of insertions: the
                    // lines pair up by position for the word emphasis.
                    let delete_end = hunk.lines[index..]
                        .iter()
                        .position(|line| line.kind != LineKind::Delete)
                        .map_or(hunk.lines.len(), |offset| index + offset);
                    let insert_end = hunk.lines[delete_end..]
                        .iter()
                        .position(|line| line.kind != LineKind::Insert)
                        .map_or(hunk.lines.len(), |offset| delete_end + offset);
                    let deletes = &hunk.lines[index..delete_end];
                    let inserts = &hunk.lines[delete_end..insert_end];
                    let mut emphasis: Vec<EmphasisPair> = deletes
                        .iter()
                        .zip(inserts.iter())
                        .map(|(d, i)| word_emphasis(&d.text, &i.text))
                        .collect();
                    emphasis.resize(deletes.len().max(inserts.len()), (Vec::new(), Vec::new()));
                    for (offset, line) in deletes.iter().enumerate() {
                        max_number = max_number.max(old_number);
                        stats.deletions += 1;
                        let ranges = std::mem::take(&mut emphasis[offset].0);
                        push_line(&mut rows, line, old_number, ranges);
                        old_number += 1;
                    }
                    for (offset, line) in inserts.iter().enumerate() {
                        max_number = max_number.max(new_number);
                        stats.additions += 1;
                        let ranges = std::mem::take(&mut emphasis[offset].1);
                        push_line(&mut rows, line, new_number, ranges);
                        new_number += 1;
                    }
                    index = insert_end;
                }
            }
        }
    }
    if next_old <= old_lines.len() && !rows.is_empty() {
        rows.push(DiffRow::Separator(String::new()));
    }
    Diff {
        rows,
        stats,
        number_digits: max_number.to_string().len(),
    }
}

fn push_line(rows: &mut Vec<DiffRow>, line: &HunkLine, number: usize, emphasis: Vec<Range<usize>>) {
    rows.push(DiffRow::Line {
        kind: line.kind,
        number,
        text: line.text.clone(),
        emphasis,
    });
    if line.missing_newline {
        rows.push(DiffRow::NoNewline(line.kind));
    }
}

/// jsdiff's `structuredPatch` default.
const CONTEXT: usize = 4;

struct HunkLine {
    kind: LineKind,
    text: String,
    /// `\ No newline at end of file` follows the line.
    missing_newline: bool,
}

struct Hunk {
    old_start: usize,
    old_lines: usize,
    new_start: usize,
    new_lines: usize,
    lines: Vec<HunkLine>,
}

impl Hunk {
    /// `formatPatch`'s `@@` line.
    fn header(&self) -> String {
        let old_start = if self.old_lines == 0 {
            self.old_start - 1
        } else {
            self.old_start
        };
        let new_start = if self.new_lines == 0 {
            self.new_start - 1
        } else {
            self.new_start
        };
        format!(
            "@@ -{old_start},{} +{new_start},{} @@",
            self.old_lines, self.new_lines
        )
    }
}

/// jsdiff `LineDiff.tokenize`: lines keep their `\n`; the last may lack one.
fn line_tokens(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, _) in text.match_indices('\n') {
        lines.push(&text[start..=index]);
        start = index + 1;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn hunk_line(kind: LineKind, token: &str) -> HunkLine {
    HunkLine {
        kind,
        text: token.strip_suffix('\n').unwrap_or(token).to_string(),
        missing_newline: !token.ends_with('\n'),
    }
}

/// jsdiff `diffLinesResultToPatch` with `context = 4`: equal runs of at most
/// `2 * context` lines between changes stay inside one hunk; longer runs
/// close the hunk with `context` trailing lines and open the next with
/// `context` leading ones.
fn hunks(old: &[&str], new: &[&str]) -> Vec<Hunk> {
    let components = myers(old, new);
    let mut hunks = Vec::new();
    let (mut old_range_start, mut new_range_start) = (0usize, 0usize);
    let mut current: Vec<HunkLine> = Vec::new();
    let (mut old_line, mut new_line) = (1usize, 1usize);
    let last_change = components
        .iter()
        .rposition(|component| component.kind != LineKind::Context);
    for (index, component) in components.iter().enumerate() {
        let tokens: &[&str] = match component.kind {
            LineKind::Insert => &new[component.new.clone()],
            _ => &old[component.old.clone()],
        };
        if component.kind != LineKind::Context {
            if old_range_start == 0 {
                old_range_start = old_line;
                new_range_start = new_line;
                if let Some(previous) = index.checked_sub(1).map(|i| &components[i]) {
                    let leading = &old[previous.old.clone()];
                    let take = leading.len().min(CONTEXT);
                    current.extend(
                        leading[leading.len() - take..]
                            .iter()
                            .map(|token| hunk_line(LineKind::Context, token)),
                    );
                    old_range_start -= take;
                    new_range_start -= take;
                }
            }
            current.extend(tokens.iter().map(|token| hunk_line(component.kind, token)));
            if component.kind == LineKind::Insert {
                new_line += tokens.len();
            } else {
                old_line += tokens.len();
            }
        } else {
            if old_range_start != 0 {
                let trailing = last_change.is_none_or(|last| index > last);
                if tokens.len() <= CONTEXT * 2 && !trailing {
                    current.extend(
                        tokens
                            .iter()
                            .map(|token| hunk_line(LineKind::Context, token)),
                    );
                } else {
                    let take = tokens.len().min(CONTEXT);
                    current.extend(
                        tokens[..take]
                            .iter()
                            .map(|token| hunk_line(LineKind::Context, token)),
                    );
                    hunks.push(Hunk {
                        old_start: old_range_start,
                        old_lines: old_line - old_range_start + take,
                        new_start: new_range_start,
                        new_lines: new_line - new_range_start + take,
                        lines: std::mem::take(&mut current),
                    });
                    old_range_start = 0;
                    new_range_start = 0;
                }
            }
            old_line += tokens.len();
            new_line += tokens.len();
        }
    }
    if old_range_start != 0 {
        hunks.push(Hunk {
            old_start: old_range_start,
            old_lines: old_line - old_range_start,
            new_start: new_range_start,
            new_lines: new_line - new_range_start,
            lines: current,
        });
    }
    hunks
}

/// One run of jsdiff's change objects, as ranges into the token slices.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Component {
    kind: LineKind,
    old: Range<usize>,
    new: Range<usize>,
}

#[derive(Clone)]
struct PathComponent {
    count: usize,
    kind: LineKind,
}

#[derive(Clone)]
struct Path {
    old_pos: isize,
    components: Vec<PathComponent>,
}

/// jsdiff's `Diff.diffWithOptionsObj`: the forward Myers search, one path per
/// diagonal per edit length, branching from the path farthest along the old
/// sequence (a deletion before an insertion on ties) and following snakes
/// with `extractCommon`.
fn myers<T: PartialEq>(old: &[T], new: &[T]) -> Vec<Component> {
    let old_len = old.len() as isize;
    let new_len = new.len() as isize;
    let max_edit = old.len() + new.len();
    let offset = max_edit as isize + 1;
    let mut best: Vec<Option<Path>> = vec![None; 2 * (max_edit + 1) + 1];
    let extract_common = |path: &mut Path, diagonal: isize| -> isize {
        let mut old_pos = path.old_pos;
        let mut new_pos = old_pos - diagonal;
        let mut common = 0;
        while new_pos + 1 < new_len
            && old_pos + 1 < old_len
            && old[(old_pos + 1) as usize] == new[(new_pos + 1) as usize]
        {
            new_pos += 1;
            old_pos += 1;
            common += 1;
        }
        if common > 0 {
            path.components.push(PathComponent {
                count: common,
                kind: LineKind::Context,
            });
        }
        path.old_pos = old_pos;
        new_pos
    };
    let add_to_path = |path: &Path, kind: LineKind, old_inc: isize| -> Path {
        let mut components = path.components.clone();
        match components.last_mut() {
            Some(last) if last.kind == kind => last.count += 1,
            _ => components.push(PathComponent { count: 1, kind }),
        }
        Path {
            old_pos: path.old_pos + old_inc,
            components,
        }
    };
    let mut first = Path {
        old_pos: -1,
        components: Vec::new(),
    };
    let new_pos = extract_common(&mut first, 0);
    if first.old_pos + 1 >= old_len && new_pos + 1 >= new_len {
        return build(first.components, old.len(), new.len());
    }
    best[offset as usize] = Some(first);
    let (mut min_diagonal, mut max_diagonal) = (isize::MIN, isize::MAX);
    let mut edit_length: isize = 1;
    while edit_length <= max_edit as isize {
        let mut diagonal = min_diagonal.max(-edit_length);
        while diagonal <= max_diagonal.min(edit_length) {
            let remove_path = best[(diagonal - 1 + offset) as usize].take();
            let add_path = best[(diagonal + 1 + offset) as usize].clone();
            let can_add = add_path.as_ref().is_some_and(|path| {
                let add_new_pos = path.old_pos - diagonal;
                0 <= add_new_pos && add_new_pos < new_len
            });
            let can_remove = remove_path
                .as_ref()
                .is_some_and(|path| path.old_pos + 1 < old_len);
            if !can_add && !can_remove {
                best[(diagonal + offset) as usize] = None;
                diagonal += 2;
                continue;
            }
            let prefer_add = !can_remove
                || (can_add
                    && remove_path.as_ref().unwrap().old_pos < add_path.as_ref().unwrap().old_pos);
            let mut base = if prefer_add {
                add_to_path(add_path.as_ref().unwrap(), LineKind::Insert, 0)
            } else {
                add_to_path(remove_path.as_ref().unwrap(), LineKind::Delete, 1)
            };
            let new_pos = extract_common(&mut base, diagonal);
            if base.old_pos + 1 >= old_len && new_pos + 1 >= new_len {
                return build(base.components, old.len(), new.len());
            }
            if base.old_pos + 1 >= old_len {
                max_diagonal = max_diagonal.min(diagonal - 1);
            }
            if new_pos + 1 >= new_len {
                min_diagonal = min_diagonal.max(diagonal + 1);
            }
            best[(diagonal + offset) as usize] = Some(base);
            diagonal += 2;
        }
        edit_length += 1;
    }
    // Unreachable for finite inputs: the search always terminates within
    // `old.len() + new.len()` edits.
    Vec::new()
}

/// jsdiff `buildValues`: the components in order with their token ranges.
fn build(components: Vec<PathComponent>, old_len: usize, new_len: usize) -> Vec<Component> {
    let (mut old_pos, mut new_pos) = (0usize, 0usize);
    let mut out = Vec::with_capacity(components.len());
    for component in components {
        let (old, new) = match component.kind {
            LineKind::Context => {
                let ranges = (
                    old_pos..old_pos + component.count,
                    new_pos..new_pos + component.count,
                );
                old_pos += component.count;
                new_pos += component.count;
                ranges
            }
            LineKind::Insert => {
                let range = new_pos..new_pos + component.count;
                new_pos += component.count;
                (old_pos..old_pos, range)
            }
            LineKind::Delete => {
                let range = old_pos..old_pos + component.count;
                old_pos += component.count;
                (range, new_pos..new_pos)
            }
        };
        out.push(Component {
            kind: component.kind,
            old,
            new,
        });
    }
    debug_assert!(old_pos == old_len && new_pos == new_len);
    out
}

/// jsdiff `WordsWithSpaceDiff.tokenize`: newlines, runs of word characters,
/// runs of blanks, or one other character.
fn word_tokens(text: &str) -> Vec<&str> {
    let is_word = |c: char| {
        c.is_ascii_alphanumeric()
            || c == '_'
            || matches!(c as u32,
                0xAD | 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x2C6 | 0x2C8..=0x2D7 | 0x2DE..=0x2FF | 0x1E00..=0x1EFF)
    };
    let is_blank = |c: char| c.is_whitespace() && c != '\n' && c != '\r';
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let mut end = start + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            end += 1;
            chars.next();
        } else if c != '\n' {
            let class = if is_word(c) {
                Some(true)
            } else if is_blank(c) {
                Some(false)
            } else {
                None
            };
            if let Some(word) = class {
                while let Some((index, next)) = chars.peek().copied() {
                    if (word && is_word(next)) || (!word && is_blank(next)) {
                        end = index + next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
        }
        tokens.push(&text[start..end]);
    }
    tokens
}

/// `computeLineDiffDecorations` with the renderer's default `lineDiffType:
/// "word-alt"`: the changed tokens of `old` and `new` as byte ranges in each,
/// where `pushOrJoinSpan` folds a single-character neutral token (the blank
/// between two changed words) into the changed span before it.
pub fn word_emphasis(old: &str, new: &str) -> EmphasisPair {
    let old_tokens = word_tokens(old);
    let new_tokens = word_tokens(new);
    let components = myers(&old_tokens, &new_tokens);
    // `(changed, text)` spans per side, in order.
    let mut deleted: Vec<(bool, String)> = Vec::new();
    let mut inserted: Vec<(bool, String)> = Vec::new();
    let last = components.len().saturating_sub(1);
    for (index, component) in components.iter().enumerate() {
        let is_last = index == last;
        match component.kind {
            LineKind::Context => {
                let text: String = old_tokens[component.old.clone()].concat();
                push_or_join(&mut deleted, false, &text, is_last);
                push_or_join(&mut inserted, false, &text, is_last);
            }
            LineKind::Delete => {
                let text: String = old_tokens[component.old.clone()].concat();
                push_or_join(&mut deleted, true, &text, is_last);
            }
            LineKind::Insert => {
                let text: String = new_tokens[component.new.clone()].concat();
                push_or_join(&mut inserted, true, &text, is_last);
            }
        }
    }
    (span_ranges(&deleted), span_ranges(&inserted))
}

/// `pushOrJoinSpan` with `enableJoin`.
fn push_or_join(spans: &mut Vec<(bool, String)>, changed: bool, text: &str, is_last: bool) {
    let Some(last) = spans.last_mut().filter(|_| !is_last) else {
        spans.push((changed, text.to_string()));
        return;
    };
    let neutral = !changed;
    let last_neutral = !last.0;
    if neutral == last_neutral || (neutral && text.chars().count() == 1 && !last_neutral) {
        last.1.push_str(text);
        return;
    }
    spans.push((changed, text.to_string()));
}

/// The byte ranges of the changed spans.
fn span_ranges(spans: &[(bool, String)]) -> Vec<Range<usize>> {
    let mut at = 0;
    let mut ranges = Vec::new();
    for (changed, text) in spans {
        if *changed && !text.is_empty() {
            ranges.push(at..at + text.len());
        }
        at += text.len();
    }
    ranges
}

/// A markdown token colour over a byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub range: Range<usize>,
    pub color: u32,
    pub italic: bool,
}

/// The theme's markdown scopes that matter for memos and summaries.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fg: u32,
    pub bg: u32,
    pub heading: u32,
    pub list_marker: u32,
    pub bold: u32,
    pub italic: u32,
    pub code: u32,
    pub link_title: u32,
    pub link_url: u32,
    pub quote: u32,
}

/// `pierre-light`
pub const LIGHT: Palette = Palette {
    fg: 0x070707,
    bg: 0xffffff,
    heading: 0xd52c36,
    list_marker: 0xd52c36,
    bold: 0xd5a910,
    italic: 0xfc2b73,
    code: 0x199f43,
    link_title: 0x7b43f8,
    link_url: 0xfc2b73,
    quote: 0x84848a,
};

/// `pierre-dark`
pub const DARK: Palette = Palette {
    fg: 0xfbfbfb,
    bg: 0x070707,
    heading: 0xff6762,
    list_marker: 0xff6762,
    bold: 0xffd452,
    italic: 0xff678d,
    code: 0x5ecc71,
    link_title: 0x9d6afb,
    link_url: 0xff678d,
    quote: 0x84848a,
};

/// The highlighter's tokens for one markdown line: headings and quotes as a
/// whole, list markers, and the inline bold / italic / code / link spans.
pub fn markdown_tokens(line: &str, palette: &Palette) -> Vec<Token> {
    let plain = |range: Range<usize>, color: u32| Token {
        range,
        color,
        italic: false,
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent < 4 {
        let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
        if (1..=6).contains(&hashes)
            && trimmed[hashes..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            return vec![plain(0..line.len(), palette.heading)];
        }
        if trimmed.starts_with('>') {
            return vec![plain(0..line.len(), palette.quote)];
        }
    }
    let mut tokens = Vec::new();
    let mut body_start = 0;
    if let Some(marker_len) = list_marker_len(trimmed) {
        if indent > 0 {
            tokens.push(plain(0..indent, palette.fg));
        }
        tokens.push(plain(indent..indent + marker_len, palette.list_marker));
        body_start = indent + marker_len;
    }
    inline_tokens(line, body_start, palette, &mut tokens);
    tokens
}

/// `- `, `* `, `+ `, `1. `, `1) ` (marker only, without the trailing space).
fn list_marker_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let first = *bytes.first()?;
    if matches!(first, b'-' | b'*' | b'+') {
        return matches!(bytes.get(1), Some(b' ' | b'\t')).then_some(1);
    }
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0
        && matches!(bytes.get(digits), Some(b'.' | b')'))
        && matches!(bytes.get(digits + 1), Some(b' ' | b'\t'))
    {
        return Some(digits + 1);
    }
    None
}

fn inline_tokens(line: &str, start: usize, palette: &Palette, tokens: &mut Vec<Token>) {
    let bytes = line.as_bytes();
    let mut plain_start = start;
    let mut at = start;
    let flush = |tokens: &mut Vec<Token>, plain_start: usize, at: usize| {
        if at > plain_start {
            tokens.push(Token {
                range: plain_start..at,
                color: palette.fg,
                italic: false,
            });
        }
    };
    while at < bytes.len() {
        let rest = &line[at..];
        let styled: Option<(usize, u32, bool)> = if let Some(code) = rest.strip_prefix('`') {
            code.find('`').map(|end| (end + 2, palette.code, false))
        } else if let Some(delim) = ["**", "__"].iter().find(|d| rest.starts_with(**d)) {
            rest[2..]
                .find(delim)
                .filter(|end| *end > 0)
                .map(|end| (end + 4, palette.bold, false))
        } else if rest.starts_with('*') || rest.starts_with('_') {
            let delim = &rest[..1];
            rest[1..]
                .find(delim)
                .filter(|end| *end > 0)
                .map(|end| (end + 2, palette.italic, true))
        } else if rest.starts_with('[') {
            link_len(rest).map(|len| (len, 0, false))
        } else {
            None
        };
        match styled {
            Some((len, 0, _)) => {
                flush(tokens, plain_start, at);
                link_tokens(line, at, at + len, palette, tokens);
                at += len;
                plain_start = at;
            }
            Some((len, color, italic)) => {
                flush(tokens, plain_start, at);
                tokens.push(Token {
                    range: at..at + len,
                    color,
                    italic,
                });
                at += len;
                plain_start = at;
            }
            None => {
                at += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    flush(tokens, plain_start, at);
}

/// `[title](url)` at the start of `text`.
fn link_len(text: &str) -> Option<usize> {
    let close = text.find("](")?;
    let url_end = text[close + 2..].find(')')?;
    Some(close + 2 + url_end + 1)
}

/// `[` `]` `(` `)` in the theme's punctuation colour, the title and url in
/// theirs.
fn link_tokens(line: &str, start: usize, end: usize, palette: &Palette, tokens: &mut Vec<Token>) {
    let text = &line[start..end];
    let close = text.find("](").unwrap_or(0);
    let punct = |range: Range<usize>| Token {
        range,
        color: palette.heading,
        italic: false,
    };
    tokens.push(punct(start..start + 1));
    tokens.push(Token {
        range: start + 1..start + close,
        color: palette.link_title,
        italic: false,
    });
    tokens.push(punct(start + close..start + close + 2));
    tokens.push(Token {
        range: start + close + 2..end - 1,
        color: palette.link_url,
        italic: false,
    });
    tokens.push(punct(end - 1..end));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_pair_deletions_with_insertions_for_emphasis() {
        let out = diff("# Agenda\n\n- Old item\n\n- Keep me", "# Agenda\n\n- Item");
        assert_eq!(
            out.stats,
            Stats {
                additions: 1,
                deletions: 3
            }
        );
        assert_eq!(out.number_digits, 1);
        assert_eq!(
            out.rows,
            vec![
                DiffRow::Line {
                    kind: LineKind::Context,
                    number: 1,
                    text: "# Agenda".into(),
                    emphasis: vec![]
                },
                DiffRow::Line {
                    kind: LineKind::Context,
                    number: 2,
                    text: String::new(),
                    emphasis: vec![]
                },
                DiffRow::Line {
                    kind: LineKind::Delete,
                    number: 3,
                    text: "- Old item".into(),
                    emphasis: vec![Range { start: 2, end: 10 }]
                },
                DiffRow::Line {
                    kind: LineKind::Delete,
                    number: 4,
                    text: String::new(),
                    emphasis: vec![]
                },
                DiffRow::Line {
                    kind: LineKind::Delete,
                    number: 5,
                    text: "- Keep me".into(),
                    emphasis: vec![]
                },
                DiffRow::NoNewline(LineKind::Delete),
                DiffRow::Line {
                    kind: LineKind::Insert,
                    number: 3,
                    text: "- Item".into(),
                    emphasis: vec![Range { start: 2, end: 6 }]
                },
                DiffRow::NoNewline(LineKind::Insert),
            ]
        );
    }

    /// jsdiff branches from the path farthest along the old text, so the
    /// blank line pairs up early and the first insertion carries the
    /// emphasis — what `@pierre/diffs` renders for the same summary.
    #[test]
    fn lines_align_like_jsdiff() {
        let old = "# Zed Corp review\n\n- Zed Corp shipped the build.";
        let new = "# Release Status\n\n- The build passed overnight, so the team is on track for the Thursday release.\n- Open bugs were reviewed and the blocking ones were assigned.\n- QA signed off on the release candidate.\n\n# Next Steps\n\n- Ship the release on Thursday.\n";
        let out = diff(old, new);
        let kinds: Vec<(LineKind, usize)> = out
            .rows
            .iter()
            .filter_map(|row| match row {
                DiffRow::Line { kind, number, .. } => Some((*kind, *number)),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (LineKind::Delete, 1),
                (LineKind::Insert, 1),
                (LineKind::Context, 2),
                (LineKind::Delete, 3),
                (LineKind::Insert, 3),
                (LineKind::Insert, 4),
                (LineKind::Insert, 5),
                (LineKind::Insert, 6),
                (LineKind::Insert, 7),
                (LineKind::Insert, 8),
                (LineKind::Insert, 9),
            ]
        );
        assert_eq!(
            out.stats,
            Stats {
                additions: 8,
                deletions: 2
            }
        );
        // "- Zed Corp shipped the build." against "- The build passed ...":
        // `Zed Corp shipped` and `build` change, `- `, ` the ` and `.` stay.
        // `word-alt` folds the blanks between changed words into the span,
        // so "Zed Corp shipped " is one span; " the " stays neutral.
        let DiffRow::Line { emphasis, .. } = &out.rows[3] else {
            panic!()
        };
        assert_eq!(
            emphasis,
            &vec![Range { start: 2, end: 19 }, Range { start: 23, end: 28 }]
        );
        let DiffRow::Line { emphasis, .. } = &out.rows[5] else {
            panic!()
        };
        assert_eq!(
            emphasis,
            &vec![Range { start: 2, end: 33 }, Range { start: 37, end: 78 }]
        );
        assert!(matches!(&out.rows[4], DiffRow::NoNewline(LineKind::Delete)));
    }

    #[test]
    fn separators_mark_skipped_context() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old.replace("line 10\n", "line ten\n");
        let out = diff(&old, &new);
        // Four lines of context on each side of the change.
        assert!(matches!(&out.rows[0], DiffRow::Separator(header) if header == "@@ -6,9 +6,9 @@"));
        assert!(matches!(
            &out.rows[1],
            DiffRow::Line {
                kind: LineKind::Context,
                number: 6,
                ..
            }
        ));
        assert!(matches!(out.rows.last(), Some(DiffRow::Separator(header)) if header.is_empty()));
        assert_eq!(out.number_digits, 2);
        assert!(diff("same\n", "same\n").rows.is_empty());
        // Two changes eight lines apart share a hunk; nine apart do not.
        let close = old.replace("line 10\n", "x\n").replace("line 19\n", "y\n");
        assert_eq!(
            diff(&old, &close)
                .rows
                .iter()
                .filter(|r| matches!(r, DiffRow::Separator(_)))
                .count(),
            1
        );
        let apart = old.replace("line 5\n", "x\n").replace("line 15\n", "y\n");
        assert_eq!(
            diff(&old, &apart)
                .rows
                .iter()
                .filter(|r| matches!(r, DiffRow::Separator(_)))
                .count(),
            2
        );
    }

    #[test]
    fn word_emphasis_merges_adjacent_words() {
        assert_eq!(
            word_emphasis("- Old item", "- Item"),
            (
                vec![Range { start: 2, end: 10 }],
                vec![Range { start: 2, end: 6 }]
            )
        );
        assert_eq!(
            word_emphasis("a b c", "a x c"),
            (
                vec![Range { start: 2, end: 3 }],
                vec![Range { start: 2, end: 3 }]
            )
        );
    }

    #[test]
    fn markdown_tokens_follow_the_theme_scopes() {
        let p = &LIGHT;
        assert_eq!(
            markdown_tokens("# Agenda", p),
            vec![Token {
                range: 0..8,
                color: p.heading,
                italic: false
            }]
        );
        assert_eq!(
            markdown_tokens("- Old **bold** and `code`", p),
            vec![
                Token {
                    range: 0..1,
                    color: p.list_marker,
                    italic: false
                },
                Token {
                    range: 1..6,
                    color: p.fg,
                    italic: false
                },
                Token {
                    range: 6..14,
                    color: p.bold,
                    italic: false
                },
                Token {
                    range: 14..19,
                    color: p.fg,
                    italic: false
                },
                Token {
                    range: 19..25,
                    color: p.code,
                    italic: false
                },
            ]
        );
        assert_eq!(
            markdown_tokens("1. *it*", p),
            vec![
                Token {
                    range: 0..2,
                    color: p.list_marker,
                    italic: false
                },
                Token {
                    range: 2..3,
                    color: p.fg,
                    italic: false
                },
                Token {
                    range: 3..7,
                    color: p.italic,
                    italic: true
                },
            ]
        );
        assert_eq!(
            markdown_tokens("> quote", p),
            vec![Token {
                range: 0..7,
                color: p.quote,
                italic: false
            }]
        );
        assert_eq!(
            markdown_tokens("see [docs](https://x.y)", p),
            vec![
                Token {
                    range: 0..4,
                    color: p.fg,
                    italic: false
                },
                Token {
                    range: 4..5,
                    color: p.heading,
                    italic: false
                },
                Token {
                    range: 5..9,
                    color: p.link_title,
                    italic: false
                },
                Token {
                    range: 9..11,
                    color: p.heading,
                    italic: false
                },
                Token {
                    range: 11..22,
                    color: p.link_url,
                    italic: false
                },
                Token {
                    range: 22..23,
                    color: p.heading,
                    italic: false
                },
            ]
        );
        assert!(markdown_tokens("", p).is_empty());
    }
}
