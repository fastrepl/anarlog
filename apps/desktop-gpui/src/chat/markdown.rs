//! Assistant markdown, rendered like shipping's Streamdown overrides in
//! `apps/desktop/src/chat/components/message/normal.tsx`. Parsing reuses the TipTap
//! converter; GFM tables, which it drops, are split out first.

use std::ops::Range;

use std::path::PathBuf;

use gpui::{
    AnyElement, ClipboardItem, FontStyle, FontWeight, HighlightStyle, InteractiveText,
    SharedString, StrikethroughStyle, StyledText, UnderlineStyle, div, prelude::*, px, svg,
};
use serde_json::Value;

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

pub struct Style {
    pub colors: Theme,
    pub mono: SharedString,
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
                .rounded(px(12.))
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
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors.border)
                            .child(download_button(
                                id,
                                format!("code.{extension}"),
                                code.clone(),
                                colors,
                            ))
                            .child(copy_button(id, code.clone(), colors)),
                    ),
                )
                .child(
                    div()
                        .p_4()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.background)
                        .font_family(style.mono.clone())
                        .child(SharedString::from(code)),
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
        Block::Table { header, rows } => {
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
            frame(colors)
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_1()
                        .child(copy_button(id, table_markdown(header, rows), colors))
                        .child(download_button(
                            id,
                            "table.csv".into(),
                            table_csv(header, rows),
                            colors,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.background)
                        .child(row(header, "h", true).bg(colors.muted.opacity(0.8)))
                        .children(rows.iter().enumerate().map(|(index, cells)| {
                            row(cells, &index.to_string(), false)
                                .border_t_1()
                                .border_color(colors.border)
                        })),
                )
                .into_any_element()
        }
    }
}

/// Streamdown's table and code-block frame.
fn frame(colors: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .p_2()
        .rounded(px(8.))
        .border_1()
        .border_color(colors.border)
}

fn action(id: String, path: &'static str, colors: &Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .p_1()
        .cursor_pointer()
        .child(
            svg()
                .path(path)
                .size(px(14.))
                .text_color(colors.muted_foreground),
        )
}

fn copy_button(id: &str, text: String, colors: &Theme) -> impl IntoElement {
    action(format!("{id}-copy"), "chat/CopyIcon.svg", colors)
        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

fn download_button(id: &str, name: String, contents: String, colors: &Theme) -> impl IntoElement {
    action(format!("{id}-download"), "chat/Download01Icon.svg", colors).on_click(move |_, _, cx| {
        let directory = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let target = cx.prompt_for_new_path(&directory, Some(&name));
        let csv = contents.clone();
        cx.background_executor()
            .spawn(async move {
                if let Ok(Ok(Some(path))) = target.await
                    && let Err(error) = std::fs::write(&path, csv)
                {
                    tracing::error!(%error, "markdown download failed");
                }
            })
            .detach();
    })
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

fn table_markdown(header: &[Inline], rows: &[Vec<Inline>]) -> String {
    let line = |cells: &[Inline]| {
        format!(
            "| {} |",
            cells
                .iter()
                .map(|cell| cell.text.replace('|', "\\|"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    std::iter::once(line(header))
        .chain(std::iter::once(format!(
            "| {} |",
            vec!["---"; header.len()].join(" | ")
        )))
        .chain(rows.iter().map(|row| line(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn table_csv(header: &[Inline], rows: &[Vec<Inline>]) -> String {
    std::iter::once(header)
        .chain(rows.iter().map(Vec::as_slice))
        .map(|cells| {
            cells
                .iter()
                .map(|cell| {
                    if cell.text.contains([',', '"', '\n']) {
                        format!("\"{}\"", cell.text.replace('"', "\"\""))
                    } else {
                        cell.text.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_element(text: &Inline, style: &Style, id: &str) -> AnyElement {
    let highlights = text
        .spans
        .iter()
        .map(|span| {
            (
                span.range.clone(),
                HighlightStyle {
                    font_weight: if span.bold {
                        Some(FontWeight::SEMIBOLD)
                    } else {
                        span.link.as_ref().map(|_| FontWeight::MEDIUM)
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
                },
            )
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
    let links = text
        .spans
        .iter()
        .filter_map(|span| Some((span.range.clone(), span.link.clone()?)))
        .collect::<Vec<_>>();
    if links.is_empty() {
        return styled.into_any_element();
    }
    let ranges = links.iter().map(|(range, _)| range.clone()).collect();
    InteractiveText::new(SharedString::from(id.to_owned()), styled)
        .on_click(ranges, move |index, _, cx| cx.open_url(&links[index].1))
        .into_any_element()
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
        let widths = column_widths(header, rows);
        assert_eq!(widths[0], (4. * 8. + 32., 4.));
        assert!(widths[1].1 > widths[0].1);
    }
}
