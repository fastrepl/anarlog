//! The chat composer: shipping's `ChatEditor` (`packages/editor/src/chat`) on the native
//! editor model. Paragraphs hold text and inline `@` mention chips; Enter sends, Shift+Enter
//! starts a new paragraph, and the arrow keys at the draft's edges recall sent prompts.

use std::{ops::Range, sync::Arc};

use desktop_runtime::{CancellationToken, RuntimeHandle};
use gpui::{
    Anchor, AnyElement, App, AvailableSpace, Bounds, ClipboardItem, Context, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    FontWeight, GlobalElementId, InspectorElementId, IntoElement, KeyDownEvent, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ScrollHandle, SharedString,
    StyledText, TextRun, UTF16Selection, Window, anchored, canvas, deferred, div, fill, point,
    prelude::*, px, rgb, size, svg,
};
use serde_json::{Map, Value, json};

use super::search::{MentionItem, MentionKind, SearchIndex};
use crate::{
    editor::{
        document::{Document, NodeRef},
        model::{EditorModel, Selection},
        surface::{BlockStyle, Row, Span, VisibleLayout},
    },
    ui::{caret::Caret, theme::theme},
};

pub enum ComposerEvent {
    /// The draft changed through typing; history recalls and resets do not emit it.
    Changed,
    Submit,
    /// Up at the start (`true`) or down at the end of the draft asks for a sent prompt.
    Recall(bool),
}

/// A reference the next message carries, from an `@` mention in the draft.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextRef {
    pub kind: MentionKind,
    pub id: String,
}

impl ContextRef {
    /// Shipping's `ContextRef` with `source: "manual"`.
    pub fn to_json(&self) -> Value {
        let field = match self.kind {
            MentionKind::Session => "sessionId",
            MentionKind::Human => "humanId",
            MentionKind::Organization => "organizationId",
            MentionKind::Folder => "folderId",
        };
        json!({
            "kind": self.kind.key(),
            "key": format!("{}:manual:{}", self.kind.key(), self.id),
            "source": "manual",
            field: self.id,
        })
    }
}

struct Mention {
    start: usize,
    query: String,
    selected: usize,
    items: Vec<MentionItem>,
    generation: u64,
}

pub struct Composer {
    runtime: RuntimeHandle,
    index: SearchIndex,
    model: EditorModel,
    rows: Vec<Arc<Row>>,
    layouts: Vec<VisibleLayout>,
    focus: FocusHandle,
    caret: Option<Entity<Caret>>,
    scroll: ScrollHandle,
    reveal: bool,
    mention: Option<Mention>,
    dismissed: Option<usize>,
    generation: u64,
    search: CancellationToken,
    dragging: bool,
    /// Dropped images' thumbnails, by attachment id.
    thumbnails: std::collections::HashMap<String, Arc<gpui::Image>>,
    /// Room kept clear for the send controls drawn over the draft's right edge.
    pub padding_right: f32,
    /// Paragraph bottom margin: 4px in the right panel; the floating composer removes it.
    pub paragraph_gap: f32,
    pub max_height: f32,
    pub placeholder: SharedString,
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Focusable for Composer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub fn empty_doc() -> Value {
    json!({"type": "doc", "content": [{"type": "paragraph"}]})
}

fn model(doc: &Value) -> EditorModel {
    let document = Document::parse(doc.to_string().into())
        .or_else(|_| Document::parse(empty_doc().to_string().into()))
        .expect("an empty chat document parses");
    EditorModel::new(document)
}

impl Composer {
    pub fn new(runtime: RuntimeHandle, index: SearchIndex, cx: &mut Context<Self>) -> Self {
        let model = model(&empty_doc());
        let rows = project(&model);
        Self {
            runtime,
            index,
            model,
            rows,
            layouts: Vec::new(),
            focus: cx.focus_handle(),
            caret: None,
            scroll: ScrollHandle::new(),
            reveal: false,
            mention: None,
            dismissed: None,
            generation: 0,
            search: CancellationToken::new(),
            dragging: false,
            thumbnails: Default::default(),
            padding_right: 0.,
            paragraph_gap: 0.,
            max_height: 144.,
            placeholder: "Ask anything".into(),
        }
    }

    pub fn doc(&self) -> Value {
        self.model.document.root.value()
    }

    /// Replaces the draft, placing the caret at its start or end, like `replaceContent`.
    pub fn set_doc(&mut self, doc: &Value, at_start: bool, cx: &mut Context<Self>) {
        self.model = model(doc);
        self.model.move_document_boundary(!at_start, false);
        self.refresh(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_doc(&empty_doc(), true, cx);
    }

    /// The draft as sent: text with mentions as `@label`, one line per paragraph.
    pub fn text(&self) -> String {
        doc_text(&self.doc())
    }

    pub fn has_text(&self) -> bool {
        !self.text().trim().is_empty()
    }

    pub fn context_refs(&self) -> Vec<ContextRef> {
        context_refs(&self.doc())
    }

    /// Dictation's `insertText`: spaced from its neighbours, replacing the selection.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let value = text.trim();
        if value.is_empty() {
            return;
        }
        let range = self.model.selection.range();
        let before = self.char_before(range.start);
        let after = self.char_after(range.end);
        let mut insertion = String::new();
        if before.is_some_and(|c| !c.is_whitespace()) {
            insertion.push(' ');
        }
        insertion.push_str(value);
        if after.is_some_and(|c| !c.is_whitespace()) {
            insertion.push(' ');
        }
        let _ = self.model.replace(range, &insertion);
        self.edited(cx);
    }

    fn char_before(&self, position: usize) -> Option<char> {
        let row = self.row_at(position)?;
        let byte = row.byte(position)?;
        row.text[..byte].chars().next_back()
    }

    fn char_after(&self, position: usize) -> Option<char> {
        let row = self.row_at(position)?;
        let byte = row.byte(position)?;
        row.text[byte..].chars().next()
    }

    fn row_at(&self, position: usize) -> Option<&Arc<Row>> {
        self.rows.iter().find(|row| row.byte(position).is_some())
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.rows = project(&self.model);
        self.reveal = true;
        if let Some(caret) = &self.caret {
            Caret::reset_entity(caret, cx);
        }
        self.update_mention(cx);
        cx.notify();
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
        cx.emit(ComposerEvent::Changed);
    }

    fn apply(
        &mut self,
        edit: impl FnOnce(&mut EditorModel) -> Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        let revision = self.model.revision;
        let _ = edit(&mut self.model);
        if self.model.revision != revision {
            self.edited(cx);
        } else {
            self.refresh(cx);
        }
    }

    /// Shipping's `findMention`: the last "@" before the caret that starts the paragraph or
    /// follows whitespace, with no whitespace after it.
    fn find_mention(&self) -> Option<(usize, String)> {
        let selection = self.model.selection;
        if !selection.is_empty() || self.model.composing() {
            return None;
        }
        let row = self.row_at(selection.head)?;
        let caret = selection.head - row.start;
        let mut before = String::new();
        for span in &row.spans {
            if span.position >= caret {
                break;
            }
            if span.atom {
                before.push('\u{fffc}');
            } else {
                let units = (caret - span.position).min(span.units);
                let text = &row.text[span.bytes.clone()];
                let end = crate::editor::document::utf8(text, units).ok()?;
                before.push_str(&text[..end]);
            }
        }
        let at = before.rfind('@')?;
        if before[..at]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace())
        {
            return None;
        }
        let query = &before[at + 1..];
        if query.chars().any(char::is_whitespace) {
            return None;
        }
        Some((
            row.start + before[..at].encode_utf16().count(),
            query.into(),
        ))
    }

    fn update_mention(&mut self, cx: &mut Context<Self>) {
        let Some((start, query)) = self.find_mention() else {
            self.dismissed = None;
            self.close_mention();
            return;
        };
        if self.dismissed == Some(start) {
            self.close_mention();
            return;
        }
        if self
            .mention
            .as_ref()
            .is_some_and(|mention| mention.start == start && mention.query == query)
        {
            return;
        }
        self.search.cancel();
        self.search = CancellationToken::new();
        self.generation += 1;
        let generation = self.generation;
        let items = self
            .mention
            .take()
            .filter(|mention| mention.start == start)
            .map(|mention| mention.items)
            .unwrap_or_default();
        self.mention = Some(Mention {
            start,
            query: query.clone(),
            selected: 0,
            items,
            generation,
        });
        let search =
            super::search::mentions(&self.index, &self.runtime, query, self.search.clone());
        cx.spawn(async move |this, cx| {
            let items = search.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(mention) = &mut this.mention
                    && mention.generation == generation
                {
                    // A failed search shows no suggestions, like shipping's catch.
                    mention.items = items.unwrap_or_default();
                    mention.selected = 0;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn close_mention(&mut self) {
        if self.mention.take().is_some() {
            self.search.cancel();
        }
    }

    fn mention_open(&self) -> bool {
        self.mention
            .as_ref()
            .is_some_and(|mention| !mention.items.is_empty())
    }

    fn insert_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(mention) = &self.mention else {
            return;
        };
        let Some(item) = mention.items.get(index).cloned() else {
            return;
        };
        let start = mention.start;
        self.close_mention();
        let head = self.model.selection.head;
        self.apply(
            |model| {
                model.transaction(|model| {
                    model.select(Selection {
                        anchor: start,
                        head,
                    });
                    model.insert_inline_atom(
                        "mention-@",
                        Map::from_iter([
                            ("id".into(), json!(item.id)),
                            ("type".into(), json!(item.kind.key())),
                            ("label".into(), json!(item.label)),
                        ]),
                    )?;
                    let caret = model.selection.head;
                    model.replace(caret..caret, " ")
                })
            },
            cx,
        );
    }

    fn hit(&self, position: Point<Pixels>) -> Option<usize> {
        let contains = |layout: &&VisibleLayout| {
            let bounds = layout.layout.bounds();
            position.y >= bounds.top() && position.y < bounds.bottom()
        };
        self.layouts
            .iter()
            .find(contains)
            .or_else(|| {
                self.layouts.iter().min_by(|a, b| {
                    let distance = |layout: &VisibleLayout| {
                        let bounds = layout.layout.bounds();
                        (f32::from(bounds.center().y) - f32::from(position.y)).abs()
                    };
                    distance(a).total_cmp(&distance(b))
                })
            })
            .map(|layout| layout.position(position))
    }

    fn caret_bounds(&self, position: usize) -> Option<Bounds<Pixels>> {
        self.layouts
            .iter()
            .find_map(|layout| layout.bounds_for(position..position))
    }

    fn at_start(&self) -> bool {
        self.model.selection.is_empty()
            && self.model.selection.head == self.model.document.edge_caret(false)
    }

    fn at_end(&self) -> bool {
        self.model.selection.is_empty()
            && self.model.selection.head == self.model.document.edge_caret(true)
    }

    /// Moves to the row above or below by pixel position; `None` when there is none.
    fn vertical(&self, down: bool) -> Option<usize> {
        let caret = self.caret_bounds(self.model.selection.head)?;
        let target = point(
            caret.left(),
            caret.center().y + caret.size.height * if down { 1. } else { -1. },
        );
        self.layouts
            .iter()
            .find(|layout| {
                let bounds = layout.layout.bounds();
                target.y >= bounds.top() && target.y < bounds.bottom()
            })
            .map(|layout| layout.position(target))
    }

    fn line_edge(&self, end: bool) -> Option<usize> {
        let caret = self.caret_bounds(self.model.selection.head)?;
        let layout = self.layouts.iter().find(|layout| {
            layout
                .bounds_for(self.model.selection.head..self.model.selection.head)
                .is_some()
        })?;
        let bounds = layout.layout.bounds();
        Some(layout.position(point(
            if end {
                bounds.right() + px(10.)
            } else {
                bounds.left() - px(10.)
            },
            caret.center().y,
        )))
    }

    fn move_to(&mut self, position: usize, extend: bool, cx: &mut Context<Self>) {
        let anchor = if extend {
            self.model.selection.anchor
        } else {
            position
        };
        self.model.select(Selection {
            anchor,
            head: position,
        });
        self.refresh(cx);
    }

    fn selected_text(&self) -> String {
        let range = self.model.selection.range();
        text_between(&self.rows, range)
    }

    fn paste(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.replace("\r\n", "\n");
        self.apply(
            |model| {
                model.transaction(|model| {
                    let range = model.selection.range();
                    model.replace(range, "")?;
                    for (index, line) in text.split('\n').enumerate() {
                        if index > 0 {
                            model.split_block()?;
                        }
                        if !line.is_empty() {
                            let caret = model.selection.head;
                            model.replace(caret..caret, line)?;
                        }
                    }
                    Ok(())
                })
            },
            cx,
        );
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let secondary = modifiers.secondary(cx);
        if let Some(caret) = &self.caret {
            Caret::reset_entity(caret, cx);
        }
        if self.mention_open() {
            let mention = self.mention.as_mut().expect("open mention");
            let len = mention.items.len();
            match key {
                "up" | "down" => {
                    mention.selected =
                        (mention.selected + if key == "down" { 1 } else { len - 1 }) % len;
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "escape" => {
                    self.dismissed = Some(mention.start);
                    self.close_mention();
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    let selected = mention.selected;
                    self.insert_mention(selected, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if self.model.composing() {
            return;
        }
        let shift = modifiers.shift;
        let handled = match key {
            "enter" if shift => {
                self.apply(EditorModel::split_block, cx);
                true
            }
            "enter" => {
                // An open "@" query swallows Enter, like shipping's submit command.
                if self.find_mention().is_none() {
                    cx.emit(ComposerEvent::Submit);
                }
                true
            }
            "up" | "down" if !secondary && !modifiers.alt => {
                let down = key == "down";
                if !shift
                    && self.find_mention().is_none()
                    && (if down { self.at_end() } else { self.at_start() })
                {
                    cx.emit(ComposerEvent::Recall(!down));
                } else {
                    let target = self
                        .vertical(down)
                        .unwrap_or_else(|| self.model.document.edge_caret(down));
                    self.move_to(target, shift, cx);
                }
                true
            }
            "up" | "down" | "home" | "end"
                if secondary || matches!(key, "home" | "end") && modifiers.control =>
            {
                self.model
                    .move_document_boundary(matches!(key, "down" | "end"), shift);
                self.refresh(cx);
                true
            }
            "left" | "right" if secondary => {
                if let Some(target) = self.line_edge(key == "right") {
                    self.move_to(target, shift, cx);
                }
                true
            }
            "home" | "end" => {
                if let Some(target) = self.line_edge(key == "end") {
                    self.move_to(target, shift, cx);
                }
                true
            }
            "left" | "right" if modifiers.alt => {
                self.apply(|model| model.move_word(key == "right", shift), cx);
                true
            }
            "left" | "right" => {
                self.apply(|model| model.move_grapheme(key == "right", shift), cx);
                true
            }
            "backspace" if secondary => {
                if self.model.selection.is_empty()
                    && let Some(start) = self.line_edge(false)
                {
                    let head = self.model.selection.head;
                    self.model.select(Selection {
                        anchor: head,
                        head: start,
                    });
                }
                self.apply(|model| model.replace(model.selection.range(), ""), cx);
                true
            }
            "backspace" | "delete" if modifiers.alt => {
                self.apply(
                    |model| {
                        if model.selection.is_empty() {
                            model.move_word(key == "delete", true)?;
                        }
                        model.replace(model.selection.range(), "")
                    },
                    cx,
                );
                true
            }
            "backspace" | "delete" => {
                self.apply(|model| model.delete(key == "delete"), cx);
                true
            }
            "a" if secondary => {
                let end = self.model.document.units();
                self.model.select(Selection {
                    anchor: 0,
                    head: end,
                });
                self.refresh(cx);
                true
            }
            "z" if secondary && shift => {
                self.apply(EditorModel::redo, cx);
                true
            }
            "z" if secondary => {
                self.apply(EditorModel::undo, cx);
                true
            }
            "y" if secondary && !cfg!(target_os = "macos") => {
                self.apply(EditorModel::redo, cx);
                true
            }
            // With nothing selected here, the chat copies its message selection instead.
            "c" | "x" if secondary && !self.model.selection.is_empty() => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text()));
                if key == "x" {
                    self.apply(|model| model.replace(model.selection.range(), ""), cx);
                }
                true
            }
            "v" if secondary => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste(&text, cx);
                }
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    /// Shipping's chat `fileHandlerPlugin` drop: each file becomes an attachment chip at the
    /// caret, images (8 MB at most) carrying their data URL.
    fn drop_files(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        let job = cx.background_executor().spawn(async move {
            let mut too_large = false;
            let files = paths
                .into_iter()
                .filter_map(|path| {
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    let mime = mime_guess::from_path(&path)
                        .first_raw()
                        .unwrap_or_default()
                        .to_owned();
                    let size = std::fs::metadata(&path).ok()?.len();
                    let image = mime.starts_with("image/");
                    if image && size > MAX_IMAGE_BYTES {
                        too_large = true;
                        return None;
                    }
                    let bytes = if image {
                        Some(std::fs::read(&path).ok()?)
                    } else {
                        None
                    };
                    Some((name, mime, size, bytes))
                })
                .collect::<Vec<_>>();
            (files, too_large)
        });
        cx.spawn(async move |this, cx| {
            let (files, too_large) = job.await;
            let _ = this.update(cx, |this, cx| {
                if too_large {
                    crate::application::toasts::error("Images must be 8 MB or smaller.", cx);
                }
                for (name, mime, size, bytes) in files {
                    this.insert_attachment(name, mime, size, bytes, cx);
                }
            });
        })
        .detach();
    }

    fn insert_attachment(
        &mut self,
        name: String,
        mime: String,
        size: u64,
        bytes: Option<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        use base64::Engine;
        let id = uuid::Uuid::new_v4().to_string();
        let url = bytes.map(|bytes| {
            let url = format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            );
            if let Some(format) = gpui::ImageFormat::from_mime_type(&mime) {
                self.thumbnails
                    .insert(id.clone(), Arc::new(gpui::Image::from_bytes(format, bytes)));
            }
            url
        });
        let attrs = Map::from_iter([
            ("id".into(), json!(id)),
            ("name".into(), json!(name)),
            ("mimeType".into(), json!(mime)),
            ("url".into(), json!(url)),
            ("size".into(), json!(size)),
        ]);
        self.apply(
            |model| {
                model.transaction(|model| {
                    model.insert_inline_atom("attachment", attrs)?;
                    let head = model.selection.head;
                    model.replace(head..head, " ")
                })
            },
            cx,
        );
    }

    /// Shipping's `AttachmentChipView`: a file icon or image thumbnail, the name, and a remove
    /// button.
    fn attachment_chip(
        &self,
        node: &NodeRef,
        position: usize,
        colors: &crate::ui::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let attr = |key| node.attr(key).and_then(Value::as_str).unwrap_or_default();
        let thumbnail = self.thumbnails.get(attr("id")).cloned();
        div()
            .mt(px(-1.))
            .flex()
            .items_center()
            .gap_1()
            .px(px(6.))
            .py(px(2.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.muted)
            .text_xs()
            .line_height(px(16.))
            .text_color(colors.muted_foreground)
            .child(match thumbnail {
                Some(image) => gpui::img(image)
                    .size(px(16.))
                    .flex_shrink_0()
                    .object_fit(gpui::ObjectFit::Cover)
                    .into_any_element(),
                None => svg()
                    .path("editor/File01Icon.svg")
                    .size(px(12.))
                    .flex_shrink_0()
                    .text_color(colors.muted_foreground)
                    .into_any_element(),
            })
            .child(
                div()
                    .max_w(px(120.))
                    .truncate()
                    .child(attachment_label(attr("name"))),
            )
            .child(
                div()
                    .id(("chat-attachment-remove", position))
                    .flex_shrink_0()
                    .p(px(2.))
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.apply(|model| model.replace(position..position + 1, ""), cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        svg()
                            .path("editor/XIcon.svg")
                            .size(px(10.))
                            .text_color(colors.muted_foreground),
                    ),
            )
            .into_any_element()
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let Some(position) = self.hit(event.position) else {
            return;
        };
        match event.click_count {
            2 => {
                let _ = self.model.select_word_at(position);
            }
            3.. => {
                let _ = self.model.select_block_at(position);
            }
            _ => {
                self.model.select(Selection {
                    anchor: if event.modifiers.shift {
                        self.model.selection.anchor
                    } else {
                        position
                    },
                    head: position,
                });
                self.dragging = true;
            }
        }
        self.refresh(cx);
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.dragging = false;
            return;
        }
        if self.dragging
            && let Some(position) = self.hit(event.position)
            && position != self.model.selection.head
        {
            let anchor = self.model.selection.anchor;
            self.model.select(Selection {
                anchor,
                head: position,
            });
            self.refresh(cx);
        }
    }

    fn row(
        &self,
        index: usize,
        row: &Arc<Row>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let base = window.text_style().to_run(0);
        let runs = if row.spans.is_empty() {
            vec![TextRun {
                len: 1,
                ..base.clone()
            }]
        } else {
            row.spans
                .iter()
                .map(|span| {
                    let mut run = TextRun {
                        len: span.bytes.len(),
                        ..base.clone()
                    };
                    if span.node.kind() == "mention-@" {
                        run.font.weight = crate::ui::theme::MEDIUM;
                    }
                    // The chip is drawn over its reserved text.
                    if span.node.kind() == "attachment" {
                        run.color = gpui::transparent_black();
                    }
                    run
                })
                .collect()
        };
        let text = if row.text.is_empty() {
            SharedString::from(" ")
        } else {
            row.text.clone()
        };
        let atoms = row
            .spans
            .iter()
            .filter_map(|span| match span.node.kind() {
                "mention-@" => Some((span.bytes.start, mention_avatar(&span.node, &colors))),
                "attachment" => Some((
                    span.bytes.start,
                    self.attachment_chip(&span.node, row.start + span.position, &colors, cx),
                )),
                _ => None,
            })
            .collect();
        let styled = StyledText::new(text).with_runs(runs);
        let layout = styled.layout().clone();
        let entity = cx.entity();
        let row = row.clone();
        let placeholder = (index == 0 && row.text.is_empty()).then(|| self.placeholder.clone());
        div()
            .relative()
            .w_full()
            .mb(px(self.paragraph_gap))
            .when_some(placeholder, |view, placeholder| {
                view.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .text_color(colors.muted_foreground)
                        .child(placeholder),
                )
            })
            .child(InlineAtoms {
                text: styled,
                atoms,
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, cx| {
                        let visible = VisibleLayout {
                            row: row.clone(),
                            layout: layout.clone(),
                            block_start: 0,
                        };
                        entity.update(cx, |this, cx| {
                            let focused = this.focus.is_focused(window);
                            let caret_on = this
                                .caret
                                .as_ref()
                                .is_some_and(|caret| caret.read(cx).visible(window));
                            paint_selection(
                                &visible,
                                this.model.selection,
                                focused,
                                caret_on,
                                colors.card_foreground,
                                colors.sidebar_accent,
                                window,
                            );
                            this.layouts.push(visible);
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    fn mention_menu(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mention = self
            .mention
            .as_ref()
            .filter(|mention| !mention.items.is_empty())?;
        let anchor = self.caret_bounds(mention.start)?;
        let colors = theme(window);
        let height = mention.items.len() as f32 * 32. + 10.;
        let below =
            f32::from(anchor.bottom()) + 4. + height <= f32::from(window.viewport_size().height);
        let (position, corner) = if below {
            (
                point(anchor.left(), anchor.bottom() + px(4.)),
                Anchor::TopLeft,
            )
        } else {
            (
                point(anchor.left(), anchor.top() - px(4.)),
                Anchor::BottomLeft,
            )
        };
        let items = mention.items.iter().enumerate().map(|(index, item)| {
            let path = match item.kind {
                MentionKind::Session => "editor/NoteIcon.svg",
                MentionKind::Human => "editor/UserIcon.svg",
                MentionKind::Organization => "editor/Building03Icon.svg",
                MentionKind::Folder => "chat/Folder01Icon.svg",
            };
            div()
                .id(("chat-mention-option", index))
                .w_full()
                .h(px(32.))
                .px_2()
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .text_sm()
                .when(index == mention.selected, |view| view.bg(colors.muted))
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if let Some(mention) = &mut this.mention
                        && mention.selected != index
                    {
                        mention.selected = index;
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.insert_mention(index, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(
                    svg()
                        .path(path)
                        .size(px(16.))
                        .flex_none()
                        .text_color(colors.muted_foreground),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .line_height(px(16.))
                        .child(SharedString::from(item.label.clone())),
                )
        });
        Some(
            deferred(
                anchored()
                    .position(position)
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("chat-mention-menu")
                            .occlude()
                            .max_w(px(280.))
                            .p_1()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.popover)
                            .text_color(colors.popover_foreground)
                            .shadow(vec![shadow(10., 15., -3.), shadow(4., 6., -4.)])
                            .children(items),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

fn shadow(y: f32, blur: f32, spread: f32) -> gpui::BoxShadow {
    gpui::BoxShadow {
        color: gpui::black().opacity(0.1),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

/// Shipping's `FACEHASH_BG_CLASSES`, picked by facehash's `stringHash`.
const FACEHASH_BACKGROUNDS: [u32; 10] = [
    0xfffbeb, 0xfff1f2, 0xf5f3ff, 0xeff6ff, 0xf0fdfa, 0xf0fdf4, 0xecfeff, 0xfdf4ff, 0xeef2ff,
    0xfefce8,
];

fn string_hash(value: &str) -> u32 {
    let hash = value.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit))
    });
    hash.unsigned_abs()
}

/// The chip's 1em avatar: a facehash tile for people, the type icon otherwise.
fn mention_avatar(node: &NodeRef, colors: &crate::ui::theme::Theme) -> AnyElement {
    let attr = |key| node.attr(key).and_then(Value::as_str).unwrap_or_default();
    let tile = div()
        .size(px(14.))
        .mt(px(2.))
        .flex()
        .items_center()
        .justify_center();
    match attr("type") {
        "human" => {
            let name = if attr("label").is_empty() {
                attr("id")
            } else {
                attr("label")
            };
            let name = if name.is_empty() { "?" } else { name };
            let background =
                FACEHASH_BACKGROUNDS[string_hash(name) as usize % FACEHASH_BACKGROUNDS.len()];
            // Facehash's face at chip size: two eyes over a small initial.
            let ink = gpui::Hsla::from(rgb(0x0c0a09));
            let eye = |left: f32| {
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(4.))
                    .w(px(3.))
                    .h(px(1.))
                    .bg(ink.opacity(0.7))
            };
            tile.relative()
                .bg(rgb(background))
                .child(eye(5.5))
                .child(eye(10.5))
                .child(
                    div()
                        .absolute()
                        .left(px(8.))
                        .top(px(6.))
                        .text_size(px(4.5))
                        .line_height(px(4.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ink.opacity(0.5))
                        .child(
                            name.chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string(),
                        ),
                )
                .into_any_element()
        }
        kind => tile
            .child(
                svg()
                    .path(match kind {
                        "session" => "editor/NoteIcon.svg",
                        "organization" => "editor/Building03Icon.svg",
                        "folder" => "chat/Folder01Icon.svg",
                        _ => "editor/UserIcon.svg",
                    })
                    .size(px(14.))
                    .text_color(colors.muted_foreground),
            )
            .into_any_element(),
    }
}

fn paint_selection(
    layout: &VisibleLayout,
    selection: Selection,
    focused: bool,
    caret_visible: bool,
    caret_color: gpui::Hsla,
    selection_color: gpui::Hsla,
    window: &mut Window,
) {
    if !focused || (selection.is_empty() && !caret_visible) {
        return;
    }
    let row = &layout.row;
    let row_end = row.start + row.position(row.text.len()) - row.start;
    let range = selection.range();
    if range.end < row.start || range.start > row_end {
        return;
    }
    let (start, end) = (range.start.max(row.start), range.end.min(row_end));
    let at = |position: usize| {
        row.byte(position)
            .and_then(|byte| layout.layout.position_for_index(byte))
    };
    let (Some(a), Some(b)) = (at(start), at(end)) else {
        return;
    };
    let height = layout.layout.line_height();
    let bounds = layout.layout.bounds();
    if selection.is_empty() {
        window.paint_quad(fill(Bounds::new(a, size(px(1.), height)), caret_color));
        return;
    }
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

/// The draft's paragraphs as rows; mention chips read as a 1em avatar gap and their label,
/// cut at 20 characters like shipping's `MentionNodeView`.
fn project(model: &EditorModel) -> Vec<Arc<Row>> {
    let root = &model.document.root;
    (0..root.children.len())
        .filter_map(|index| {
            let node = root.children.get(index)?;
            let mut text = String::new();
            let mut spans = Vec::new();
            let mut position = 0;
            node.children.visit(&mut |child| {
                let begin = text.len();
                if let Some(content) = &child.text {
                    text.push_str(&content.as_string());
                } else if child.kind() == "hardBreak" {
                    text.push('\n');
                } else if child.kind() == "attachment" {
                    // Room for the chip drawn over it: icon, name and remove button.
                    text.push_str("\u{2003}\u{2003}");
                    text.push_str(&attachment_label(
                        child
                            .attr("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    ));
                    text.push('\u{2003}');
                } else if child.kind() == "mention-@" {
                    text.push_str("\u{2003}\u{2009}");
                    text.push_str(&chip_label(
                        child
                            .attr("label")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    ));
                } else {
                    let label = child
                        .attr("name")
                        .or_else(|| child.attr("label"))
                        .and_then(Value::as_str)
                        .unwrap_or(child.kind());
                    text.push_str(&format!("[{label}]"));
                }
                let units = child.text.as_ref().map_or(1, |text| text.units());
                spans.push(Span {
                    bytes: begin..text.len(),
                    position,
                    units,
                    atom: child.text.is_none(),
                    marks: child.marks().to_vec(),
                    node: child.clone(),
                });
                position += units;
            });
            Some(Arc::new(Row {
                id: node.id,
                start: root.children.prefix(index) + 1,
                text: text.into(),
                spans,
                kind: node.kind().into(),
                level: 0,
                style: BlockStyle::default(),
            }))
        })
        .collect()
}

/// Shipping's chat images may be 8 MB at most.
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// `AttachmentChipView`'s name: cut at 24 characters, "file" when unnamed.
fn attachment_label(name: &str) -> String {
    if name.is_empty() {
        "file".into()
    } else if name.chars().count() > 24 {
        format!("{}\u{2026}", name.chars().take(24).collect::<String>())
    } else {
        name.into()
    }
}

pub fn chip_label(label: &str) -> String {
    if label.chars().count() > 20 {
        format!("{}\u{2026}", label.chars().take(20).collect::<String>())
    } else {
        label.into()
    }
}

/// Plain text between two document positions, rows joined by newlines.
fn text_between(rows: &[Arc<Row>], range: Range<usize>) -> String {
    rows.iter()
        .filter_map(|row| {
            let end = row.start + row.spans.iter().map(|span| span.units).sum::<usize>();
            if range.end < row.start || range.start > end {
                return None;
            }
            let from = row.byte(range.start.max(row.start))?;
            let to = row.byte(range.end.min(end))?;
            Some(row.text[from..to].replace(['\u{2003}', '\u{2009}'], ""))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Shipping's `proseMirrorJsonToText`, keeping a line break between paragraphs.
pub fn doc_text(doc: &Value) -> String {
    fn inline(node: &Value) -> String {
        match node["type"].as_str() {
            Some("text") => node["text"].as_str().unwrap_or_default().to_owned(),
            Some(kind) if kind.starts_with("mention-") => format!(
                "@{}",
                node["attrs"]["label"]
                    .as_str()
                    .filter(|label| !label.is_empty())
                    .or_else(|| node["attrs"]["id"].as_str())
                    .unwrap_or_default()
            ),
            _ => node["content"]
                .as_array()
                .map(|children| children.iter().map(inline).collect())
                .unwrap_or_default(),
        }
    }
    doc["content"]
        .as_array()
        .map(|blocks| blocks.iter().map(inline).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Shipping's `extractContextRefsFromTiptapJson`: each distinct mention, in order.
pub fn context_refs(doc: &Value) -> Vec<ContextRef> {
    fn visit(node: &Value, refs: &mut Vec<ContextRef>) {
        if node["type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("mention-"))
            && let (Some(kind), Some(id)) = (
                node["attrs"]["type"].as_str().and_then(MentionKind::parse),
                node["attrs"]["id"].as_str(),
            )
        {
            let reference = ContextRef {
                kind,
                id: id.into(),
            };
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
        for child in node["content"].as_array().into_iter().flatten() {
            visit(child, refs);
        }
    }
    let mut refs = Vec::new();
    visit(doc, &mut refs);
    refs
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.caret.is_none() {
            self.caret = Some(Caret::new(&self.focus, window, cx));
        }
        if let Some(caret) = &self.caret {
            let enabled = self.model.selection.is_empty();
            caret.update(cx, |caret, cx| caret.set_enabled(enabled, window, cx));
        }
        let menu = self.mention_menu(window, cx);
        self.layouts.clear();
        let rows = self
            .rows
            .clone()
            .iter()
            .enumerate()
            .map(|(index, row)| self.row(index, row, window, cx))
            .collect::<Vec<_>>();
        let entity = cx.entity();
        let reveal = cx.entity();
        div()
            .id("chat-composer")
            .key_context("ChatComposer")
            .track_focus(&self.focus)
            .relative()
            .w_full()
            .min_w_0()
            .cursor_text()
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_drop(
                cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                    this.drop_files(paths.paths().to_vec(), window, cx)
                }),
            )
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(
                div()
                    .id("chat-composer-scroll")
                    .w_full()
                    .max_h(px(self.max_height))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .pr(px(self.padding_right))
                            .children(rows),
                    ),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        let focus = entity.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        reveal.update(cx, |this, _| this.reveal_caret(window));
                    },
                )
                .absolute()
                .inset_0(),
            )
            .children(menu)
    }
}

impl Composer {
    /// Keeps the caret inside the scrolled draft after an edit.
    fn reveal_caret(&mut self, window: &mut Window) {
        if !std::mem::take(&mut self.reveal) {
            return;
        }
        let Some(caret) = self.caret_bounds(self.model.selection.head) else {
            return;
        };
        let viewport = self.scroll.bounds();
        let offset = self.scroll.offset();
        let shift = if caret.top() < viewport.top() {
            viewport.top() - caret.top()
        } else if caret.bottom() > viewport.bottom() {
            viewport.bottom() - caret.bottom()
        } else {
            return;
        };
        self.scroll.set_offset(point(offset.x, offset.y + shift));
        window.refresh();
    }
}

impl EntityInputHandler for Composer {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *actual = Some(range.clone());
        Some(text_between(&self.rows, range))
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.model.selection;
        Some(UTF16Selection {
            range: selection.range(),
            reversed: selection.head < selection.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.model.marked_range()
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.model.commit_composition();
        self.edited(cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .or_else(|| self.model.marked_range())
            .unwrap_or_else(|| self.model.selection.range());
        let text = text.to_owned();
        self.apply(
            move |model| {
                let result = model.type_text(range, &text);
                if result.is_ok() {
                    model.commit_composition();
                }
                result
            },
            cx,
        );
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let end = text.encode_utf16().count();
        let text = text.to_owned();
        self.apply(
            move |model| model.compose(range, &text, selected.unwrap_or(end..end)),
            cx,
        );
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.layouts
            .iter()
            .find_map(|layout| layout.bounds_for(range.clone()))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.hit(position)
    }
}

/// A `StyledText` with elements drawn at glyph positions, for the chips' avatars.
struct InlineAtoms {
    text: StyledText,
    atoms: Vec<(usize, AnyElement)>,
}

impl Element for InlineAtoms {
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
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.text.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text.prepaint(id, inspector, bounds, state, window, cx);
        let layout = self.text.layout().clone();
        for (byte, atom) in &mut self.atoms {
            if let Some(origin) = layout.position_for_index(*byte) {
                atom.prepaint_as_root(
                    origin,
                    size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                    window,
                    cx,
                );
            }
        }
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text
            .paint(id, inspector, bounds, state, prepaint, window, cx);
        for (_, atom) in &mut self.atoms {
            atom.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(content: Value) -> Value {
        json!({"type": "doc", "content": content})
    }

    fn mention(kind: &str, id: &str, label: &str) -> Value {
        json!({"type": "mention-@", "attrs": {"type": kind, "id": id, "label": label}})
    }

    #[test]
    fn sent_text_spells_mentions_and_keeps_paragraphs() {
        let draft = doc(json!([
            {"type": "paragraph", "content": [{"type": "text", "text": "Prep for "}, mention("session", "s1", "Weekly sync")]},
            {"type": "paragraph"},
            {"type": "paragraph", "content": [{"type": "text", "text": "with "}, mention("human", "h1", "")]},
        ]));
        assert_eq!(doc_text(&draft), "Prep for @Weekly sync\n\nwith @h1");
    }

    #[test]
    fn context_refs_follow_mentions_once_each_with_shipping_keys() {
        let draft = doc(json!([{"type": "paragraph", "content": [
            mention("folder", "CS 101", "CS 101"),
            mention("folder", "CS 101", "CS 101"),
            mention("organization", "o1", "Northwind"),
            mention("unknown", "x", "x"),
        ]}]));
        let refs = context_refs(&draft);
        assert_eq!(refs.len(), 2);
        assert_eq!(
            refs[0].to_json(),
            json!({"kind": "folder", "key": "folder:manual:CS 101", "source": "manual", "folderId": "CS 101"})
        );
        assert_eq!(refs[1].to_json()["organizationId"], "o1");
    }

    #[test]
    fn chip_labels_cut_at_twenty_characters() {
        assert_eq!(
            chip_label("Quarterly planning review"),
            "Quarterly planning r\u{2026}"
        );
        assert_eq!(chip_label("Maya Chen"), "Maya Chen");
    }

    #[test]
    fn facehash_backgrounds_follow_its_string_hash() {
        // Values from facehash's `stringHash` in Node.
        assert_eq!(string_hash("Maya Chen"), 1_658_687_022);
        assert_eq!(string_hash("Ana"), 65_972);
        assert_eq!(string_hash(""), 0);
    }

    #[gpui::test]
    fn dropped_files_become_chips_that_stay_out_of_the_sent_text(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        cx.update(crate::ui::theme::UiFonts::init);
        let (composer, cx) =
            cx.add_window_view(|_, cx| Composer::new(runtime.clone(), SearchIndex::default(), cx));
        let photo = directory.path().join("whiteboard.png");
        let notes = directory.path().join("notes.txt");
        std::fs::write(&photo, b"\x89PNG\r\n\x1a\n").unwrap();
        std::fs::write(&notes, b"agenda").unwrap();
        let position = point(px(20.), px(10.));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_event(gpui::FileDropEvent::Entered {
            position,
            paths: gpui::ExternalPaths(vec![photo, notes].into()),
        });
        cx.simulate_event(gpui::FileDropEvent::Submit { position });
        let mut chips = Vec::new();
        for _ in 0..200 {
            cx.run_until_parked();
            chips = composer.read_with(cx, |composer, _| {
                composer.doc()["content"][0]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|node| node["type"] == "attachment")
                    .map(|node| node["attrs"].clone())
                    .collect::<Vec<_>>()
            });
            if chips.len() == 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(chips.len(), 2);
        assert_eq!(chips[0]["name"], "whiteboard.png");
        assert!(
            chips[0]["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("data:image/png;base64,"))
        );
        assert_eq!(
            (chips[1]["name"].clone(), chips[1]["url"].clone()),
            (json!("notes.txt"), Value::Null)
        );
        composer.update(cx, |composer, cx| {
            assert!(!composer.has_text());
            composer.insert_text("Summarize these", cx);
            assert_eq!(composer.text().trim(), "Summarize these");
        });
    }
}
