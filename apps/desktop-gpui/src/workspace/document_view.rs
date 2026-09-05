//! `packages/editor/src/styles/prosemirror/note-typography.css` in GPUI terms:
//! 1rem/1.5 body, `padding-block: 0.125em` on every block, heading scale
//! 1.25em/1.125em/1em, custom list markers cycling filled circle → hollow
//! circle → square, links in blue-600 with underline.

use std::cell::Cell;

use gpui::{
    AnyElement, Div, ElementInputHandler, Entity, Focusable as _, HighlightStyle, MouseButton,
    MouseDownEvent, SharedString, StyledText, TextStyle, Window, canvas, div, fill, prelude::*, px,
    size,
};

use super::Workspace;
use crate::document::{Block, Span};
use crate::editor::BodyEditor;
use crate::theme::{Theme, alpha};
use crate::ui::TailwindText as _;

const BODY_PX: f32 = 16.0;

/// Paints one quad per wrapped line the byte range covers. Line ends are
/// found by bisecting on the y of `position_for_index`, which is monotonic.
fn paint_selection(
    layout: &gpui::TextLayout,
    range: std::ops::Range<usize>,
    color: gpui::Rgba,
    window: &mut Window,
) {
    let line_height = layout.line_height();
    let mut start = range.start;
    while start < range.end {
        let Some(origin) = layout.position_for_index(start) else {
            return;
        };
        // Largest index in (start, end] still on this line.
        let (mut lo, mut hi) = (start, range.end);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            match layout.position_for_index(mid) {
                Some(p) if p.y == origin.y => lo = mid,
                _ => hi = mid - 1,
            }
        }
        let end_x = layout
            .position_for_index(lo)
            .map(|p| p.x)
            .unwrap_or(origin.x);
        // A line break inside the range keeps a visible sliver.
        let width = (end_x - origin.x).max(px(4.0));
        window.paint_quad(fill(
            gpui::Bounds::new(origin, size(width, line_height)),
            color,
        ));
        if lo >= range.end {
            break;
        }
        start = lo + 1;
        while start < range.end
            && layout
                .position_for_index(start)
                .is_some_and(|p| p.y == origin.y)
        {
            start += 1;
        }
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
}

impl Workspace {
    pub(super) fn document_renderer(&self, window: &Window) -> DocumentRenderer {
        let mut base = window.text_style();
        base.font_size = px(BODY_PX).into();
        base.color = self.theme.foreground.into();
        if let Some(family) = &self.font_family {
            base.font_family = family.clone();
        }
        DocumentRenderer {
            base,
            mono_family: self.mono_font_family.clone(),
            theme: self.theme,
            editor: None,
            next_textblock: Cell::new(0),
        }
    }

    pub(super) fn document_editor_renderer(
        &self,
        editor: Entity<BodyEditor>,
        window: &Window,
    ) -> DocumentRenderer {
        let mut renderer = self.document_renderer(window);
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
    fn textblock(&self, wrapper: Div, text: StyledText) -> AnyElement {
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
            Block::Paragraph(spans) => self.textblock(
                div().py(pad).min_h(px(BODY_PX * 1.5 + 4.0)),
                self.text(spans, &self.base),
            ),
            Block::Heading { level, spans } => {
                let (em, weight, line_height) = match level {
                    1 => (1.25, gpui::FontWeight::BOLD, 1.4),
                    2 => (1.125, gpui::FontWeight::SEMIBOLD, 1.4444),
                    _ => (1.0, gpui::FontWeight::SEMIBOLD, 1.5),
                };
                let mut style = self.base.clone();
                style.font_weight = weight;
                style.font_size = px(BODY_PX * em).into();
                self.textblock(
                    div()
                        .py(pad)
                        .text_size(px(BODY_PX * em))
                        .line_height(px(BODY_PX * em * line_height)),
                    self.text(spans, &style),
                )
            }
            Block::List { ordered, items } => div()
                .flex()
                .flex_col()
                .children(items.iter().enumerate().map(|(index, item)| {
                    div()
                        .relative()
                        .flex()
                        .child(
                            // `li { padding-left: 1.5em }` with the marker at `left: 0.5em`.
                            div()
                                .flex_shrink_0()
                                .w(px(BODY_PX * 1.5))
                                .h(px(BODY_PX * 1.5 + 4.0))
                                .flex()
                                .items_center()
                                .justify_center()
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
                let mut style = self.base.clone();
                style.font_size = px(BODY_PX * 0.875).into();
                if let Some(family) = &self.mono_family {
                    style.font_family = family.clone();
                }
                let span = Span {
                    text: code.clone(),
                    ..Span::default()
                };
                self.textblock(
                    div()
                        .my(pad)
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(theme.accent)
                        .text_size(px(BODY_PX * 0.875))
                        .when_some(self.mono_family.clone(), |code, family| {
                            code.font_family(family)
                        }),
                    self.text(std::slice::from_ref(&span), &style),
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
        if let Some(checked) = checked {
            return div()
                .size(px(14.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(ink)
                .when(checked, |b| {
                    b.bg(theme.foreground).border_color(theme.foreground)
                })
                .into_any_element();
        }
        if ordered {
            return div()
                .text_color(ink)
                .text_size(px(BODY_PX))
                .child(SharedString::from(format!("{}.", index + 1)))
                .into_any_element();
        }
        match depth % 3 {
            0 => div().size(px(BODY_PX * 0.5)).rounded_full().bg(ink),
            1 => div()
                .size(px(BODY_PX * 0.5))
                .rounded_full()
                .border(px(1.5))
                .border_color(ink),
            _ => div().size(px(BODY_PX * 0.42)).rounded(px(1.6)).bg(ink),
        }
        .into_any_element()
    }

    fn text(&self, spans: &[Span], base: &TextStyle) -> StyledText {
        let mut text = String::new();
        let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
        for span in spans {
            let start = text.len();
            text.push_str(&span.text);
            let highlight = HighlightStyle {
                font_weight: span.bold.then_some(gpui::FontWeight::BOLD),
                font_style: span.italic.then_some(gpui::FontStyle::Italic),
                color: span.link.is_some().then(|| self.theme.link.into()),
                background_color: span.code.then(|| self.theme.accent.into()),
                underline: (span.underline || span.link.is_some()).then(|| gpui::UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(self.theme.link.into()),
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
        StyledText::new(text).with_default_highlights(base, highlights)
    }
}
