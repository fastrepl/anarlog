//! `packages/editor/src/styles/prosemirror/note-typography.css` in GPUI terms:
//! 1rem/1.5 body, `padding-block: 0.125em` on every block, heading scale
//! 1.25em/1.125em/1em, custom list markers cycling filled circle → hollow
//! circle → square, links in blue-600 with underline.

use gpui::{
    AnyElement, HighlightStyle, SharedString, StyledText, TextStyle, Window, div, prelude::*, px,
};

use super::Workspace;
use crate::document::{Block, Span};
use crate::theme::{Theme, alpha};
use crate::ui::TailwindText as _;

const BODY_PX: f32 = 16.0;

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
        }
    }
}

impl DocumentRenderer {
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
            Block::Paragraph(spans) => div()
                .py(pad)
                .min_h(px(BODY_PX * 1.5 + 4.0))
                .child(self.text(spans, &self.base))
                .into_any_element(),
            Block::Heading { level, spans } => {
                let (em, weight, line_height) = match level {
                    1 => (1.25, gpui::FontWeight::BOLD, 1.4),
                    2 => (1.125, gpui::FontWeight::SEMIBOLD, 1.4444),
                    _ => (1.0, gpui::FontWeight::SEMIBOLD, 1.5),
                };
                let mut style = self.base.clone();
                style.font_weight = weight;
                style.font_size = px(BODY_PX * em).into();
                div()
                    .py(pad)
                    .text_size(px(BODY_PX * em))
                    .line_height(px(BODY_PX * em * line_height))
                    .child(self.text(spans, &style))
                    .into_any_element()
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
            Block::Code(code) => div()
                .my(pad)
                .px_3()
                .py_2()
                .rounded_md()
                .bg(theme.accent)
                .text_size(px(BODY_PX * 0.875))
                .when_some(self.mono_family.clone(), |code, family| {
                    code.font_family(family)
                })
                .child(SharedString::from(code.clone()))
                .into_any_element(),
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
