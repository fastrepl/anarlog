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
    /// A `max-width` for the paragraph itself, used when the container is
    /// sized by its content (an intrinsically sized flex item) and offers no
    /// definite width to wrap at.
    max_width: Option<Pixels>,
    /// `text-wrap: pretty` (the app's global rule for `p`): WebKit's
    /// constrained line breaking that avoids orphans.
    pretty: bool,
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
            max_width: None,
            pretty: false,
            layout: ProseLayout::default(),
        }
    }

    /// A paragraph whose layout handle the caller keeps for hit testing.
    pub fn with_layout(
        text: impl Into<SharedString>,
        runs: Vec<TextRun>,
        font_size: Pixels,
        line_height: Pixels,
        layout: ProseLayout,
    ) -> Self {
        let mut this = Self::new(text, runs, font_size, line_height);
        this.layout = layout;
        this
    }

    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
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

    /// `width: fit-content`: the widest line's visible extent (hanging
    /// spaces excluded).
    pub fn content_width(&self) -> Option<Pixels> {
        let inner = self.0.borrow();
        let inner = inner.as_ref()?;
        inner
            .lines
            .iter()
            .map(|line| {
                line.shaped
                    .unwrapped_layout
                    .x_for_index(line.range.len() - line.hanging)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
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
        let max_width = self.max_width;
        let pretty = self.pretty;
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known_dimensions, available_space, window, _cx| {
                let wrap_width = known_dimensions
                    .width
                    .or(match available_space.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    })
                    .map(|width| max_width.map_or(width, |max| width.min(max)))
                    .or(max_width);
                if let Some(inner) = layout.0.borrow().as_ref()
                    && inner.wrap_width == wrap_width
                {
                    return inner.size;
                }
                let lines = break_lines(&text, &runs, font_size, wrap_width, pretty, window);
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

/// A background painted behind a byte range of a [`ProseText`], one quad per
/// visual line the range touches: `inset_x` widens it like the transcript
/// line's `-mx-0.5 px-0.5`, `radius` rounds its corners.
pub struct Highlight {
    pub range: Range<usize>,
    pub color: gpui::Rgba,
    pub inset_x: Pixels,
    pub radius: Pixels,
}

/// A [`ProseText`] with range highlights painted under the glyphs (the
/// transcript's current-line and hovered-word backgrounds, search matches).
pub struct HighlightedProseText {
    text: ProseText,
    highlights: Vec<Highlight>,
}

impl HighlightedProseText {
    pub fn new(text: ProseText, highlights: Vec<Highlight>) -> Self {
        Self { text, highlights }
    }
}

impl Element for HighlightedProseText {
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
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text
            .prepaint(id, inspector_id, bounds, state, window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.text.layout().clone();
        for highlight in &self.highlights {
            for span in layout.line_spans(highlight.range.clone()) {
                let quad_bounds = Bounds::new(
                    Point::new(span.origin.x - highlight.inset_x, span.origin.y),
                    Size::new(span.size.width + highlight.inset_x * 2.0, span.size.height),
                );
                window.paint_quad(
                    gpui::fill(quad_bounds, highlight.color)
                        .corner_radii(gpui::Corners::all(highlight.radius)),
                );
            }
        }
        self.text
            .paint(id, inspector_id, bounds, state, prepaint, window, cx);
    }
}

impl IntoElement for HighlightedProseText {
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
    pretty: bool,
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
    let x_for_index = |index| whole.unwrapped_layout.x_for_index(index);
    let breaks = match wrap_width {
        Some(width) if pretty => pretty_break_offsets(text, width, x_for_index),
        _ => break_offsets(text, wrap_width, x_for_index),
    };

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

/// WebKit's `text-wrap: pretty` (`InlineContentConstrainer::prettifyRange`):
/// lines break only at the greedy breaker's opportunities, every line that
/// leaves more than two words behind it must measure within
/// `[max - 90, max]` (the ideal width `max - 45` ± 3 × 15), and among the
/// feasible break sequences the one with the least total raggedness
/// `Σ 100·|(width - ideal) / 15|³` wins — the last line included, which is
/// what pulls words down to avoid a short last line. Paragraphs whose ideal
/// width is narrower than their widest word, and paragraphs without a
/// feasible sequence, fall back to greedy breaking. Forced newlines split
/// the text into independently constrained chunks.
pub(crate) fn pretty_break_offsets(
    text: &str,
    max_width: Pixels,
    x_for_index: impl Fn(usize) -> Pixels,
) -> Vec<usize> {
    const STRETCH: f32 = 15.0;
    const MAX_STRETCH: f32 = 3.0;
    const LAST_LINE_PREFERRED_WORDS: usize = 2;
    let max = f32::from(max_width);
    let ideal = max - STRETCH * MAX_STRETCH;
    let greedy = break_offsets(text, Some(max_width), &x_for_index);
    if ideal <= 0.0 {
        return greedy;
    }
    let x = |index: usize| f32::from(x_for_index(index));
    let raggedness = |width: f32| 100.0 * ((width - ideal) / STRETCH).abs().powi(3);

    let mut result = Vec::new();
    let mut chunk_start = 0;
    for chunk in text.split_inclusive('\n') {
        let chunk_end = chunk_start + chunk.len();
        let content_end = chunk_end - usize::from(chunk.ends_with('\n'));
        // Break opportunities inside the chunk, then the chunk end.
        let mut opportunities = vec![chunk_start];
        let graphemes: Vec<(usize, &str)> = text[chunk_start..content_end]
            .grapheme_indices(true)
            .map(|(offset, grapheme)| (chunk_start + offset, grapheme))
            .collect();
        for (ix, (offset, grapheme)) in graphemes.iter().enumerate() {
            let following = graphemes.get(ix + 1).map(|(_, g)| *g);
            let next_offset = graphemes
                .get(ix + 1)
                .map(|(o, _)| *o)
                .unwrap_or(content_end);
            if following.is_some()
                && is_break_opportunity(grapheme, following)
                && next_offset < content_end
            {
                // One opportunity per run of spaces: the position after it.
                if !is_space(following.unwrap_or("")) && *offset + grapheme.len() == next_offset {
                    opportunities.push(next_offset);
                }
            }
        }
        opportunities.push(content_end);
        let count = opportunities.len();
        // Visible width of a line from opportunity `a` to `b`: hanging
        // trailing spaces excluded.
        let line_width = |a: usize, b: usize| -> f32 {
            let start = opportunities[a];
            let end = opportunities[b];
            let trimmed = end - hanging_len(&text[start..end]);
            (x(trimmed) - x(start)).max(0.0)
        };
        let widest_word = (0..count.saturating_sub(1))
            .map(|a| line_width(a, a + 1))
            .fold(0.0f32, f32::max);
        // `validIdealLineWidth`, or a single-line chunk: auto layout.
        if count <= 2 || ideal < widest_word {
            result.extend(
                greedy
                    .iter()
                    .copied()
                    .filter(|end| *end > chunk_start && *end <= chunk_end),
            );
            chunk_start = chunk_end;
            continue;
        }
        // Knuth–Plass over the opportunities: best[b] = (cost, previous).
        let mut best: Vec<(f32, usize)> = vec![(f32::INFINITY, 0); count];
        best[0] = (0.0, 0);
        for b in 1..count {
            let remaining_words = count - 1 - b;
            let constrained = remaining_words > LAST_LINE_PREFERRED_WORDS;
            for a in (0..b).rev() {
                if !best[a].0.is_finite() {
                    continue;
                }
                let width = line_width(a, b);
                if width > max {
                    break;
                }
                if constrained && width < ideal - STRETCH * MAX_STRETCH {
                    continue;
                }
                let cost = best[a].0 + raggedness(width);
                if cost < best[b].0 {
                    best[b] = (cost, a);
                }
            }
        }
        if !best[count - 1].0.is_finite() {
            result.extend(
                greedy
                    .iter()
                    .copied()
                    .filter(|end| *end > chunk_start && *end <= chunk_end),
            );
            chunk_start = chunk_end;
            continue;
        }
        let mut ends = Vec::new();
        let mut b = count - 1;
        while b > 0 {
            ends.push(opportunities[b]);
            b = best[b].1;
        }
        ends.reverse();
        // The chunk's last line carries the forced newline.
        if let Some(last) = ends.last_mut() {
            *last = chunk_end;
        }
        result.extend(ends);
        chunk_start = chunk_end;
    }
    if text.is_empty() || text.ends_with('\n') {
        // Match the greedy breaker: an empty trailing line after a newline.
        if result.last().copied() != Some(text.len()) || text.is_empty() {
            result.push(text.len());
        }
    }
    result
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
    fn long_sentences_wrap_at_the_last_fitting_space() {
        let text = "the selected speech-to-text provider is not available for batch transcription on this platform. Configure a batch-capable speech-to-text provider.";
        let breaks = break_offsets(text, Some(px(448.0)), |index| px(index as f32 * 6.0));
        assert!(breaks.len() > 1, "{breaks:?}");
        for window in breaks.windows(2) {
            assert!(
                (window[1] - window[0]) as f32 * 6.0 <= 448.0 + 6.0 * 2.0,
                "{breaks:?}"
            );
        }
    }

    /// The cases below were measured on WebKitGTK 2.52 with a 10px
    /// JetBrains Mono (6px per character) in a 400px column, where the
    /// ideal width is 355px and constrained lines must reach 310px.
    fn mono6(index: usize) -> Pixels {
        px(index as f32 * 6.0)
    }

    fn pretty_lines(text: &str, width: f32) -> Vec<usize> {
        let breaks = pretty_break_offsets(text, px(width), mono6);
        let mut start = 0;
        breaks
            .into_iter()
            .map(|end| {
                let len = text[start..end].trim_end().len();
                start = end;
                len
            })
            .collect()
    }

    #[test]
    fn pretty_pulls_words_down_to_avoid_a_short_last_line() {
        // 12 five-letter words: greedy gives 65 + 5; WebKit lays out 53 + 17.
        let words = |n: usize| vec!["aaaaa"; n].join(" ");
        assert_eq!(pretty_lines(&words(12), 400.0), vec![53, 17]);
        assert_eq!(pretty_lines(&words(16), 400.0), vec![53, 41]);
        assert_eq!(pretty_lines(&words(22), 400.0), vec![65, 65]);
        assert_eq!(pretty_lines(&words(23), 400.0), vec![53, 53, 29]);
        assert_eq!(pretty_lines(&words(24), 400.0), vec![53, 53, 35]);
    }

    #[test]
    fn pretty_leaves_two_words_behind_but_keeps_earlier_lines_full() {
        let first = "aaaaaaaaa aaaaaaaaa aaaaaaaaa aaaaaaaaa aaaaaaaaa aaaaaaaaaaaa";
        // A one-word last line moves one word down…
        assert_eq!(pretty_lines(&format!("{first} bbbbb"), 400.0), vec![49, 18]);
        assert_eq!(pretty_lines(&format!("{first} bb cc"), 400.0), vec![62, 5]);
        assert_eq!(pretty_lines(&format!("{first} p qq"), 400.0), vec![62, 4]);
        // …or two when the total raggedness is lower.
        assert_eq!(pretty_lines(&format!("{first} p q r"), 400.0), vec![62, 5]);
        // A two-word last line is left alone.
        assert_eq!(
            pretty_lines(&format!("{first} bbbbb ccccc"), 400.0),
            vec![62, 11]
        );
        // A line followed by exactly two words may fall under the bound.
        let short = "aaaaaaaaa aaaaaaaaa aaaaaaaaa aaaaaaaaa aaaaaaaaa";
        assert_eq!(
            pretty_lines(&format!("{short} pppppppppppp qqqq"), 400.0),
            vec![49, 17]
        );
        assert_eq!(
            pretty_lines(&format!("{short} pppppppppppp qqqqqqqqqqqq"), 400.0),
            vec![49, 25]
        );
    }

    #[test]
    fn pretty_falls_back_to_greedy_for_single_lines_wide_words_and_newlines() {
        assert_eq!(pretty_lines("aaaaa bbbbb", 400.0), vec![11]);
        // A 61-character word is wider than the ideal 355px: auto layout.
        let wide = format!("aa {} bb cc", "w".repeat(61));
        assert_eq!(
            pretty_break_offsets(&wide, px(400.0), mono6),
            break_offsets(&wide, Some(px(400.0)), mono6)
        );
        // Forced newlines split the chunks; each keeps the greedy shape here.
        assert_eq!(
            pretty_break_offsets("ab\ncd", px(400.0), mono6),
            break_offsets("ab\ncd", Some(px(400.0)), mono6)
        );
        assert_eq!(
            pretty_break_offsets("ab\n", px(400.0), mono6),
            break_offsets("ab\n", Some(px(400.0)), mono6)
        );
        assert_eq!(pretty_break_offsets("", px(400.0), mono6), vec![0]);
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
