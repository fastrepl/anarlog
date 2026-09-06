//! A paragraph element that breaks lines the way WebKit lays out the
//! ProseMirror editor (`white-space: pre-wrap; word-wrap: break-word`).
//!
//! GPUI's own wrapper adds up per-character advances of the base font, so
//! kerning and bold runs make it break a few pixels early. This element shapes
//! the whole paragraph first and breaks greedily at UAX #14 opportunities
//! using the shaped positions, with trailing spaces hanging past the edge.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Point, SharedString, Size, Style, TextAlign, TextRun, Window,
    WrappedLine, px,
};
use unicode_segmentation::UnicodeSegmentation;

pub struct ProseText {
    text: SharedString,
    runs: Vec<TextRun>,
    font_size: Pixels,
    line_height: Pixels,
    /// `text-align: center`: each line is offset by half the free width.
    centered: bool,
    layout: ProseLayout,
}

/// The laid-out lines of a [`ProseText`], shared with the editor for caret
/// placement and hit testing (a cheap clone, like `TextLayout`).
#[derive(Clone, Default)]
pub struct ProseLayout(Rc<RefCell<Option<Inner>>>);

struct Inner {
    lines: Vec<Line>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    size: Size<Pixels>,
    bounds: Option<Bounds<Pixels>>,
}

struct Line {
    /// Byte range in the paragraph text, including the hanging spaces.
    range: Range<usize>,
    /// Bytes of hanging whitespace (and the forced break) at the end.
    hanging: usize,
    /// Shaped without a wrap width; `shape_text` keeps a font per run where
    /// `shape_line` would merge runs that only differ in font.
    shaped: WrappedLine,
}

impl ProseText {
    pub fn new(
        text: impl Into<SharedString>,
        runs: Vec<TextRun>,
        font_size: Pixels,
        line_height: Pixels,
    ) -> Self {
        Self {
            text: text.into(),
            runs,
            font_size,
            line_height,
            centered: false,
            layout: ProseLayout::default(),
        }
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    pub fn layout(&self) -> &ProseLayout {
        &self.layout
    }
}

impl ProseLayout {
    pub fn line_height(&self) -> Pixels {
        self.0
            .borrow()
            .as_ref()
            .map(|inner| inner.line_height)
            .unwrap_or_default()
    }

    /// Window position of the caret before the character at `index`. An index
    /// on a wrap boundary belongs to the start of the next line, like a
    /// downstream caret affinity.
    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let inner = self.0.borrow();
        let inner = inner.as_ref()?;
        let bounds = inner.bounds?;
        let line_ix = inner
            .lines
            .iter()
            .position(|line| index < line.range.end)
            .unwrap_or(inner.lines.len().saturating_sub(1));
        let line = inner.lines.get(line_ix)?;
        let local = index.saturating_sub(line.range.start).min(line.range.len());
        let x = line.shaped.unwrapped_layout.x_for_index(local);
        Some(Point::new(
            bounds.left() + x,
            bounds.top() + inner.line_height * line_ix as f32,
        ))
    }

    /// The character at a window position: `Ok` when the point is over the
    /// text, `Err` with the nearest boundary otherwise.
    pub fn index_for_position(&self, position: Point<Pixels>) -> Result<usize, usize> {
        let inner = self.0.borrow();
        let Some(inner) = inner.as_ref() else {
            return Err(0);
        };
        let Some(bounds) = inner.bounds else {
            return Err(0);
        };
        if inner.lines.is_empty() {
            return Err(0);
        }
        let y = position.y - bounds.top();
        if y < px(0.0) {
            return Err(0);
        }
        let line_ix = (f32::from(y) / f32::from(inner.line_height)).floor() as usize;
        if line_ix >= inner.lines.len() {
            let last = inner.lines.last().unwrap();
            return Err(last.range.end - last.hanging);
        }
        let line = &inner.lines[line_ix];
        let x = position.x - bounds.left();
        if x < px(0.0) {
            return Err(line.range.start);
        }
        match line.shaped.unwrapped_layout.index_for_x(x) {
            Some(local) => Ok(line.range.start + local),
            // Past the end of the line: before the hanging spaces, so the caret
            // sits after the last visible character.
            None => Err(line.range.end - line.hanging),
        }
    }

    /// Horizontal extents of `range` on each line it touches, in window
    /// coordinates, for painting a selection.
    pub fn line_spans(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        let inner = self.0.borrow();
        let Some(inner) = inner.as_ref() else {
            return Vec::new();
        };
        let Some(bounds) = inner.bounds else {
            return Vec::new();
        };
        inner
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| range.start < line.range.end && range.end > line.range.start)
            .map(|(ix, line)| {
                let start = range.start.max(line.range.start) - line.range.start;
                let end = range.end.min(line.range.end) - line.range.start;
                let x0 = line.shaped.unwrapped_layout.x_for_index(start);
                let x1 = line.shaped.unwrapped_layout.x_for_index(end);
                Bounds::new(
                    Point::new(
                        bounds.left() + x0,
                        bounds.top() + inner.line_height * ix as f32,
                    ),
                    Size::new(x1 - x0, inner.line_height),
                )
            })
            .collect()
    }
}

impl Element for ProseText {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text = self.text.clone();
        let runs = self.runs.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let layout = self.layout.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known_dimensions, available_space, window, _cx| {
                let wrap_width = known_dimensions.width.or(match available_space.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                if let Some(inner) = layout.0.borrow().as_ref()
                    && inner.wrap_width == wrap_width
                {
                    return inner.size;
                }
                let lines = break_lines(&text, &runs, font_size, wrap_width, window);
                let width = wrap_width.unwrap_or_else(|| {
                    lines
                        .iter()
                        .map(|line| line.shaped.width())
                        .fold(px(0.0), Pixels::max)
                        .ceil()
                });
                let size = Size::new(width, line_height * lines.len().max(1) as f32);
                layout.0.borrow_mut().replace(Inner {
                    lines,
                    line_height,
                    wrap_width,
                    size,
                    bounds: None,
                });
                size
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        if let Some(inner) = self.layout.0.borrow_mut().as_mut() {
            inner.bounds = Some(bounds);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let inner = self.layout.0.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        let mut origin = bounds.origin;
        for line in &inner.lines {
            let mut line_origin = origin;
            if self.centered {
                // Hanging whitespace does not count toward the centred width.
                let visible = &self.text[line.range.start..line.range.end - line.hanging];
                let width = line.shaped.unwrapped_layout.x_for_index(visible.len());
                line_origin.x += ((bounds.size.width - width) / 2.0).max(px(0.0));
            }
            line.shaped
                .paint_background(
                    line_origin,
                    inner.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            line.shaped
                .paint(
                    line_origin,
                    inner.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            origin.y += inner.line_height;
        }
    }
}

impl IntoElement for ProseText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Shape the paragraph once, choose the break offsets, then shape each line
/// for painting and hit testing.
fn break_lines(
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    wrap_width: Option<Pixels>,
    window: &Window,
) -> Vec<Line> {
    let text_system = window.text_system();
    let shape = |text: SharedString, runs: &[TextRun]| -> Option<WrappedLine> {
        text_system
            .shape_text(text, font_size, runs, None, None)
            .ok()
            .and_then(|mut lines| lines.pop())
    };
    // Newlines are forced breaks the breaker handles; measure them as spaces
    // so the whole paragraph shapes as one line.
    let measured: SharedString = text.replace('\n', " ").into();
    let Some(whole) = shape(measured, runs) else {
        return Vec::new();
    };
    let breaks = break_offsets(text, wrap_width, |index| {
        whole.unwrapped_layout.x_for_index(index)
    });

    let mut lines = Vec::with_capacity(breaks.len());
    let mut start = 0;
    for end in breaks {
        let hanging = hanging_len(&text[start..end]);
        let newline = text[start..end].ends_with('\n') as usize;
        let visible: SharedString = text[start..end - newline].to_string().into();
        let line_runs = slice_runs(runs, start..end - newline);
        let Some(shaped) = shape(visible, &line_runs) else {
            break;
        };
        lines.push(Line {
            range: start..end,
            hanging,
            shaped,
        });
        start = end;
    }
    lines
}

/// Trailing whitespace (and forced break) that hangs past the wrap width.
fn hanging_len(line: &str) -> usize {
    line.len() - line.trim_end_matches([' ', '\t', '\n']).len()
}

/// Greedy line breaking: the line breaks at the last UAX #14 opportunity
/// before the first grapheme whose visible extent (hanging spaces excluded)
/// overflows the wrap width. A word wider than the line breaks inside itself
/// (`word-wrap: break-word`). Returns the end offset of every line; a
/// trailing forced break yields a final empty line, as ProseMirror's trailing
/// `<br>` does.
pub(crate) fn break_offsets(
    text: &str,
    wrap_width: Option<Pixels>,
    x_for_index: impl Fn(usize) -> Pixels,
) -> Vec<usize> {
    let graphemes: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
    let mut breaks = Vec::new();
    let mut line_start = 0;
    let mut last_opportunity: Option<usize> = None;
    let mut ix = 0;
    while ix < graphemes.len() {
        let (offset, grapheme) = graphemes[ix];
        let next_offset = graphemes.get(ix + 1).map(|(o, _)| *o).unwrap_or(text.len());
        if grapheme == "\n" {
            breaks.push(next_offset);
            line_start = next_offset;
            last_opportunity = None;
            ix += 1;
            continue;
        }
        if let Some(wrap_width) = wrap_width
            && !is_space(grapheme)
            && offset > line_start
            && x_for_index(next_offset) - x_for_index(line_start) > wrap_width
        {
            let break_at = last_opportunity
                .filter(|opportunity| *opportunity > line_start)
                .unwrap_or(offset);
            breaks.push(break_at);
            line_start = break_at;
            last_opportunity = None;
            ix = graphemes
                .iter()
                .position(|(o, _)| *o >= break_at)
                .unwrap_or(graphemes.len());
            continue;
        }
        let following = graphemes.get(ix + 1).map(|(_, g)| *g);
        if is_break_opportunity(grapheme, following) {
            last_opportunity = Some(next_offset);
        }
        ix += 1;
    }
    breaks.push(text.len());
    breaks
}

fn is_space(grapheme: &str) -> bool {
    matches!(grapheme, " " | "\t" | "\u{200B}")
}

/// A subset of UAX #14: after spaces, after hyphens before a non-digit, after
/// dashes, and between CJK ideographs, hiragana, katakana, and hangul.
fn is_break_opportunity(grapheme: &str, following: Option<&str>) -> bool {
    if is_space(grapheme) {
        return true;
    }
    let following_first = following.and_then(|g| g.chars().next());
    if following.is_none() {
        return false;
    }
    let first = grapheme.chars().next().unwrap_or('\0');
    match first {
        '-' | '\u{2010}' | '\u{00AD}' => following_first.is_some_and(|c| !c.is_ascii_digit()),
        '\u{2013}' | '\u{2014}' | '/' => following_first.is_some_and(|c| !c.is_ascii_digit()),
        c if is_cjk(c) => following_first.is_some_and(|c| is_cjk(c) || c.is_alphanumeric()),
        _ => following_first.is_some_and(is_cjk),
    }
}

fn is_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF
    )
}

/// The runs covering `range` of the text, clipped to it.
fn slice_runs(runs: &[TextRun], range: Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut offset = 0;
    for run in runs {
        let run_range = offset..offset + run.len;
        offset += run.len;
        let start = run_range.start.max(range.start);
        let end = run_range.end.min(range.end);
        if start < end {
            out.push(TextRun {
                len: end - start,
                ..run.clone()
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ASCII character is 10px wide.
    fn monospace(index: usize) -> Pixels {
        px(index as f32 * 10.0)
    }

    #[test]
    fn breaks_after_the_last_space_that_fits_and_lets_spaces_hang() {
        // "aaaa bbbb cccc": 4-char words, 90px line -> "aaaa bbbb " | "cccc".
        let breaks = break_offsets("aaaa bbbb cccc", Some(px(90.0)), monospace);
        assert_eq!(breaks, vec![10, 14]);
        // The trailing space of the first line hangs past the 90px edge.
        let breaks = break_offsets("aaaa bbbb cccc", Some(px(85.0)), monospace);
        assert_eq!(breaks, vec![5, 10, 14]);
    }

    #[test]
    fn a_word_wider_than_the_line_breaks_inside_itself() {
        let breaks = break_offsets("abcdefghij", Some(px(45.0)), monospace);
        assert_eq!(breaks, vec![4, 8, 10]);
    }

    #[test]
    fn hyphens_and_newlines_are_opportunities() {
        let breaks = break_offsets("top-right", Some(px(50.0)), monospace);
        assert_eq!(breaks, vec![4, 9]);
        let breaks = break_offsets("ab\ncd", Some(px(500.0)), monospace);
        assert_eq!(breaks, vec![3, 5]);
        assert_eq!(break_offsets("", Some(px(100.0)), monospace), vec![0]);
        // A trailing hard break keeps an empty last line.
        assert_eq!(
            break_offsets("ab\n", Some(px(100.0)), monospace),
            vec![3, 3]
        );
    }

    #[test]
    fn digits_after_a_hyphen_keep_the_word_together() {
        // "-1" is not a break; the whole token moves down as one word.
        let breaks = break_offsets("ab cd-12", Some(px(60.0)), monospace);
        assert_eq!(breaks, vec![3, 8]);
    }

    #[test]
    fn runs_are_clipped_to_the_line() {
        let font = gpui::font("Test");
        let run = |len| TextRun {
            len,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let sliced = slice_runs(&[run(4), run(6)], 2..7);
        assert_eq!(sliced.iter().map(|r| r.len).collect::<Vec<_>>(), vec![2, 3]);
    }
}
