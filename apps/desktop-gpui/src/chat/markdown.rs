//! Assistant markdown, rendered like shipping's Streamdown overrides in
//! `apps/desktop/src/chat/components/message/normal.tsx`. Parsing reuses the TipTap
//! converter; GFM tables, which it drops, are split out first.

use std::{cell::RefCell, ops::Range, path::PathBuf, rc::Rc};

use gpui::{
    AnyElement, ClipboardItem, FontStyle, FontWeight, HighlightStyle, InteractiveText, MouseButton,
    Pixels, Point, SharedString, StrikethroughStyle, StyledText, TextLayout, UnderlineStyle,
    WeakEntity, Window, anchored, canvas, deferred, div, fill, point, prelude::*, px, size, svg,
};
use serde_json::Value;

use super::ChatView;
use crate::ui::a11y::Accessible;
use crate::ui::theme::Theme;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Item>,
    },
    Code {
        language: Option<String>,
        code: String,
    },
    Quote(Vec<Block>),
    Rule,
    Table {
        header: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

pub fn parse(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut prose = String::new();
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut fence: Option<&str> = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(&trimmed[..3]);
        } else if line.contains('|') && lines.get(index + 1).is_some_and(|next| is_delimiter(next))
        {
            blocks.extend(parse_prose(&std::mem::take(&mut prose)));
            let header = cells(line);
            let mut rows = Vec::new();
            index += 2;
            while index < lines.len()
                && lines[index].contains('|')
                && !lines[index].trim().is_empty()
            {
                let mut row = cells(lines[index]);
                row.resize(header.len(), String::new());
                rows.push(row.iter().map(|cell| inline_markdown(cell)).collect());
                index += 1;
            }
            blocks.push(Block::Table {
                header: header.iter().map(|cell| inline_markdown(cell)).collect(),
                rows,
            });
            continue;
        }
        prose.push_str(line);
        prose.push('\n');
        index += 1;
    }
    blocks.extend(parse_prose(&prose));
    blocks
}

fn is_delimiter(line: &str) -> bool {
    let cells = cells(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim_start_matches(':').trim_end_matches(':');
            !cell.is_empty() && cell.chars().all(|c| c == '-')
        })
}

fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(|cell| cell.trim().to_owned()).collect()
}

fn inline_markdown(text: &str) -> Inline {
    match parse_prose(text).into_iter().next() {
        Some(Block::Paragraph(inline)) => inline,
        _ => Inline {
            text: text.to_owned(),
            spans: Vec::new(),
        },
    }
}

fn parse_prose(markdown: &str) -> Vec<Block> {
    if markdown.trim().is_empty() {
        return Vec::new();
    }
    match anlg_tiptap::md_to_tiptap_json(markdown) {
        Ok(doc) => blocks(&doc),
        Err(_) => vec![Block::Paragraph(Inline {
            text: markdown.trim_end().to_owned(),
            spans: Vec::new(),
        })],
    }
}

fn children(node: &Value) -> &[Value] {
    node["content"].as_array().map_or(&[], Vec::as_slice)
}

fn blocks(node: &Value) -> Vec<Block> {
    children(node).iter().filter_map(block).collect()
}

fn block(node: &Value) -> Option<Block> {
    Some(match node["type"].as_str()? {
        "paragraph" => Block::Paragraph(inline(node)),
        "heading" => Block::Heading(
            node["attrs"]["level"].as_u64().unwrap_or(1) as u8,
            inline(node),
        ),
        "bulletList" | "orderedList" | "taskList" => Block::List {
            ordered: node["type"] == "orderedList",
            start: node["attrs"]["start"].as_u64().unwrap_or(1),
            items: children(node)
                .iter()
                .map(|item| Item {
                    checked: (item["type"] == "taskItem")
                        .then(|| item["attrs"]["checked"].as_bool().unwrap_or(false)),
                    blocks: blocks(item),
                })
                .collect(),
        },
        "codeBlock" => Block::Code {
            language: node["attrs"]["language"].as_str().map(str::to_owned),
            code: inline(node).text,
        },
        "blockquote" => Block::Quote(blocks(node)),
        "horizontalRule" => Block::Rule,
        "image" => Block::Paragraph(Inline {
            text: node["attrs"]["alt"].as_str().unwrap_or_default().to_owned(),
            spans: Vec::new(),
        }),
        _ => return None,
    })
}

fn inline(node: &Value) -> Inline {
    let mut result = Inline::default();
    for child in children(node) {
        let text = match child["type"].as_str() {
            Some("text") => child["text"].as_str().unwrap_or_default(),
            Some("hardBreak") => "\n",
            Some(kind) if kind.starts_with("mention") => {
                child["attrs"]["label"].as_str().unwrap_or_default()
            }
            _ => continue,
        };
        let mut span = Span::default();
        for mark in child["marks"].as_array().into_iter().flatten() {
            match mark["type"].as_str() {
                Some("bold") => span.bold = true,
                Some("italic") => span.italic = true,
                Some("strike") => span.strike = true,
                Some("code") => span.code = true,
                Some("link") => span.link = mark["attrs"]["href"].as_str().map(str::to_owned),
                _ => {}
            }
        }
        let start = result.text.len();
        if span.code {
            // Inline code is padded (`px-1.5`) inside its background.
            result.text.push_str(CODE_PAD);
            result.text.push_str(text);
            result.text.push_str(CODE_PAD);
        } else {
            result.text.push_str(text);
        }
        span.range = start..result.text.len();
        if span.bold || span.italic || span.strike || span.code || span.link.is_some() {
            result.spans.push(span);
        }
    }
    result
}

const CODE_PAD: &str = "\u{2009}\u{2009}";

#[derive(Clone)]
pub struct Style {
    pub colors: Theme,
    pub mono: SharedString,
    pub chat: WeakEntity<ChatView>,
    pub menu: Option<(String, TableMenu)>,
    pub copied: Option<String>,
    pub selection: Option<(Caret, Caret)>,
    pub texts: Rc<RefCell<Vec<Painted>>>,
    /// Streamdown disables block controls while a reply streams.
    pub streaming: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableMenu {
    Copy,
    Download,
}

/// A point in the rendered messages: a text element's key and a byte in its text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Caret {
    pub key: SharedString,
    pub byte: usize,
}

/// A text element as last painted, in document order.
#[derive(Clone)]
pub struct Painted {
    pub key: SharedString,
    pub text: SharedString,
    pub layout: TextLayout,
}

/// Message interactions Streamdown provides in the browser: dragging a text selection across
/// blocks, the table copy/download menus and the fullscreen table.
#[derive(Default)]
pub struct Interaction {
    pub menu: Option<(String, TableMenu)>,
    pub copied: Option<String>,
    pub fullscreen: Option<(String, Vec<Inline>, Vec<Vec<Inline>>)>,
    pub selection: Option<(Caret, Caret)>,
    pub selecting: bool,
    pub texts: Rc<RefCell<Vec<Painted>>>,
}

impl Interaction {
    pub fn close(&mut self) {
        self.menu = None;
        self.fullscreen = None;
        self.selection = None;
        self.selecting = false;
    }

    fn caret_at(&self, position: Point<Pixels>, nearest: bool) -> Option<Caret> {
        let texts = self.texts.borrow();
        let inside = texts.iter().find(|painted| {
            let bounds = painted.layout.bounds();
            bounds.contains(&position)
        });
        let painted = inside.or_else(|| {
            nearest
                .then(|| {
                    texts.iter().min_by(|a, b| {
                        let distance = |painted: &Painted| {
                            let bounds = painted.layout.bounds();
                            let y = f32::from(position.y);
                            if y < f32::from(bounds.top()) {
                                f32::from(bounds.top()) - y
                            } else {
                                (y - f32::from(bounds.bottom())).max(0.)
                            }
                        };
                        distance(a).total_cmp(&distance(b))
                    })
                })
                .flatten()
        })?;
        let byte = painted
            .layout
            .index_for_position(position)
            .unwrap_or_else(|byte| byte);
        Some(Caret {
            key: painted.key.clone(),
            byte,
        })
    }

    /// Starts a selection at a pressed point; false when it is not over message text.
    pub fn press(&mut self, position: Point<Pixels>) -> bool {
        self.menu = None;
        let caret = self.caret_at(position, false);
        self.selecting = caret.is_some();
        self.selection = caret.map(|caret| (caret.clone(), caret));
        self.selecting
    }

    pub fn drag(&mut self, position: Point<Pixels>) -> bool {
        if !self.selecting {
            return false;
        }
        let Some(head) = self.caret_at(position, true) else {
            return false;
        };
        match &mut self.selection {
            Some((_, current)) if *current != head => {
                *current = head;
                true
            }
            _ => false,
        }
    }

    /// The selected text, element by element in document order.
    pub fn selected_text(&self) -> Option<String> {
        let (anchor, head) = self.selection.as_ref()?;
        let texts = self.texts.borrow();
        let index = |caret: &Caret| texts.iter().position(|painted| painted.key == caret.key);
        let (a, b) = (index(anchor)?, index(head)?);
        let ((start, from), (end, to)) = if (a, anchor.byte) <= (b, head.byte) {
            ((a, anchor.byte), (b, head.byte))
        } else {
            ((b, head.byte), (a, anchor.byte))
        };
        let parts = texts[start..=end]
            .iter()
            .enumerate()
            .map(|(offset, painted)| {
                let text = painted.text.as_ref();
                let first = if offset == 0 { from.min(text.len()) } else { 0 };
                let last = if start + offset == end {
                    to.min(text.len())
                } else {
                    text.len()
                };
                text.get(first..last.max(first))
                    .unwrap_or_default()
                    .replace(CODE_PAD, "")
            })
            .collect::<Vec<_>>();
        let text = parts.join("\n");
        (!text.is_empty()).then_some(text)
    }
}

pub fn render(blocks: &[Block], style: &Style, id: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .px(px(2.))
        .py_1()
        .text_sm()
        .line_height(px(20.))
        .children(render_blocks(blocks, style, id))
        .into_any_element()
}

/// A tool card's `MarkdownPreview`: Streamdown's own blocks at 13px `leading-relaxed`. The
/// desktop stylesheet keeps their heading sizes, `mt-6 mb-2` and `py-1` items, but not the
/// list indent, so bullets hang outside the clipped preview.
pub fn render_preview(blocks: &[Block], style: &Style, id: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(21.125))
        .children(preview_blocks(blocks, style, id))
        .into_any_element()
}

fn preview_blocks(blocks: &[Block], style: &Style, id: &str) -> Vec<AnyElement> {
    let margin = |block: &Block| match block {
        Block::Heading(..) => (24., 8.),
        Block::Quote(_) | Block::Code { .. } => (16., 16.),
        _ => (0., 0.),
    };
    let mut previous_bottom: Option<f32> = None;
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let id = format!("{id}-{index}");
            let (top, bottom) = margin(block);
            // Adjacent margins collapse; the preview's padding keeps the first and last.
            let gap = previous_bottom.map_or(top, |previous| previous.max(top));
            previous_bottom = Some(bottom);
            let last = index + 1 == blocks.len();
            let element = match block {
                Block::Heading(level, text) => {
                    let (size, line) = match level {
                        1 => (30., 36.),
                        2 => (24., 32.),
                        3 => (20., 28.),
                        4 => (18., 28.),
                        5 => (16., 24.),
                        _ => (14., 20.),
                    };
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(size))
                        .line_height(px(line))
                        .child(text_element(text, style, &id))
                        .into_any_element()
                }
                Block::List { items, .. } => div()
                    .flex()
                    .flex_col()
                    .children(items.iter().enumerate().map(|(position, item)| {
                        div().py_1().flex().flex_col().children(preview_blocks(
                            &item.blocks,
                            style,
                            &format!("{id}-{position}"),
                        ))
                    }))
                    .into_any_element(),
                Block::Quote(blocks) => div()
                    .border_l_4()
                    .border_color(style.colors.muted_foreground.opacity(0.3))
                    .pl_4()
                    .italic()
                    .flex()
                    .flex_col()
                    .children(preview_blocks(blocks, style, &id))
                    .into_any_element(),
                block => render_block(block, style, &id),
            };
            div()
                .min_w_0()
                .mt(px(gap))
                .when(last, |view| view.mb(px(bottom)))
                .child(element)
                .into_any_element()
        })
        .collect()
}

/// Top and bottom margins from the chat's Streamdown overrides. Streamdown's own `my-*`
/// classes are not generated by the desktop stylesheet, so its other blocks have none.
fn margins(block: &Block) -> (f32, f32) {
    match block {
        Block::Paragraph(_) => (0., 6.),
        Block::Heading(1 | 2, _) => (12., 4.),
        Block::Heading(3, _) => (8., 4.),
        Block::Heading(..) => (24., 8.),
        Block::List { .. } => (0., 4.),
        Block::Code { .. } | Block::Quote(_) | Block::Table { .. } | Block::Rule => (0., 0.),
    }
}

/// Vertical margins collapse between siblings, and the first/last child drop theirs.
fn gaps(blocks: &[Block]) -> Vec<f32> {
    blocks
        .windows(2)
        .map(|pair| margins(&pair[0]).1.max(margins(&pair[1]).0))
        .collect()
}

fn render_blocks(blocks: &[Block], style: &Style, id: &str) -> Vec<AnyElement> {
    let gaps = gaps(blocks);
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let id = format!("{id}-{index}");
            let element = render_block(block, style, &id);
            div()
                .min_w_0()
                .when_some(gaps.get(index), |view, gap| view.mb(px(*gap)))
                .child(element)
                .into_any_element()
        })
        .collect()
}

fn render_block(block: &Block, style: &Style, id: &str) -> AnyElement {
    let colors = &style.colors;
    match block {
        Block::Paragraph(text) => text_element(text, style, id),
        Block::Heading(level, text) => div()
            .font_weight(FontWeight::SEMIBOLD)
            .map(|view| match level {
                1 | 2 | 5 => view.text_base().line_height(px(24.)),
                4 => view.text_lg().line_height(px(28.)),
                _ => view,
            })
            .child(text_element(text, style, id))
            .into_any_element(),
        Block::List {
            ordered,
            start,
            items,
        } => div()
            .flex()
            .flex_col()
            .children(items.iter().enumerate().map(|(position, item)| {
                let marker = match (item.checked, ordered) {
                    (Some(true), _) => "☑".to_owned(),
                    (Some(false), _) => "☐".to_owned(),
                    (None, true) => format!("{}.", start + position as u64),
                    (None, false) => "•".to_owned(),
                };
                // Outside markers: discs sit near the gutter's start, numbers end by the text.
                let disc = item.checked.is_none() && !ordered;
                div()
                    .flex()
                    .when(position + 1 < items.len(), |view| view.mb_1())
                    .child(
                        div()
                            .w(px(20.))
                            .flex_shrink_0()
                            .flex()
                            .when(disc, |view| view.pl(px(5.)))
                            .when(!disc, |view| view.justify_end().pr(px(5.)))
                            .child(marker),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .children(render_blocks(
                                &item.blocks,
                                style,
                                &format!("{id}-{position}"),
                            )),
                    )
            }))
            .into_any_element(),
        Block::Code { language, code } => {
            let code = code.trim_end().to_owned();
            let extension = language.clone().unwrap_or_else(|| "txt".into());
            frame(colors)
                .gap_2()
                .child(
                    div()
                        .h_8()
                        .flex()
                        .items_center()
                        .ml_1()
                        .font_family(style.mono.clone())
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(language.clone().unwrap_or_default().to_lowercase()),
                )
                .child(
                    div().h_8().flex().items_center().justify_end().child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px(px(6.))
                            .py_1()
                            .border_1()
                            .border_color(colors.border)
                            .child(download_button(
                                id,
                                format!("code.{extension}"),
                                code.clone(),
                                style,
                            ))
                            .child(copy_button(id, code.clone(), style)),
                    ),
                )
                .child(
                    div()
                        .p_4()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.background)
                        .font_family(style.mono.clone())
                        .child(text_element(
                            &Inline {
                                text: code,
                                spans: Vec::new(),
                            },
                            style,
                            &format!("{id}-code"),
                        )),
                )
                .into_any_element()
        }
        Block::Quote(blocks) => div()
            .pl_4()
            .italic()
            .text_color(colors.muted_foreground)
            .children(render_blocks(blocks, style, id))
            .into_any_element(),
        Block::Rule => div().h(px(1.)).bg(colors.border).into_any_element(),
        Block::Table { header, rows } => frame(colors)
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(table_controls(id, header, rows, style)),
            )
            .child(
                table(header, rows, style, id)
                    .overflow_hidden()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.background),
            )
            .into_any_element(),
    }
}

/// The table's rows: a muted header, then bordered body rows.
pub fn table(header: &[Inline], rows: &[Vec<Inline>], style: &Style, id: &str) -> gpui::Div {
    let colors = &style.colors;
    let widths = column_widths(header, rows);
    let row = |cells: &[Inline], key: &str, head: bool| {
        div()
            .flex()
            .children(cells.iter().zip(&widths).enumerate().map(
                |(column, (text, (minimum, weight)))| {
                    let mut cell = div().flex_basis(px(*minimum)).flex_shrink_0();
                    cell.style().flex_grow = Some(*weight);
                    cell.px_4()
                        .py_2()
                        .when(head, |view| view.font_weight(FontWeight::SEMIBOLD))
                        .child(text_element(text, style, &format!("{id}-{key}-{column}")))
                },
            ))
    };
    div()
        .flex()
        .flex_col()
        .child(row(header, "h", true).bg(colors.muted.opacity(0.8)))
        .children(rows.iter().enumerate().map(|(index, cells)| {
            row(cells, &index.to_string(), false)
                .border_t_1()
                .border_color(colors.border)
        }))
}

/// Streamdown's copy menu (Markdown, CSV, TSV), download menu (CSV, Markdown) and, outside
/// fullscreen, the fullscreen button.
pub fn table_controls(
    id: &str,
    header: &[Inline],
    rows: &[Vec<Inline>],
    style: &Style,
) -> gpui::Div {
    let fullscreen = id.ends_with("-fullscreen");
    let copy_id = format!("{id}-copy");
    let copied = style.copied.as_deref() == Some(copy_id.as_str());
    let open = |menu| {
        style
            .menu
            .as_ref()
            .is_some_and(|(open, kind)| open == id && *kind == menu)
    };
    let copy_formats = [
        ("Markdown", table_markdown(header, rows)),
        ("CSV", table_csv(header, rows)),
        ("TSV", table_tsv(header, rows)),
    ];
    let download_formats = [
        ("CSV", "table.csv", table_csv(header, rows)),
        ("Markdown", "table.md", table_markdown(header, rows)),
    ];
    let chat = style.chat.clone();
    let table_id = id.to_owned();
    let copy_items = copy_formats
        .into_iter()
        .enumerate()
        .map(|(index, (label, text))| {
            let (chat, copy_id) = (chat.clone(), copy_id.clone());
            menu_item(format!("{id}-copy-{index}"), label, style, move |cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                let copy_id = copy_id.clone();
                let _ = chat.update(cx, |chat, cx| chat.mark_copied(copy_id, cx));
            })
        })
        .collect::<Vec<_>>();
    let download_items = download_formats
        .into_iter()
        .enumerate()
        .map(|(index, (label, name, contents))| {
            let chat = chat.clone();
            menu_item(format!("{id}-download-{index}"), label, style, move |cx| {
                save(name.into(), contents.clone(), cx);
                let _ = chat.update(cx, |chat, cx| {
                    chat.markdown.menu = None;
                    cx.notify();
                });
            })
        })
        .collect::<Vec<_>>();
    let toggle_menu = |menu: TableMenu| {
        let (chat, table_id) = (chat.clone(), table_id.clone());
        move |cx: &mut gpui::App| {
            let _ = chat.update(cx, |chat, cx| {
                let current = chat.markdown.menu.take();
                if current != Some((table_id.clone(), menu)) {
                    chat.markdown.menu = Some((table_id.clone(), menu));
                }
                cx.notify();
            });
        }
    };
    let toggle = |menu: TableMenu| {
        let toggle = toggle_menu(menu);
        move |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut gpui::App| {
            cx.stop_propagation();
            toggle(cx);
        }
    };
    let press = |menu: TableMenu| {
        let toggle = toggle_menu(menu);
        move |_: Option<&gpui::accesskit::ActionData>, _: &mut Window, cx: &mut gpui::App| {
            toggle(cx)
        }
    };
    let header_cells = header.to_vec();
    let body_rows = rows.to_vec();
    let expand_chat = chat.clone();
    let expand_id = table_id.clone();
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .relative()
                .child(
                    control(
                        copy_id.clone(),
                        if copied {
                            "chat/CheckIcon.svg"
                        } else {
                            "chat/CopyIcon.svg"
                        },
                        style,
                    )
                    .aria_expanded(open(TableMenu::Copy))
                    .on_a11y_action(gpui::AccessibleAction::Click, press(TableMenu::Copy))
                    .on_mouse_down(MouseButton::Left, toggle(TableMenu::Copy)),
                )
                .when(open(TableMenu::Copy), |view| {
                    view.child(menu(copy_items, style))
                }),
        )
        .child(
            div()
                .relative()
                .child(
                    control(format!("{id}-download"), "chat/Download01Icon.svg", style)
                        .aria_expanded(open(TableMenu::Download))
                        .on_a11y_action(gpui::AccessibleAction::Click, press(TableMenu::Download))
                        .on_mouse_down(MouseButton::Left, toggle(TableMenu::Download)),
                )
                .when(open(TableMenu::Download), |view| {
                    view.child(menu(download_items, style))
                }),
        )
        .when(!fullscreen, |view| {
            view.child(
                control(
                    format!("{id}-fullscreen-open"),
                    "chat/ArrowExpandIcon.svg",
                    style,
                )
                .on_click(move |_, _, cx| {
                    let table = (
                        format!("{expand_id}-fullscreen"),
                        header_cells.clone(),
                        body_rows.clone(),
                    );
                    let _ = expand_chat.update(cx, |chat, cx| {
                        chat.markdown.menu = None;
                        chat.markdown.fullscreen = Some(table);
                        cx.notify();
                    });
                }),
            )
        })
}

fn menu(items: Vec<gpui::Stateful<gpui::Div>>, style: &Style) -> impl IntoElement {
    let colors = &style.colors;
    div().absolute().top_full().right_0().mt_1().child(deferred(
        anchored().anchor(gpui::Anchor::TopRight).child(
            div()
                .id("chat-table-menu")
                .role(gpui::Role::Menu)
                .occlude()
                .min_w(px(120.))
                .overflow_hidden()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .text_color(colors.foreground)
                .shadow_lg()
                .children(items),
        ),
    ))
}

fn menu_item(
    id: String,
    label: &'static str,
    style: &Style,
    choose: impl Fn(&mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let choose = Rc::new(choose);
    let press = choose.clone();
    div()
        .id(SharedString::from(id))
        .a11y_menu_item(label)
        .on_a11y_action(gpui::AccessibleAction::Click, move |_, _, cx| press(cx))
        .w_full()
        .px_3()
        .py_2()
        .text_sm()
        .line_height(px(20.))
        .cursor_pointer()
        .hover(|view| view.bg(style.colors.muted.opacity(0.4)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            choose(cx);
        })
        .child(label)
}

/// Streamdown's table and code-block frame.
fn frame(colors: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .p_2()
        .border_1()
        .border_color(colors.border)
}

/// Streamdown's block control: `p-1 text-muted-foreground hover:text-foreground`, disabled
/// while the reply streams.
fn control(id: String, path: &'static str, style: &Style) -> gpui::Stateful<gpui::Div> {
    let colors = &style.colors;
    let group = SharedString::from(id.clone());
    div()
        .id(SharedString::from(id))
        .a11y_button(match path {
            "chat/CopyIcon.svg" | "chat/CheckIcon.svg" => "Copy",
            "chat/Download01Icon.svg" => "Download",
            _ => "Open fullscreen",
        })
        .a11y_disabled(style.streaming)
        .group(group.clone())
        .p_1()
        .when(style.streaming, |view| view.opacity(0.5))
        .when(!style.streaming, |view| view.cursor_pointer())
        .child(
            svg()
                .path(path)
                .size(px(14.))
                .text_color(colors.muted_foreground)
                .when(!style.streaming, |icon| {
                    icon.group_hover(group, |style| style.text_color(colors.foreground))
                }),
        )
}

fn copy_button(id: &str, text: String, style: &Style) -> impl IntoElement {
    let copy_id = format!("{id}-copy");
    let copied = style.copied.as_deref() == Some(copy_id.as_str());
    let chat = style.chat.clone();
    control(
        copy_id.clone(),
        if copied {
            "chat/CheckIcon.svg"
        } else {
            "chat/CopyIcon.svg"
        },
        style,
    )
    .on_click(move |_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        let copy_id = copy_id.clone();
        let _ = chat.update(cx, |chat, cx| chat.mark_copied(copy_id, cx));
    })
}

fn download_button(id: &str, name: String, contents: String, style: &Style) -> impl IntoElement {
    control(format!("{id}-download"), "chat/Download01Icon.svg", style)
        .on_click(move |_, _, cx| save(name.clone(), contents.clone(), cx))
}

fn save(name: String, contents: String, cx: &mut gpui::App) {
    let directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let target = cx.prompt_for_new_path(&directory, Some(&name));
    cx.background_executor()
        .spawn(async move {
            if let Ok(Ok(Some(path))) = target.await
                && let Err(error) = std::fs::write(&path, contents)
            {
                tracing::error!(%error, "markdown download failed");
            }
        })
        .detach();
}

/// Approximates a browser's automatic table layout: each column starts at its
/// min-content width (headers never wrap, body cells wrap at words), and leftover space
/// goes to columns with longer content. Returns (minimum px, grow weight) per column.
fn column_widths(header: &[Inline], rows: &[Vec<Inline>]) -> Vec<(f32, f32)> {
    const CHAR: f32 = 8.;
    const PADDING: f32 = 32.;
    (0..header.len())
        .map(|column| {
            let cells = || rows.iter().filter_map(|row| row.get(column));
            let word = cells()
                .flat_map(|cell| cell.text.split_whitespace())
                .map(|word| word.chars().count())
                .chain(std::iter::once(header[column].text.chars().count()))
                .max()
                .unwrap_or(0);
            let longest = cells()
                .chain(std::iter::once(&header[column]))
                .map(|cell| cell.text.chars().count())
                .max()
                .unwrap_or(0);
            (word as f32 * CHAR + PADDING, longest as f32)
        })
        .collect()
}

/// A cell as the browser reads it for export: its visible text, trimmed.
fn cell(inline: &Inline) -> String {
    inline.text.replace(CODE_PAD, "").trim().to_owned()
}

/// Streamdown's `tableDataToMarkdown`: `\\` and `|` escaped, short rows padded.
fn table_markdown(header: &[Inline], rows: &[Vec<Inline>]) -> String {
    let escape = |text: String| text.replace('\\', "\\\\").replace('|', "\\|");
    let line = |cells: Vec<String>| format!("| {} |", cells.join(" | "));
    std::iter::once(line(header.iter().map(|text| escape(cell(text))).collect()))
        .chain(std::iter::once(line(vec!["---".into(); header.len()])))
        .chain(rows.iter().map(|row| {
            line(
                (0..header.len().max(row.len()))
                    .map(|index| {
                        row.get(index)
                            .map(|text| escape(cell(text)))
                            .unwrap_or_default()
                    })
                    .collect(),
            )
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Streamdown's `tableDataToCSV`: fields with a quote, comma or newline are quoted.
fn table_csv(header: &[Inline], rows: &[Vec<Inline>]) -> String {
    let field = |text: &Inline| {
        let text = cell(text);
        if text.contains('"') {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else if text.contains([',', '\n']) {
            format!("\"{text}\"")
        } else {
            text
        }
    };
    std::iter::once(header)
        .chain(rows.iter().map(Vec::as_slice))
        .map(|cells| cells.iter().map(field).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Streamdown's `tableDataToTSV`: tabs and line breaks written as escapes.
fn table_tsv(header: &[Inline], rows: &[Vec<Inline>]) -> String {
    let field = |text: &Inline| {
        cell(text)
            .replace('\t', "\\t")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    };
    std::iter::once(header)
        .chain(rows.iter().map(Vec::as_slice))
        .map(|cells| cells.iter().map(field).collect::<Vec<_>>().join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_element(text: &Inline, style: &Style, id: &str) -> AnyElement {
    let element = styled_text(text, style, id);
    let layout = element.1;
    let key = SharedString::from(id.to_owned());
    let painted_text = SharedString::from(text.text.clone());
    let texts = style.texts.clone();
    let selection = style.selection.clone();
    div()
        .relative()
        // Painted first, so the selection sits behind the text like the browser's.
        .child(
            canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    let mut texts = texts.borrow_mut();
                    paint_selection(&texts, &key, &layout, selection.as_ref(), window);
                    texts.push(Painted {
                        key: key.clone(),
                        text: painted_text.clone(),
                        layout: layout.clone(),
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(element.0)
        .into_any_element()
}

/// The webview's text selection colour as shipping renders it.
const SELECTION: u32 = 0xafd3fa;

/// Paints this element's share of the selection; elements painted earlier come first.
fn paint_selection(
    earlier: &[Painted],
    key: &SharedString,
    layout: &TextLayout,
    selection: Option<&(Caret, Caret)>,
    window: &mut Window,
) {
    let Some((anchor, head)) = selection else {
        return;
    };
    let seen = |caret: &Caret| earlier.iter().any(|painted| painted.key == caret.key);
    let len = layout.len();
    let range = match (anchor.key == *key, head.key == *key) {
        (true, true) => anchor.byte.min(head.byte)..anchor.byte.max(head.byte),
        (true, false) if seen(head) => 0..anchor.byte,
        (true, false) => anchor.byte..len,
        (false, true) if seen(anchor) => 0..head.byte,
        (false, true) => head.byte..len,
        (false, false) if seen(anchor) != seen(head) => 0..len,
        _ => return,
    };
    if range.is_empty() {
        return;
    }
    let (Some(a), Some(b)) = (
        layout.position_for_index(range.start),
        layout.position_for_index(range.end.min(len)),
    ) else {
        return;
    };
    let height = layout.line_height();
    let bounds = layout.bounds();
    let mut y = a.y;
    while y <= b.y {
        let left = if y == a.y { a.x } else { bounds.left() };
        let right = if y == b.y { b.x } else { bounds.right() };
        window.paint_quad(fill(
            gpui::Bounds::new(point(left, y), size((right - left).max(px(1.)), height)),
            gpui::rgb(SELECTION),
        ));
        y += height;
    }
}

fn styled_text(text: &Inline, style: &Style, id: &str) -> (AnyElement, TextLayout) {
    let highlights = text
        .spans
        .iter()
        .flat_map(|span| {
            // A code span's padding gets its own runs: GPUI only switches a font for runs
            // that lie wholly inside the override, so the code itself must be its own run.
            let pad = if span.code {
                CODE_PAD.len().min(span.range.len() / 2)
            } else {
                0
            };
            let (start, end) = (span.range.start, span.range.end);
            let ranges = if pad > 0 {
                vec![start..start + pad, start + pad..end - pad, end - pad..end]
            } else {
                vec![span.range.clone()]
            };
            let highlight = HighlightStyle {
                font_weight: if span.bold {
                    Some(FontWeight::SEMIBOLD)
                } else {
                    span.link.as_ref().map(|_| crate::ui::theme::MEDIUM)
                },
                font_style: span.italic.then_some(FontStyle::Italic),
                color: span.link.as_ref().map(|_| style.colors.primary),
                background_color: span.code.then_some(style.colors.muted),
                strikethrough: span.strike.then(|| StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                }),
                underline: span.link.is_some().then(|| UnderlineStyle {
                    thickness: px(1.),
                    color: None,
                    wavy: false,
                }),
                ..HighlightStyle::default()
            };
            ranges.into_iter().map(move |range| (range, highlight))
        })
        .collect::<Vec<_>>();
    let mono = text
        .spans
        .iter()
        .filter(|span| span.code)
        .map(|span| {
            let pad = CODE_PAD.len().min(span.range.len() / 2);
            (
                span.range.start + pad..span.range.end - pad,
                style.mono.clone(),
            )
        })
        .collect::<Vec<_>>();
    let styled = StyledText::new(SharedString::from(text.text.clone()))
        .with_highlights(highlights)
        .with_font_family_overrides(mono);
    let layout = styled.layout().clone();
    let links = text
        .spans
        .iter()
        .filter_map(|span| Some((span.range.clone(), span.link.clone()?)))
        .collect::<Vec<_>>();
    if links.is_empty() {
        return (styled.into_any_element(), layout);
    }
    let ranges = links.iter().map(|(range, _)| range.clone()).collect();
    let element = InteractiveText::new(SharedString::from(id.to_owned()), styled)
        .on_click(ranges, move |index, _, cx| {
            let url = &links[index].1;
            if opens_externally(url) {
                cx.open_url(url);
            }
        })
        .into_any_element();
    (element, layout)
}

/// Model-written links open only on the web or in mail, like the editor's: never `file:` or
/// another app's URL handler.
pub(crate) fn opens_externally(url: &str) -> bool {
    ["https://", "http://", "mailto:"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(range: Range<usize>) -> Span {
        Span {
            range,
            ..Span::default()
        }
    }

    #[test]
    fn parses_headings_lists_and_inline_marks() {
        let blocks = parse(
            "## Decisions\n\n- Ship **Friday**\n- Read [notes](https://x.test)\n\n1. `cargo test`\n",
        );
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Heading(2, text) if text.text == "Decisions"));
        let Block::List {
            ordered: false,
            items,
            ..
        } = &blocks[1]
        else {
            panic!("bullet list");
        };
        let Block::Paragraph(first) = &items[0].blocks[0] else {
            panic!("paragraph");
        };
        assert_eq!(first.text, "Ship Friday");
        assert_eq!(
            first.spans,
            vec![Span {
                bold: true,
                ..span(5..11)
            }]
        );
        let Block::Paragraph(second) = &items[1].blocks[0] else {
            panic!("paragraph");
        };
        assert_eq!(second.spans[0].link.as_deref(), Some("https://x.test"));
        let Block::List {
            ordered: true,
            start: 1,
            items,
        } = &blocks[2]
        else {
            panic!("ordered list");
        };
        let Block::Paragraph(code) = &items[0].blocks[0] else {
            panic!("paragraph");
        };
        assert!(code.spans[0].code);
    }

    #[test]
    fn splits_gfm_tables_out_of_prose_but_not_out_of_code_fences() {
        let blocks = parse(
            "Owners:\n\n| Task | Owner |\n|:---|---:|\n| Docs | **Ana** |\n| QA |\n\nDone.\n\n```\na | b\n--|--\n```\n",
        );
        assert!(matches!(&blocks[0], Block::Paragraph(text) if text.text == "Owners:"));
        let Block::Table { header, rows } = &blocks[1] else {
            panic!("table");
        };
        assert_eq!(header[1].text, "Owner");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1].text, "Ana");
        assert!(rows[0][1].spans[0].bold);
        assert_eq!(rows[1][1].text, "");
        assert!(matches!(&blocks[2], Block::Paragraph(text) if text.text == "Done."));
        assert!(matches!(&blocks[3], Block::Code { code, .. } if code == "a | b\n--|--"));
    }

    #[test]
    fn sibling_margins_collapse_like_streamdown() {
        let blocks = parse("Intro\n\n### Owners\n\n- one\n\n| a |\n|---|\n| b |\n\nAfter");
        assert!(matches!(blocks[3], Block::Table { .. }));
        assert_eq!(gaps(&blocks), vec![8., 4., 4., 0.]);
    }

    #[test]
    fn tables_copy_as_markdown_and_download_as_csv() {
        let Block::Table { header, rows } =
            &parse("| Task | Note |\n|---|---|\n| QA | a, \"b\" |\n")[0]
        else {
            panic!("table");
        };
        assert_eq!(
            table_markdown(header, rows),
            "| Task | Note |\n| --- | --- |\n| QA | a, \"b\" |"
        );
        assert_eq!(table_csv(header, rows), "Task,Note\nQA,\"a, \"\"b\"\"\"");
        assert_eq!(table_tsv(header, rows), "Task\tNote\nQA\ta, \"b\"");
        let widths = column_widths(header, rows);
        assert_eq!(widths[0], (4. * 8. + 32., 4.));
        assert!(widths[1].1 > widths[0].1);
    }

    #[test]
    fn chat_links_open_only_web_and_mail_targets() {
        for url in [
            "https://anarlog.so",
            "HTTP://example.com",
            "mailto:team@example.com",
        ] {
            assert!(opens_externally(url), "{url}");
        }
        for url in [
            "file:///etc/passwd",
            "x-apple.systempreferences:",
            "javascript:x",
            "anarlog://",
        ] {
            assert!(!opens_externally(url), "{url}");
        }
    }
}
