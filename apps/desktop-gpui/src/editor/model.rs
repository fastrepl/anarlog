//! Edits applied directly to the TipTap/ProseMirror JSON of a note body, so
//! everything the shell does not understand (mentions, attachments, unknown
//! marks) survives a round trip untouched and `serde_json::to_string` yields
//! the same bytes `doc.toJSON()` would.

use serde_json::{Map, Value, json};

/// Node types whose content is inline text (ProseMirror "textblocks").
const TEXTBLOCKS: [&str; 3] = ["paragraph", "heading", "codeBlock"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    /// Index into [`Doc::textblocks`].
    pub block: usize,
    /// Byte offset into the textblock's plain text.
    pub offset: usize,
}

pub struct Doc {
    root: Value,
    /// Paths (indices into successive `content` arrays) of every textblock,
    /// in document order. Rebuilt after each structural change.
    textblocks: Vec<Vec<usize>>,
}

impl Doc {
    pub fn parse(body: &str) -> Self {
        let root = serde_json::from_str::<Value>(body)
            .ok()
            .filter(|value| value.get("type").and_then(Value::as_str) == Some("doc"))
            .unwrap_or_else(empty_doc);
        let mut doc = Self {
            root,
            textblocks: Vec::new(),
        };
        doc.reindex();
        doc
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.root).expect("json value serialises")
    }

    pub fn root(&self) -> &Value {
        &self.root
    }

    pub fn textblock_count(&self) -> usize {
        self.textblocks.len()
    }

    /// `isPristineNoteDoc`: nothing typed yet.
    pub fn is_pristine(&self) -> bool {
        let content = children(&self.root);
        content.is_empty()
            || (content.len() == 1
                && content[0].get("type").and_then(Value::as_str) == Some("paragraph")
                && children(&content[0]).is_empty())
    }

    /// Plain text of a textblock, matching what the renderer lays out.
    pub fn text(&self, block: usize) -> String {
        self.textblocks
            .get(block)
            .and_then(|path| node_at(&self.root, path))
            .map(plain_text)
            .unwrap_or_default()
    }

    /// Makes sure an empty document has a paragraph to type into, like the
    /// editor's schema (`doc+` requires at least one block).
    pub fn ensure_textblock(&mut self) {
        if self.textblocks.is_empty() {
            let content = self.root_content_mut();
            content.push(json!({ "type": "paragraph" }));
            self.reindex();
        }
    }

    pub fn insert_text(&mut self, caret: Caret, text: &str) -> Caret {
        let Some(path) = self.textblocks.get(caret.block).cloned() else {
            return caret;
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return caret;
        };
        let inline = inline_content_mut(node);
        let (index, offset) = locate(inline, caret.offset);
        match index {
            Some(index) if inline[index].get("type").and_then(Value::as_str) == Some("text") => {
                // `ResolvedPos.marks`: `link` is `inclusive: false`, so text
                // typed at a link's edge takes the node's other marks only,
                // unless the neighbour across the edge carries the same link.
                let len = inline_text(&inline[index]).len();
                let at_edge = (offset == len && len > 0) || (offset == 0 && index == 0);
                if at_edge && has_mark(&inline[index], "link") {
                    let neighbour = if offset == 0 {
                        None
                    } else {
                        inline.get(index + 1)
                    };
                    let same_link = neighbour.is_some_and(|next| {
                        next.get("type").and_then(Value::as_str) == Some("text")
                            && super::links::link_mark_of(next)
                                == super::links::link_mark_of(&inline[index])
                    });
                    if !same_link {
                        let mut piece = inline[index].clone();
                        piece["text"] = Value::String(text.to_string());
                        set_mark(&mut piece, "link", false);
                        let at = if offset == 0 { index } else { index + 1 };
                        inline.insert(at, piece);
                        merge_adjacent_text(inline);
                        return Caret {
                            block: caret.block,
                            offset: caret.offset + text.len(),
                        };
                    }
                }
                let node = &mut inline[index];
                let existing = node.get("text").and_then(Value::as_str).unwrap_or("");
                let mut updated = String::with_capacity(existing.len() + text.len());
                updated.push_str(&existing[..offset]);
                updated.push_str(text);
                updated.push_str(&existing[offset..]);
                node["text"] = Value::String(updated);
            }
            Some(index) => {
                // Boundary next to an atom (hard break, mention): a fresh
                // unmarked text node before it.
                inline.insert(index, text_node(text, None));
            }
            None => inline.push(text_node(text, None)),
        }
        Caret {
            block: caret.block,
            offset: caret.offset + text.len(),
        }
    }

    /// Deletes `range` (byte offsets within one textblock's plain text).
    pub fn delete_range(&mut self, block: usize, range: std::ops::Range<usize>) {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return;
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return;
        };
        let inline = inline_content_mut(node);
        let mut cursor = 0usize;
        let mut kept = Vec::with_capacity(inline.len());
        for mut child in inline.drain(..) {
            let len = inline_text(&child).len();
            let start = cursor;
            let end = cursor + len;
            cursor = end;
            let overlap_start = range.start.max(start);
            let overlap_end = range.end.min(end);
            if overlap_start >= overlap_end {
                kept.push(child);
                continue;
            }
            if child.get("type").and_then(Value::as_str) == Some("text") {
                let text = child.get("text").and_then(Value::as_str).unwrap_or("");
                let mut updated = String::new();
                updated.push_str(&text[..overlap_start - start]);
                updated.push_str(&text[overlap_end - start..]);
                if updated.is_empty() {
                    continue;
                }
                child["text"] = Value::String(updated);
                kept.push(child);
            } else if overlap_start == start && overlap_end == end {
                // Atom fully inside the range: removed.
            } else {
                kept.push(child);
            }
        }
        *inline = kept;
        merge_adjacent_text(inline);
        if inline.is_empty() {
            node.as_object_mut().map(|object| object.remove("content"));
        }
    }

    /// `splitBlock`: the text after the caret moves into a new block after
    /// this one. Splitting at the end of a heading yields a paragraph; inside a
    /// list item the item itself is split so the list keeps its shape.
    pub fn split_block(&mut self, caret: Caret) -> Caret {
        let Some(path) = self.textblocks.get(caret.block).cloned() else {
            return caret;
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return caret;
        };
        let text_len = plain_text(node).len();
        let (head, tail) = split_inline(inline_content_mut(node), caret.offset);
        let node_type = node
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("paragraph")
            .to_string();
        let attrs = node.get("attrs").cloned();
        set_inline_content(node, head);
        let new_type = if caret.offset >= text_len || node_type == "codeBlock" {
            "paragraph".to_string()
        } else {
            node_type
        };
        let mut new_block = Map::new();
        new_block.insert("type".into(), Value::String(new_type.clone()));
        if new_type != "paragraph"
            && let Some(attrs) = attrs
        {
            new_block.insert("attrs".into(), attrs);
        }
        let mut new_block = Value::Object(new_block);
        set_inline_content(&mut new_block, tail);

        // A textblock directly inside a list item splits the item instead.
        let (parent_path, index) = path.split_at(path.len() - 1);
        let parent_is_item = node_at(&self.root, parent_path)
            .and_then(|parent| parent.get("type").and_then(Value::as_str))
            .is_some_and(|kind| kind == "listItem" || kind == "taskItem");
        if parent_is_item && !parent_path.is_empty() {
            let (list_path, item_index) = parent_path.split_at(parent_path.len() - 1);
            let item = node_at(&self.root, parent_path)
                .cloned()
                .unwrap_or(Value::Null);
            let item_type = item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("listItem")
                .to_string();
            let mut new_item = Map::new();
            new_item.insert("type".into(), Value::String(item_type));
            // `splitListItem(itemType)` without `itemAttrs` copies the item's
            // attrs; a task item's duplicated ids are then re-issued by the
            // identity sweep (`ensure_task_identity`).
            if let Some(attrs) = item.get("attrs") {
                new_item.insert("attrs".into(), attrs.clone());
            }
            new_item.insert("content".into(), Value::Array(vec![new_block]));
            if let Some(list) = node_at_mut(&mut self.root, list_path) {
                let siblings = content_mut(list);
                siblings.insert(item_index[0] + 1, Value::Object(new_item));
            }
        } else if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
            content_mut(parent).insert(index[0] + 1, new_block);
        }
        self.reindex();
        Caret {
            block: caret.block + 1,
            offset: 0,
        }
    }

    /// `joinBackward` from the start of a textblock: its content joins the
    /// previous textblock. Returns the caret at the join point.
    pub fn join_backward(&mut self, block: usize) -> Option<Caret> {
        if block == 0 || block >= self.textblocks.len() {
            return None;
        }
        let path = self.textblocks[block].clone();
        let previous_path = self.textblocks[block - 1].clone();
        let node = node_at(&self.root, &path)?.clone();
        let moved = children(&node).to_vec();
        let previous = node_at_mut(&mut self.root, &previous_path)?;
        let join_offset = plain_text(previous).len();
        let inline = inline_content_mut(previous);
        inline.extend(moved);
        merge_adjacent_text(inline);
        if inline.is_empty() {
            previous
                .as_object_mut()
                .map(|object| object.remove("content"));
        }
        self.remove_node(&path);
        self.reindex();
        Some(Caret {
            block: block - 1,
            offset: join_offset,
        })
    }

    /// Plain text between two carets, blocks joined with newlines.
    pub fn text_between(&self, from: Caret, to: Caret) -> String {
        let (from, to) = order(from, to);
        if from.block == to.block {
            let text = self.text(from.block);
            return text[from.offset.min(text.len())..to.offset.min(text.len())].to_string();
        }
        let mut out = String::new();
        let first = self.text(from.block);
        out.push_str(&first[from.offset.min(first.len())..]);
        for block in from.block + 1..to.block {
            out.push('\n');
            out.push_str(&self.text(block));
        }
        out.push('\n');
        let last = self.text(to.block);
        out.push_str(&last[..to.offset.min(last.len())]);
        out
    }

    /// `deleteSelection`: removes everything between two carets, joining the
    /// end block into the start block. Returns the collapsed caret.
    pub fn delete_between(&mut self, from: Caret, to: Caret) -> Caret {
        let (from, to) = order(from, to);
        if from == to {
            return from;
        }
        if from.block == to.block {
            self.delete_range(from.block, from.offset..to.offset);
            return from;
        }
        let first_len = self.text(from.block).len();
        self.delete_range(from.block, from.offset..first_len);
        self.delete_range(to.block, 0..to.offset);
        // Drop the blocks strictly in between, last first so indices hold.
        for block in (from.block + 1..to.block).rev() {
            let path = self.textblocks[block].clone();
            self.remove_node(&path);
            self.reindex();
        }
        // `to` is now directly after `from`.
        self.join_backward(from.block + 1);
        from
    }

    /// Whether every character between the carets carries `mark`
    /// (ProseMirror's `rangeHasMark`, which drives `toggleMark`).
    pub fn range_has_mark(&self, from: Caret, to: Caret, mark: &str) -> bool {
        let (from, to) = order(from, to);
        let mut any = false;
        for block in from.block..=to.block {
            let Some(node) = self
                .textblocks
                .get(block)
                .and_then(|path| node_at(&self.root, path))
            else {
                continue;
            };
            let start = if block == from.block { from.offset } else { 0 };
            let end = if block == to.block {
                to.offset
            } else {
                plain_text(node).len()
            };
            let mut cursor = 0usize;
            for child in children(node) {
                let len = inline_text(child).len();
                let (a, b) = (cursor, cursor + len);
                cursor = b;
                if a.max(start) >= b.min(end)
                    || child.get("type").and_then(Value::as_str) != Some("text")
                {
                    continue;
                }
                any = true;
                if !has_mark(child, mark) {
                    return false;
                }
            }
        }
        any
    }

    /// `toggleMark`: adds the mark to every text node in the range (splitting
    /// nodes at the boundaries) or removes it when the whole range has it.
    pub fn toggle_mark(&mut self, from: Caret, to: Caret, mark: &str) {
        let (from, to) = order(from, to);
        let add = !self.range_has_mark(from, to, mark);
        for block in from.block..=to.block {
            let Some(path) = self.textblocks.get(block).cloned() else {
                continue;
            };
            let Some(node) = node_at_mut(&mut self.root, &path) else {
                continue;
            };
            let start = if block == from.block { from.offset } else { 0 };
            let end = if block == to.block {
                to.offset
            } else {
                plain_text(node).len()
            };
            let inline = inline_content_mut(node);
            let mut cursor = 0usize;
            let mut rebuilt = Vec::with_capacity(inline.len() + 2);
            for child in inline.drain(..) {
                let len = inline_text(&child).len();
                let (a, b) = (cursor, cursor + len);
                cursor = b;
                let is_text = child.get("type").and_then(Value::as_str) == Some("text");
                let (lo, hi) = (a.max(start), b.min(end));
                if !is_text || lo >= hi {
                    rebuilt.push(child);
                    continue;
                }
                let text = child
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let pieces = [(a, lo, false), (lo, hi, true), (hi, b, false)];
                for (s, e, inside) in pieces {
                    if s >= e {
                        continue;
                    }
                    let mut piece = child.clone();
                    piece["text"] = Value::String(text[s - a..e - a].to_string());
                    if inside {
                        set_mark(&mut piece, mark, add);
                    }
                    rebuilt.push(piece);
                }
            }
            *inline = rebuilt;
            merge_adjacent_text(inline);
        }
    }

    /// `taskIdentityPlugin`: unique, non-empty task ids after a change.
    pub fn ensure_task_identity(&mut self) -> bool {
        super::tasks::ensure_identity(&mut self.root)
    }

    /// Path of the `taskItem` a textblock sits in, if any.
    fn task_item_path(&self, block: usize) -> Option<Vec<usize>> {
        let path = self.textblocks.get(block)?;
        (1..path.len()).rev().find_map(|depth| {
            let candidate = &path[..depth];
            (node_at(&self.root, candidate)?
                .get("type")
                .and_then(Value::as_str)
                == Some("taskItem"))
            .then(|| candidate.to_vec())
        })
    }

    /// `TaskItemView`'s toggle: `setNodeMarkup` with the next status.
    pub fn toggle_task(&mut self, block: usize) -> bool {
        let Some(path) = self.task_item_path(block) else {
            return false;
        };
        let Some(item) = node_at_mut(&mut self.root, &path) else {
            return false;
        };
        let next = super::tasks::next_status(super::tasks::item_status(item));
        let object = item.as_object_mut().expect("task item object");
        let mut attrs = object
            .get("attrs")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        super::tasks::set_status(&mut attrs, next);
        let content = object.remove("content");
        object.remove("attrs");
        object.insert("attrs".into(), Value::Object(attrs));
        if let Some(content) = content {
            object.insert("content".into(), content);
        }
        true
    }

    /// `taskListRule`: `tr.replaceWith(start - 1, end, taskList)` puts a task
    /// list holding one item and the (emptied) paragraph where it stood.
    pub fn replace_with_task_list(&mut self, block: usize, checked: bool) -> Caret {
        let caret = Caret { block, offset: 0 };
        let Some(path) = self.textblocks.get(block).cloned() else {
            return caret;
        };
        let Some(paragraph) = node_at(&self.root, &path).cloned() else {
            return caret;
        };
        let status = if checked { "done" } else { "todo" };
        let attrs =
            super::tasks::item_attrs(status, &super::tasks::new_id(), &super::tasks::new_id());
        let task_list = json!({
            "type": "taskList",
            "content": [{ "type": "taskItem", "attrs": attrs, "content": [paragraph] }]
        });
        let (parent_path, index) = path.split_at(path.len() - 1);
        if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
            let siblings = content_mut(parent);
            if index[0] < siblings.len() {
                siblings[index[0]] = task_list;
            }
        }
        self.reindex();
        caret
    }

    /// `taskListRule` fired in a list item's first paragraph: `replaceWith`
    /// cannot drop the paragraph the item must start with, so ProseMirror's
    /// fitter keeps it (emptied) and places the task list after it. The caret
    /// lands in the new item's paragraph.
    pub fn insert_task_list_after(&mut self, block: usize, checked: bool) -> Caret {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return Caret { block, offset: 0 };
        };
        let status = if checked { "done" } else { "todo" };
        let attrs =
            super::tasks::item_attrs(status, &super::tasks::new_id(), &super::tasks::new_id());
        let task_list = json!({
            "type": "taskList",
            "content": [{ "type": "taskItem", "attrs": attrs, "content": [{ "type": "paragraph" }] }]
        });
        let (parent_path, index) = path.split_at(path.len() - 1);
        if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
            content_mut(parent).insert(index[0] + 1, task_list);
        }
        self.reindex();
        Caret {
            block: block + 1,
            offset: 0,
        }
    }

    /// The autolink and link-boundary-guard passes ProseMirror appends to
    /// every change, over one textblock; `changed` is the block-relative
    /// range the change touched (zero-width for a deletion or caret edit).
    pub fn maintain_links(&mut self, block: usize, changed: std::ops::Range<usize>) {
        use super::links::{autolink_edits, boundary_guard_edits};
        if self.block_type(block).as_deref() == Some("codeBlock") {
            return;
        }
        let guard_edits = {
            let Some(node) = self
                .textblocks
                .get(block)
                .and_then(|path| node_at(&self.root, path))
            else {
                return;
            };
            boundary_guard_edits(&positioned(children(node)), &changed)
        };
        for edit in guard_edits {
            self.apply_link_edit(block, edit);
        }
        let auto_edits = {
            let Some(node) = self
                .textblocks
                .get(block)
                .and_then(|path| node_at(&self.root, path))
            else {
                return;
            };
            let inline = positioned(children(node));
            let linked: Vec<(usize, usize)> = inline
                .iter()
                .filter(|(_, child)| has_mark(child, "link"))
                .map(|(pos, child)| (*pos, pos + inline_text(child).len()))
                .collect();
            // `rangeHasMark`: any linked text inside the candidate range.
            autolink_edits(&inline, |from, to| {
                linked.iter().any(|(a, b)| *a < to && from < *b)
            })
        };
        for edit in auto_edits {
            self.apply_link_edit(block, edit);
        }
    }

    /// `ResolvedPos.marksAcross` for the `link` mark: the link on the text
    /// under `from`, kept across a replacement only when the text at `to`
    /// carries the same link (so retyping inside a link keeps it, and
    /// replacing the whole link drops it).
    pub fn link_across(&self, from: Caret, to: Caret) -> Option<Value> {
        if from.block != to.block {
            return None;
        }
        let node = self
            .textblocks
            .get(from.block)
            .and_then(|path| node_at(&self.root, path))?;
        let inline = positioned(children(node));
        let node_after = |offset: usize| {
            inline
                .iter()
                .find(|(pos, child)| *pos <= offset && offset < pos + inline_text(child).len())
                .or_else(|| inline.iter().find(|(pos, _)| *pos == offset))
                .map(|(_, child)| *child)
        };
        let start = node_after(from.offset)?;
        let mark = super::links::link_mark_of(start)?.clone();
        let end = node_after(to.offset)?;
        (super::links::link_mark_of(end) == Some(&mark)).then_some(mark)
    }

    /// The `href` of the link on the character at `offset`, if any.
    pub fn link_href_at(&self, block: usize, offset: usize) -> Option<String> {
        let node = self
            .textblocks
            .get(block)
            .and_then(|path| node_at(&self.root, path))?;
        positioned(children(node))
            .into_iter()
            .find(|(pos, child)| *pos <= offset && offset < pos + inline_text(child).len())
            .and_then(|(_, child)| super::links::link_href(child).map(str::to_string))
    }

    /// Puts `mark` (a full `link` mark) on a block-relative byte range.
    pub fn set_link(&mut self, block: usize, from: usize, to: usize, mark: Value) {
        self.set_link_range(block, from, to, Some(mark));
    }

    fn apply_link_edit(&mut self, block: usize, edit: super::links::LinkEdit) {
        use super::links::LinkEdit;
        let (from, to, mark) = match edit {
            LinkEdit::Remove { from, to } => (from, to, None),
            LinkEdit::Set { from, to, mark } => (from, to, Some(mark)),
        };
        self.set_link_range(block, from, to, mark);
    }

    /// `removeMark` / `addMark` for the `link` mark over a block-relative
    /// byte range, splitting text nodes at the boundaries like `toggle_mark`.
    fn set_link_range(&mut self, block: usize, start: usize, end: usize, mark: Option<Value>) {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return;
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return;
        };
        let inline = inline_content_mut(node);
        let mut cursor = 0usize;
        let mut rebuilt = Vec::with_capacity(inline.len() + 2);
        for child in inline.drain(..) {
            let len = inline_text(&child).len();
            let (a, b) = (cursor, cursor + len);
            cursor = b;
            let is_text = child.get("type").and_then(Value::as_str) == Some("text");
            let (lo, hi) = (a.max(start), b.min(end));
            if !is_text || lo >= hi {
                rebuilt.push(child);
                continue;
            }
            let text = child
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            for (s, e, inside) in [(a, lo, false), (lo, hi, true), (hi, b, false)] {
                if s >= e {
                    continue;
                }
                let mut piece = child.clone();
                piece["text"] = Value::String(text[s - a..e - a].to_string());
                if inside {
                    set_mark(&mut piece, "link", false);
                    if let Some(mark) = &mark {
                        add_mark_value(&mut piece, mark.clone());
                    }
                }
                rebuilt.push(piece);
            }
        }
        *inline = rebuilt;
        merge_adjacent_text(inline);
    }

    /// Node type of a textblock (`paragraph`, `heading`, `codeBlock`).
    pub fn block_type(&self, block: usize) -> Option<String> {
        self.textblocks
            .get(block)
            .and_then(|path| node_at(&self.root, path))
            .and_then(|node| node.get("type").and_then(Value::as_str))
            .map(str::to_string)
    }

    /// Node type of the textblock's parent (`doc`, `listItem`, `blockquote`...).
    pub fn parent_type(&self, block: usize) -> Option<String> {
        let path = self.textblocks.get(block)?;
        node_at(&self.root, &path[..path.len() - 1])
            .and_then(|node| node.get("type").and_then(Value::as_str))
            .map(str::to_string)
    }

    /// `setBlockType`: change a textblock's type, replacing its attrs.
    pub fn set_block_type(&mut self, block: usize, kind: &str, attrs: Option<Value>) {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return;
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return;
        };
        let content = node.get("content").cloned();
        let mut object = Map::new();
        object.insert("type".into(), Value::String(kind.to_string()));
        if let Some(attrs) = attrs {
            object.insert("attrs".into(), attrs);
        }
        if let Some(content) = content {
            object.insert("content".into(), content);
        }
        *node = Value::Object(object);
        self.reindex();
    }

    /// `wrappingInputRule`: wrap the textblock in `list > listItem` (or a
    /// `blockquote`). A preceding sibling list of the same type absorbs the
    /// new item when `join_previous` holds, like `canJoin` + `joinPredicate`.
    pub fn wrap_block(
        &mut self,
        block: usize,
        wrapper: &str,
        attrs: Option<Value>,
        join_previous: bool,
    ) {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return;
        };
        let (parent_path, index) = path.split_at(path.len() - 1);
        let index = index[0];
        let Some(parent) = node_at_mut(&mut self.root, parent_path) else {
            return;
        };
        let siblings = content_mut(parent);
        let textblock = siblings.remove(index);
        let wrapped = if wrapper == "blockquote" {
            let mut quote = Map::new();
            quote.insert("type".into(), Value::String("blockquote".into()));
            quote.insert("content".into(), Value::Array(vec![textblock]));
            Value::Object(quote)
        } else {
            let mut item = Map::new();
            item.insert("type".into(), Value::String("listItem".into()));
            item.insert("content".into(), Value::Array(vec![textblock]));
            if join_previous
                && index > 0
                && siblings[index - 1].get("type").and_then(Value::as_str) == Some(wrapper)
            {
                content_mut(&mut siblings[index - 1]).push(Value::Object(item));
                self.reindex();
                return;
            }
            let mut list = Map::new();
            list.insert("type".into(), Value::String(wrapper.to_string()));
            if let Some(attrs) = attrs {
                list.insert("attrs".into(), attrs);
            }
            list.insert("content".into(), Value::Array(vec![Value::Object(item)]));
            Value::Object(list)
        };
        siblings.insert(index, wrapped);
        self.reindex();
    }

    /// `orderedListRule`'s `joinPredicate`: the sibling before the textblock
    /// is a list of `kind` whose numbering continues at `number`.
    pub fn previous_sibling_list_continues_at(
        &self,
        block: usize,
        kind: &str,
        number: u64,
    ) -> bool {
        let Some(path) = self.textblocks.get(block) else {
            return false;
        };
        let (parent_path, index) = path.split_at(path.len() - 1);
        if index[0] == 0 {
            return false;
        }
        let Some(previous) =
            node_at(&self.root, parent_path).and_then(|parent| children(parent).get(index[0] - 1))
        else {
            return false;
        };
        if previous.get("type").and_then(Value::as_str) != Some(kind) {
            return false;
        }
        let start = previous
            .get("attrs")
            .and_then(|attrs| attrs.get("start"))
            .and_then(Value::as_u64)
            .unwrap_or(1);
        children(previous).len() as u64 + start == number
    }

    /// `horizontalRuleRule`: the textblock (which held only the shortcut)
    /// becomes a rule followed by an empty paragraph.
    pub fn replace_block_with_rule(&mut self, block: usize) -> Caret {
        let Some(path) = self.textblocks.get(block).cloned() else {
            return Caret { block, offset: 0 };
        };
        let (parent_path, index) = path.split_at(path.len() - 1);
        let index = index[0];
        if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
            let siblings = content_mut(parent);
            siblings[index] = json!({ "type": "horizontalRule" });
            siblings.insert(index + 1, json!({ "type": "paragraph" }));
        }
        self.reindex();
        Caret { block, offset: 0 }
    }

    /// Number of items the list containing `block` has and the item's index,
    /// when the block is the first child of a list item.
    fn list_item_position(&self, block: usize) -> Option<(Vec<usize>, usize)> {
        let path = self.textblocks.get(block)?;
        if path.len() < 3 || path[path.len() - 1] != 0 {
            return None;
        }
        let item_path = &path[..path.len() - 1];
        let list_path = &item_path[..item_path.len() - 1];
        let item = node_at(&self.root, item_path)?;
        let kind = item.get("type").and_then(Value::as_str)?;
        if kind != "listItem" && kind != "taskItem" {
            return None;
        }
        Some((list_path.to_vec(), item_path[item_path.len() - 1]))
    }

    /// Whether the textblock starts a list item.
    pub fn in_list_item(&self, block: usize) -> bool {
        self.list_item_position(block).is_some()
    }

    /// Whether the textblock is its parent's first child.
    pub fn is_first_child(&self, block: usize) -> bool {
        self.textblocks
            .get(block)
            .and_then(|path| path.last())
            .is_some_and(|index| *index == 0)
    }

    /// Whether the textblock starts the first item of its list.
    pub fn is_first_list_item(&self, block: usize) -> bool {
        self.list_item_position(block)
            .is_some_and(|(_, index)| index == 0)
    }

    /// `liftListItem` for a top-level item: its content leaves the list. A
    /// first item goes before the list, a last item after it, and a middle
    /// item splits the list in two around it.
    pub fn lift_list_item(&mut self, block: usize) -> Option<Caret> {
        let (list_path, item_index) = self.list_item_position(block)?;
        let (grand_path, list_index) = list_path.split_at(list_path.len() - 1);
        let list_index = list_index[0];
        let list = node_at_mut(&mut self.root, &list_path)?;
        let items = content_mut(list);
        let item = items.remove(item_index);
        let lifted = children(&item).to_vec();
        let remaining_after: Vec<Value> = if item_index < items.len() {
            items.drain(item_index..).collect()
        } else {
            Vec::new()
        };
        let list_empty = items.is_empty();
        let list_type = list
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("bulletList")
            .to_string();
        let list_attrs = list.get("attrs").cloned();

        let grand = node_at_mut(&mut self.root, grand_path)?;
        let siblings = content_mut(grand);
        let mut insert_at = list_index + 1;
        if list_empty {
            siblings.remove(list_index);
            insert_at = list_index;
        } else if item_index == 0 {
            insert_at = list_index;
        }
        let lifted_len = lifted.len();
        for (offset, node) in lifted.into_iter().enumerate() {
            siblings.insert(insert_at + offset, node);
        }
        if !remaining_after.is_empty() {
            let mut tail = Map::new();
            tail.insert("type".into(), Value::String(list_type));
            if let Some(attrs) = list_attrs {
                tail.insert("attrs".into(), attrs);
            }
            tail.insert("content".into(), Value::Array(remaining_after));
            siblings.insert(insert_at + lifted_len, Value::Object(tail));
        }
        self.reindex();
        Some(Caret { block, offset: 0 })
    }

    /// `sinkListItem`: nest the item under the previous item as a sublist.
    pub fn sink_list_item(&mut self, block: usize) -> bool {
        let Some((list_path, item_index)) = self.list_item_position(block) else {
            return false;
        };
        if item_index == 0 {
            return false;
        }
        let Some(list) = node_at_mut(&mut self.root, &list_path) else {
            return false;
        };
        let list_type = list
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("bulletList")
            .to_string();
        let items = content_mut(list);
        let item = items.remove(item_index);
        let previous = &mut items[item_index - 1];
        let previous_children = content_mut(previous);
        match previous_children.last_mut() {
            Some(last) if last.get("type").and_then(Value::as_str) == Some(list_type.as_str()) => {
                content_mut(last).push(item);
            }
            _ => {
                let mut sublist = Map::new();
                sublist.insert("type".into(), Value::String(list_type));
                sublist.insert("content".into(), Value::Array(vec![item]));
                previous_children.push(Value::Object(sublist));
            }
        }
        self.reindex();
        true
    }

    /// Marks typed text would inherit at the caret (`$from.marks()`): those
    /// of the text node before it, or after it at the start of the block.
    pub fn marks_at(&self, caret: Caret) -> Vec<&'static str> {
        let Some(node) = self
            .textblocks
            .get(caret.block)
            .and_then(|path| node_at(&self.root, path))
        else {
            return Vec::new();
        };
        let inline = children(node);
        let (index, _) = locate(inline, caret.offset);
        let Some(child) = index.and_then(|index| inline.get(index)) else {
            return Vec::new();
        };
        MARK_RANK
            .iter()
            .copied()
            .filter(|mark| *mark != "link" && has_mark(child, mark))
            .collect()
    }

    /// Makes the text between the carets carry exactly `marks` (links aside).
    pub fn set_marks(&mut self, from: Caret, to: Caret, marks: &[&'static str]) {
        for mark in MARK_RANK.iter().copied().filter(|mark| *mark != "link") {
            let wanted = marks.contains(&mark);
            if self.range_has_mark(from, to, mark) != wanted {
                self.toggle_mark(from, to, mark);
            }
        }
    }

    /// Removes a textblock and any list item / list left empty by it.
    fn remove_node(&mut self, path: &[usize]) {
        let (parent_path, index) = path.split_at(path.len() - 1);
        let Some(parent) = node_at_mut(&mut self.root, parent_path) else {
            return;
        };
        let parent_type = parent
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let siblings = content_mut(parent);
        if index[0] < siblings.len() {
            siblings.remove(index[0]);
        }
        let now_empty = siblings.is_empty();
        let removable = matches!(
            parent_type.as_str(),
            "listItem" | "taskItem" | "bulletList" | "orderedList" | "taskList" | "blockquote"
        );
        if now_empty && removable && !parent_path.is_empty() {
            self.remove_node(parent_path);
        } else if now_empty {
            parent
                .as_object_mut()
                .map(|object| object.remove("content"));
        }
    }

    fn root_content_mut(&mut self) -> &mut Vec<Value> {
        content_mut(&mut self.root)
    }

    fn reindex(&mut self) {
        let mut paths = Vec::new();
        collect_textblocks(&self.root, &mut Vec::new(), &mut paths);
        self.textblocks = paths;
    }
}

fn empty_doc() -> Value {
    json!({ "type": "doc", "content": [] })
}

fn collect_textblocks(node: &Value, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
    if TEXTBLOCKS.contains(&kind) {
        out.push(path.clone());
        return;
    }
    for (index, child) in children(node).iter().enumerate() {
        path.push(index);
        collect_textblocks(child, path, out);
        path.pop();
    }
}

fn children(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn content_mut(node: &mut Value) -> &mut Vec<Value> {
    let object = node.as_object_mut().expect("prosemirror nodes are objects");
    if !object.get("content").is_some_and(Value::is_array) {
        object.insert("content".into(), Value::Array(Vec::new()));
    }
    object
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .expect("content is an array")
}

fn inline_content_mut(node: &mut Value) -> &mut Vec<Value> {
    content_mut(node)
}

fn set_inline_content(node: &mut Value, inline: Vec<Value>) {
    let object = node.as_object_mut().expect("node object");
    if inline.is_empty() {
        object.remove("content");
    } else {
        object.insert("content".into(), Value::Array(inline));
    }
}

fn node_at<'a>(root: &'a Value, path: &[usize]) -> Option<&'a Value> {
    path.iter()
        .try_fold(root, |node, &index| children(node).get(index))
}

fn node_at_mut<'a>(root: &'a mut Value, path: &[usize]) -> Option<&'a mut Value> {
    path.iter().try_fold(root, |node, &index| {
        node.get_mut("content")
            .and_then(Value::as_array_mut)
            .and_then(|content| content.get_mut(index))
    })
}

/// Text an inline node contributes to the block's plain text; must agree
/// with `document::inline_nodes`.
fn inline_text(node: &Value) -> String {
    match node.get("type").and_then(Value::as_str) {
        Some("text") => node
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        Some("hardBreak") => "\n".to_string(),
        Some(kind) if kind.starts_with("mention-") => {
            let label = node
                .get("attrs")
                .and_then(|attrs| attrs.get("label"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("@{label}")
        }
        _ => String::new(),
    }
}

fn plain_text(node: &Value) -> String {
    children(node).iter().map(inline_text).collect()
}

/// Which inline child a byte offset falls in, and the offset within it.
/// Boundaries prefer the preceding text node (ProseMirror inherits the marks
/// before the cursor), except at the very start of the block.
fn locate(inline: &[Value], offset: usize) -> (Option<usize>, usize) {
    let mut cursor = 0usize;
    for (index, child) in inline.iter().enumerate() {
        let len = inline_text(child).len();
        let is_text = child.get("type").and_then(Value::as_str) == Some("text");
        if offset < cursor + len || (offset == cursor + len && is_text && (offset > 0 || len > 0)) {
            if is_text {
                return (Some(index), offset - cursor);
            }
            // Inside or at the end of an atom: insert after it.
            let next = index + 1;
            if next < inline.len()
                && inline[next].get("type").and_then(Value::as_str) == Some("text")
            {
                return (Some(next), 0);
            }
            return (
                if next < inline.len() {
                    Some(next)
                } else {
                    None
                },
                0,
            );
        }
        cursor += len;
    }
    (None, 0)
}

fn split_inline(inline: &mut Vec<Value>, offset: usize) -> (Vec<Value>, Vec<Value>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut cursor = 0usize;
    for child in inline.drain(..) {
        let len = inline_text(&child).len();
        let start = cursor;
        cursor += len;
        if cursor <= offset {
            head.push(child);
        } else if start >= offset {
            tail.push(child);
        } else if child.get("type").and_then(Value::as_str) == Some("text") {
            let text = child
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let split_at = offset - start;
            let mut left = child.clone();
            left["text"] = Value::String(text[..split_at].to_string());
            let mut right = child;
            right["text"] = Value::String(text[split_at..].to_string());
            head.push(left);
            tail.push(right);
        } else {
            tail.push(child);
        }
    }
    (head, tail)
}

pub fn order(a: Caret, b: Caret) -> (Caret, Caret) {
    if (a.block, a.offset) <= (b.block, b.offset) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Mark names in schema order (`packages/editor/src/note/schema.ts`), which
/// is the rank ProseMirror keeps a node's `marks` array sorted by.
const MARK_RANK: [&str; 7] = [
    "bold",
    "italic",
    "underline",
    "strike",
    "code",
    "link",
    "highlight",
];

fn mark_rank(mark: &Value) -> usize {
    mark.get("type")
        .and_then(Value::as_str)
        .and_then(|name| MARK_RANK.iter().position(|m| *m == name))
        .unwrap_or(MARK_RANK.len())
}

fn has_mark(node: &Value, mark: &str) -> bool {
    node.get("marks")
        .and_then(Value::as_array)
        .is_some_and(|marks| {
            marks
                .iter()
                .any(|m| m.get("type").and_then(Value::as_str) == Some(mark))
        })
}

/// `Mark.addToSet` / `removeFromSet`; an empty set drops the `marks` key like
/// `Node.toJSON` does.
fn set_mark(node: &mut Value, mark: &str, add: bool) {
    let mut marks: Vec<Value> = node
        .get("marks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    marks.retain(|m| m.get("type").and_then(Value::as_str) != Some(mark));
    if add {
        marks.push(json!({ "type": mark }));
        marks.sort_by_key(mark_rank);
    }
    let object = node.as_object_mut().expect("text node object");
    if marks.is_empty() {
        object.remove("marks");
    } else {
        // Keep TipTap's key order: type, marks, text.
        let text = object.remove("text");
        object.remove("marks");
        object.insert("marks".into(), Value::Array(marks));
        if let Some(text) = text {
            object.insert("text".into(), text);
        }
    }
}

/// Inline children with their block-relative byte offsets.
fn positioned(inline: &[Value]) -> Vec<(usize, &Value)> {
    let mut cursor = 0usize;
    inline
        .iter()
        .map(|child| {
            let pos = cursor;
            cursor += inline_text(child).len();
            (pos, child)
        })
        .collect()
}

/// `Mark.addToSet` with a full mark value (type and attrs), keeping rank order.
fn add_mark_value(node: &mut Value, mark: Value) {
    let name = mark
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut marks: Vec<Value> = node
        .get("marks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    marks.retain(|m| m.get("type").and_then(Value::as_str) != Some(name.as_str()));
    marks.push(mark);
    marks.sort_by_key(mark_rank);
    let object = node.as_object_mut().expect("text node object");
    let text = object.remove("text");
    object.remove("marks");
    object.insert("marks".into(), Value::Array(marks));
    if let Some(text) = text {
        object.insert("text".into(), text);
    }
}

fn text_node(text: &str, marks: Option<Value>) -> Value {
    let mut node = Map::new();
    node.insert("type".into(), Value::String("text".into()));
    if let Some(marks) = marks {
        node.insert("marks".into(), marks);
    }
    node.insert("text".into(), Value::String(text.to_string()));
    Value::Object(node)
}

/// ProseMirror normalises adjacent text nodes with identical marks into one.
fn merge_adjacent_text(inline: &mut Vec<Value>) {
    let mut merged: Vec<Value> = Vec::with_capacity(inline.len());
    for child in inline.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.get("type").and_then(Value::as_str) == Some("text")
            && child.get("type").and_then(Value::as_str) == Some("text")
            && last.get("marks") == child.get("marks")
        {
            let combined = format!(
                "{}{}",
                last.get("text").and_then(Value::as_str).unwrap_or(""),
                child.get("text").and_then(Value::as_str).unwrap_or("")
            );
            last["text"] = Value::String(combined);
            continue;
        }
        merged.push(child);
    }
    *inline = merged;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret(block: usize, offset: usize) -> Caret {
        Caret { block, offset }
    }

    #[test]
    fn typing_into_an_empty_note_produces_tiptap_json() {
        let mut doc = Doc::parse("");
        assert!(doc.is_pristine());
        doc.ensure_textblock();
        assert!(doc.is_pristine());
        let c = doc.insert_text(caret(0, 0), "Hi");
        assert_eq!(c, caret(0, 2));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hi"}]}]}"#
        );
        assert!(!doc.is_pristine());
    }

    #[test]
    fn inserting_at_a_boundary_inherits_the_preceding_marks() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Gamma body with "},{"type":"text","marks":[{"type":"bold"}],"text":"bold"},{"type":"text","text":" word."}]},{"type":"paragraph"}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(doc.textblock_count(), 2);
        assert_eq!(doc.text(0), "Gamma body with bold word.");
        doc.insert_text(caret(0, 20), "er");
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Gamma body with "},{"type":"text","marks":[{"type":"bold"}],"text":"bolder"},{"type":"text","text":" word."}]},{"type":"paragraph"}]}"#
        );
        // Untouched documents serialise byte-for-byte.
        assert_eq!(Doc::parse(body).to_json(), body);
    }

    #[test]
    fn deleting_across_nodes_merges_neighbours_and_drops_empty_content() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"ab"},{"type":"text","marks":[{"type":"bold"}],"text":"cd"},{"type":"text","text":"ef"}]}]}"#;
        let mut doc = Doc::parse(body);
        doc.delete_range(0, 2..4);
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"abef"}]}]}"#
        );
        doc.delete_range(0, 0..4);
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph"}]}"#
        );
        assert!(doc.is_pristine());
    }

    #[test]
    fn enter_splits_paragraphs_and_headings_like_split_block() {
        let body = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Title"}]},{"type":"paragraph","content":[{"type":"text","text":"one two"}]}]}"#;
        let mut doc = Doc::parse(body);
        let c = doc.split_block(caret(1, 3));
        assert_eq!(c, caret(2, 0));
        assert_eq!(doc.text(1), "one");
        assert_eq!(doc.text(2), " two");
        // End of a heading: the new block is a paragraph, a mid-heading split keeps the heading.
        let c = doc.split_block(caret(0, 5));
        assert_eq!(c, caret(1, 0));
        assert_eq!(doc.root()["content"][1], json!({ "type": "paragraph" }));
        doc.split_block(caret(0, 2));
        assert_eq!(
            doc.root()["content"][1],
            json!({ "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "tle" }] })
        );
    }

    #[test]
    fn enter_inside_a_list_item_splits_the_item() {
        let body = r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        let c = doc.split_block(caret(0, 5));
        assert_eq!(c, caret(1, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]}]},{"type":"listItem","content":[{"type":"paragraph"}]}]}]}"#
        );
        // Backspace at the start of the new item removes it and rejoins.
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(0, 5));
        assert_eq!(doc.to_json(), body);
    }

    #[test]
    fn backspace_at_block_start_joins_and_removes_empty_lists() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(0, 1));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"ab"}]}]}"#
        );
        assert!(doc.join_backward(0).is_none());
    }

    #[test]
    fn deleting_across_blocks_joins_the_ends() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one two"}]},{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"gone"}]},{"type":"paragraph","content":[{"type":"text","text":"three four"}]}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(
            doc.text_between(caret(0, 4), caret(2, 5)),
            "two\ngone\nthree"
        );
        let c = doc.delete_between(caret(2, 5), caret(0, 4));
        assert_eq!(c, caret(0, 4));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one  four"}]}]}"#
        );
    }

    #[test]
    fn toggling_marks_splits_nodes_and_keeps_schema_rank_order() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"make this bold"}]}]}"#;
        let mut doc = Doc::parse(body);
        doc.toggle_mark(caret(0, 5), caret(0, 9), "bold");
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"make "},{"type":"text","marks":[{"type":"bold"}],"text":"this"},{"type":"text","text":" bold"}]}]}"#
        );
        // Italic on a wider range: the bold node gets both marks, bold first.
        doc.toggle_mark(caret(0, 0), caret(0, 14), "italic");
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","marks":[{"type":"italic"}],"text":"make "},{"type":"text","marks":[{"type":"bold"},{"type":"italic"}],"text":"this"},{"type":"text","marks":[{"type":"italic"}],"text":" bold"}]}]}"#
        );
        assert!(doc.range_has_mark(caret(0, 0), caret(0, 14), "italic"));
        assert!(!doc.range_has_mark(caret(0, 0), caret(0, 14), "bold"));
        // Toggling again removes it everywhere and merges the plain nodes back.
        doc.toggle_mark(caret(0, 0), caret(0, 14), "italic");
        doc.toggle_mark(caret(0, 5), caret(0, 9), "bold");
        assert_eq!(doc.to_json(), body);
    }

    #[test]
    fn block_types_lists_and_rules_follow_the_input_rules() {
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Title"}]}]}"#,
        );
        doc.set_block_type(0, "heading", Some(json!({ "level": 2 })));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Title"}]}]}"#
        );
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
        assert_eq!(doc.parent_type(0).as_deref(), Some("doc"));
        doc.set_block_type(0, "paragraph", None);
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Title"}]}]}"#
        );

        // `- ` wraps in a bullet list; a following `- ` joins the same list.
        doc.wrap_block(0, "bulletList", None, true);
        let c = doc.split_block(caret(0, 5));
        doc.lift_list_item(c.block).unwrap();
        doc.insert_text(caret(1, 0), "next");
        doc.wrap_block(1, "bulletList", None, true);
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Title"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"next"}]}]}]}]}"#
        );
        assert!(doc.in_list_item(1));

        // Tab nests the second item under the first.
        assert!(doc.sink_list_item(1));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Title"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"next"}]}]}]}]}]}]}"#
        );

        // Ordered lists carry their start attr; `---` becomes a rule.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"paragraph"},{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}"#,
        );
        doc.wrap_block(2, "orderedList", Some(json!({ "start": 3 })), true);
        let c = doc.replace_block_with_rule(1);
        assert_eq!(c, caret(1, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"horizontalRule"},{"type":"paragraph"},{"type":"orderedList","attrs":{"start":3},"content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#
        );
    }

    #[test]
    fn lifting_a_middle_item_splits_the_list() {
        let body = r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"1"}]}]},{"type":"listItem","content":[{"type":"paragraph"}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"3"}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(doc.lift_list_item(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"1"}]}]}]},{"type":"paragraph"},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"3"}]}]}]}]}"#
        );
        // Lifting the only item removes the list.
        assert_eq!(doc.lift_list_item(0), Some(caret(0, 0)));
        assert_eq!(doc.block_type(0).as_deref(), Some("paragraph"));
        assert_eq!(doc.parent_type(0).as_deref(), Some("doc"));
        assert!(!doc.in_list_item(0));
    }

    #[test]
    fn atoms_keep_their_place_and_text_offsets_match_rendering() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"},{"type":"hardBreak"},{"type":"text","text":"b"}]}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(doc.text(0), "a\nb");
        doc.insert_text(caret(0, 2), "X");
        assert_eq!(doc.text(0), "a\nXb");
        doc.delete_range(0, 1..2);
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"aXb"}]}]}"#
        );
    }

    /// Types `text` one character at a time with the link passes after each,
    /// like `insertText` followed by the plugins' `appendTransaction`.
    fn type_with_links(doc: &mut Doc, mut at: Caret, text: &str) -> Caret {
        for ch in text.chars() {
            at = doc.insert_text(at, &ch.to_string());
            doc.maintain_links(at.block, at.offset..at.offset);
        }
        at
    }

    fn linked(text: &str, href: &str, target: Value) -> Value {
        json!({
            "type": "text",
            "marks": [{ "type": "link", "attrs": { "href": href, "target": target } }],
            "text": text
        })
    }

    fn paragraph(content: Vec<Value>) -> String {
        json!({ "type": "doc", "content": [{ "type": "paragraph", "content": content }] })
            .to_string()
    }

    // `autolink.test.ts`
    #[test]
    fn autolink_links_bare_domains_with_an_https_href() {
        let mut doc = Doc::parse("");
        doc.ensure_textblock();
        type_with_links(&mut doc, caret(0, 0), "x.com");
        assert_eq!(
            doc.to_json(),
            paragraph(vec![linked("x.com", "https://x.com", Value::Null)])
        );
    }

    #[test]
    fn autolink_links_paths_without_swallowing_trailing_sentence_punctuation() {
        let mut doc = Doc::parse("");
        doc.ensure_textblock();
        type_with_links(
            &mut doc,
            caret(0, 0),
            "See linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview.",
        );
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                json!({ "type": "text", "text": "See " }),
                linked(
                    "linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview",
                    "https://linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview",
                    Value::Null
                ),
                json!({ "type": "text", "text": "." }),
            ])
        );
    }

    #[test]
    fn autolink_does_not_link_email_domains() {
        let mut doc = Doc::parse("");
        doc.ensure_textblock();
        type_with_links(&mut doc, caret(0, 0), "email support@x.com");
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                json!({ "type": "text", "text": "email support@x.com" })
            ])
        );
    }

    #[test]
    fn autolink_extends_a_bare_domain_link_when_adjacent_path_text_is_typed() {
        let mut doc = Doc::parse("");
        doc.ensure_textblock();
        let end = type_with_links(&mut doc, caret(0, 0), "x.com");
        type_with_links(&mut doc, end, "/getcharnotes");
        assert_eq!(
            doc.to_json(),
            paragraph(vec![linked(
                "x.com/getcharnotes",
                "https://x.com/getcharnotes",
                Value::Null
            )])
        );
    }

    #[test]
    fn autolink_keeps_a_custom_href_when_unrelated_text_changes() {
        let mut doc = Doc::parse(&paragraph(vec![
            linked("x.com", "https://x.com/docs?ref=note", Value::Null),
            json!({ "type": "text", "text": " note" }),
        ]));
        let end = doc.insert_text(caret(0, 10), "!");
        doc.maintain_links(0, end.offset..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                linked("x.com", "https://x.com/docs?ref=note", Value::Null),
                json!({ "type": "text", "text": " note!" }),
            ])
        );
    }

    #[test]
    fn autolink_preserves_link_attrs_when_adjacent_path_text_is_typed() {
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::String("_blank".into()),
        )]));
        let end = doc.insert_text(caret(0, 5), "/docs");
        doc.maintain_links(0, end.offset..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![linked(
                "x.com/docs",
                "https://x.com/docs",
                Value::String("_blank".into())
            )])
        );
    }

    #[test]
    fn typing_after_a_link_does_not_inherit_the_non_inclusive_mark() {
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        // A space cannot extend the URL, so it stays outside the link.
        let end = doc.insert_text(caret(0, 5), " ");
        doc.maintain_links(0, end.offset..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                linked("x.com", "https://x.com", Value::Null),
                json!({ "type": "text", "text": " " }),
            ])
        );
        // Text typed before the link at the block start is unlinked too.
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        let end = doc.insert_text(caret(0, 0), "go ");
        doc.maintain_links(0, end.offset..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                json!({ "type": "text", "text": "go " }),
                linked("x.com", "https://x.com", Value::Null),
            ])
        );
    }

    #[test]
    fn retyping_inside_a_link_keeps_it_and_an_invalid_tld_drops_it() {
        // `insertText(text, from, to)` carries the link across the replaced
        // range only while the text after it is still linked (`marksAcross`):
        // replacing the tail leaves "x." linked and the new text plain.
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        assert_eq!(doc.link_across(caret(0, 2), caret(0, 5)), None);
        doc.delete_range(0, 2..5);
        let end = doc.insert_text(caret(0, 2), "zzz");
        doc.maintain_links(0, 2..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![
                linked("x.", "https://x.com", Value::Null),
                json!({ "type": "text", "text": "zzz" }),
            ])
        );

        // Replacing inside the link keeps it, and the guard drops it because
        // "x.zzzom" looks like a URL without being one.
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        let carried = doc.link_across(caret(0, 2), caret(0, 3)).unwrap();
        doc.delete_range(0, 2..3);
        let end = doc.insert_text(caret(0, 2), "zzz");
        doc.set_link(0, 2, end.offset, carried);
        doc.maintain_links(0, 2..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![json!({ "type": "text", "text": "x.zzzom" })])
        );

        // Replacing "x" with "y" inside the link rewrites the href instead.
        let mut doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        let carried = doc.link_across(caret(0, 0), caret(0, 1)).unwrap();
        doc.delete_range(0, 0..1);
        let end = doc.insert_text(caret(0, 0), "y");
        doc.set_link(0, 0, end.offset, carried);
        doc.maintain_links(0, 0..end.offset);
        assert_eq!(
            doc.to_json(),
            paragraph(vec![linked("y.com", "https://y.com", Value::Null)])
        );

        // Replacing the whole link drops the mark (`marksAcross` finds no
        // node after the range), so the typed text is autolinked afresh.
        let doc = Doc::parse(&paragraph(vec![linked(
            "x.com",
            "https://x.com",
            Value::Null,
        )]));
        assert_eq!(doc.link_across(caret(0, 0), caret(0, 5)), None);
    }
}
