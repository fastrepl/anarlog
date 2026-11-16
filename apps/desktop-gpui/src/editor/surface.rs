use std::{ops::Range, sync::Arc};

use gpui::{
    Bounds, Context, FontStyle, FontWeight, IntoElement, Pixels, Point, SharedString,
    StrikethroughStyle, StyledText, TextLayout, TextRun, UnderlineStyle, Window, canvas, div, fill,
    point, prelude::*, px, size,
};
use serde_json::Value;

use super::{
    EditorPane,
    document::{Document, NodeRef, utf8},
    model::Selection,
    sequence::Measured,
};
use crate::ui::theme::theme;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockStyle {
    pub depth: usize,
    lists: usize,
    bullets: usize,
    marker: String,
    guides: Vec<usize>,
    parent_is_item: bool,
}

impl BlockStyle {
    fn indent(&self) -> usize {
        self.depth * 20 + self.lists * 4
    }

    fn child(&self, node: &NodeRef, index: usize) -> Self {
        let mut child = self.clone();
        if matches!(node.kind(), "bulletList" | "orderedList" | "taskList") {
            if self.parent_is_item {
                child.guides.push(self.indent());
            }
            child.lists += 1;
            child.depth += 1;
        } else if node.kind() == "blockquote" {
            child.depth += 1;
        }
        if node.kind() == "bulletList" {
            child.bullets += 1;
        }
        child.marker = match node.kind() {
            "orderedList" => format!(
                "{}.",
                node.attr("start").and_then(Value::as_u64).unwrap_or(1) + index as u64
            ),
            "bulletList" => "•".into(),
            "taskItem" => if node.task_done() { "[x]" } else { "[ ]" }.into(),
            _ if index == 0 => self.marker.clone(),
            _ => String::new(),
        };
        child.parent_is_item = matches!(node.kind(), "listItem" | "taskItem");
        child
    }
}

#[cfg(test)]
pub fn block_target(document: &Document, index: usize) -> Option<(NodeRef, usize, usize, String)> {
    block_layout(document, index)
        .map(|(node, start, style)| (node, start, style.depth, style.marker))
}

pub fn block_layout(document: &Document, mut index: usize) -> Option<(NodeRef, usize, BlockStyle)> {
    let mut node = document.root.clone();
    let mut start = 0;
    let mut style = BlockStyle::default();
    while node.projects_children() {
        let (child_index, remaining, child) = node.children.locate_render(index)?;
        style = style.child(&node, child_index);
        start += usize::from(node.kind() != "doc") + node.children.prefix(child_index);
        node = child.clone();
        index = remaining;
    }
    Some((node, start, style))
}

pub fn block_index(document: &Document, position: usize) -> Option<usize> {
    let resolved = document.resolve(position).ok()?;
    let mut node = document.root.clone();
    let mut index = 0;
    for child_index in resolved.path {
        if !node.projects_children() {
            break;
        }
        index += node.children.render_prefix(child_index);
        node = node.children.get(child_index)?.clone();
    }
    Some(index)
}

#[derive(Clone)]
pub struct Span {
    pub bytes: Range<usize>,
    pub position: usize,
    pub units: usize,
    pub atom: bool,
    pub marks: Vec<Value>,
    pub node: NodeRef,
}

#[derive(Clone)]
pub struct Row {
    pub id: u64,
    pub start: usize,
    pub text: SharedString,
    pub spans: Vec<Span>,
    pub kind: String,
    pub level: u8,
    pub style: BlockStyle,
}

impl Row {
    pub fn position(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        for span in &self.spans {
            if byte <= span.bytes.end {
                return self.start
                    + span.position
                    + if span.atom {
                        usize::from(byte > span.bytes.start + span.bytes.len() / 2) * span.units
                    } else {
                        self.text[span.bytes.start..byte.max(span.bytes.start)]
                            .encode_utf16()
                            .count()
                    };
            }
        }
        self.start
    }

    pub fn byte(&self, position: usize) -> Option<usize> {
        let offset = position.checked_sub(self.start)?;
        if self.spans.is_empty() {
            return (offset == 0).then_some(0);
        }
        for span in &self.spans {
            if (span.position..=span.position + span.units).contains(&offset) {
                return if span.atom {
                    Some(if offset == span.position {
                        span.bytes.start
                    } else {
                        span.bytes.end
                    })
                } else {
                    utf8(&self.text[span.bytes.clone()], offset - span.position)
                        .ok()
                        .map(|n| n + span.bytes.start)
                };
            }
        }
        None
    }
}

pub struct ProjectedBlock {
    pub source: NodeRef,
    pub style: BlockStyle,
    pub rows: Vec<Arc<Row>>,
    pub grid: Option<Grid>,
}

pub struct Grid {
    pub columns: u16,
    pub cells: Vec<GridCell>,
}

pub struct GridCell {
    pub row: i16,
    pub column: i16,
    pub colspan: u16,
    pub rowspan: u16,
    pub header: bool,
    pub rows: Range<usize>,
}

impl ProjectedBlock {
    #[cfg(test)]
    pub fn build(source: NodeRef) -> Self {
        Self::with_layout(source, BlockStyle::default())
    }

    #[cfg(test)]
    pub fn with_context(source: NodeRef, depth: usize, marker: String) -> Self {
        Self::with_layout(
            source,
            BlockStyle {
                depth,
                marker,
                ..Default::default()
            },
        )
    }

    pub fn with_layout(source: NodeRef, style: BlockStyle) -> Self {
        fn visit(node: &NodeRef, start: usize, style: BlockStyle, rows: &mut Vec<Arc<Row>>) {
            if node.is_textblock() {
                let mut text = String::new();
                let mut spans = Vec::new();
                let mut position = 0;
                node.children.visit(&mut |child| {
                    let begin = text.len();
                    if let Some(content) = &child.text {
                        text.push_str(&content.as_string());
                    } else if child.kind() == "hardBreak" {
                        text.push('\n');
                    } else {
                        let label = child
                            .attr("label")
                            .or_else(|| child.attr("name"))
                            .or_else(|| child.attr("alt"))
                            .and_then(Value::as_str)
                            .unwrap_or(child.kind());
                        text.push_str(&format!("[{label}]"));
                    }
                    spans.push(Span {
                        bytes: begin..text.len(),
                        position,
                        units: child.units(),
                        atom: child.text.is_none(),
                        marks: child.marks().to_vec(),
                        node: child.clone(),
                    });
                    position += child.units();
                });
                rows.push(Arc::new(Row {
                    id: node.id,
                    start: start + 1,
                    text: text.into(),
                    spans,
                    kind: node.kind().into(),
                    level: node.attr("level").and_then(Value::as_u64).unwrap_or(1) as u8,
                    style,
                }));
            } else if node.is_atom() || !node.known() {
                let label = node
                    .attr("name")
                    .or_else(|| node.attr("alt"))
                    .or_else(|| node.attr("label"))
                    .and_then(Value::as_str)
                    .unwrap_or(node.kind());
                let text: SharedString = if node.kind() == "horizontalRule" {
                    "────────────".into()
                } else if !node.known() {
                    format!("[Unsupported {} — preserved read-only]", node.kind()).into()
                } else {
                    format!("[{label}]").into()
                };
                rows.push(Arc::new(Row {
                    id: node.id,
                    start,
                    spans: vec![Span {
                        bytes: 0..text.len(),
                        position: 0,
                        units: node.units(),
                        atom: true,
                        marks: Vec::new(),
                        node: node.clone(),
                    }],
                    text,
                    kind: node.kind().into(),
                    level: 0,
                    style,
                }));
            } else {
                for index in 0..node.children.len() {
                    let child = node.children.get(index).expect("child");
                    visit(
                        child,
                        start + 1 + node.children.prefix(index),
                        style.child(node, index),
                        rows,
                    );
                }
            }
        }
        let mut rows = Vec::new();
        visit(&source, 0, style.clone(), &mut rows);
        let grid = if source.kind() == "table" {
            let mut cells = Vec::new();
            let mut occupied = std::collections::HashMap::new();
            let mut columns = 0;
            for ri in 0..source.children.len() {
                let row = source.children.get(ri).expect("row");
                let mut column = 0;
                for ci in 0..row.children.len() {
                    while occupied.get(&column).is_some_and(|until| *until > ri) {
                        column += 1;
                    }
                    let cell = row.children.get(ci).expect("cell");
                    let colspan = cell
                        .attr("colspan")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        .clamp(1, 1000) as u16;
                    let rowspan = cell
                        .attr("rowspan")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        .clamp(1, 1000) as u16;
                    let start = 2 + source.children.prefix(ri) + row.children.prefix(ci);
                    let end = start + cell.units();
                    let first = rows.partition_point(|row| row.start < start);
                    let last = rows.partition_point(|row| row.start < end);
                    cells.push(GridCell {
                        row: ri as i16 + 1,
                        column: column as i16 + 1,
                        colspan,
                        rowspan,
                        header: cell.kind() == "tableHeader",
                        rows: first..last,
                    });
                    for index in column..column + usize::from(colspan) {
                        occupied.insert(index, ri + usize::from(rowspan));
                    }
                    column += usize::from(colspan);
                    columns = columns.max(column);
                }
            }
            Some(Grid {
                columns: columns as u16,
                cells,
            })
        } else {
            None
        };
        Self {
            source,
            style,
            rows,
            grid,
        }
    }
}

#[derive(Clone)]
pub struct VisibleLayout {
    pub row: Arc<Row>,
    pub layout: TextLayout,
    pub block_start: usize,
}

impl VisibleLayout {
    pub fn position(&self, point: Point<Pixels>) -> usize {
        let byte = self
            .layout
            .index_for_position(point)
            .unwrap_or_else(|byte| byte);
        self.block_start + self.row.position(byte)
    }

    pub fn bounds_for(&self, range: Range<usize>) -> Option<Bounds<Pixels>> {
        let start = self.row.byte(range.start.checked_sub(self.block_start)?)?;
        let end = self.row.byte(range.end.checked_sub(self.block_start)?)?;
        let first = self.layout.position_for_index(start)?;
        let last = self.layout.position_for_index(end)?;
        Some(Bounds::new(
            first,
            size((last.x - first.x).max(px(1.)), self.layout.line_height()),
        ))
    }
}

pub fn render_row(
    row: Arc<Row>,
    block_start: usize,
    current: bool,
    in_cell: bool,
    window: &mut Window,
    cx: &mut Context<EditorPane>,
) -> gpui::AnyElement {
    let colors = theme(window);
    let mut base = window.text_style().to_run(0);
    base.color = colors.foreground;
    if row.style.marker == "[x]" {
        base.color.a *= 0.5;
        base.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        });
    }
    let mut runs = Vec::new();
    for span in &row.spans {
        let mut run = TextRun {
            len: span.bytes.len(),
            ..base.clone()
        };
        for mark in &span.marks {
            match mark.get("type").and_then(Value::as_str).unwrap_or("") {
                "bold" => run.font.weight = FontWeight::BOLD,
                "italic" => run.font.style = FontStyle::Italic,
                "code" => run.font.family = crate::ui::theme::monospace_font(cx),
                "link" | "underline" => {
                    run.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: None,
                        wavy: false,
                    });
                    if mark["type"] == "link" {
                        run.color = gpui::rgb(0x2563eb).into();
                    }
                }
                "strike" => {
                    run.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: None,
                    })
                }
                "highlight" => run.background_color = Some(gpui::rgba(0xfacc1540).into()),
                _ => {}
            }
        }
        if row.kind == "codeBlock" {
            run.font.family = crate::ui::theme::monospace_font(cx);
        }
        if row.kind == "heading" {
            run.font.weight = if row.level == 1 {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            };
        }
        runs.push(run);
    }
    let text = if row.text.is_empty() {
        " ".into()
    } else {
        row.text.clone()
    };
    if runs.is_empty() {
        runs.push(TextRun {
            len: text.len(),
            ..base
        });
    }
    let text = StyledText::new(text).with_runs(runs);
    let layout = text.layout().clone();
    let visible = VisibleLayout {
        row: row.clone(),
        layout: layout.clone(),
        block_start,
    };
    let entity = cx.entity();
    let font_size = if row.kind == "heading" {
        match row.level {
            1 => 20.,
            2 => 18.,
            _ => 16.,
        }
    } else if row.kind == "codeBlock" {
        14.
    } else {
        16.
    };
    let line_height = if row.kind == "heading" {
        match row.level {
            1 => 28.,
            2 => 26.,
            _ => 24.,
        }
    } else if row.kind == "codeBlock" {
        20.
    } else {
        24.
    };
    let inset = if in_cell { 0. } else { 12. };
    let indent = row.style.indent() as f32;
    let marker_left = inset + indent - 24.;
    div()
        .relative()
        .w_full()
        .min_h(px(line_height))
        .py(px(font_size * 0.125))
        .pl(px(inset + indent))
        .pr(px(if in_cell { 0. } else { 12. }))
        .text_size(px(font_size))
        .line_height(px(line_height))
        .when(row.kind == "codeBlock", |div| {
            div.bg(colors.muted).my_2().py_4().rounded_md()
        })
        .children(row.style.guides.iter().map(|offset| {
            div()
                .absolute()
                .left(px(inset + *offset as f32 - 16.5))
                .top_0()
                .bottom_0()
                .w(px(1.))
                .bg(colors.foreground.opacity(0.3))
        }))
        .when(!row.style.marker.is_empty(), |div| {
            let marker = row.style.marker.clone();
            if matches!(marker.as_str(), "[ ]" | "[x]") {
                div.child(
                    gpui::div()
                        .id(("task", row.id))
                        .absolute()
                        .left(px(marker_left))
                        .cursor_pointer()
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if let Some(model) = &mut this.model {
                                    let before = model.revision;
                                    let result = model.toggle_task(block_start + row.start);
                                    this.edited(before, result, cx);
                                }
                                cx.stop_propagation();
                            }),
                        )
                        .child(if marker == "[x]" { "☑" } else { "☐" }),
                )
            } else {
                let color = colors.foreground.opacity(0.65);
                if marker == "•" {
                    let variant = row.style.bullets.saturating_sub(1).min(5) % 3;
                    let diameter = font_size * if variant == 2 { 0.42 } else { 0.5 };
                    div.child(
                        gpui::div()
                            .absolute()
                            .left(px(marker_left + font_size * 0.5 - diameter / 2.))
                            .top(px(font_size * 0.875 - diameter / 2.))
                            .size(px(diameter))
                            .when_else(
                                variant == 2,
                                |view| view.rounded(px(font_size * 0.1)),
                                |view| view.rounded_full(),
                            )
                            .when_else(
                                variant == 1,
                                |view| view.border(px(1.5)).border_color(color),
                                |view| view.bg(color),
                            ),
                    )
                } else {
                    div.child(
                        gpui::div()
                            .absolute()
                            .left(px(marker_left))
                            .w(px(font_size))
                            .text_center()
                            .text_color(color)
                            .child(marker),
                    )
                }
            }
        })
        .child(text)
        .child(
            canvas(
                move |_, _, _| (),
                move |_, (), window, app| {
                    entity.update(app, |this, cx| {
                        if !current {
                            return;
                        }
                        if let Some(model) = &this.model {
                            paint_selection(
                                &visible,
                                model.selection,
                                this.focus.is_focused(window),
                                this.caret.read(cx).visible(window),
                                window,
                                colors.foreground,
                                colors.sidebar_accent,
                            );
                        }
                        this.layouts.insert(visible.row.id, visible);
                    });
                },
            )
            .absolute()
            .size_full()
            .top_0()
            .left_0(),
        )
        .into_any_element()
}

fn paint_selection(
    layout: &VisibleLayout,
    selection: Selection,
    focused: bool,
    caret_visible: bool,
    window: &mut Window,
    caret_color: gpui::Hsla,
    selection_color: gpui::Hsla,
) {
    if !focused || (selection.is_empty() && !caret_visible) {
        return;
    }
    let row_start = layout.block_start + layout.row.start;
    let row_end = layout.block_start + layout.row.position(layout.row.text.len());
    let selection = selection.range();
    if selection.end < row_start || selection.start > row_end {
        return;
    }
    let start = selection.start.max(row_start);
    let end = selection.end.min(row_end);
    let Some(a) = layout
        .row
        .byte(start - layout.block_start)
        .and_then(|byte| layout.layout.position_for_index(byte))
    else {
        return;
    };
    let Some(b) = layout
        .row
        .byte(end - layout.block_start)
        .and_then(|byte| layout.layout.position_for_index(byte))
    else {
        return;
    };
    let height = layout.layout.line_height();
    let bounds = layout.layout.bounds();
    if selection.is_empty() {
        window.paint_quad(fill(Bounds::new(a, size(px(1.), height)), caret_color));
    } else {
        let mut y = a.y;
        while y <= b.y {
            let left = if y == a.y { a.x } else { bounds.left() };
            let right = if y == b.y { b.x } else { bounds.right() };
            window.paint_quad(fill(
                Bounds::new(point(left, y), size((right - left).max(px(1.)), height)),
                selection_color.opacity(0.4),
            ));
            y += height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::model::EditorModel;
    use serde_json::json;

    #[test]
    fn virtual_rows_preserve_nested_guides_and_unicode_positions() {
        let raw = json!({"type":"doc","content":[
            {"type":"bulletList","content":[
                {"type":"listItem","content":[
                    {"type":"paragraph","content":[{"type":"text","text":"First α"}]}
                ]},
                {"type":"listItem","content":[
                    {"type":"paragraph","content":[{"type":"text","text":"Second 日本"}]},
                    {"type":"bulletList","content":[
                        {"type":"listItem","content":[
                            {"type":"paragraph","content":[{"type":"text","text":"Nested 🚀"}]},
                            {"type":"paragraph","content":[{"type":"text","text":"Continued"}]}
                        ]}
                    ]}
                ]}
            ]}
        ]})
        .to_string();
        let document = Document::parse(raw.clone().into()).unwrap();
        let whole = ProjectedBlock::build(document.root.children.get(0).unwrap().clone());
        assert_eq!(whole.rows.len(), 4);
        for (index, (indent, guides, marker, bullets)) in [
            (24, vec![], "•", 1),
            (24, vec![], "•", 1),
            (48, vec![24], "•", 2),
            (48, vec![24], "", 2),
        ]
        .into_iter()
        .enumerate()
        {
            let (node, start, style) = block_layout(&document, index).unwrap();
            assert_eq!(style.indent(), indent);
            assert_eq!(style.guides, guides);
            assert_eq!(style.marker, marker);
            assert_eq!(style.bullets, bullets);
            let block = ProjectedBlock::with_layout(node.clone(), style);
            let row = &block.rows[0];
            assert_eq!(row.style, whole.rows[index].style);
            assert_eq!(row.text, whole.rows[index].text);
            assert_eq!(start + row.start, whole.rows[index].start);
            let end = row.position(row.text.len());
            assert_eq!(row.byte(end), Some(row.text.len()));
            assert!(Arc::ptr_eq(
                &document.resolve(start + end).unwrap().node,
                &node
            ));
        }
        assert_eq!(document.serialize().unwrap().as_ref(), raw);
    }

    #[test]
    fn indentation_changes_cache_context_without_replacing_text_nodes() {
        let document = Document::parse(json!({"type":"doc","content":[
            {"type":"bulletList","content":[
                {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"First"}]}]},
                {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Second 🚀"}]}]}
            ]}
        ]}).to_string().into()).unwrap();
        let mut model = EditorModel::new(document);
        let (node, start, initial) = block_layout(&model.document, 1).unwrap();
        let cached = ProjectedBlock::with_layout(node.clone(), initial.clone());
        model.select(Selection::caret(start + 1));
        model.indent_list(false).unwrap();
        let (indented, _, style) = block_layout(&model.document, 1).unwrap();
        assert!(Arc::ptr_eq(&node, &indented));
        assert_ne!(cached.style, style);
        assert_eq!(style.indent(), 48);
        assert_eq!(style.bullets, 2);
        assert_eq!(style.guides, vec![24]);
        model.indent_list(true).unwrap();
        let (restored, _, style) = block_layout(&model.document, 1).unwrap();
        assert!(Arc::ptr_eq(&node, &restored));
        assert_eq!(style, initial);
    }

    #[test]
    fn bullet_styles_ignore_quote_and_ordered_list_depth() {
        let document = Document::parse(
            json!({"type":"doc","content":[
                {"type":"blockquote","content":[{"type":"bulletList","content":[
                    {"type":"listItem","content":[
                        {"type":"paragraph","content":[{"type":"text","text":"Bullet"}]},
                        {"type":"orderedList","attrs":{"start":4},"content":[
                            {"type":"listItem","content":[
                                {"type":"paragraph","content":[{"type":"text","text":"Ordered"}]},
                                {"type":"bulletList","content":[{"type":"listItem","content":[
                                    {"type":"paragraph","content":[{"type":"text","text":"Nested"}]}
                                ]}]}
                            ]}
                        ]}
                    ]}
                ]}]}
            ]})
            .to_string()
            .into(),
        )
        .unwrap();
        let (_, _, ordered) = block_layout(&document, 1).unwrap();
        assert_eq!(ordered.marker, "4.");
        let (_, _, nested) = block_layout(&document, 2).unwrap();
        assert_eq!(nested.depth, 4);
        assert_eq!(nested.bullets, 2);
        assert_eq!(nested.indent(), 92);
        assert_eq!(nested.guides, vec![44, 68]);
    }
}
