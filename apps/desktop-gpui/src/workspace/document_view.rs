//! `packages/editor/src/styles/prosemirror/note-typography.css` in GPUI terms:
//! 1rem/1.5 body, `padding-block: 0.125em` on every block, heading scale
//! 1.25em/1.125em/1em, custom list markers cycling filled circle → hollow
//! circle → square, links in blue-600 with underline.

use std::cell::Cell;

use gpui::{
    AnyElement, Div, ElementInputHandler, Entity, Focusable as _, HighlightStyle, MouseButton,
    MouseDownEvent, Pixels, SharedString, StyledText, TextRun, TextStyle, Window, canvas, div,
    fill, prelude::*, px, size,
};

use super::Workspace;
use crate::document::{Block, Span};
use crate::editor::BodyEditor;
use crate::prose_text::{ProseLayout, ProseText};
use crate::theme::{Theme, alpha};
use crate::ui::TailwindText as _;

const BODY_PX: f32 = 16.0;

/// WebCore stores a unitless `line-height` as a single-precision percentage
/// and truncates the used value to whole pixels, so `18px * 1.4444` lays out
/// as 25px, not 26.
pub(super) fn webkit_line_height(font_px: f32, ratio: f32) -> f32 {
    let percent = ratio * 100.0;
    (percent * font_px / 100.0).floor()
}

/// Paints one quad per wrapped line the byte range covers.
fn paint_selection(
    layout: &ProseLayout,
    range: std::ops::Range<usize>,
    color: gpui::Rgba,
    window: &mut Window,
) {
    for mut span in layout.line_spans(range) {
        // A line break inside the range keeps a visible sliver.
        span.size.width = span.size.width.max(px(4.0));
        window.paint_quad(fill(span, color));
    }
}

pub(super) fn has_visible_content(block: &Block) -> bool {
    match block {
        Block::Paragraph(spans) => spans.iter().any(|span| !span.text.trim().is_empty()),
        Block::Heading { spans, .. } => spans.iter().any(|span| !span.text.trim().is_empty()),
        Block::List { items, .. } => items
            .iter()
            .any(|item| item.checked.is_some() || item.blocks.iter().any(has_visible_content)),
        Block::Blockquote(blocks) => blocks.iter().any(has_visible_content),
        Block::Code(code) => !code.trim().is_empty(),
        Block::HorizontalRule | Block::Image { .. } => true,
    }
}

pub(super) struct DocumentRenderer {
    base: TextStyle,
    mono_family: Option<SharedString>,
    theme: Theme,
    /// Present when the document is the editable memo: textblocks report
    /// their layout to the editor, place the caret on click, and paint it.
    editor: Option<Entity<BodyEditor>>,
    next_textblock: Cell<usize>,
    /// `placeholderPlugin`: the empty textblock holding the selection anchor
    /// shows the placeholder text.
    placeholder: Option<(usize, SharedString)>,
    /// Overrides the `blue-600` link colour (and makes links `font-medium`).
    link_color: Option<gpui::Rgba>,
}

impl Workspace {
    pub(super) fn document_renderer(&self, window: &Window) -> DocumentRenderer {
        let mut base = window.text_style();
        base.font_size = px(BODY_PX).into();
        base.line_height = px(BODY_PX * 1.5).into();
        base.color = self.theme.foreground.into();
        // `.ProseMirror { font-variant-ligatures: none }`
        base.font_features = gpui::FontFeatures(std::sync::Arc::new(vec![("liga".into(), 0)]));
        if let Some(family) = &self.font_family {
            base.font_family = family.clone();
        }
        DocumentRenderer {
            base,
            mono_family: self.mono_font_family.clone(),
            theme: self.theme,
            editor: None,
            next_textblock: Cell::new(0),
            placeholder: None,
            link_color: None,
        }
    }

    pub(super) fn document_editor_renderer(
        &self,
        editor: Entity<BodyEditor>,
        window: &Window,
        cx: &gpui::App,
    ) -> DocumentRenderer {
        let mut renderer = self.document_renderer(window);
        renderer.placeholder = {
            let editor = editor.read(cx);
            // ProseMirror always has a selection; before the first focus it
            // sits at the document start.
            editor
                .caret()
                .map(|caret| caret.block)
                .or_else(|| (editor.doc().textblock_count() <= 1).then_some(0))
                .filter(|block| editor.doc().text(*block).is_empty())
                .map(|block| (block, SharedString::from("Start writing...")))
        };
        renderer.editor = Some(editor);
        renderer
    }
}

impl DocumentRenderer {
    /// Root wrapper for an editable document: registers the input handler so
    /// typed and composed text reaches the editor, and lets a click below the
    /// last block land the caret at the end.
    pub(super) fn editable_root(
        &self,
        editor: &Entity<BodyEditor>,
        children: Vec<AnyElement>,
        cx: &gpui::App,
    ) -> AnyElement {
        let focus_handle = editor.read(cx).focus_handle(cx);
        let handler_editor = editor.clone();
        let click_editor = editor.clone();
        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .children(children)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &focus_handle,
                            ElementInputHandler::new(bounds, handler_editor.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(
                // The `flex-1` tail below the content: `trailing-empty-line-click`.
                div()
                    .id("editor-tail")
                    .h(px(BODY_PX * 1.5 * 4.0))
                    .w_full()
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        click_editor.update(cx, |editor, cx| editor.place_caret_at_end(window, cx));
                    }),
            )
            .into_any_element()
    }

    /// Wraps a textblock's text with the editor hooks when editing.
    fn textblock(&self, wrapper: Div, text: ProseText) -> AnyElement {
        let Some(editor) = &self.editor else {
            return wrapper.child(text).into_any_element();
        };
        let index = self.next_textblock.get();
        self.next_textblock.set(index + 1);
        let layout = text.layout().clone();
        let paint_editor = editor.clone();
        let click_editor = editor.clone();
        let drag_editor = editor.clone();
        let caret_color = self.theme.foreground;
        let selection_color = self.theme.selection;
        let placeholder = self
            .placeholder
            .as_ref()
            .filter(|(block, _)| *block == index)
            .map(|(_, text)| text.clone());
        let muted = self.theme.muted_foreground;
        let text = match placeholder {
            // `.n::before { content: attr(data-placeholder) }` at the block's origin.
            Some(placeholder) => div()
                .relative()
                .child(text)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .text_color(muted)
                        .child(placeholder),
                )
                .into_any_element(),
            None => text.into_any_element(),
        };
        wrapper
            .id(("textblock", index))
            .relative()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    click_editor.update(cx, |editor, cx| {
                        editor.place_caret_at(
                            index,
                            event.position,
                            event.modifiers.shift,
                            window,
                            cx,
                        )
                    });
                },
            )
            .on_mouse_move(move |event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    drag_editor.update(cx, |editor, cx| editor.drag_to(index, event.position, cx));
                }
            })
            .child(text)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let (caret, selection) = paint_editor.update(cx, |editor, _| {
                            editor.record_layout(index, layout.clone(), bounds);
                            let focused = editor.is_focused(window);
                            (
                                editor
                                    .caret()
                                    .filter(|caret| caret.block == index && focused),
                                editor.selection_in_block(index).filter(|_| focused),
                            )
                        });
                        if let Some(range) = selection {
                            paint_selection(&layout, range, selection_color, window);
                        }
                        if let Some(caret) = caret
                            && let Some(position) = layout.position_for_index(caret.offset)
                        {
                            window.paint_quad(fill(
                                gpui::Bounds::new(position, size(px(1.0), layout.line_height())),
                                caret_color,
                            ));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }

    pub(super) fn blocks(&self, blocks: &[Block], depth: usize) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.block(block, depth))
            .collect()
    }

    fn block(&self, block: &Block, depth: usize) -> AnyElement {
        let theme = self.theme;
        // `.note-typography > * { padding-block: 0.125em }`
        let pad = px(BODY_PX * 0.125);
        match block {
            // The editor's paragraphs compute `text-wrap: wrap` (measured on
            // the running app), not the global `p { text-wrap: pretty }`.
            Block::Paragraph(spans) => self.textblock(
                div().py(pad).min_h(px(BODY_PX * 1.5 + 4.0)),
                self.prose(spans, &self.base, px(BODY_PX * 1.5)),
            ),
            Block::Heading { level, spans } => {
                let (em, weight, ratio) = match level {
                    1 => (1.25, gpui::FontWeight::BOLD, 1.4),
                    2 => (1.125, gpui::FontWeight::SEMIBOLD, 1.4444),
                    _ => (1.0, gpui::FontWeight::SEMIBOLD, 1.5),
                };
                let font_px = BODY_PX * em;
                let mut style = self.base.clone();
                style.font_weight = weight;
                style.font_size = px(font_px).into();
                self.textblock(
                    div()
                        // `padding-block: 0.125em` scales with the heading's own size.
                        .py(px(font_px * 0.125))
                        .text_size(px(font_px))
                        .line_height(px(webkit_line_height(font_px, ratio))),
                    self.prose(spans, &style, px(webkit_line_height(font_px, ratio))),
                )
            }
            Block::List { ordered, items } => div()
                .flex()
                .flex_col()
                // `li > ul::before`: a 1px guide rail at `left: calc(-1em - 0.5px)`
                // in `currentColor` at 30%, centred under the parent marker.
                // WebKit snaps the half pixel to the nearer device pixel.
                .when(depth > 0, |list| {
                    list.relative().child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(-BODY_PX))
                            .w(px(1.0))
                            .h_full()
                            .bg(alpha(theme.foreground, 0.3)),
                    )
                })
                .children(items.iter().enumerate().map(|(index, item)| {
                    div()
                        .relative()
                        .flex()
                        .child(
                            // `li { padding-left: 1.5em }`; bullets are centred at
                            // `left: 0.5em`, `top: 0.125em + 0.75em`, ordered markers fill a
                            // `1em` box at `left: 0` with centred text.
                            div()
                                .relative()
                                .flex_shrink_0()
                                .w(px(BODY_PX * 1.5))
                                .h(px(BODY_PX * 1.5 + 4.0))
                                .child(self.marker(item.checked, *ordered, index, depth)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .flex_1()
                                .children(self.blocks(&item.blocks, depth + 1)),
                        )
                }))
                .into_any_element(),
            Block::Blockquote(blocks) => div()
                .py(pad)
                .pl_3()
                .border_l_2()
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
                .children(self.blocks(blocks, depth + 1))
                .into_any_element(),
            Block::Code(code) => {
                // `pre`: 0.875em, line-height 1.4286, `margin-block: 0.5em`,
                // `padding: 1em 0.75em`, `rounded-md`, wrapping (`pre-wrap`).
                let font_px = BODY_PX * 0.875;
                let line_height = px(webkit_line_height(font_px, 1.4286));
                let mut style = self.base.clone();
                style.font_size = px(font_px).into();
                if let Some(family) = &self.mono_family {
                    style.font_family = family.clone();
                }
                let span = Span {
                    text: code.clone(),
                    ..Span::default()
                };
                self.textblock(
                    div()
                        .my(px(font_px * 0.5))
                        .px(px(font_px * 0.75))
                        .py(px(font_px))
                        .rounded_md()
                        .bg(theme.accent)
                        .text_size(px(font_px))
                        .line_height(line_height)
                        .when_some(self.mono_family.clone(), |code, family| {
                            code.font_family(family)
                        }),
                    self.prose(std::slice::from_ref(&span), &style, line_height),
                )
            }
            Block::HorizontalRule => div()
                .my(px(BODY_PX * 0.75))
                .h(px(1.0))
                .bg(theme.border)
                .into_any_element(),
            Block::Image { alt } => div()
                .my(pad)
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
                .tw_text_sm()
                .child(SharedString::from(if alt.is_empty() {
                    "Image".to_string()
                } else {
                    format!("Image: {alt}")
                }))
                .into_any_element(),
        }
    }

    /// Unordered markers cycle by depth (filled circle, hollow circle, square,
    /// then repeat); ordered lists count; task items draw a checkbox.
    fn marker(
        &self,
        checked: Option<bool>,
        ordered: bool,
        index: usize,
        depth: usize,
    ) -> AnyElement {
        let theme = self.theme;
        let ink = alpha(theme.foreground, 0.65);
        let centre = |size: f32| {
            div()
                .absolute()
                .left(px(BODY_PX * 0.5 - size / 2.0))
                .top(px(BODY_PX * 0.875 - size / 2.0))
                .size(px(size))
        };
        if let Some(checked) = checked {
            return centre(14.0)
                .rounded(px(3.0))
                .border_1()
                .border_color(ink)
                .when(checked, |b| {
                    b.bg(theme.foreground).border_color(theme.foreground)
                })
                .into_any_element();
        }
        if ordered {
            // `ol > li::before { top: 0.125em; left: 0; width: 1em; line-height: 1.5 }`
            return div()
                .absolute()
                .left_0()
                .top(px(BODY_PX * 0.125))
                .w(px(BODY_PX))
                .flex()
                .justify_center()
                .text_color(ink)
                .text_size(px(BODY_PX))
                .line_height(px(BODY_PX * 1.5))
                .child(SharedString::from(format!("{}.", index + 1)))
                .into_any_element();
        }
        match depth % 3 {
            0 => centre(BODY_PX * 0.5).rounded_full().bg(ink),
            1 => centre(BODY_PX * 0.5)
                .rounded_full()
                .border(px(1.5))
                .border_color(ink),
            _ => centre(BODY_PX * 0.42).rounded(px(1.6)).bg(ink),
        }
        .into_any_element()
    }

    /// Inline runs at an explicit size, for prose outside the note body; links
    /// use `link_color` (streamdown's `text-foreground font-medium underline`).
    pub(super) fn inline_text(
        &self,
        spans: &[Span],
        font_size: Pixels,
        line_height: Pixels,
        link_color: gpui::Rgba,
    ) -> StyledText {
        let mut base = self.base.clone();
        base.font_size = font_size.into();
        base.line_height = line_height.into();
        let renderer = DocumentRenderer {
            base: base.clone(),
            mono_family: self.mono_family.clone(),
            theme: self.theme,
            editor: None,
            next_textblock: std::cell::Cell::new(0),
            placeholder: None,
            link_color: Some(link_color),
        };
        renderer.text(spans, &base)
    }

    /// A block's inline content as a WebKit-wrapped paragraph.
    fn prose(&self, spans: &[Span], base: &TextStyle, line_height: Pixels) -> ProseText {
        let (text, highlights) = self.inline_runs(spans);
        let mut runs: Vec<TextRun> = Vec::new();
        let mut ix = 0;
        for (range, highlight) in highlights {
            if ix < range.start {
                runs.push(base.to_run(range.start - ix));
            }
            runs.push(base.clone().highlight(highlight).to_run(range.len()));
            ix = range.end;
        }
        if ix < text.len() {
            runs.push(base.to_run(text.len() - ix));
        }
        let font_size = base.font_size.to_pixels(px(16.0));
        ProseText::new(text, runs, font_size, line_height)
    }

    fn text(&self, spans: &[Span], base: &TextStyle) -> StyledText {
        let (text, highlights) = self.inline_runs(spans);
        StyledText::new(text).with_default_highlights(base, highlights)
    }

    /// The concatenated text of the spans and their highlight ranges.
    fn inline_runs(
        &self,
        spans: &[Span],
    ) -> (String, Vec<(std::ops::Range<usize>, HighlightStyle)>) {
        let mut text = String::new();
        let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
        for span in spans {
            let start = text.len();
            text.push_str(&span.text);
            let link_color = self.link_color.unwrap_or(self.theme.link);
            let highlight = HighlightStyle {
                font_weight: if span.bold {
                    Some(gpui::FontWeight::BOLD)
                } else if span.link.is_some() && self.link_color.is_some() {
                    Some(gpui::FontWeight::MEDIUM)
                } else {
                    None
                },
                font_style: span.italic.then_some(gpui::FontStyle::Italic),
                color: span.link.is_some().then(|| link_color.into()),
                background_color: span.code.then(|| self.theme.accent.into()),
                underline: (span.underline || span.link.is_some()).then(|| gpui::UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(
                        if self.link_color.is_some() {
                            alpha(link_color, 0.5)
                        } else {
                            link_color
                        }
                        .into(),
                    ),
                    wavy: false,
                }),
                strikethrough: span.strike.then(|| gpui::StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(self.theme.foreground.into()),
                }),
                ..HighlightStyle::default()
            };
            if highlight != HighlightStyle::default() {
                highlights.push((start..text.len(), highlight));
            }
        }
        (text, highlights)
    }
}
