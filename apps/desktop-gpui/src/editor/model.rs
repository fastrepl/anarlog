use std::{collections::VecDeque, ops::Range, sync::Arc};

use serde_json::{Map, Value, json};
use unicode_segmentation::UnicodeSegmentation;

use super::{
    document::{
        Document, EditResult, Node, NodeRef, concat_inline, inline_text, split_inline, utf8,
    },
    sequence::{Measured, Sequence},
};

const HISTORY_BYTES: usize = 8 * 1024 * 1024;
const HISTORY_ENTRIES: usize = 2048;
const MAX_INSERT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn caret(position: usize) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Clone)]
struct Checkpoint {
    root: NodeRef,
    selection: Selection,
}

#[derive(Clone)]
struct HistoryEntry {
    before: Checkpoint,
    after: Checkpoint,
    bytes: usize,
}

struct Composition {
    before: Checkpoint,
    range: Range<usize>,
    bytes: usize,
}

#[derive(Clone, Debug)]
pub struct Mapping {
    pub old: Range<usize>,
    pub inserted: usize,
}

impl Mapping {
    pub fn map(&self, position: usize, after: bool) -> usize {
        if position < self.old.start {
            position
        } else if position > self.old.end {
            position - self.old.len() + self.inserted
        } else {
            self.old.start + if after { self.inserted } else { 0 }
        }
    }

    pub fn map_anchor(&self, range: Range<usize>) -> Option<Range<usize>> {
        let start = self.map(range.start, true);
        let end = self.map(range.end, false);
        (start < end).then_some(start..end)
    }
}

pub struct EditorModel {
    pub document: Document,
    pub selection: Selection,
    pub revision: u64,
    pub last_mapping: Option<Mapping>,
    pub read_only: bool,
    stored_marks: Option<Vec<Value>>,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_bytes: usize,
    composition: Option<Composition>,
    batch_cost: Option<usize>,
}

impl EditorModel {
    pub fn new(document: Document) -> Self {
        let selection = Selection::caret(document.first_caret());
        Self {
            document,
            selection,
            revision: 0,
            last_mapping: None,
            read_only: false,
            stored_marks: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_bytes: 0,
            composition: None,
            batch_cost: None,
        }
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            root: self.document.root.clone(),
            selection: self.selection,
        }
    }

    fn restore(&mut self, checkpoint: &Checkpoint) {
        self.document.root = checkpoint.root.clone();
        self.selection = checkpoint.selection;
        self.stored_marks = None;
        self.last_mapping = None;
        self.revision += 1;
    }

    fn ensure_editable(&self) -> EditResult<()> {
        if self.read_only {
            Err("Document is read-only".into())
        } else {
            Ok(())
        }
    }

    fn record(&mut self, before: Checkpoint, bytes: usize) {
        self.revision += 1;
        if let Some(cost) = &mut self.batch_cost {
            *cost += bytes;
            return;
        }
        if let Some(composition) = &mut self.composition {
            composition.bytes += bytes;
            return;
        }
        self.push_history(before, bytes);
    }

    pub fn transaction(
        &mut self,
        action: impl FnOnce(&mut Self) -> EditResult<()>,
    ) -> EditResult<()> {
        self.ensure_editable()?;
        if self.batch_cost.is_some() {
            return action(self);
        }
        let before = self.checkpoint();
        let revision = self.revision;
        let mapping = self.last_mapping.take();
        let marks = self.stored_marks.clone();
        self.batch_cost = Some(0);
        let result = action(self);
        let cost = self.batch_cost.take().expect("active transaction");
        if result.is_err() {
            self.document.root = before.root;
            self.selection = before.selection;
            self.revision = revision;
            self.last_mapping = mapping;
            self.stored_marks = marks;
        } else if revision != self.revision {
            if let Some(composition) = &mut self.composition {
                composition.bytes += cost;
            } else {
                self.push_history(before, cost);
            }
        }
        result
    }

    fn push_history(&mut self, before: Checkpoint, bytes: usize) {
        self.redo.clear();
        let entry = HistoryEntry {
            before,
            after: self.checkpoint(),
            bytes,
        };
        self.history_bytes += bytes;
        self.undo.push_back(entry);
        while (self.history_bytes > HISTORY_BYTES && self.undo.len() > 1)
            || self.undo.len() > HISTORY_ENTRIES
        {
            self.history_bytes -= self.undo.pop_front().expect("nonempty history").bytes;
        }
    }

    pub fn select(&mut self, selection: Selection) {
        self.selection = selection;
        self.stored_marks = None;
    }

    pub fn move_document_boundary(&mut self, end: bool, extend: bool) {
        let head = self.document.edge_caret(end);
        self.select(Selection {
            anchor: if extend { self.selection.anchor } else { head },
            head,
        });
    }

    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.composition
            .as_ref()
            .map(|composition| composition.range.clone())
    }

    pub fn composing(&self) -> bool {
        self.composition.is_some()
    }

    pub fn replace(&mut self, range: Range<usize>, text: &str) -> EditResult<()> {
        self.ensure_editable()?;
        if text.len() > MAX_INSERT_BYTES {
            return Err("Paste exceeds the 1 MiB transaction limit".into());
        }
        if range.start > range.end {
            return Err("Invalid replacement range".into());
        }
        if range.start == 0 && range.end == self.document.units() {
            super::transform::removable(&self.document.root)?;
            let fields = Arc::new(Map::from_iter([("type".into(), json!("text"))]));
            let paragraph = Node::fresh(
                "paragraph",
                if text.is_empty() {
                    Sequence::default()
                } else {
                    Sequence::one(Node::text_node(text, fields))
                },
            );
            let before = self.checkpoint();
            self.document.root = self.document.root.with_children(Sequence::one(paragraph));
            self.selection = Selection::caret(1 + text.encode_utf16().count());
            self.last_mapping = Some(Mapping {
                old: range.clone(),
                inserted: self.document.units(),
            });
            self.record(before, text.len() + range.len() * 4 + 4096);
            return Ok(());
        }
        let from = self.document.resolve(range.start)?;
        let to = self.document.resolve(range.end)?;
        super::transform::removable_inline(
            &from.node.children,
            from.offset..if from.path == to.path {
                to.offset
            } else {
                from.node.children.units()
            },
        )?;
        if from.path != to.path {
            super::transform::removable_inline(&to.node.children, 0..to.offset)?;
        }
        if from.path != to.path {
            let from_cell = from.path.iter().enumerate().find_map(|(index, _)| {
                self.document
                    .node(&from.path[..=index])
                    .filter(|node| matches!(node.kind(), "tableCell" | "tableHeader"))
                    .map(|_| &from.path[..=index])
            });
            let to_cell = to.path.iter().enumerate().find_map(|(index, _)| {
                self.document
                    .node(&to.path[..=index])
                    .filter(|node| matches!(node.kind(), "tableCell" | "tableHeader"))
                    .map(|_| &to.path[..=index])
            });
            if from_cell != to_cell && (from_cell.is_some() || to_cell.is_some()) {
                let blocks = super::transform::selected_blocks(&self.document, range.clone())?;
                return self.transaction(|model| {
                    for block in blocks.iter().rev() {
                        let start = range.start.max(block.start);
                        let end = range.end.min(block.start + block.node.children.units());
                        model.replace(
                            start..end,
                            if block.node.id == from.node.id {
                                text
                            } else {
                                ""
                            },
                        )?;
                    }
                    model.selection = Selection::caret(range.start + text.encode_utf16().count());
                    Ok(())
                });
            }
        }
        let (left, _) = split_inline(&from.node.children, from.offset)?;
        let (_, right) = split_inline(&to.node.children, to.offset)?;
        let fields = self.insertion_fields(&from.node, from.offset);
        let inserted = if text.is_empty() {
            Sequence::default()
        } else {
            Sequence::one(Node::text_node(text, fields))
        };
        let children = concat_inline(&concat_inline(&left, &inserted), &right);
        let before = self.checkpoint();
        if from.path == to.path {
            self.document
                .replace_node(&from.path, from.node.with_children(children));
        } else {
            self.document.root =
                super::transform::replace_across(&self.document, &from, &to, children)?;
        }
        let inserted = text.encode_utf16().count();
        self.selection = Selection::caret(range.start + inserted);
        let removed = range.len();
        self.last_mapping = Some(Mapping {
            old: range,
            inserted,
        });
        self.record(
            before,
            text.len() + removed * 4 + 2048 * (from.path.len() + to.path.len()),
        );
        Ok(())
    }

    pub fn type_text(&mut self, range: Range<usize>, text: &str) -> EditResult<()> {
        if range.is_empty() && text.chars().count() == 1 && self.composition.is_none() {
            let resolved = self.document.resolve(range.start)?;
            let fields = self.insertion_fields(&resolved.node, resolved.offset);
            let marks = fields.get("marks").and_then(Value::as_array);
            if resolved.node.kind() != "codeBlock"
                && !marks.is_some_and(|marks| marks.iter().any(|mark| mark["type"] == "code"))
            {
                let mut offset = resolved.offset.saturating_sub(4);
                let tail = loop {
                    match split_inline(&resolved.node.children, offset) {
                        Ok((_, right)) => {
                            let left = split_inline(&right, resolved.offset - offset)?.0;
                            break inline_text(&left);
                        }
                        Err(_) if offset < resolved.offset => offset += 1,
                        Err(error) => return Err(error),
                    }
                };
                if text == "." && tail.ends_with("..") {
                    return self.replace(range.start - 2..range.end, "…");
                }
                if text == "-"
                    && tail.ends_with('-')
                    && tail.chars().rev().nth(1).is_some_and(|c| c != '-')
                {
                    return self.replace(range.start - 1..range.end, "—");
                }
                if matches!(text, "\"" | "'") {
                    let open = tail
                        .chars()
                        .last()
                        .is_none_or(|c| c.is_whitespace() || "{[(<'\"‘“".contains(c));
                    let quote = match (text, open) {
                        ("\"", true) => "“",
                        ("\"", false) => "”",
                        (_, true) => "‘",
                        _ => "’",
                    };
                    return self.replace(range, quote);
                }
            }
        }
        self.replace(range, text)
    }

    pub fn insert_slice(&mut self, fragment: Document, open: bool) -> EditResult<()> {
        self.transaction(|model| model.insert_slice_inner(fragment, open))
    }

    fn insert_slice_inner(&mut self, fragment: Document, open: bool) -> EditResult<()> {
        self.ensure_editable()?;
        if self.composing() {
            return Err("Finish composition before pasting rich content".into());
        }
        let range = self.selection.range();
        let empty_document = range.is_empty()
            && range.start == self.document.first_caret()
            && self.document.root.children.len() == 1
            && self
                .document
                .root
                .children
                .get(0)
                .is_some_and(|node| node.kind() == "paragraph" && node.children.len() == 0);
        if (range.start == 0 && range.end == self.document.units()) || empty_document {
            super::transform::removable(&self.document.root)?;
            let before = self.checkpoint();
            let cost = fragment.original.len() + range.len() * 4 + 4096;
            self.document.root = self
                .document
                .root
                .with_children(fragment.root.children.clone());
            self.selection = Selection::caret(self.document.edge_caret(true));
            self.record(before, cost);
            return Ok(());
        }
        let mut from = self.document.resolve(range.start)?;
        let mut to = self.document.resolve(range.end)?;
        if from.path != to.path {
            self.replace(range, "")?;
            from = self.document.resolve(self.selection.head)?;
            to = self.document.resolve(self.selection.head)?;
        }
        let range = self.selection.range();
        super::transform::removable_inline(&from.node.children, from.offset..to.offset)?;
        let first = fragment
            .root
            .children
            .get(0)
            .ok_or("Empty clipboard slice")?;
        let last = fragment
            .root
            .children
            .get(fragment.root.children.len() - 1)
            .expect("first");
        if from.node.kind() == "codeBlock" {
            super::transform::removable(&fragment.root)?;
            let blocks = super::transform::selected_blocks(&fragment, 0..fragment.units())?;
            let text = blocks
                .iter()
                .map(|block| inline_text(&block.node.children))
                .collect::<Vec<_>>()
                .join("\n");
            return self.replace(range, &text);
        }
        let (left, _) = split_inline(&from.node.children, from.offset)?;
        let (_, right) = split_inline(&from.node.children, to.offset)?;
        let before = self.checkpoint();
        if fragment.root.children.len() == 1 && open && first.is_textblock() {
            self.document.replace_node(
                &from.path,
                from.node.with_children(concat_inline(
                    &concat_inline(&left, &first.children),
                    &right,
                )),
            );
            self.selection = Selection::caret(range.start + first.children.units());
        } else {
            let path = &from.path[..from.path.len() - 1];
            let index = *from.path.last().expect("textblock");
            let parent = self.document.node(path).expect("parent");
            let mut inserted = fragment.root.children.clone();
            if open && first.is_textblock() && last.is_textblock() {
                inserted = inserted.splice(
                    0..1,
                    &Sequence::one(
                        from.node
                            .with_children(concat_inline(&left, &first.children)),
                    ),
                );
                inserted = inserted.splice(
                    inserted.len() - 1..inserted.len(),
                    &Sequence::one(last.with_children(concat_inline(&last.children, &right))),
                );
                let caret = from.start - 1
                    + inserted.prefix(inserted.len() - 1)
                    + 1
                    + last.children.units();
                self.selection = Selection::caret(caret);
            } else {
                inserted = Sequence::one(from.node.with_children(left))
                    .concat(&inserted)
                    .concat(&Sequence::one(Node::fresh("paragraph", right)));
                self.selection =
                    Selection::caret(from.start - 1 + inserted.prefix(inserted.len() - 1) + 1);
            }
            self.document.replace_node(
                path,
                parent.with_children(parent.children.splice(index..index + 1, &inserted)),
            );
        }
        self.record(before, fragment.original.len() + range.len() * 4 + 8192);
        Ok(())
    }

    fn insertion_fields(&self, block: &NodeRef, offset: usize) -> Arc<Map<String, Value>> {
        let previous = block.children.locate(offset.saturating_sub(1));
        let next = block.children.locate(offset);
        let source = previous
            .as_ref()
            .map(|(_, _, node)| *node)
            .or_else(|| next.as_ref().map(|(_, _, node)| *node));
        let marks = self.stored_marks.clone().unwrap_or_else(|| {
            let mut marks = source.map(|node| node.marks().to_vec()).unwrap_or_default();
            let at_link_end = offset == block.children.units()
                || match (previous, next) {
                    (Some((a, _, _)), Some((b, _, _))) => a != b,
                    _ => true,
                };
            if at_link_end || offset == 0 {
                marks.retain(|mark| mark.get("type").and_then(Value::as_str) != Some("link"));
            }
            marks
        });
        if let Some(node) = source.filter(|node| node.text.is_some() && node.marks() == marks) {
            return node.fields.clone();
        }
        let mut fields = source
            .filter(|node| node.text.is_some())
            .map(|node| (*node.fields).clone())
            .unwrap_or_else(|| Map::from_iter([("type".into(), json!("text"))]));
        if marks.is_empty() {
            fields.remove("marks");
        } else {
            fields.insert("marks".into(), marks.into());
        }
        Arc::new(fields)
    }

    pub fn compose(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Range<usize>,
    ) -> EditResult<()> {
        let replacement = range
            .or_else(|| self.marked_range())
            .unwrap_or_else(|| self.selection.range());
        if selected.start > selected.end || selected.end > text.encode_utf16().count() {
            return Err("Composition selection is outside its text".into());
        }
        utf8(text, selected.start)?;
        utf8(text, selected.end)?;
        let new_composition = self.composition.is_none();
        if new_composition {
            self.composition = Some(Composition {
                before: self.checkpoint(),
                range: replacement.clone(),
                bytes: 0,
            });
        }
        if let Err(error) = self.replace(replacement.clone(), text) {
            if new_composition {
                self.composition = None;
            }
            return Err(error);
        }
        let end = replacement.start + text.encode_utf16().count();
        self.composition
            .as_mut()
            .expect("composition started")
            .range = replacement.start..end;
        self.selection = Selection {
            anchor: replacement.start + selected.start,
            head: replacement.start + selected.end,
        };
        Ok(())
    }

    pub fn commit_composition(&mut self) {
        if let Some(composition) = self.composition.take() {
            self.push_history(composition.before, composition.bytes.max(2048));
        }
    }

    pub fn cancel_composition(&mut self) {
        if let Some(composition) = self.composition.take() {
            self.restore(&composition.before);
        }
    }

    pub fn undo(&mut self) -> EditResult<()> {
        self.ensure_editable()?;
        if self.composing() {
            self.cancel_composition();
            return Ok(());
        }
        if let Some(entry) = self.undo.pop_back() {
            self.history_bytes -= entry.bytes;
            self.restore(&entry.before);
            self.redo.push(entry);
        }
        Ok(())
    }

    pub fn redo(&mut self) -> EditResult<()> {
        self.ensure_editable()?;
        if self.composing() {
            return Err("Finish composition before redo".into());
        }
        if let Some(entry) = self.redo.pop() {
            self.restore(&entry.after);
            self.history_bytes += entry.bytes;
            self.undo.push_back(entry);
        }
        Ok(())
    }

    pub fn toggle_mark(&mut self, kind: &str) -> EditResult<()> {
        self.apply_mark(kind, None)
    }

    fn apply_mark(&mut self, kind: &str, enable: Option<bool>) -> EditResult<()> {
        self.ensure_editable()?;
        let range = self.selection.range();
        let from = self.document.resolve(range.start)?;
        let to = self.document.resolve(range.end)?;
        if from.node.kind() == "codeBlock" {
            return Err("Code blocks do not accept formatting marks".into());
        }
        if kind == "code" {
            let mut unknown = false;
            from.node.children.visit(&mut |node| {
                unknown |= node.marks().iter().any(|mark| {
                    !matches!(
                        mark["type"].as_str(),
                        Some(
                            "bold"
                                | "italic"
                                | "underline"
                                | "strike"
                                | "highlight"
                                | "link"
                                | "code"
                        )
                    )
                });
            });
            if unknown {
                return Err(
                    "Code formatting would discard an unknown mark; original retained".into(),
                );
            }
        }
        if self.selection.is_empty() {
            let fields = self.insertion_fields(&from.node, from.offset);
            let mut marks = fields
                .get("marks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            toggle_marks(&mut marks, kind, None);
            self.stored_marks = Some(marks);
            return Ok(());
        }
        if from.path != to.path {
            let blocks = super::transform::selected_blocks(&self.document, range.clone())?;
            let mut all_marked = true;
            for block in &blocks {
                let end = range
                    .end
                    .saturating_sub(block.start)
                    .min(block.node.children.units());
                let start = range.start.saturating_sub(block.start);
                let selected = split_inline(&split_inline(&block.node.children, end)?.0, start)?.1;
                selected.visit(&mut |node| {
                    if node.text.is_some() && block.node.kind() != "codeBlock" {
                        all_marked &= node.marks().iter().any(|mark| mark["type"] == kind);
                    }
                });
            }
            return self.transaction(|model| {
                let selection = model.selection;
                for block in blocks {
                    model.selection = Selection {
                        anchor: range.start.max(block.start),
                        head: range.end.min(block.start + block.node.children.units()),
                    };
                    if !model.selection.is_empty() && block.node.kind() != "codeBlock" {
                        model.apply_mark(kind, Some(enable.unwrap_or(!all_marked)))?;
                    }
                }
                model.selection = selection;
                Ok(())
            });
        }
        let (through, right) = split_inline(&from.node.children, to.offset)?;
        let (left, selected) = split_inline(&through, from.offset)?;
        let mut all_marked = true;
        selected.visit(&mut |node| {
            if node.text.is_some() {
                all_marked &= node.marks().iter().any(|mark| mark["type"] == kind);
            }
        });
        let mut changed = Vec::new();
        selected.visit(&mut |node| {
            if node.text.is_none() {
                changed.push(node.clone());
                return;
            }
            let mut fields = (*node.fields).clone();
            let mut marks = node.marks().to_vec();
            toggle_marks(&mut marks, kind, Some(enable.unwrap_or(!all_marked)));
            if marks.is_empty() {
                fields.remove("marks");
            } else {
                fields.insert("marks".into(), marks.into());
            }
            changed.push(node.with_fields(fields));
        });
        let before = self.checkpoint();
        self.document.replace_node(
            &from.path,
            from.node.with_children(concat_inline(
                &concat_inline(&left, &Sequence::from_items(changed)),
                &right,
            )),
        );
        self.record(before, 4096 + selected.len() * 1024);
        Ok(())
    }

    pub fn set_link(&mut self, href: &str) -> EditResult<()> {
        self.ensure_editable()?;
        if !href.is_empty() && !super::clipboard::openable_link(href) {
            return Err("Links must use HTTP or HTTPS".into());
        }
        let range = self.selection.range();
        let from = self.document.resolve(range.start)?;
        let to = self.document.resolve(range.end)?;
        if from.path != to.path {
            let blocks = super::transform::selected_blocks(&self.document, range.clone())?;
            return self.transaction(|model| {
                let selection = model.selection;
                for block in blocks {
                    model.selection = Selection {
                        anchor: range.start.max(block.start),
                        head: range.end.min(block.start + block.node.children.units()),
                    };
                    if !model.selection.is_empty() && block.node.kind() != "codeBlock" {
                        model.set_link(href)?;
                    }
                }
                model.selection = selection;
                Ok(())
            });
        }
        if range.is_empty() || from.node.kind() == "codeBlock" {
            return Err("Select link text inside one non-code block".into());
        }
        let (through, right) = split_inline(&from.node.children, to.offset)?;
        let (left, selected) = split_inline(&through, from.offset)?;
        let mut linked = Vec::new();
        selected.visit(&mut |node| {
            if node.text.is_none() {
                linked.push(node.clone());
                return;
            }
            let mut fields = (*node.fields).clone();
            let mut marks = node.marks().to_vec();
            marks.retain(|mark| mark["type"] != "link");
            if !href.is_empty() {
                marks.push(json!({"type":"link","attrs":{"href":href,"target":null}}));
            }
            fields.insert("marks".into(), marks.into());
            linked.push(node.with_fields(fields));
        });
        let before = self.checkpoint();
        self.document.replace_node(
            &from.path,
            from.node.with_children(concat_inline(
                &concat_inline(&left, &Sequence::from_items(linked)),
                &right,
            )),
        );
        self.record(before, selected.len() * 1024 + 4096);
        Ok(())
    }

    pub fn insert_inline_atom(&mut self, kind: &str, attrs: Map<String, Value>) -> EditResult<()> {
        self.transaction(|model| model.insert_inline_atom_inner(kind, attrs))
    }

    fn insert_inline_atom_inner(
        &mut self,
        kind: &str,
        attrs: Map<String, Value>,
    ) -> EditResult<()> {
        self.ensure_editable()?;
        if !matches!(kind, "mention-@" | "appLink" | "hardBreak") {
            return Err("Unsupported inline atom".into());
        }
        let range = self.selection.range();
        let mut from = self.document.resolve(range.start)?;
        let mut to = self.document.resolve(range.end)?;
        if from.node.kind() == "codeBlock" {
            return if kind == "hardBreak" {
                self.replace(range, "\n")
            } else {
                Err("Inline mentions cannot be inserted in code".into())
            };
        }
        if from.path != to.path {
            self.replace(range, "")?;
            from = self.document.resolve(self.selection.head)?;
            to = self.document.resolve(self.selection.head)?;
        }
        let range = self.selection.range();
        super::transform::removable_inline(&from.node.children, from.offset..to.offset)?;
        let (left, _) = split_inline(&from.node.children, from.offset)?;
        let (_, right) = split_inline(&from.node.children, to.offset)?;
        let node = Node::fresh(kind, Sequence::default());
        let mut fields = (*node.fields).clone();
        fields.insert("attrs".into(), attrs.into());
        let before = self.checkpoint();
        self.document.replace_node(
            &from.path,
            from.node.with_children(
                left.concat(&Sequence::one(node.with_fields(fields)))
                    .concat(&right),
            ),
        );
        self.selection = Selection::caret(range.start + 1);
        self.last_mapping = Some(Mapping {
            old: range,
            inserted: 1,
        });
        self.record(before, 8192);
        Ok(())
    }

    pub fn insert_block_atom(&mut self, kind: &str, attrs: Map<String, Value>) -> EditResult<()> {
        self.transaction(|model| model.insert_block_atom_inner(kind, attrs))
    }

    fn insert_block_atom_inner(&mut self, kind: &str, attrs: Map<String, Value>) -> EditResult<()> {
        self.ensure_editable()?;
        if !matches!(kind, "horizontalRule" | "image" | "fileAttachment") {
            return Err("Unsupported block atom".into());
        }
        if attrs.contains_key("path") || attrs.contains_key("src") {
            return Err(
                "Use a catalogued attachment ID; local paths must not enter stored documents"
                    .into(),
            );
        }
        if kind != "horizontalRule"
            && attrs
                .get("attachmentId")
                .and_then(Value::as_str)
                .is_none_or(|id| id.is_empty())
        {
            return Err("Attachment must have a catalogued attachment ID".into());
        }
        if !self.selection.is_empty() {
            self.replace(self.selection.range(), "")?;
        }
        let resolved = self.document.resolve(self.selection.head)?;
        let node = Node::fresh(kind, Sequence::default());
        let mut fields = (*node.fields).clone();
        fields.insert("attrs".into(), attrs.into());
        let (left, right) = split_inline(&resolved.node.children, resolved.offset)?;
        let path = &resolved.path[..resolved.path.len() - 1];
        let index = *resolved.path.last().expect("textblock");
        let parent = self.document.node(path).expect("parent");
        if !matches!(
            parent.kind(),
            "doc" | "blockquote" | "listItem" | "taskItem" | "tableCell" | "tableHeader"
        ) {
            return Err(
                "Future container cannot accept block attachments; original retained".into(),
            );
        }
        let before = self.checkpoint();
        let inserted = Sequence::from_items([
            resolved.node.with_children(left),
            node.with_fields(fields),
            Node::fresh("paragraph", right),
        ]);
        self.document.replace_node(
            path,
            parent.with_children(parent.children.splice(index..index + 1, &inserted)),
        );
        self.selection = Selection::caret(resolved.start - 1 + inserted.prefix(2) + 1);
        self.record(before, 8192);
        Ok(())
    }

    pub fn set_block(&mut self, kind: &str, level: Option<u8>) -> EditResult<()> {
        self.ensure_editable()?;
        let resolved = self.document.resolve(self.selection.head)?;
        let mut fields = (*resolved.node.fields).clone();
        fields.insert("type".into(), json!(kind));
        if let Some(level) = level {
            let mut attrs = resolved.node.attrs().cloned().unwrap_or_default();
            attrs.insert("level".into(), json!(level.clamp(1, 6)));
            fields.insert("attrs".into(), attrs.into());
        }
        let children = if kind == "codeBlock" {
            let mut invalid = false;
            let mut children = Vec::new();
            resolved.node.children.visit(&mut |child| {
                if child.kind() == "hardBreak" {
                    let mut fields = (*child.fields).clone();
                    fields.insert("type".into(), json!("text"));
                    children.push(Node::text_node("\n", Arc::new(fields)));
                } else if child.text.is_some()
                    && child.marks().iter().all(|mark| {
                        matches!(
                            mark["type"].as_str(),
                            Some(
                                "bold"
                                    | "italic"
                                    | "underline"
                                    | "strike"
                                    | "code"
                                    | "highlight"
                                    | "link"
                            )
                        )
                    })
                {
                    let mut fields = (*child.fields).clone();
                    fields.remove("marks");
                    children.push(child.with_fields(fields));
                } else {
                    invalid = true;
                }
            });
            if invalid {
                return Err("Code conversion would remove embedded content or future marks; original retained".into());
            }
            Sequence::from_items(children)
        } else {
            resolved.node.children.clone()
        };
        let before = self.checkpoint();
        self.document.replace_node(
            &resolved.path,
            resolved.node.with_fields(fields).with_children(children),
        );
        self.record(before, 4096);
        Ok(())
    }

    pub fn remove_block_atom(&mut self, position: usize, id: u64) -> EditResult<()> {
        self.ensure_editable()?;
        let mut node = self.document.root.clone();
        let mut offset = position;
        let mut path = Vec::new();
        loop {
            if !node.known() {
                return Err("Future attachment container retained unchanged".into());
            }
            let (index, inner, child) = node
                .children
                .locate(offset)
                .ok_or("Attachment no longer exists")?;
            path.push(index);
            if inner == 0 && child.id == id {
                node = child.clone();
                break;
            }
            offset = inner
                .checked_sub(1)
                .ok_or("Attachment moved; retry removal")?;
            node = child.clone();
        }
        if !matches!(node.kind(), "image" | "fileAttachment" | "horizontalRule") {
            return Err("Only attachment or divider nodes can be removed here".into());
        }
        let next = super::transform::adjacent_block(&self.document, &path, true)
            .map(|next| next - node.units())
            .or_else(|| super::transform::adjacent_block(&self.document, &path, false));
        let index = path.pop().expect("atom path");
        let parent = self
            .document
            .node(&path)
            .ok_or("Missing attachment parent")?;
        let empty = parent.children.len() == 1 || next.is_none();
        let replacement = if empty {
            Sequence::one(Node::fresh("paragraph", Sequence::default()))
        } else {
            Sequence::default()
        };
        let target = if empty {
            position + 1
        } else {
            next.ok_or("No editable block remains")?
        };
        let inserted = replacement.units();
        let before = self.checkpoint();
        self.document.replace_node(
            &path,
            parent.with_children(parent.children.splice(index..index + 1, &replacement)),
        );
        self.selection = Selection::caret(target);
        self.last_mapping = Some(Mapping {
            old: position..position + node.units(),
            inserted,
        });
        self.record(before, 8192);
        Ok(())
    }

    pub fn split_block(&mut self) -> EditResult<()> {
        self.transaction(Self::split_block_inner)
    }

    fn split_block_inner(&mut self) -> EditResult<()> {
        self.ensure_editable()?;
        if !self.selection.is_empty() {
            self.replace(self.selection.range(), "")?;
        }
        let resolved = self.document.resolve(self.selection.head)?;
        if resolved.node.kind() == "codeBlock" {
            if resolved.offset == resolved.node.children.units() && resolved.offset > 0 {
                let tail = split_inline(&resolved.node.children, resolved.offset - 1)?.1;
                if inline_text(&tail) == "\n" {
                    let path = &resolved.path[..resolved.path.len() - 1];
                    let index = *resolved.path.last().expect("textblock");
                    let parent = self.document.node(path).expect("parent");
                    let before = self.checkpoint();
                    let trimmed = split_inline(&resolved.node.children, resolved.offset - 1)?.0;
                    let replacement = if trimmed.len() == 0 {
                        Sequence::one(Node::fresh("paragraph", Sequence::default()))
                    } else {
                        Sequence::from_items([
                            resolved.node.with_children(trimmed),
                            Node::fresh("paragraph", Sequence::default()),
                        ])
                    };
                    let position =
                        resolved.start - 1 + replacement.prefix(replacement.len() - 1) + 1;
                    self.document.replace_node(
                        path,
                        parent
                            .with_children(parent.children.splice(index..index + 1, &replacement)),
                    );
                    self.selection = Selection::caret(position);
                    self.record(before, 8192);
                    return Ok(());
                }
            }
            return self.replace(self.selection.range(), "\n");
        }
        let (left, right) = split_inline(&resolved.node.children, resolved.offset)?;
        let path = &resolved.path[..resolved.path.len() - 1];
        let index = *resolved.path.last().expect("textblock");
        let parent = self.document.node(path).expect("parent");
        if matches!(parent.kind(), "listItem" | "taskItem") && index == 0 {
            let list_path = &path[..path.len() - 1];
            let item_index = *path.last().expect("list item");
            let list = self.document.node(list_path).expect("list");
            if resolved.node.children.len() == 0 {
                return self.indent_list(true);
            }
            let before = self.checkpoint();
            let left_item = parent.with_children(Sequence::one(resolved.node.with_children(left)));
            let content =
                Sequence::one(Node::fresh("paragraph", right)).concat(&parent.children.split(1).1);
            let right_item = if parent.kind() == "taskItem" {
                fresh_task(content)
            } else {
                Node::fresh("listItem", content)
            };
            self.document.replace_node(
                list_path,
                list.with_children(list.children.splice(
                    item_index..item_index + 1,
                    &Sequence::from_items([left_item, right_item]),
                )),
            );
            self.selection = Selection::caret(self.selection.head + 4);
            self.record(before, 8192);
            return Ok(());
        }
        let before = self.checkpoint();
        let right = Node::fresh("paragraph", right);
        self.document.replace_node(
            path,
            parent.with_children(parent.children.splice(
                index..index + 1,
                &Sequence::from_items([resolved.node.with_children(left), right]),
            )),
        );
        self.selection = Selection::caret(self.selection.head + 2);
        self.last_mapping = Some(Mapping {
            old: before.selection.range(),
            inserted: 2,
        });
        self.record(before, 8192);
        Ok(())
    }

    pub fn wrap_block(&mut self, kind: &str) -> EditResult<()> {
        self.ensure_editable()?;
        let resolved = self.document.resolve(self.selection.head)?;
        let (replacement, depth) = match kind {
            "blockquote" => (Node::fresh(kind, Sequence::one(resolved.node.clone())), 1),
            "bulletList" | "orderedList" | "taskList" => {
                if resolved.node.kind() != "paragraph" {
                    return Err("Lists require a paragraph".into());
                }
                (
                    Node::fresh(
                        kind,
                        Sequence::one(if kind == "taskList" {
                            fresh_task(Sequence::one(resolved.node.clone()))
                        } else {
                            Node::fresh("listItem", Sequence::one(resolved.node.clone()))
                        }),
                    ),
                    2,
                )
            }
            _ => return Err("Unsupported wrapping command".into()),
        };
        let before = self.checkpoint();
        self.document.replace_node(&resolved.path, replacement);
        self.selection.anchor += depth;
        self.selection.head += depth;
        self.record(before, 8192);
        Ok(())
    }

    pub fn indent_list(&mut self, outdent: bool) -> EditResult<()> {
        self.ensure_editable()?;
        let resolved = self.document.resolve(self.selection.head)?;
        if resolved.path.len() < 3 {
            return Err("Selection is not in a list".into());
        }
        let item_path = &resolved.path[..resolved.path.len() - 1];
        let list_path = &item_path[..item_path.len() - 1];
        let item = self.document.node(item_path).expect("item");
        let list = self.document.node(list_path).expect("list");
        if !matches!(item.kind(), "listItem" | "taskItem")
            || !matches!(list.kind(), "bulletList" | "orderedList" | "taskList")
        {
            return Err("Selection is not in a list item".into());
        }
        let item_index = *item_path.last().expect("item");
        let before = self.checkpoint();
        let paragraph_offset = item
            .children
            .prefix(*resolved.path.last().expect("paragraph"))
            + 2
            + resolved.offset;
        let caret;
        if !outdent {
            if item_index == 0 {
                return Err("The first list item has no preceding item to nest under".into());
            }
            let previous = list.children.get(item_index - 1).expect("previous");
            let nested_index = previous.children.len().saturating_sub(1);
            let existing = previous
                .children
                .get(nested_index)
                .filter(|node| node.kind() == list.kind());
            let previous_start = self.document.position_at_path(list_path).expect("list")
                + 1
                + list.children.prefix(item_index - 1);
            caret = previous_start
                + 1
                + if let Some(nested) = existing {
                    previous.children.prefix(nested_index) + 1 + nested.children.units()
                } else {
                    previous.children.units() + 1
                }
                + paragraph_offset;
            let nested = if let Some(nested) = existing {
                nested.with_children(nested.children.concat(&Sequence::one(item.clone())))
            } else {
                Node::fresh(list.kind(), Sequence::one(item.clone()))
            };
            let previous = previous.with_children(if existing.is_some() {
                previous
                    .children
                    .splice(nested_index..nested_index + 1, &Sequence::one(nested))
            } else {
                previous.children.concat(&Sequence::one(nested))
            });
            self.document.replace_node(
                list_path,
                list.with_children(
                    list.children
                        .splice(item_index - 1..item_index + 1, &Sequence::one(previous)),
                ),
            );
        } else {
            if list_path.len() < 3 {
                let container_path = &list_path[..list_path.len() - 1];
                let list_index = *list_path.last().expect("list");
                let container = self.document.node(container_path).expect("container");
                let (leading, rest) = list.children.split(item_index);
                let trailing = rest.split(1).1;
                let mut replacement = Sequence::default();
                if leading.len() > 0 {
                    replacement = replacement.concat(&Sequence::one(list.with_children(leading)));
                }
                let position = self.document.position_at_path(list_path).expect("list")
                    + replacement.units()
                    + paragraph_offset
                    - 1;
                replacement = replacement.concat(&item.children);
                if trailing.len() > 0 {
                    replacement = replacement.concat(&Sequence::one(list.with_children(trailing)));
                }
                self.document.replace_node(
                    container_path,
                    container.with_children(
                        container
                            .children
                            .splice(list_index..list_index + 1, &replacement),
                    ),
                );
                self.selection = Selection::caret(position);
                self.record(before, 16384);
                return Ok(());
            }
            let parent_item_path = &list_path[..list_path.len() - 1];
            let outer_path = &parent_item_path[..parent_item_path.len() - 1];
            let parent_item = self.document.node(parent_item_path).expect("parent item");
            let outer = self.document.node(outer_path).expect("outer list");
            if !matches!(parent_item.kind(), "listItem" | "taskItem") {
                return Err("Cannot outdent across this container".into());
            }
            let nested_index = *list_path.last().expect("nested list");
            let parent_index = *parent_item_path.last().expect("parent item");
            let kept = list.children.split(item_index).0;
            let trailing = list.children.split(item_index + 1).1;
            let item = if trailing.len() > 0 {
                item.with_children(
                    item.children
                        .concat(&Sequence::one(list.with_children(trailing))),
                )
            } else {
                item
            };
            let nested = if kept.len() == 0 {
                Sequence::default()
            } else {
                Sequence::one(list.with_children(kept))
            };
            let parent_item = parent_item.with_children(
                parent_item
                    .children
                    .splice(nested_index..nested_index + 1, &nested),
            );
            caret = self
                .document
                .position_at_path(outer_path)
                .expect("outer list")
                + 1
                + outer.children.prefix(parent_index)
                + super::sequence::Measured::units(&parent_item)
                + paragraph_offset;
            self.document.replace_node(
                outer_path,
                outer.with_children(outer.children.splice(
                    parent_index..parent_index + 1,
                    &Sequence::from_items([parent_item, item]),
                )),
            );
        }
        self.selection = Selection::caret(caret);
        self.record(before, 16384);
        Ok(())
    }

    pub fn update_node_attrs(
        &mut self,
        path: &[usize],
        attrs: Map<String, Value>,
    ) -> EditResult<()> {
        self.ensure_editable()?;
        let node = self.document.node(path).ok_or("Node no longer exists")?;
        if !matches!(node.kind(), "image" | "taskItem") {
            return Err("Unsupported metadata command".into());
        }
        if attrs
            .keys()
            .any(|key| !matches!(key.as_str(), "editorWidth" | "status" | "checked"))
        {
            return Err(
                "Identity and attachment paths cannot be changed through metadata commands".into(),
            );
        }
        let mut merged = node.attrs().cloned().unwrap_or_default();
        for (key, value) in attrs {
            if key == "editorWidth" {
                let width = value.as_f64().ok_or("Image width must be numeric")?;
                merged.insert(key, json!(width.clamp(15., 100.)));
            } else {
                merged.insert(key, value);
            }
        }
        let mut fields = (*node.fields).clone();
        fields.insert("attrs".into(), merged.into());
        let before = self.checkpoint();
        self.document.replace_node(path, node.with_fields(fields));
        self.record(before, 4096);
        Ok(())
    }

    pub fn move_grapheme(&mut self, forward: bool, extend: bool) -> EditResult<()> {
        let range = self.selection.range();
        if !extend && !range.is_empty() {
            self.selection = Selection::caret(if forward { range.end } else { range.start });
            return Ok(());
        }
        let resolved = self.document.resolve(self.selection.head)?;
        // Only the bounded neighborhood is materialized, even in a multi-megabyte paragraph.
        let start = resolved.offset.saturating_sub(128);
        let end = (resolved.offset + 128).min(resolved.node.children.units());
        let mut safe_start = start;
        let mut safe_end = end;
        let (left, _) = loop {
            if let Ok(parts) = split_inline(&resolved.node.children, safe_end) {
                break parts;
            }
            safe_end -= 1;
        };
        let (_, slice) = loop {
            if let Ok(parts) = split_inline(&left, safe_start) {
                break parts;
            }
            safe_start += 1;
        };
        let text = inline_text(&slice);
        let cursor = resolved.offset - safe_start;
        let mut position = if forward {
            resolved.node.children.units()
        } else {
            0
        };
        let mut units = 0;
        for grapheme in text.graphemes(true) {
            if forward && units > cursor {
                position = safe_start + units;
                break;
            }
            if !forward && units >= cursor {
                break;
            }
            if !forward {
                position = safe_start + units;
            }
            units += grapheme.encode_utf16().count();
        }
        if forward && position == resolved.node.children.units() && units > cursor {
            position = safe_start + units;
        }
        if (!forward && position == safe_start && safe_start > 0)
            || (forward && position >= safe_end && safe_end < resolved.node.children.units())
        {
            return Err("Grapheme exceeds the native navigation window; selection retained".into());
        }
        if (forward && resolved.offset == resolved.node.children.units())
            || (!forward && resolved.offset == 0)
        {
            if let Some(candidate) =
                super::transform::adjacent_block(&self.document, &resolved.path, forward)
            {
                position = candidate;
            } else {
                return Ok(());
            }
        } else {
            position += resolved.start;
        }
        self.selection.head = position;
        if !extend {
            self.selection.anchor = position;
        }
        self.stored_marks = None;
        Ok(())
    }

    pub fn move_word(&mut self, forward: bool, extend: bool) -> EditResult<()> {
        let resolved = self.document.resolve(self.selection.head)?;
        let mut start = resolved.offset.saturating_sub(4096);
        let mut end = (resolved.offset + 4096).min(resolved.node.children.units());
        let through = loop {
            match split_inline(&resolved.node.children, end) {
                Ok((through, _)) => break through,
                Err(_) => end -= 1,
            }
        };
        let slice = loop {
            match split_inline(&through, start) {
                Ok((_, slice)) => break slice,
                Err(_) => start += 1,
            }
        };
        let text = inline_text(&slice);
        let cursor = resolved.offset - start;
        let mut boundaries = vec![0];
        for (byte, word) in text.unicode_word_indices() {
            let position = text[..byte].encode_utf16().count();
            boundaries.push(if forward {
                position + word.encode_utf16().count()
            } else {
                position
            });
        }
        boundaries.push(text.encode_utf16().count());
        let target = if forward {
            boundaries.into_iter().find(|position| *position > cursor)
        } else {
            boundaries
                .into_iter()
                .rev()
                .find(|position| *position < cursor)
        };
        if let Some(target) = target {
            self.selection.head = resolved.start + start + target;
            if !extend {
                self.selection.anchor = self.selection.head;
            }
            self.stored_marks = None;
            Ok(())
        } else {
            self.move_grapheme(forward, extend)
        }
    }

    pub fn insert_table(&mut self) -> EditResult<()> {
        let cell = || {
            Node::fresh(
                "tableCell",
                Sequence::one(Node::fresh("paragraph", Sequence::default())),
            )
        };
        let row = || Node::fresh("tableRow", Sequence::from_items([cell(), cell(), cell()]));
        let table = Node::fresh("table", Sequence::from_items([row(), row(), row()]));
        let mut fragment = self.document.clone();
        fragment.root = Node::fresh("doc", Sequence::one(table));
        self.insert_slice(fragment, false)
    }

    pub fn move_vertical_blocks(
        &mut self,
        forward: bool,
        count: usize,
        extend: bool,
    ) -> EditResult<()> {
        let mut resolved = self.document.resolve(self.selection.head)?;
        let column = resolved.offset;
        for _ in 0..count.clamp(1, 200) {
            let Some(position) =
                super::transform::adjacent_block(&self.document, &resolved.path, forward)
            else {
                break;
            };
            resolved = self.document.resolve(position)?;
        }
        let mut offset = column.min(resolved.node.children.units());
        if split_inline(&resolved.node.children, offset).is_err() {
            offset = offset.saturating_sub(1);
        }
        let position = resolved.start + offset;
        self.select(Selection {
            anchor: if extend {
                self.selection.anchor
            } else {
                position
            },
            head: position,
        });
        Ok(())
    }

    pub fn table_move(&mut self, forward: bool) -> EditResult<()> {
        self.transaction(|model| model.table_move_inner(forward))
    }

    fn table_move_inner(&mut self, forward: bool) -> EditResult<()> {
        let resolved = self.document.resolve(self.selection.head)?;
        let cell_depth = (0..resolved.path.len())
            .find(|index| {
                self.document
                    .node(&resolved.path[..=*index])
                    .is_some_and(|node| matches!(node.kind(), "tableCell" | "tableHeader"))
            })
            .ok_or("Selection is not in a table")?;
        let row_path = &resolved.path[..cell_depth];
        let table_path = &row_path[..row_path.len() - 1];
        let table = self.document.node(table_path).expect("table");
        let row_index = *row_path.last().expect("row");
        let cell_index = resolved.path[cell_depth];
        let row = self.document.node(row_path).expect("row");
        let next = if forward {
            if cell_index + 1 < row.children.len() {
                (row_index, cell_index + 1)
            } else if let Some(next) = (row_index + 1..table.children.len()).find(|index| {
                table
                    .children
                    .get(*index)
                    .is_some_and(|row| row.children.len() > 0)
            }) {
                (next, 0)
            } else {
                self.table_add_row_after(Some(table.children.len() - 1))?;
                (table.children.len(), 0)
            }
        } else if cell_index > 0 {
            (row_index, cell_index - 1)
        } else if let Some(previous) = (0..row_index).rev().find(|index| {
            table
                .children
                .get(*index)
                .is_some_and(|row| row.children.len() > 0)
        }) {
            (
                previous,
                table.children.get(previous).expect("row").children.len() - 1,
            )
        } else {
            return Ok(());
        };
        let mut path = table_path.to_vec();
        path.extend([next.0, next.1, 0]);
        let position = self
            .document
            .position_at_path(&path)
            .ok_or("Table cell has no paragraph")?
            + 1;
        self.select(Selection::caret(position));
        Ok(())
    }

    pub fn table_add_row(&mut self) -> EditResult<()> {
        self.table_add_row_after(None)
    }

    fn table_add_row_after(&mut self, after: Option<usize>) -> EditResult<()> {
        self.ensure_editable()?;
        let resolved = self.document.resolve(self.selection.head)?;
        let depth = (0..resolved.path.len())
            .find(|index| {
                self.document
                    .node(&resolved.path[..=*index])
                    .is_some_and(|node| node.kind() == "table")
            })
            .ok_or("Selection is not in a table")?;
        let path = &resolved.path[..=depth];
        let table = self.document.node(path).expect("table");
        let row_index = after.unwrap_or(resolved.path[depth + 1]);
        let mut occupied = Vec::new();
        let mut rows = table.children.clone();
        for index in 0..=row_index {
            let row = rows.get(index).ok_or("Missing table row")?;
            let mut children = row.children.clone();
            let mut column = 0;
            for cell_index in 0..row.children.len() {
                let cell = row.children.get(cell_index).expect("cell");
                while occupied.get(column).is_some_and(|end| *end > index) {
                    column += 1;
                }
                let colspan = cell
                    .attr("colspan")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 1000) as usize;
                let rowspan = cell
                    .attr("rowspan")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 1000) as usize;
                occupied.resize(occupied.len().max(column + colspan), 0);
                occupied[column..column + colspan].fill(index + rowspan);
                column += colspan;
                if index + rowspan > row_index + 1 {
                    let mut fields = (*cell.fields).clone();
                    let mut attrs = cell.attrs().cloned().unwrap_or_default();
                    attrs.insert("rowspan".into(), json!(rowspan + 1));
                    fields.insert("attrs".into(), attrs.into());
                    children = children.splice(
                        cell_index..cell_index + 1,
                        &Sequence::one(cell.with_fields(fields)),
                    );
                }
            }
            rows = rows.splice(
                index..index + 1,
                &Sequence::one(row.with_children(children)),
            );
        }
        let cells = occupied
            .iter()
            .filter(|end| **end <= row_index + 1)
            .map(|_| {
                Node::fresh(
                    "tableCell",
                    Sequence::one(Node::fresh("paragraph", Sequence::default())),
                )
            })
            .collect::<Vec<_>>();
        let before = self.checkpoint();
        self.document.replace_node(
            path,
            table.with_children(rows.splice(
                row_index + 1..row_index + 1,
                &Sequence::one(Node::fresh("tableRow", Sequence::from_items(cells))),
            )),
        );
        self.record(before, 8192);
        Ok(())
    }

    pub fn toggle_task(&mut self, position: usize) -> EditResult<()> {
        let resolved = self.document.resolve(position)?;
        for depth in (1..resolved.path.len()).rev() {
            let path = &resolved.path[..depth];
            if let Some(node) = self.document.node(path)
                && node.kind() == "taskItem"
            {
                let checked = node.task_done();
                return self.update_node_attrs(
                    path,
                    Map::from_iter([
                        ("checked".into(), json!(!checked)),
                        (
                            "status".into(),
                            json!(if checked { "todo" } else { "done" }),
                        ),
                    ]),
                );
            }
        }
        Err("Selection is not in a task".into())
    }

    pub fn delete(&mut self, forward: bool) -> EditResult<()> {
        if self.selection.is_empty() {
            self.move_grapheme(forward, true)?;
        }
        self.replace(self.selection.range(), "")
    }
}

fn fresh_task(content: Sequence<NodeRef>) -> NodeRef {
    let node = Node::fresh("taskItem", content);
    let mut fields = (*node.fields).clone();
    fields.insert(
        "attrs".into(),
        json!({
            "taskId": uuid::Uuid::new_v4().to_string(),
            "taskItemId": uuid::Uuid::new_v4().to_string(),
            "status": "todo",
            "checked": false
        }),
    );
    node.with_fields(fields)
}

fn toggle_marks(marks: &mut Vec<Value>, kind: &str, enabled: Option<bool>) {
    let active = marks.iter().any(|mark| mark["type"] == kind);
    let enabled = enabled.unwrap_or(!active);
    if enabled && kind == "code" {
        marks.clear();
    }
    if enabled && kind != "code" {
        marks.retain(|mark| mark["type"] != "code");
    }
    marks.retain(|mark| mark["type"] != kind);
    if enabled {
        marks.push(json!({"type": kind}));
    }
}
