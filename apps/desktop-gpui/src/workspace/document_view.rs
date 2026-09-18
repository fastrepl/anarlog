//! `packages/editor/src/styles/prosemirror/note-typography.css` in GPUI terms:
//! 1rem/1.5 body, `padding-block: 0.125em` on every block, heading scale
//! 1.25em/1.125em/1em, custom list markers cycling filled circle → hollow
//! circle → square, links in blue-600 with underline.

use std::cell::Cell;
use std::path::PathBuf;

use gpui::{
    AnyElement, Div, ElementInputHandler, Entity, Focusable as _, HighlightStyle, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, StyledText, TextRun, TextStyle, Window, canvas,
    div, fill, img, point, prelude::*, px, relative, size,
};

use super::Workspace;
use crate::document::{Block, FileAttachment, Image, Span};
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

/// `formatFileSize`
pub(super) fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
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
        Block::HorizontalRule | Block::Image(_) | Block::FileAttachment(_) => true,
        Block::Clip => false,
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
    /// Set while rendering a checked task item: its first paragraph paints
    /// the `li[data-checked="true"] > div > p` strikethrough.
    strike_next: Cell<bool>,
    /// The mention chips of the paragraph being built (`prose` fills it,
    /// `textblock` paints their avatars over the reserved placeholder).
    mentions_next: std::cell::RefCell<Vec<(std::ops::Range<usize>, String, String)>>,
    /// `placeholderPlugin`: the empty textblock holding the selection anchor
    /// shows the placeholder text.
    pub(super) placeholder: Option<(usize, SharedString)>,
    /// Overrides the `blue-600` link colour (and makes links `font-medium`).
    link_color: Option<gpui::Rgba>,
    /// `documentTitlePlaceholder` applies while the selection anchor sits in
    /// the (empty) title heading; ProseMirror starts there, so a document
    /// without a caret yet shows it too.
    title_placeholder: bool,
    /// `useAttachmentResolver`: `<session dir>/attachments`, where an
    /// attachment id names its file.
    attachments_dir: Option<PathBuf>,
    /// An image resize in progress: the image's document-order index and its
    /// draft pixel width.
    image_draft: Option<(usize, Pixels)>,
    /// Document-order counters for the image and file-attachment atoms, so
    /// their controls address the right node.
    next_image: Cell<usize>,
    next_file: Cell<usize>,
    /// The line box's depth below an `inline-block` sitting on the baseline:
    /// the body font's rounded descent plus half-leading at the 24px line.
    inline_block_gap: Pixels,
    /// The body font's rounded ascent plus descent at 16px: the inline box
    /// height a `<mark>` background covers.
    inline_box_height: f32,
    /// Lets the atoms open tooltips (an image's `title`).
    tooltips: Option<super::tooltip::TooltipHost>,
    /// `currentColor` while inside a blockquote (`color: muted-foreground`).
    text_color: Cell<Option<gpui::Rgba>>,
    /// What the block being rendered is a direct child of: `.note-typography
    /// > *` pads top-level blocks, `li > p` pads an item's paragraphs, a
    /// blockquote's children carry no padding and are spaced by
    /// `blockquote > * + *`, and `li > ul/ol` draws the guide rail.
    container: Cell<Container>,
    /// How many non-task `ul`s enclose the list being rendered: `ul ul`
    /// markers are hollow, `ul ul ul` squares, then the cycle repeats.
    bullet_depth: Cell<usize>,
    /// How many ordered lists enclose the list being rendered: `ol ol` markers
    /// count in lower-alpha, `ol ol ol` in lower-roman, then the cycle repeats.
    ordered_depth: Cell<usize>,
}

impl Workspace {
    /// The body font's ascent and descent at 16px, rounded the way WebKit's
    /// `FontMetrics` are for line layout.
    pub(super) fn body_font_metrics(&self, window: &Window) -> (f32, f32) {
        let mut font = window.text_style().font();
        if let Some(family) = &self.font_family {
            font.family = family.clone();
        }
        let font_id = window.text_system().resolve_font(&font);
        let ascent = f32::from(window.text_system().ascent(font_id, px(BODY_PX))).round();
        let descent = f32::from(window.text_system().descent(font_id, px(BODY_PX)))
            .abs()
            .round();
        (ascent, descent)
    }

    /// The inline text box of a 16px / 24px body line sits `half-leading`
    /// inside the line box on both sides (`Range.getClientRects()`).
    pub(super) fn body_half_leading(&self, window: &Window) -> Pixels {
        let (ascent, descent) = self.body_font_metrics(window);
        px(((BODY_PX * 1.5) - ascent - descent) / 2.0)
    }

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
        let (ascent, descent) = self.body_font_metrics(window);
        let inline_block_gap = px((BODY_PX * 1.5 - ascent + descent) / 2.0);
        let inline_box_height = ascent + descent;
        DocumentRenderer {
            base,
            mono_family: self.mono_font_family.clone(),
            theme: self.theme,
            editor: None,
            next_textblock: Cell::new(0),
            strike_next: Cell::new(false),
            mentions_next: Default::default(),
            placeholder: None,
            title_placeholder: true,
            link_color: None,
            attachments_dir: None,
            image_draft: None,
            next_image: Cell::new(0),
            next_file: Cell::new(0),
            inline_block_gap,
            inline_box_height,
            tooltips: None,
            text_color: Cell::new(None),
            container: Cell::new(Container::Root),
            bullet_depth: Cell::new(0),
            ordered_depth: Cell::new(0),
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
        renderer.title_placeholder = editor.read(cx).caret().is_none_or(|caret| caret.block == 0);
        renderer.image_draft = editor.read(cx).image_resize_draft();
        renderer.editor = Some(editor);
        renderer
    }
}

impl DocumentRenderer {
    /// The enhanced editor's `documentTitlePlaceholder`: only the title
    /// heading has a placeholder, never the body blocks.
    pub(super) fn for_title_document(mut self) -> Self {
        self.placeholder = None;
        self
    }

    /// Resolves attachment ids against the session's `attachments` folder.
    pub(super) fn for_session(mut self, session_dir: &std::path::Path) -> Self {
        self.attachments_dir = Some(anlg_fs_sync_core::attachments::dir(session_dir));
        self
    }

    /// Enables the atoms' tooltips (the `title` of an image).
    pub(super) fn with_tooltips(mut self, host: super::tooltip::TooltipHost) -> Self {
        self.tooltips = Some(host);
        self
    }

    fn attachment_path(&self, attachment_id: Option<&str>) -> Option<PathBuf> {
        let dir = self.attachments_dir.as_ref()?;
        anlg_fs_sync_core::attachments::path_in(dir, attachment_id?)
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
        let drop_editor = editor.clone();
        let mention_drop_editor = editor.clone();
        div()
            .relative()
            .flex()
            .flex_1()
            .flex_col()
            .w_full()
            .on_drop(move |paths: &gpui::ExternalPaths, window, cx| {
                let position = window.mouse_position();
                drop_editor.update(cx, |editor, cx| {
                    editor.drop_paths(paths.paths().to_vec(), position, cx)
                });
            })
            // `sessionMentionDropPlugin`: a dragged note lands as a mention
            // chip plus a space under the pointer.
            .on_drop(move |drag: &super::session_drag::SessionDrag, window, cx| {
                let position = window.mouse_position();
                let item = drag.mention_item();
                mention_drop_editor.update(cx, |editor, cx| {
                    editor.drop_mention(item, position, window, cx)
                });
            })
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
                        handler_editor
                            .update(cx, |editor, _| editor.set_root_width(bounds.size.width));
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(
                // `.prosemirror-editor { min-height: 100% }`: the editor fills
                // the viewport, and a press below the last block focuses the
                // trailing empty line (`note-input`'s container mousedown).
                div()
                    .id("editor-tail")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        click_editor.update(cx, |editor, cx| {
                            editor.focus_trailing_empty_line(window, cx)
                        });
                    }),
            )
            .into_any_element()
    }

    /// Wraps a textblock's text with the editor hooks when editing.
    fn textblock(&self, wrapper: Div, text: ProseText) -> AnyElement {
        // `background: linear-gradient(currentColor, currentColor) 0 55% / 100%
        // 1px no-repeat` on a `width: fit-content` paragraph: one 1px line at
        // 55% of the block's height, as wide as its widest line.
        let strike = self.strike_next.replace(false).then(|| {
            let layout = text.layout().clone();
            let ink = self.theme.foreground;
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let Some(width) = layout.content_width() else {
                        return;
                    };
                    let y = bounds.top() + bounds.size.height * 0.55;
                    window.paint_quad(fill(
                        gpui::Bounds::new(
                            point(bounds.left(), px(f32::from(y).floor())),
                            size(width, px(1.0)),
                        ),
                        ink,
                    ));
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        });
        let avatars = self.mention_avatars(&text);
        let Some(editor) = &self.editor else {
            return wrapper
                .relative()
                .child(text)
                .children(strike)
                .children(avatars)
                .into_any_element();
        };
        let index = self.next_textblock.get();
        self.next_textblock.set(index + 1);
        let layout = text.layout().clone();
        let paint_editor = editor.clone();
        let click_editor = editor.clone();
        let drag_editor = editor.clone();
        let up_editor = editor.clone();
        let caret_color = self.theme.foreground;
        let selection_color = self.theme.selection;
        let placeholder = self
            .placeholder
            .as_ref()
            .filter(|(block, _)| *block == index)
            .map(|(_, text)| text.clone());
        // `.note-typography.prosemirror-editor .is-empty::before`: muted
        // foreground at `opacity: 0.6`.
        let muted = alpha(self.theme.muted_foreground, 0.6);
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
                            event.click_count,
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
            .on_mouse_up(
                MouseButton::Left,
                move |event: &gpui::MouseUpEvent, _, cx| {
                    up_editor.update(cx, |editor, cx| {
                        editor.end_mouse_click(index, event.position, cx)
                    });
                },
            )
            .child(text)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let (caret, selection, matches) = paint_editor.update(cx, |editor, _| {
                            editor.record_layout(index, layout.clone(), bounds);
                            let focused = editor.is_focused(window);
                            (
                                editor
                                    .caret()
                                    .filter(|caret| caret.block == index && focused)
                                    .map(|caret| (caret, editor.caret_upstream())),
                                editor.selection_in_block(index).filter(|_| focused),
                                editor.search_ranges_in_block(index),
                            )
                        });
                        // `.ProseMirror-search-match` (`#ffff0054`) and
                        // `.ProseMirror-active-search-match` (`#ff6a0054`).
                        for (range, active) in matches {
                            let color = if active {
                                gpui::Rgba {
                                    r: 1.0,
                                    g: 0x6a as f32 / 255.0,
                                    b: 0.0,
                                    a: 0x54 as f32 / 255.0,
                                }
                            } else {
                                gpui::Rgba {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 0.0,
                                    a: 0x54 as f32 / 255.0,
                                }
                            };
                            for span in layout.line_spans(range) {
                                window.paint_quad(fill(span, color));
                            }
                        }
                        if let Some(range) = selection {
                            paint_selection(&layout, range, selection_color, window);
                        }
                        if let Some((caret, upstream)) = caret
                            && let Some(position) =
                                layout.position_for_index_biased(caret.offset, upstream)
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
            .children(strike)
            .children(avatars)
            .into_any_element()
    }

    /// `MentionAvatar` painted over each chip's placeholder: a 1em circle in
    /// the facehash colour with the initial for humans, the Note / Buildings /
    /// User glyph in `muted-foreground` otherwise; `vertical-align: middle`
    /// with `top: -2px`.
    fn mention_avatars(&self, text: &ProseText) -> Option<AnyElement> {
        let mentions = std::mem::take(&mut *self.mentions_next.borrow_mut());
        if mentions.is_empty() {
            return None;
        }
        let layout = text.layout().clone();
        let theme = self.theme;
        let font = self.base.font();
        Some(
            canvas(
                |_, _, _| (),
                move |_, _, window, cx| {
                    let em = px(BODY_PX);
                    let line_height = layout.line_height();
                    let em_space = crate::mention::AVATAR_PLACEHOLDER.chars().next().unwrap();
                    for (range, kind, label) in &mentions {
                        let Some(slot) = layout
                            .line_spans(range.start..range.start + em_space.len_utf8())
                            .into_iter()
                            .next()
                        else {
                            continue;
                        };
                        let origin = point(
                            slot.origin.x,
                            slot.origin.y + (line_height - em) / 2.0 - px(2.0),
                        );
                        crate::mention::paint_avatar(
                            window,
                            cx,
                            gpui::Bounds::new(origin, size(em, em)),
                            kind,
                            label,
                            &font,
                            theme.muted_foreground,
                            theme.dark,
                        );
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element(),
        )
    }

    pub(super) fn blocks(&self, blocks: &[Block], depth: usize) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.block(block, depth))
            .collect()
    }

    /// `Streamdown` with `chatComponents` inside a `text-sm` bubble: 14px
    /// text on 20px lines, `p { mb-1.5 last:mb-0 }`, `h1/h2 { mt-3 mb-1
    /// text-base font-semibold first:mt-0 }`, `h3 { mt-2 mb-1 text-sm
    /// font-semibold first:mt-0 }`, `ul/ol { mb-1 pl-5 }`, `li { mb-1 }`.
    pub(super) fn chat_blocks(&self, blocks: &[Block]) -> Vec<AnyElement> {
        let count = blocks.len();
        blocks
            .iter()
            .enumerate()
            .map(|(index, block)| self.chat_block(block, index == 0, index + 1 == count))
            .collect()
    }

    fn chat_block(&self, block: &Block, first: bool, last: bool) -> AnyElement {
        const CHAT_PX: f32 = 14.0;
        let theme = self.theme;
        let line = px(20.0);
        let mut style = self.base.clone();
        style.font_size = px(CHAT_PX).into();
        style.line_height = line.into();
        match block {
            Block::Paragraph(spans) => div()
                .when(!last, |p| p.mb(px(6.0)))
                .child(self.prose(spans, &style, line))
                .into_any_element(),
            Block::Heading { level, spans } => {
                let (font_px, top) = match level {
                    1 | 2 => (16.0, 12.0),
                    _ => (CHAT_PX, 8.0),
                };
                let line = px(if font_px > CHAT_PX { 24.0 } else { 20.0 });
                let mut style = style.clone();
                style.font_weight = gpui::FontWeight::SEMIBOLD;
                style.font_size = px(font_px).into();
                style.line_height = line.into();
                div()
                    .when(!first, |h| h.mt(px(top)))
                    .mb(px(4.0))
                    .text_size(px(font_px))
                    .line_height(line)
                    .child(self.prose(spans, &style, line))
                    .into_any_element()
            }
            Block::List {
                ordered,
                start,
                items,
            } => div()
                .flex()
                .flex_col()
                .mb(px(4.0))
                .pl(px(20.0))
                .children(items.iter().enumerate().map(|(index, item)| {
                    // `list-disc` / `list-decimal` markers sit in the `pl-5` gutter.
                    let marker: AnyElement = if *ordered {
                        div()
                            .absolute()
                            .right(px(6.0))
                            .top_0()
                            .text_size(px(CHAT_PX))
                            .line_height(line)
                            .child(SharedString::from(format!("{}.", *start + index as u64)))
                            .into_any_element()
                    } else {
                        div()
                            .absolute()
                            .left(px(7.0))
                            .top(px(7.5))
                            .size(px(5.0))
                            .rounded_full()
                            .bg(theme.foreground)
                            .into_any_element()
                    };
                    div()
                        .relative()
                        .flex()
                        .mb(px(4.0))
                        .child(
                            div()
                                .absolute()
                                .left(px(-20.0))
                                .top_0()
                                .w(px(20.0))
                                .h(line)
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .flex_1()
                                .children(self.chat_blocks(&item.blocks)),
                        )
                }))
                .into_any_element(),
            Block::Blockquote(blocks) => div()
                .pl_3()
                .border_l_2()
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
                .children(self.chat_blocks(blocks))
                .into_any_element(),
            Block::Code(code) => {
                let mut style = style.clone();
                style.font_size = px(13.0).into();
                if let Some(family) = &self.mono_family {
                    style.font_family = family.clone();
                }
                let span = Span {
                    text: code.clone(),
                    ..Span::default()
                };
                div()
                    .my(px(6.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded_md()
                    .bg(theme.accent)
                    .text_size(px(13.0))
                    .line_height(line)
                    .when_some(self.mono_family.clone(), |code, family| {
                        code.font_family(family)
                    })
                    .child(self.prose(std::slice::from_ref(&span), &style, line))
                    .into_any_element()
            }
            Block::HorizontalRule => div()
                .my(px(8.0))
                .h(px(1.0))
                .bg(theme.border)
                .into_any_element(),
            Block::Image(Image { alt, .. }) => div()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(alt.clone()))
                .into_any_element(),
            Block::FileAttachment(file) => div()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(file.name.clone()))
                .into_any_element(),
            Block::Clip => div().into_any_element(),
        }
    }

    /// `MarkdownPreview`: plain `Streamdown` in the tool card at `text-[13px]
    /// leading-relaxed text-muted-foreground`. Only the Streamdown classes the
    /// desktop bundle also uses elsewhere exist there (its chunk is not a
    /// Tailwind source): `mt-6 mb-2 font-semibold` on headings with `text-3xl`
    /// / `text-xl` / `text-lg` (no `text-2xl`, so `h2` stays 13px), `space-y-4`
    /// between blocks (a heading's `mb-2` wins), `list-disc` outside the
    /// unpadded list (the marker sits left of the content edge), `li { py-1 }`.
    pub(super) fn preview_blocks(&self, blocks: &[Block], color: gpui::Rgba) -> Vec<AnyElement> {
        let count = blocks.len();
        let margins = |block: &Block, last: bool| -> (f32, f32) {
            match block {
                Block::Heading { .. } => (24.0, 8.0),
                Block::HorizontalRule => (24.0, 24.0),
                _ => (0.0, if last { 0.0 } else { 16.0 }),
            }
        };
        let mut elements = Vec::with_capacity(count);
        let mut previous_bottom = 0.0_f32;
        for (index, block) in blocks.iter().enumerate() {
            let last = index + 1 == count;
            let (top, bottom) = margins(block, last);
            // Adjacent block margins collapse; the first one keeps its own
            // top margin inside the `overflow-y-auto` box.
            let gap = if index == 0 {
                top
            } else {
                top.max(previous_bottom)
            };
            elements.push(
                div()
                    .when(gap > 0.0, |block| block.mt(px(gap)))
                    .child(self.preview_block(block, color))
                    .into_any_element(),
            );
            previous_bottom = bottom;
        }
        if previous_bottom > 0.0 {
            elements.push(div().h(px(previous_bottom)).into_any_element());
        }
        elements
    }

    fn preview_block(&self, block: &Block, color: gpui::Rgba) -> AnyElement {
        const PREVIEW_PX: f32 = 13.0;
        // `leading-relaxed` at 13px is 21.125px, laid out as 21px by WebKit.
        const PREVIEW_LINE_PX: f32 = 21.0;
        let line = px(PREVIEW_LINE_PX);
        let mut style = self.base.clone();
        style.font_size = px(PREVIEW_PX).into();
        style.line_height = line.into();
        style.color = color.into();
        match block {
            Block::Paragraph(spans) => div()
                .text_size(px(PREVIEW_PX))
                .line_height(line)
                .child(self.prose(spans, &style, line))
                .into_any_element(),
            Block::Heading { level, spans } => {
                let (font_px, line_px) = match level {
                    1 => (30.0, 36.0),
                    3 => (20.0, 28.0),
                    4 => (18.0, 28.0),
                    5 => (16.0, 24.0),
                    _ => (PREVIEW_PX, PREVIEW_LINE_PX),
                };
                let line = px(line_px);
                let mut style = style.clone();
                style.font_weight = gpui::FontWeight::SEMIBOLD;
                style.font_size = px(font_px).into();
                style.line_height = line.into();
                div()
                    .text_size(px(font_px))
                    .line_height(line)
                    .child(self.prose(spans, &style, line))
                    .into_any_element()
            }
            Block::List {
                ordered,
                start,
                items,
            } => div()
                .flex()
                .flex_col()
                .children(items.iter().enumerate().map(|(index, item)| {
                    // `list-style-position: outside` without padding: the
                    // marker hangs left of the content and gets clipped.
                    let marker: AnyElement = if *ordered {
                        div()
                            .absolute()
                            .right(px(6.0))
                            .top_0()
                            .text_size(px(PREVIEW_PX))
                            .line_height(line)
                            .text_color(color)
                            .child(SharedString::from(format!("{}.", *start + index as u64)))
                            .into_any_element()
                    } else {
                        div()
                            .absolute()
                            .left(px(-16.0))
                            .top(px(12.0))
                            .size(px(5.0))
                            .rounded_full()
                            .bg(color)
                            .into_any_element()
                    };
                    div()
                        .relative()
                        .py_1()
                        .text_size(px(PREVIEW_PX))
                        .line_height(line)
                        .child(marker)
                        .child(
                            div().flex().flex_col().min_w_0().children(
                                item.blocks
                                    .iter()
                                    .map(|block| self.preview_block(block, color)),
                            ),
                        )
                }))
                .into_any_element(),
            Block::Blockquote(blocks) => div()
                .pl_4()
                .italic()
                .children(blocks.iter().map(|block| self.preview_block(block, color)))
                .into_any_element(),
            Block::Code(code) => {
                let mut style = style.clone();
                style.font_size = px(14.0).into();
                if let Some(family) = &self.mono_family {
                    style.font_family = family.clone();
                }
                let span = Span {
                    text: code.clone(),
                    ..Span::default()
                };
                div()
                    .rounded(px(4.0))
                    .bg(self.theme.muted)
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_size(px(14.0))
                    .line_height(line)
                    .when_some(self.mono_family.clone(), |code, family| {
                        code.font_family(family)
                    })
                    .child(self.prose(std::slice::from_ref(&span), &style, line))
                    .into_any_element()
            }
            Block::HorizontalRule => div().h(px(1.0)).bg(self.theme.border).into_any_element(),
            Block::Image(Image { alt, .. }) => div()
                .text_color(color)
                .child(SharedString::from(alt.clone()))
                .into_any_element(),
            Block::FileAttachment(file) => div()
                .text_color(color)
                .child(SharedString::from(file.name.clone()))
                .into_any_element(),
            Block::Clip => div().into_any_element(),
        }
    }

    /// `.note-typography.note-title-editor > h1:first-child` (the enhanced
    /// editor's `enforceTitleHeading`): the first block is the session title
    /// at `1.5rem / 1.875rem` with `margin-bottom: 1rem`, showing the
    /// `documentTitlePlaceholder` (`Untitled`) while empty.
    pub(super) fn title_blocks(&self, blocks: &[Block]) -> Vec<AnyElement> {
        let Some((Block::Heading { level: 1, spans }, rest)) = blocks.split_first() else {
            return self.blocks(blocks, 0);
        };
        let font_px = 24.0;
        let line = px(30.0);
        let mut style = self.base.clone();
        style.font_weight = gpui::FontWeight::BOLD;
        style.font_size = px(font_px).into();
        let empty = spans.iter().all(|span| span.text.trim().is_empty());
        let text = self.prose(spans, &style, line);
        let title = self.textblock(
            div()
                .py(px(font_px * 0.125))
                .mb_4()
                .text_size(px(font_px))
                .line_height(line),
            text,
        );
        // `documentTitlePlaceholder`: `Untitled` over the empty title heading,
        // in the editor too (the placeholder is an overlay, not text).
        let title = if empty && self.title_placeholder {
            let mut placeholder_style = style.clone();
            placeholder_style.color = alpha(self.theme.muted_foreground, 0.6).into();
            let placeholder = self.prose(
                &[Span {
                    text: "Untitled".to_string(),
                    ..Span::default()
                }],
                &placeholder_style,
                line,
            );
            div()
                .relative()
                .child(title)
                .child(
                    div()
                        .absolute()
                        .top(px(font_px * 0.125))
                        .left_0()
                        .child(placeholder),
                )
                .into_any_element()
        } else {
            title
        };
        std::iter::once(title).chain(self.blocks(rest, 0)).collect()
    }

    /// The body style in the current `currentColor`.
    fn block_base(&self) -> TextStyle {
        let mut base = self.base.clone();
        if let Some(color) = self.text_color.get() {
            base.color = color.into();
        }
        base
    }

    fn block(&self, block: &Block, depth: usize) -> AnyElement {
        let theme = self.theme;
        // `.note-typography > * { padding-block: 0.125em }`
        let pad = px(BODY_PX * 0.125);
        let container = self.container.get();
        // A paragraph's own vertical padding: top-level blocks and `li > p`
        // have it, a blockquote's children do not.
        let text_pad = match container {
            Container::Root | Container::ListItem => pad,
            Container::Blockquote => px(0.0),
        };
        match block {
            // The editor's paragraphs compute `text-wrap: wrap` (measured on
            // the running app), not the global `p { text-wrap: pretty }`.
            Block::Paragraph(spans) => self.textblock(
                div().py(text_pad).min_h(px(BODY_PX * 1.5) + text_pad * 2.0),
                self.prose(spans, &self.block_base(), px(BODY_PX * 1.5)),
            ),
            Block::Heading { level, spans } => {
                let (em, weight, ratio) = match level {
                    1 => (1.25, gpui::FontWeight::BOLD, 1.4),
                    2 => (1.125, gpui::FontWeight::SEMIBOLD, 1.4444),
                    _ => (1.0, gpui::FontWeight::SEMIBOLD, 1.5),
                };
                let font_px = BODY_PX * em;
                let mut style = self.block_base();
                style.font_weight = weight;
                style.font_size = px(font_px).into();
                self.textblock(
                    div()
                        // `padding-block: 0.125em` scales with the heading's own
                        // size; only `.note-typography > *` pads a heading.
                        .py(if container == Container::Root {
                            px(font_px * 0.125)
                        } else {
                            px(0.0)
                        })
                        .text_size(px(font_px))
                        .line_height(px(webkit_line_height(font_px, ratio))),
                    self.prose(spans, &style, px(webkit_line_height(font_px, ratio))),
                )
            }
            Block::List {
                ordered,
                start,
                items,
            } => {
                let current = self.text_color.get().unwrap_or(theme.foreground);
                let ordered_depth = self.ordered_depth.get();
                let bullet_depth = self.bullet_depth.get();
                let task_list = items.first().is_some_and(|item| item.checked.is_some());
                if *ordered {
                    self.ordered_depth.set(ordered_depth + 1);
                } else if !task_list {
                    self.bullet_depth.set(bullet_depth + 1);
                }
                let outer_container = self.container.replace(Container::ListItem);
                let list = div()
                    .flex()
                    .flex_col()
                    // `li > ul::before`: a 1px guide rail at `left: calc(-1em - 0.5px)`
                    // in `currentColor` at 30%, centred under the parent marker.
                    // WebKit snaps the half pixel to the nearer device pixel.
                    .when(outer_container == Container::ListItem, |list| {
                        list.relative().child(
                            div()
                                .absolute()
                                .top_0()
                                .left(px(-BODY_PX))
                                .w(px(1.0))
                                .h_full()
                                .bg(alpha(current, 0.3)),
                        )
                    })
                    .children(items.iter().enumerate().map(|(index, item)| {
                        // The marker renders before the item's blocks, so the next
                        // textblock index is the item's first paragraph.
                        let first_block = self.next_textblock.get();
                        let checked = item.checked == Some(true);
                        self.strike_next.set(checked);
                        let number =
                            ordered.then(|| ordered_marker(*start + index as u64, ordered_depth));
                        div()
                            .relative()
                            .flex()
                            .child(
                                // `li { padding-left: 1.5em }`; bullets are centred at
                                // `left: 0.5em`, `top: 0.125em + 0.75em`, ordered markers fill a
                                // `1em` box at `left: 0` with centred text; a task item's
                                // checkbox sits at `left: 0.75rem` of its `-0.75rem` li.
                                div()
                                    .relative()
                                    .flex_shrink_0()
                                    .w(px(BODY_PX * 1.5))
                                    .h(px(BODY_PX * 1.5 + 4.0))
                                    .child(self.marker(
                                        item.checked,
                                        number,
                                        bullet_depth,
                                        first_block,
                                    )),
                            )
                            .child(
                                // `li[data-checked="true"] > div { opacity: 0.5 }`
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .flex_1()
                                    .when(checked, |content| content.opacity(0.5))
                                    .children(self.blocks(&item.blocks, depth + 1)),
                            )
                    }))
                    .into_any_element();
                self.container.set(outer_container);
                self.ordered_depth.set(ordered_depth);
                self.bullet_depth.set(bullet_depth);
                list
            }
            // `blockquote { border-left: 3px solid border; padding-inline: 1em
            // 0.75rem; padding-block: 0.125em; color: muted-foreground }`, the
            // colour being `currentColor` for everything inside, and
            // `blockquote > * + * { margin-top: 0.5em }`.
            Block::Blockquote(blocks) => {
                let outer = self.text_color.replace(Some(theme.muted_foreground));
                let outer_container = self.container.replace(Container::Blockquote);
                let children: Vec<AnyElement> = blocks
                    .iter()
                    .enumerate()
                    .map(|(index, block)| {
                        let element = self.block(block, depth + 1);
                        if index == 0 {
                            element
                        } else {
                            // `0.5em` of the child's own font size.
                            div()
                                .mt(px(block_font_px(block) * 0.5))
                                .child(element)
                                .into_any_element()
                        }
                    })
                    .collect();
                self.container.set(outer_container);
                self.text_color.set(outer);
                div()
                    .py(pad)
                    .pl(px(BODY_PX))
                    .pr(px(12.0))
                    .border_l(px(3.0))
                    .border_color(theme.border)
                    .text_color(theme.muted_foreground)
                    .children(children)
                    .into_any_element()
            }
            Block::Code(code) => {
                // `pre`: 0.875em, line-height 1.4286, `margin-block: 0.5em`,
                // `padding: 1em 0.75em`, `rounded-md`, wrapping (`pre-wrap`).
                let font_px = BODY_PX * 0.875;
                let line_height = px(webkit_line_height(font_px, 1.4286));
                let mut style = self.block_base();
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
            Block::Image(image) => self.image_block(image, pad),
            Block::FileAttachment(file) => self.file_attachment_block(file, pad),
            // `div[data-type="clip"]` is empty: `padding-block: 0.125em` only.
            Block::Clip => div().py(pad).into_any_element(),
        }
    }

    /// `ResizableImageView` (its root is a plain block child, so only the
    /// `padding-block: 0.125em` applies; the `.node-image` rules match
    /// nothing): the `inline-block` frame at `editorWidth%` of the editor sits
    /// on the line's baseline, so the strut's descent follows it; the image is
    /// `rounded-md bg-card` with the `ring-border ring-offset-2` hover ring
    /// and — while editing — the two resize pills.
    fn image_block(&self, image: &Image, pad: Pixels) -> AnyElement {
        let theme = self.theme;
        let nth = self.next_image.replace(self.next_image.get() + 1);
        let source: Option<gpui::ImageSource> = self
            .attachment_path(image.attachment_id.as_deref())
            .map(gpui::ImageSource::from)
            .or_else(|| {
                let src = image.src.as_deref()?;
                (src.starts_with("https://") || src.starts_with("http://"))
                    .then(|| gpui::ImageSource::from(SharedString::from(src.to_string())))
            });
        let editor = self.editor.clone();
        // A drag in progress paints its pixel width; otherwise the stored
        // percentage of the editor.
        let draft = self
            .image_draft
            .filter(|(index, _)| *index == nth)
            .map(|(_, width)| width);
        let handle = |left: bool, editor: Option<Entity<BodyEditor>>| {
            div()
                .id((
                    if left {
                        "image-resize-left"
                    } else {
                        "image-resize-right"
                    },
                    nth,
                ))
                .absolute()
                .top_0()
                .bottom_0()
                .map(|handle| {
                    if left {
                        handle.left(px(4.0))
                    } else {
                        handle.right(px(4.0))
                    }
                })
                .flex()
                .items_center()
                .opacity(0.0)
                .group_hover("note-image", |handle| handle.opacity(1.0))
                .child(
                    div()
                        .flex()
                        .h(px(56.0))
                        .w(px(16.0))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(theme.border)
                        .bg(alpha(theme.card, 0.95))
                        .shadow_sm()
                        .cursor(gpui::CursorStyle::ResizeLeftRight)
                        .child(
                            div()
                                .h(px(32.0))
                                .w(px(4.0))
                                .rounded_full()
                                .bg(theme.muted_foreground),
                        ),
                )
                .when_some(
                    editor,
                    |handle: gpui::Stateful<Div>, editor: Entity<BodyEditor>| {
                        handle.on_mouse_down(
                            MouseButton::Left,
                            move |event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                editor.update(cx, |editor, cx| {
                                    editor.begin_image_resize(nth, left, event.position.x, cx)
                                });
                            },
                        )
                    },
                )
        };
        let bounds_editor = editor.clone();
        // `<img title>`: the toolkit's tooltip after the system hover delay.
        let title = image
            .title
            .as_deref()
            .and_then(crate::document::image_title)
            .zip(self.tooltips.as_ref());
        let frame = div()
            .id(("note-image", nth))
            .group("note-image")
            .relative()
            .map(|frame| match draft {
                Some(width) => frame.w(width),
                None => frame.w(relative(image.editor_width as f32 / 100.0)),
            })
            .max_w_full()
            .rounded(px(6.0))
            // `hover:ring-1 ring-border ring-offset-2 ring-offset-card`.
            .child(
                crate::ui::ring(theme.card, 2.0, 0.0, 0.0, 6.0)
                    .invisible()
                    .group_hover("note-image", |ring| ring.visible()),
            )
            .child(
                crate::ui::ring(theme.border, 1.0, 2.0, 0.0, 6.0)
                    .invisible()
                    .group_hover("note-image", |ring| ring.visible()),
            )
            .map(|frame| match source {
                Some(source) => frame.child(
                    img(source)
                        .w_full()
                        .rounded(px(6.0))
                        .bg(theme.card)
                        .with_fallback({
                            let alt = image.alt.clone();
                            let color = theme.muted_foreground;
                            move || {
                                div()
                                    .text_color(color)
                                    .child(SharedString::from(alt.clone()))
                                    .into_any_element()
                            }
                        }),
                ),
                None => frame.child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(image.alt.clone())),
                ),
            })
            .when_some(bounds_editor, |frame, editor| {
                frame.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, _, cx| {
                            editor.update(cx, |editor, _| editor.set_image_bounds(nth, bounds));
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
            .when(self.editor.is_some(), |frame| {
                frame
                    .child(handle(true, editor.clone()))
                    .child(handle(false, editor.clone()))
            });
        div()
            .pt(pad)
            .pb(pad + self.inline_block_gap)
            .map(|block| match title {
                Some((title, host)) => block.child(host.trigger(
                    super::tooltip::TooltipSpec::title(format!("note-image-title-{nth}"), title),
                    frame,
                )),
                None => block.child(frame),
            })
            .into_any_element()
    }

    /// `FileAttachmentView`: the `my-1 rounded-lg border bg-muted px-3 py-2.5`
    /// card with the 40px icon (or image thumbnail), the name and size, and
    /// the open / remove buttons shown on hover.
    fn file_attachment_block(&self, file: &FileAttachment, pad: Pixels) -> AnyElement {
        let theme = self.theme;
        let nth = self.next_file.replace(self.next_file.get() + 1);
        let path = self
            .attachment_path(file.attachment_id.as_deref())
            .map(|path| path.to_string_lossy().to_string())
            .or_else(|| file.path.clone());
        let is_image = file.mime_type.starts_with("image/");
        let icon_name = if is_image {
            "image"
        } else {
            match file.mime_type.as_str() {
                "application/pdf"
                | "text/plain"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "application/msword" => "file-text",
                "text/csv"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                | "application/vnd.ms-excel" => "file-spreadsheet",
                _ => "file",
            }
        };
        let display_name = if file.name.is_empty() {
            "file".to_string()
        } else if file.name.chars().count() > 60 {
            format!("{}\u{2026}", file.name.chars().take(60).collect::<String>())
        } else {
            file.name.clone()
        };
        let size_label = file.size.map(format_file_size);
        let thumbnail = path.as_deref().filter(|_| is_image).map(PathBuf::from);
        let action = |id: &'static str, glyph: &'static str| {
            div()
                .id((id, nth))
                .p(px(4.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .child(crate::ui::icon(glyph, px(14.0), theme.muted_foreground))
        };
        let open_path = path.clone();
        let editor = self.editor.clone();
        div()
            .py(pad)
            .child(
                div()
                    .id(("file-attachment", nth))
                    .group("file-attachment")
                    // `my-1`, but `.prosemirror-editor :first-child { margin-top: 0 }`.
                    .mb(px(4.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .px(px(12.0))
                    .py(px(10.0))
                    .hover(move |style| style.bg(theme.accent))
                    .child(match thumbnail {
                        Some(path) => img(path)
                            .size(px(40.0))
                            .flex_shrink_0()
                            .rounded(px(4.0))
                            .object_fit(gpui::ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .flex()
                            .size(px(40.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.0))
                            .bg(alpha(theme.accent, 0.6))
                            .child(crate::ui::icon(icon_name, px(20.0), theme.muted_foreground))
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(SharedString::from(display_name)),
                            )
                            .when_some(size_label, |column, label| {
                                column.child(
                                    div()
                                        .tw_text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(label)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(4.0))
                            .opacity(0.0)
                            .group_hover("file-attachment", |row| row.opacity(1.0))
                            .when_some(open_path, |row, path| {
                                row.child(
                                    action("file-attachment-open", "arrow-square-out")
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            cx.stop_propagation();
                                            if path.starts_with("https://") {
                                                crate::opener::open_url(&path);
                                            } else {
                                                crate::opener::open_path(std::path::Path::new(
                                                    &path,
                                                ));
                                            }
                                        }),
                                )
                            })
                            .when_some(editor, |row, editor| {
                                row.child(action("file-attachment-remove", "x").on_mouse_down(
                                    MouseButton::Left,
                                    move |_, _, cx| {
                                        cx.stop_propagation();
                                        editor.update(cx, |editor, cx| {
                                            editor.remove_block_atom("fileAttachment", nth, cx)
                                        });
                                    },
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }

    /// Unordered markers cycle with the enclosing `ul`s (filled circle,
    /// hollow circle, square, then repeat); ordered lists count; task items
    /// draw a checkbox.
    fn marker(
        &self,
        checked: Option<bool>,
        number: Option<String>,
        bullet_depth: usize,
        first_block: usize,
    ) -> AnyElement {
        let theme = self.theme;
        // `color-mix(in oklab, currentColor, transparent 35%)`
        let ink = alpha(self.text_color.get().unwrap_or(theme.foreground), 0.65);
        let centre = |size: f32| {
            div()
                .absolute()
                .left(px(BODY_PX * 0.5 - size / 2.0))
                .top(px(BODY_PX * 0.875 - size / 2.0))
                .size(px(size))
        };
        if let Some(checked) = checked {
            return self.task_checkbox(checked, first_block);
        }
        if let Some(number) = number {
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
                .child(SharedString::from(number))
                .into_any_element();
        }
        match bullet_depth % 3 {
            0 => centre(BODY_PX * 0.5).rounded_full().bg(ink),
            1 => centre(BODY_PX * 0.5)
                .rounded_full()
                .border(px(1.5))
                .border_color(ink),
            _ => centre(BODY_PX * 0.42).rounded(px(1.6)).bg(ink),
        }
        .into_any_element()
    }

    /// `TaskCheckbox` (`task-list.css`): a `1em` box at `top: 0.375em` with a
    /// 1.5px (one device pixel) border at 65% foreground and a 5px radius;
    /// checked fills it with the foreground and draws the rotated-L check in
    /// the background colour. Interactive only while editing.
    fn task_checkbox(&self, checked: bool, first_block: usize) -> AnyElement {
        let theme = self.theme;
        let ink = alpha(theme.foreground, 0.65);
        let check = theme.background;
        let box_px = BODY_PX;
        let editor = self.editor.clone();
        div()
            .id(("task-checkbox", first_block))
            .absolute()
            .left_0()
            .top(px(BODY_PX * 0.375))
            .size(px(box_px))
            .rounded(px(BODY_PX * 0.3125))
            .border_1()
            .border_color(ink)
            .when(checked, |b| {
                b.bg(theme.foreground).border_color(theme.foreground)
            })
            .when_some(editor, |b, editor| {
                b.cursor_pointer()
                    .when(!checked, |b| {
                        b.hover(move |s| s.border_color(theme.foreground))
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_: &gpui::ClickEvent, _, cx| {
                        editor.update(cx, |editor, cx| editor.toggle_task(first_block, cx));
                    })
            })
            .when(checked, |b| {
                b.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            // `::after`: a 0.28em × 0.55em box at (0.26em, 0.04em)
                            // inside the border with 0.125em right/bottom borders,
                            // rotated 45° about its centre.
                            let em = box_px;
                            let inset = 1.0;
                            let (x, y, w, h) = (
                                bounds.left() + px(inset + em * 0.26),
                                bounds.top() + px(inset + em * 0.04),
                                em * 0.28,
                                em * 0.55,
                            );
                            let centre = point(x + px(w / 2.0), y + px(h / 2.0));
                            let rotate = |p: Point<Pixels>| {
                                let (dx, dy) =
                                    (f32::from(p.x - centre.x), f32::from(p.y - centre.y));
                                let (s, c) = (
                                    std::f32::consts::FRAC_1_SQRT_2,
                                    std::f32::consts::FRAC_1_SQRT_2,
                                );
                                point(
                                    centre.x + px(dx * c - dy * s),
                                    centre.y + px(dx * s + dy * c),
                                )
                            };
                            let stroke = em * 0.125;
                            // The L runs down the right edge and along the bottom
                            // edge; stroke it along the border's centre line.
                            let right = x + px(w - stroke / 2.0);
                            let bottom = y + px(h - stroke / 2.0);
                            let mut path = gpui::PathBuilder::stroke(px(stroke));
                            path.move_to(rotate(point(right, y)));
                            path.line_to(rotate(point(right, bottom)));
                            path.line_to(rotate(point(x, bottom)));
                            if let Ok(path) = path.build() {
                                window.paint_path(path, check);
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
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
            strike_next: Cell::new(false),
            mentions_next: Default::default(),
            placeholder: None,
            link_color: Some(link_color),
            title_placeholder: false,
            attachments_dir: None,
            image_draft: None,
            next_image: Cell::new(0),
            next_file: Cell::new(0),
            inline_block_gap: self.inline_block_gap,
            inline_box_height: self.inline_box_height,
            tooltips: None,
            text_color: Cell::new(None),
            container: Cell::new(Container::Root),
            bullet_depth: Cell::new(0),
            ordered_depth: Cell::new(0),
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
        // `.note-typography mark`: `yellow-200` over the inline box only, with
        // `border-radius: 0.125rem`.
        let mut marks = Vec::new();
        let mut start = 0;
        for span in spans {
            let end = start + span.text.len();
            if span.highlight {
                marks.push(crate::prose_text::Highlight {
                    range: start..end,
                    color: gpui::rgb(0xfef08a),
                    inset_x: px(0.0),
                    radius: px(2.0),
                });
            }
            start = end;
        }
        let font_px = f32::from(font_size);
        let inset_y =
            px(((f32::from(line_height)) - self.inline_box_height * font_px / BODY_PX) / 2.0);
        ProseText::new(text, runs, font_size, line_height).with_inline_backgrounds(marks, inset_y)
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
            if let Some((kind, _, label)) = &span.mention {
                self.mentions_next.borrow_mut().push((
                    start..start + crate::mention::AVATAR_PLACEHOLDER.len(),
                    kind.clone(),
                    label.clone(),
                ));
            }
            let highlight = HighlightStyle {
                // `.mention { font-weight: 500 }`
                font_weight: if span.bold {
                    Some(gpui::FontWeight::BOLD)
                } else if span.mention.is_some()
                    || (span.link.is_some() && self.link_color.is_some())
                {
                    Some(gpui::FontWeight::MEDIUM)
                } else {
                    None
                },
                font_style: span.italic.then_some(gpui::FontStyle::Italic),
                // `.note-typography mark`: `yellow-200`, dark text in dark mode.
                color: if span.highlight && self.theme.dark {
                    Some(gpui::rgb(0x1c1917).into())
                } else {
                    span.link.is_some().then(|| link_color.into())
                },
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

/// The node a block is rendered directly inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Root,
    ListItem,
    Blockquote,
}

/// The font size a block's box computes, which `blockquote > * + *`'s `0.5em`
/// margin scales with.
fn block_font_px(block: &Block) -> f32 {
    match block {
        Block::Heading { level: 1, .. } => BODY_PX * 1.25,
        Block::Heading { level: 2, .. } => BODY_PX * 1.125,
        Block::Code(_) => BODY_PX * 0.875,
        _ => BODY_PX,
    }
}

/// `counter(ol-counter, <style>) "."`: decimal, lower-alpha and lower-roman
/// cycling with the ordered-list nesting depth.
fn ordered_marker(number: u64, ordered_depth: usize) -> String {
    let counter = match ordered_depth % 3 {
        0 => number.to_string(),
        1 => lower_alpha(number),
        _ => lower_roman(number),
    };
    format!("{counter}.")
}

/// CSS `lower-alpha`: a..z, then aa, ab, …; zero and below fall back to decimal.
fn lower_alpha(number: u64) -> String {
    if number == 0 {
        return "0".to_string();
    }
    let mut n = number;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    letters.iter().rev().collect()
}

/// CSS `lower-roman` for 1..=3999; decimal outside that range.
fn lower_roman(number: u64) -> String {
    if number == 0 || number >= 4000 {
        return number.to_string();
    }
    const TABLE: [(u64, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = number;
    let mut out = String::new();
    for (value, numeral) in TABLE {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod marker_tests {
    use super::*;

    #[test]
    fn ordered_markers_follow_the_css_counters() {
        assert_eq!(ordered_marker(1, 0), "1.");
        assert_eq!(ordered_marker(3, 0), "3.");
        assert_eq!(ordered_marker(1, 1), "a.");
        assert_eq!(ordered_marker(27, 1), "aa.");
        assert_eq!(ordered_marker(4, 2), "iv.");
        assert_eq!(ordered_marker(1999, 2), "mcmxcix.");
        assert_eq!(ordered_marker(2, 3), "2.");
        assert_eq!(ordered_marker(0, 2), "0.");
    }
}
