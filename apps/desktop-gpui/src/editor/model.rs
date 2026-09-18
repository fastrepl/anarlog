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

#[derive(Clone)]
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

    /// Swaps in a whole document (the result of a ProseMirror transform).
    pub fn replace_root(&mut self, root: Value) {
        self.root = root;
        self.reindex();
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

    /// `doc.lastChild` is a paragraph whose `textContent.trim()` is empty:
    /// text nodes only, so a paragraph holding just a mention or a hard
    /// break counts as blank.
    pub fn ends_in_blank_paragraph(&self) -> bool {
        children(&self.root).last().is_some_and(|last| {
            last.get("type").and_then(Value::as_str) == Some("paragraph")
                && children(last)
                    .iter()
                    .filter(|child| child.get("type").and_then(Value::as_str) == Some("text"))
                    .all(|child| {
                        child
                            .get("text")
                            .and_then(Value::as_str)
                            .is_none_or(|text| text.trim().is_empty())
                    })
        })
    }

    /// `tr.insert(doc.content.size, paragraph.create())`.
    pub fn append_paragraph(&mut self) {
        self.root_content_mut().push(json!({ "type": "paragraph" }));
        self.reindex();
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
            node_type.clone()
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
            // `splitBlock`: a split at the start of a non-empty block of a
            // non-default type leaves the empty first half as the default
            // block (`setNodeMarkup(first, deflt)`), so Enter before a
            // heading's text opens a paragraph above it.
            if caret.offset == 0
                && text_len > 0
                && node_type != "paragraph"
                && let Some(first) = content_mut(parent).get_mut(index[0])
            {
                *first = json!({ "type": "paragraph" });
            }
        }
        self.reindex();
        Caret {
            block: caret.block + 1,
            offset: 0,
        }
    }

    /// `tr.replaceSelectionWith(node)` for a block atom: a selection is
    /// deleted first, an empty textblock is replaced outright, a caret at a
    /// textblock's edge puts the node beside the nearest ancestor that takes
    /// a block there (`insertPoint`), and anywhere else the textblock splits
    /// around it (`replaceRange`'s fit). The caret lands on the following
    /// textblock, or the end of the preceding one.
    pub fn insert_block_atom(&mut self, from: Caret, to: Caret, node: Value) -> Caret {
        let caret = self.delete_between(from, to);
        let Some(path) = self.textblocks.get(caret.block).cloned() else {
            return caret;
        };
        let Some(textblock) = node_at(&self.root, &path) else {
            return caret;
        };
        let text_len = plain_text(textblock).len();
        let (parent_path, index) = path.split_at(path.len() - 1);
        let index = index[0];
        let parent_kind = |root: &Value, parent_path: &[usize]| {
            node_at(root, parent_path)
                .and_then(|parent| parent.get("type").and_then(Value::as_str))
                .unwrap_or("")
                .to_string()
        };

        let inserted_at: Vec<usize> =
            if text_len == 0 && takes_block(&parent_kind(&self.root, parent_path), index) {
                // `coveredDepths`: the empty textblock is what gets replaced.
                if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
                    content_mut(parent)[index] = node;
                }
                path.clone()
            } else if let Some(at) = (caret.offset == 0 || text_len == 0)
                .then(|| self.insert_point(&path, false))
                .flatten()
            {
                self.insert_node_at(&at, node);
                at
            } else if let Some(at) = (caret.offset >= text_len)
                .then(|| self.insert_point(&path, true))
                .flatten()
            {
                self.insert_node_at(&at, node);
                at
            } else {
                // Close the textblock, place the node, reopen the same textblock
                // type for the tail.
                let Some(textblock) = node_at_mut(&mut self.root, &path) else {
                    return caret;
                };
                let (head, tail) = split_inline(inline_content_mut(textblock), caret.offset);
                let mut tail_block = json!({ "type": textblock["type"].clone() });
                if let Some(attrs) = textblock.get("attrs") {
                    tail_block["attrs"] = attrs.clone();
                }
                set_inline_content(textblock, head);
                set_inline_content(&mut tail_block, tail);
                if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
                    let siblings = content_mut(parent);
                    siblings.insert(index + 1, tail_block);
                    siblings.insert(index + 1, node);
                }
                let mut at = parent_path.to_vec();
                at.push(index + 1);
                at
            };
        self.reindex();
        // `Selection.near(after, 1)`: forward to the next text position, else
        // back to the previous one.
        if let Some(block) = self.textblocks.iter().position(|p| *p > inserted_at) {
            return Caret { block, offset: 0 };
        }
        let block = self
            .textblocks
            .iter()
            .rposition(|p| *p < inserted_at)
            .unwrap_or(0);
        Caret {
            block,
            offset: self.text(block).len(),
        }
    }

    /// The path of the `nth` node of `kind` in document order.
    pub fn nth_block_path(&self, kind: &str, nth: usize) -> Option<Vec<usize>> {
        fn walk(
            node: &Value,
            kind: &str,
            path: &mut Vec<usize>,
            seen: &mut usize,
            nth: usize,
        ) -> Option<Vec<usize>> {
            for (index, child) in children(node).iter().enumerate() {
                path.push(index);
                if child.get("type").and_then(Value::as_str) == Some(kind) {
                    if *seen == nth {
                        return Some(path.clone());
                    }
                    *seen += 1;
                }
                if let Some(found) = walk(child, kind, path, seen, nth) {
                    return Some(found);
                }
                path.pop();
            }
            None
        }
        walk(&self.root, kind, &mut Vec::new(), &mut 0, nth)
    }

    /// `tr.setNodeMarkup(pos, undefined, { ...attrs, [key]: value })`.
    pub fn set_block_attr(&mut self, path: &[usize], key: &str, value: Value) -> bool {
        let Some(node) = node_at_mut(&mut self.root, path) else {
            return false;
        };
        let object = node.as_object_mut().expect("node object");
        let attrs = object
            .entry("attrs")
            .or_insert_with(|| Value::Object(Map::new()));
        match attrs.as_object_mut() {
            Some(attrs) => {
                attrs.insert(key.to_string(), value);
                true
            }
            None => false,
        }
    }

    /// `tr.delete(pos, pos + node.nodeSize)` for a block node; an emptied
    /// document keeps a paragraph to type into. Returns the caret to land on.
    pub fn remove_block(&mut self, path: &[usize]) -> Caret {
        self.remove_node(path);
        self.reindex();
        self.ensure_textblock();
        let block = self
            .textblocks
            .iter()
            .position(|p| p.as_slice() >= path)
            .or_else(|| self.textblocks.len().checked_sub(1))
            .unwrap_or(0);
        Caret {
            block,
            offset: if self
                .textblocks
                .get(block)
                .is_some_and(|p| p.as_slice() >= path)
            {
                0
            } else {
                self.text(block).len()
            },
        }
    }

    /// `imageTrailingParagraphPlugin`: every top-level image is followed by a
    /// paragraph. Returns the images' indices before the insertions.
    pub fn ensure_image_trailing_paragraphs(&mut self) -> Vec<usize> {
        let content = self.root_content_mut();
        let mut index = 0;
        let mut inserted_after = Vec::new();
        while index < content.len() {
            let is_image = content[index].get("type").and_then(Value::as_str) == Some("image");
            let next_is_paragraph = content
                .get(index + 1)
                .is_some_and(|next| next.get("type").and_then(Value::as_str) == Some("paragraph"));
            if is_image && !next_is_paragraph {
                content.insert(index + 1, json!({ "type": "paragraph" }));
                inserted_after.push(index - inserted_after.len());
            }
            index += 1;
        }
        if !inserted_after.is_empty() {
            self.reindex();
        }
        inserted_after
    }

    pub fn textblock_path(&self, block: usize) -> Option<Vec<usize>> {
        self.textblocks.get(block).cloned()
    }

    pub fn textblock_index_of(&self, path: &[usize]) -> Option<usize> {
        self.textblocks.iter().position(|p| p.as_slice() == path)
    }

    /// `insertPoint` for a block node beside the textblock at `path`: climbs
    /// while the ancestor sits at its parent's edge, returning the content
    /// index where the first accepting ancestor takes it.
    fn insert_point(&self, path: &[usize], after: bool) -> Option<Vec<usize>> {
        let mut node_path = path.to_vec();
        while let Some(index) = node_path.pop() {
            let parent = node_at(&self.root, &node_path)?;
            let kind = parent.get("type").and_then(Value::as_str).unwrap_or("");
            let slot = if after { index + 1 } else { index };
            if takes_block(kind, slot) {
                let mut at = node_path;
                at.push(slot);
                return Some(at);
            }
            let at_edge = if after {
                slot >= children(parent).len()
            } else {
                index == 0
            };
            if !at_edge {
                return None;
            }
        }
        None
    }

    fn insert_node_at(&mut self, at: &[usize], node: Value) {
        let (parent_path, index) = at.split_at(at.len() - 1);
        if let Some(parent) = node_at_mut(&mut self.root, parent_path) {
            let siblings = content_mut(parent);
            siblings.insert(index[0].min(siblings.len()), node);
        }
    }

    /// `joinBackward` from the start of a textblock: its content joins the
    /// previous textblock. Returns the caret at the join point.
    /// `joinBackward` at the start of textblock `block`: the cut before the
    /// nearest ancestor with a sibling before it runs `deleteBarrier`; with
    /// no such cut the block lifts out of its wrappers (`liftTarget`).
    pub fn join_backward(&mut self, block: usize) -> Option<Caret> {
        let path = self.textblocks.get(block)?.clone();
        for depth in (0..path.len()).rev() {
            if path[depth] > 0 {
                let parent_path = path[..depth].to_vec();
                let before_textblocks = node_at(&self.root, &parent_path)
                    .and_then(|parent| children(parent).get(path[depth] - 1))
                    .map(textblock_count_in)
                    .unwrap_or(0);
                return Some(match self.delete_barrier(&parent_path, path[depth])? {
                    Barrier::DeletedBefore => Caret {
                        block: block - before_textblocks,
                        offset: 0,
                    },
                    Barrier::Joined(Some(offset)) | Barrier::TextJoined(offset) => Caret {
                        block: block - 1,
                        offset,
                    },
                    Barrier::Joined(None) | Barrier::Wrapped | Barrier::Lifted => {
                        Caret { block, offset: 0 }
                    }
                });
            }
        }
        if path.len() > 1 {
            self.lift_to(&path, 0)?;
            return Some(Caret { block, offset: 0 });
        }
        None
    }

    /// `joinTaskItemBackward` for a task after the first: its paragraph's
    /// content joins the previous task's last paragraph and its remaining
    /// blocks follow, the task itself going away.
    pub fn join_task_item_backward(&mut self, block: usize) -> Option<Caret> {
        let (list_path, item_index) = self.list_item_position(block)?;
        if item_index == 0 {
            return None;
        }
        let list = node_at(&self.root, &list_path)?;
        let items = children(list);
        let previous = items.get(item_index - 1)?.clone();
        let current = items.get(item_index)?.clone();
        let previous_last = children(&previous).len().checked_sub(1)?;
        if kind(&children(&previous)[previous_last]) != "paragraph"
            || kind(children(&current).first()?) != "paragraph"
        {
            return None;
        }
        let offset = plain_text(&children(&previous)[previous_last]).len();
        let moved = children(children(&current).first()?).to_vec();
        let rest: Vec<Value> = children(&current)[1..].to_vec();
        let list = node_at_mut(&mut self.root, &list_path)?;
        let items = content_mut(list);
        items.remove(item_index);
        let previous = items.get_mut(item_index - 1)?;
        let kids = content_mut(previous);
        let last = kids.get_mut(previous_last)?;
        let inline = inline_content_mut(last);
        inline.extend(moved);
        merge_adjacent_text(inline);
        if inline.is_empty() {
            last.as_object_mut().map(|object| object.remove("content"));
        }
        kids.extend(rest);
        self.reindex();
        Some(Caret {
            block: block - 1,
            offset,
        })
    }

    /// `joinForward` at the end of textblock `block`: `deleteBarrier` at the
    /// cut after the nearest ancestor with a sibling after it. The caret
    /// stays where it was unless its own empty block was deleted.
    pub fn join_forward(&mut self, block: usize) -> Option<Caret> {
        let path = self.textblocks.get(block)?.clone();
        let offset = self.text(block).len();
        for depth in (0..path.len()).rev() {
            let count = node_at(&self.root, &path[..depth])
                .map(|parent| children(parent).len())
                .unwrap_or(0);
            if path[depth] + 1 < count {
                let parent_path = path[..depth].to_vec();
                return Some(match self.delete_barrier(&parent_path, path[depth] + 1)? {
                    Barrier::DeletedBefore => Caret {
                        block: block.min(self.textblocks.len().saturating_sub(1)),
                        offset: 0,
                    },
                    _ => Caret { block, offset },
                });
            }
        }
        None
    }

    /// prosemirror-commands' `deleteBarrier` at the cut between children
    /// `cut - 1` and `cut` of the node at `parent_path`: join compatible
    /// nodes (`joinMaybeClear`, an empty one before the cut being deleted
    /// instead), else move the node after the cut into the one before,
    /// wrapped as its content requires, else lift the first textblock after
    /// the cut up to the parent, else join the textblocks on either side.
    fn delete_barrier(&mut self, parent_path: &[usize], cut: usize) -> Option<Barrier> {
        let parent = node_at(&self.root, parent_path)?;
        let before = children(parent).get(cut - 1)?.clone();
        let after = children(parent).get(cut)?.clone();
        let before_kind = kind(&before).to_string();
        let after_kind = kind(&after).to_string();
        if compatible_content(&before_kind, &after_kind) {
            if children(&before).is_empty()
                && !is_leaf_kind(&before_kind)
                && can_remove_child(parent, cut - 1)
            {
                let parent = node_at_mut(&mut self.root, parent_path)?;
                content_mut(parent).remove(cut - 1);
                self.reindex();
                return Some(Barrier::DeletedBefore);
            }
            let joinable = !is_leaf_kind(&before_kind) && !is_leaf_kind(&after_kind);
            if can_remove_child(parent, cut) && (is_textblock_kind(&after_kind) || joinable) {
                let moved = clear_incompatible(children(&after).to_vec(), &before_kind);
                let parent = node_at_mut(&mut self.root, parent_path)?;
                let siblings = content_mut(parent);
                siblings.remove(cut);
                let target = siblings.get_mut(cut - 1)?;
                let outcome = if is_textblock_kind(&before_kind) {
                    let offset = plain_text(target).len();
                    let inline = inline_content_mut(target);
                    inline.extend(moved);
                    merge_adjacent_text(inline);
                    if inline.is_empty() {
                        target
                            .as_object_mut()
                            .map(|object| object.remove("content"));
                    }
                    Barrier::Joined(Some(offset))
                } else {
                    content_mut(target).extend(moved);
                    Barrier::Joined(None)
                };
                self.reindex();
                return Some(outcome);
            }
        }
        let can_del_after = can_remove_child(parent, cut);
        if can_del_after
            && !is_leaf_kind(&before_kind)
            && let Some(wrappers) = find_wrapping(&before_kind, &after_kind)
        {
            let mut wrapped = after.clone();
            for wrapper in wrappers.iter().rev() {
                wrapped = wrap_in(wrapper, wrapped);
            }
            let parent = node_at_mut(&mut self.root, parent_path)?;
            let siblings = content_mut(parent);
            siblings.remove(cut);
            content_mut(siblings.get_mut(cut - 1)?).push(wrapped);
            // `$joinAt`: a following node of the same type joins on too.
            if siblings
                .get(cut)
                .is_some_and(|next| kind(next) == before_kind)
                && !is_leaf_kind(&before_kind)
            {
                let next = siblings.remove(cut);
                content_mut(siblings.get_mut(cut - 1)?).extend(children(&next).iter().cloned());
            }
            self.reindex();
            return Some(Barrier::Wrapped);
        }
        if let Some(relative) = first_textblock_path(&after) {
            let mut path = parent_path.to_vec();
            path.push(cut);
            path.extend(relative);
            if path.len() > parent_path.len() + 1
                && self.lift_to(&path, parent_path.len()).is_some()
            {
                return Some(Barrier::Lifted);
            }
        }
        if can_del_after
            && let Some(last_before) = last_textblock_path(&before)
            && let Some(first_after) = first_textblock_path(&after)
            && first_after.iter().all(|index| *index == 0)
            && after_is_single_chain(&after)
        {
            let after_text = node_at(&after, &first_after)?.clone();
            let mut into = parent_path.to_vec();
            into.push(cut - 1);
            into.extend(last_before);
            let into_kind = node_at(&self.root, &into)
                .map(kind)
                .unwrap_or("")
                .to_string();
            let moved = clear_incompatible(children(&after_text).to_vec(), &into_kind);
            let target = node_at_mut(&mut self.root, &into)?;
            let offset = plain_text(target).len();
            let inline = inline_content_mut(target);
            inline.extend(moved);
            merge_adjacent_text(inline);
            if inline.is_empty() {
                target
                    .as_object_mut()
                    .map(|object| object.remove("content"));
            }
            let parent = node_at_mut(&mut self.root, parent_path)?;
            content_mut(parent).remove(cut);
            self.reindex();
            return Some(Barrier::TextJoined(offset));
        }
        None
    }

    /// `tr.lift(range, target)` for the single block at `path`: it becomes a
    /// child of the node at depth `target_depth`, every wrapper in between
    /// splitting around it, the halves keeping the wrapper's type and attrs
    /// and an empty half disappearing. `None` when the target does not admit
    /// the block or a half would be invalid (a list item without its leading
    /// paragraph), like `liftTarget` finding no target.
    fn lift_to(&mut self, path: &[usize], target_depth: usize) -> Option<()> {
        if target_depth + 1 >= path.len() {
            return None;
        }
        let target = node_at(&self.root, &path[..target_depth])?;
        let block = node_at(&self.root, path)?.clone();
        if !admits(kind(target), kind(&block)) {
            return None;
        }
        let top_index = path[target_depth];
        let top = children(target).get(top_index)?.clone();
        let (before, after) = split_around(&top, &path[target_depth + 1..])?;
        let mut replacement = Vec::new();
        replacement.extend(before);
        replacement.push(block);
        replacement.extend(after);
        let target = node_at_mut(&mut self.root, &path[..target_depth])?;
        content_mut(target).splice(top_index..=top_index, replacement);
        self.reindex();
        Some(())
    }

    /// The textblock's parent is a blockquote: its index there and the
    /// quote's child count.
    fn blockquote_position(&self, block: usize) -> Option<(Vec<usize>, usize, usize)> {
        let path = self.textblocks.get(block)?;
        if path.len() < 2 {
            return None;
        }
        let (quote_path, index) = path.split_at(path.len() - 1);
        let quote = node_at(&self.root, quote_path)?;
        if quote.get("type").and_then(Value::as_str) != Some("blockquote") {
            return None;
        }
        Some((quote_path.to_vec(), index[0], children(quote).len()))
    }

    /// `tr.lift(range, target)` for one block of a blockquote: it moves out
    /// beside the quote, the siblings before and after it each keeping a
    /// quote of their own (none when there are none). `None` when the block
    /// is not directly inside a blockquote.
    pub fn lift_out_of_blockquote(&mut self, block: usize) -> Option<Caret> {
        let (quote_path, _, _) = self.blockquote_position(block)?;
        let path = self.textblocks.get(block)?.clone();
        self.lift_to(&path, quote_path.len() - 1)?;
        Some(Caret { block, offset: 0 })
    }

    /// `liftEmptyBlock` for an empty textblock inside a blockquote: with
    /// siblings on both sides the quote splits before it (`canSplit`); as
    /// the first or last child it lifts out beside the quote, since a split
    /// would leave an empty quote.
    pub fn lift_empty_block(&mut self, block: usize) -> Option<Caret> {
        let (quote_path, index, count) = self.blockquote_position(block)?;
        if index == 0 || index + 1 >= count {
            return self.lift_out_of_blockquote(block);
        }
        let quote = node_at(&self.root, &quote_path)?.clone();
        let siblings = children(&quote);
        let requote = |blocks: &[Value]| {
            let mut requoted = quote.clone();
            requoted
                .as_object_mut()
                .map(|object| object.insert("content".into(), Value::Array(blocks.to_vec())));
            requoted
        };
        let replacement = vec![requote(&siblings[..index]), requote(&siblings[index..])];
        let (parent_path, quote_index) = quote_path.split_at(quote_path.len() - 1);
        let parent = node_at_mut(&mut self.root, parent_path)?;
        content_mut(parent).splice(quote_index[0]..=quote_index[0], replacement);
        self.reindex();
        Some(Caret { block, offset: 0 })
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

    /// `serializeClipboardText`: the selection's text with textblocks joined
    /// by a blank line (`BLOCK_SEPARATOR`).
    pub fn clipboard_text_between(&self, from: Caret, to: Caret) -> String {
        let (from, to) = order(from, to);
        if from.block == to.block {
            return self.text_between(from, to);
        }
        let mut out = String::new();
        let first = self.text(from.block);
        out.push_str(&first[from.offset.min(first.len())..]);
        for block in from.block + 1..to.block {
            out.push_str("\n\n");
            out.push_str(&self.text(block));
        }
        out.push_str("\n\n");
        let last = self.text(to.block);
        out.push_str(&last[..to.offset.min(last.len())]);
        out
    }

    /// `deleteSelection`: removes everything between two carets. Across
    /// blocks this is ProseMirror's `deleteRange` (whole blocks the range
    /// covers go, a range starting at a block's start takes that block with
    /// it while the deeper end keeps its structure, otherwise the ends
    /// join), the caret landing where `Selection.near` puts it. Returns the
    /// collapsed caret.
    pub fn delete_between(&mut self, from: Caret, to: Caret) -> Caret {
        let (from, to) = order(from, to);
        if from == to {
            return from;
        }
        if from.block == to.block {
            self.delete_range(from.block, from.offset..to.offset);
            return from;
        }
        if let Some(caret) = self.delete_range_between(from, to) {
            return caret;
        }
        self.join_delete_between(from, to)
    }

    /// `insertText(text, from, to)` over a selection spanning blocks:
    /// `replaceRangeWith` fits the text where the range starts, so the
    /// start block keeps its type and what follows the range joins it
    /// (unlike `deleteRange`). The caret lands after the text. `None` when
    /// the port cannot represent the document or the text spans lines.
    pub fn replace_between_with_text(
        &mut self,
        from: Caret,
        to: Caret,
        text: &str,
        marks: &[&'static str],
    ) -> Option<Caret> {
        if text.is_empty() || text.contains('\n') {
            return None;
        }
        let schema = super::pm::schema::schema();
        let doc = super::pm::node::Node::from_json(schema, &self.root)?;
        let (from, to) = (
            super::paste::position(&doc, from)?,
            super::paste::position(&doc, to)?,
        );
        let marks: Vec<Value> = marks.iter().map(|mark| json!({ "type": mark })).collect();
        let node = super::pm::node::Node::from_json(
            schema,
            &json!({ "type": "text", "text": text, "marks": marks }),
        )?;
        let slice = super::pm::node::Slice::new(super::pm::node::Fragment::from(vec![node]), 0, 0);
        let applied = super::pm::transform::replace_range(schema, &doc, from, to, &slice)?;
        let pos = super::pm::clipboard::near_text(schema, &applied.doc, applied.end, -1)
            .or_else(|| super::pm::clipboard::near_text(schema, &applied.doc, applied.end, 1))?;
        let caret = super::paste::caret(&applied.doc, pos)?;
        self.replace_root(applied.doc.to_json(schema));
        Some(caret)
    }

    /// Whether `splitBlock` could split after `deleteRange` removed the
    /// selection: the position the deletion leaves sits inside a textblock.
    /// A range starting at a block's start whose block goes with it leaves
    /// a position between blocks, where the Tauri editor's Enter falls back
    /// to WebKit's own handling.
    pub fn deletion_leaves_split_point(&self, from: Caret, to: Caret) -> Option<bool> {
        let schema = super::pm::schema::schema();
        let doc = super::pm::node::Node::from_json(schema, &self.root)?;
        let (from, to) = (
            super::paste::position(&doc, from)?,
            super::paste::position(&doc, to)?,
        );
        let applied = super::pm::transform::replace_range(
            schema,
            &doc,
            from,
            to,
            &super::pm::node::Slice::empty(),
        )?;
        let at = applied.doc.resolve(applied.end);
        Some(at.parent().is_textblock(schema))
    }

    /// The pre-ProseMirror deletion across blocks: the ends emptied and the
    /// end block's remainder joined into the start block, which keeps its
    /// type. What WebKit's native handling leaves when an editor command
    /// declines the selection.
    pub fn join_delete_between(&mut self, from: Caret, to: Caret) -> Caret {
        let (from, to) = order(from, to);
        if from.block == to.block {
            self.delete_range(from.block, from.offset..to.offset);
            return from;
        }
        let first_len = self.text(from.block).len();
        self.delete_range(from.block, from.offset..first_len);
        self.delete_range(to.block, 0..to.offset);
        for block in (from.block + 1..to.block).rev() {
            let path = self.textblocks[block].clone();
            self.remove_node(&path);
            self.reindex();
        }
        self.join_textblocks(from.block + 1);
        from
    }

    /// `tr.deleteRange(from, to)` through the ProseMirror port; `None` when
    /// the document holds nodes the port's schema lacks.
    fn delete_range_between(&mut self, from: Caret, to: Caret) -> Option<Caret> {
        let schema = super::pm::schema::schema();
        let doc = super::pm::node::Node::from_json(schema, &self.root)?;
        let (from, to) = (
            super::paste::position(&doc, from)?,
            super::paste::position(&doc, to)?,
        );
        let applied = super::pm::transform::replace_range(
            schema,
            &doc,
            from,
            to,
            &super::pm::node::Slice::empty(),
        )?;
        let pos = super::pm::clipboard::near_text(schema, &applied.doc, applied.end, 1)
            .or_else(|| super::pm::clipboard::near_text(schema, &applied.doc, applied.end, -1))?;
        let caret = super::paste::caret(&applied.doc, pos)?;
        self.replace_root(applied.doc.to_json(schema));
        Some(caret)
    }

    /// Moves textblock `block`'s inline content onto the end of the textblock
    /// before it and removes it (with any list item or list left empty),
    /// whatever the two blocks' ancestry: how a deleted range's ends meet.
    fn join_textblocks(&mut self, block: usize) -> Option<Caret> {
        if block == 0 || block >= self.textblocks.len() {
            return None;
        }
        let path = self.textblocks[block].clone();
        let previous_path = self.textblocks[block - 1].clone();
        let node = node_at(&self.root, &path)?.clone();
        let previous = node_at_mut(&mut self.root, &previous_path)?;
        let moved = clear_incompatible(children(&node).to_vec(), kind(previous));
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

    /// Byte ranges of the inline atoms (mentions) in a textblock's text; the
    /// caret never rests inside one.
    pub fn atom_ranges(&self, block: usize) -> Vec<std::ops::Range<usize>> {
        let Some(node) = self
            .textblocks
            .get(block)
            .and_then(|path| node_at(&self.root, path))
        else {
            return Vec::new();
        };
        positioned(children(node))
            .into_iter()
            .filter(|(_, child)| {
                child
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.starts_with("mention-"))
            })
            .map(|(pos, child)| pos..pos + inline_text(child).len())
            .collect()
    }

    /// `tr.replaceWith(from, to, [mentionNode, space])`: the caret lands
    /// after the space.
    pub fn insert_mention(
        &mut self,
        block: usize,
        range: std::ops::Range<usize>,
        item: &super::mention_picker::MentionItem,
    ) -> Caret {
        self.delete_range(block, range.clone());
        let Some(path) = self.textblocks.get(block).cloned() else {
            return Caret {
                block,
                offset: range.start,
            };
        };
        let Some(node) = node_at_mut(&mut self.root, &path) else {
            return Caret {
                block,
                offset: range.start,
            };
        };
        let mention = super::mention_picker::mention_node(item);
        let mention_len = inline_text(&mention).len();
        let inline = inline_content_mut(node);
        // Split the text node at the insertion point and drop the pieces in.
        let mut cursor = 0usize;
        let mut rebuilt = Vec::with_capacity(inline.len() + 3);
        let mut inserted = false;
        for child in inline.drain(..) {
            let len = inline_text(&child).len();
            let (a, b) = (cursor, cursor + len);
            cursor = b;
            let is_text = child.get("type").and_then(Value::as_str) == Some("text");
            if !inserted && is_text && a <= range.start && range.start <= b {
                let text = child
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let (head, tail) = text.split_at(range.start - a);
                if !head.is_empty() {
                    let mut piece = child.clone();
                    piece["text"] = Value::String(head.to_string());
                    rebuilt.push(piece);
                }
                rebuilt.push(mention.clone());
                rebuilt.push(text_node(" ", None));
                if !tail.is_empty() {
                    let mut piece = child.clone();
                    piece["text"] = Value::String(tail.to_string());
                    rebuilt.push(piece);
                }
                inserted = true;
                continue;
            }
            if !inserted && a == range.start && !is_text {
                rebuilt.push(mention.clone());
                rebuilt.push(text_node(" ", None));
                inserted = true;
            }
            rebuilt.push(child);
        }
        if !inserted {
            rebuilt.push(mention);
            rebuilt.push(text_node(" ", None));
        }
        *inline = rebuilt;
        merge_adjacent_text(inline);
        Caret {
            block,
            offset: range.start + mention_len + 1,
        }
    }

    /// Moves an offset that landed inside an atom to the edge in `direction`
    /// (`> 0` forward, `< 0` back, `0` the nearer one), like `mentionSkipPlugin`.
    pub fn snap_out_of_atoms(&self, block: usize, offset: usize, direction: isize) -> usize {
        for range in self.atom_ranges(block) {
            if range.start < offset && offset < range.end {
                let nearer_start = offset - range.start <= range.end - offset;
                return if direction > 0 || (direction == 0 && !nearer_start) {
                    range.end
                } else {
                    range.start
                };
            }
        }
        offset
    }

    /// `(type, id)` of the mention atom covering `offset`, if any.
    pub fn mention_at(&self, block: usize, offset: usize) -> Option<(String, String)> {
        let node = self
            .textblocks
            .get(block)
            .and_then(|path| node_at(&self.root, path))?;
        positioned(children(node))
            .into_iter()
            .find(|(pos, child)| {
                child
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.starts_with("mention-"))
                    && *pos <= offset
                    && offset < pos + inline_text(child).len()
            })
            .and_then(|(_, child)| {
                let attrs = child.get("attrs")?;
                Some((
                    attrs.get("type")?.as_str()?.to_string(),
                    attrs.get("id")?.as_str()?.to_string(),
                ))
            })
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

    /// `titleHeadingPlugin`'s `appendTransaction`: the document starts with
    /// an h1 (a first paragraph or heading becomes one, anything else gets
    /// an empty h1 before it), and a lone empty title gets a paragraph to
    /// type into. Returns whether a block was inserted at the top.
    pub fn enforce_title_heading(&mut self) -> bool {
        let first = self.root_content_mut().first().cloned();
        let kind = first
            .as_ref()
            .and_then(|node| node.get("type").and_then(Value::as_str))
            .map(str::to_string);
        let level = first
            .as_ref()
            .and_then(|node| node.get("attrs"))
            .and_then(|attrs| attrs.get("level"))
            .and_then(Value::as_u64);
        match kind.as_deref() {
            Some("heading") if level == Some(1) => {
                let content = self.root_content_mut();
                if content.len() == 1 && plain_text(&content[0]).trim().is_empty() {
                    content.push(json!({ "type": "paragraph" }));
                    self.reindex();
                }
                false
            }
            Some("paragraph") | Some("heading") => {
                let content = self.root_content_mut();
                if let Some(object) = content[0].as_object_mut() {
                    object.insert("type".into(), Value::String("heading".into()));
                    object.insert("attrs".into(), json!({ "level": 1 }));
                }
                if content.len() == 1 && plain_text(&content[0]).trim().is_empty() {
                    content.push(json!({ "type": "paragraph" }));
                }
                self.reindex();
                false
            }
            None => {
                // `normalizeTitleHeadingDoc` on an empty document: the title
                // heading and a paragraph to type into.
                let content = self.root_content_mut();
                content.push(json!({ "type": "heading", "attrs": { "level": 1 } }));
                content.push(json!({ "type": "paragraph" }));
                self.reindex();
                true
            }
            _ => {
                self.root_content_mut()
                    .insert(0, json!({ "type": "heading", "attrs": { "level": 1 } }));
                self.reindex();
                true
            }
        }
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
        if node_at(&self.root, grand_path)
            .is_some_and(|grand| matches!(kind(grand), "listItem" | "taskItem"))
        {
            return self.lift_to_outer_list(block, &list_path, item_index);
        }
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

    /// `liftToOuterList`: an item of a list nested in another item outdents
    /// to a sibling of that outer item. The inner items after it nest under
    /// it as their own list, the outer item's blocks after the inner list
    /// become a further item, and the outer item keeps what came before.
    fn lift_to_outer_list(
        &mut self,
        block: usize,
        list_path: &[usize],
        item_index: usize,
    ) -> Option<Caret> {
        let (outer_item_path, inner_index) = list_path.split_at(list_path.len() - 1);
        let inner_index = inner_index[0];
        let (outer_list_path, outer_index) = outer_item_path.split_at(outer_item_path.len() - 1);
        let outer_index = outer_index[0];
        let inner = node_at(&self.root, list_path)?.clone();
        let items = children(&inner);
        let outer_item = node_at(&self.root, outer_item_path)?.clone();
        let outer_kids = children(&outer_item);
        let with_content = |node: &Value, content: Vec<Value>| {
            let mut copy = node.clone();
            copy["content"] = Value::Array(content);
            copy
        };
        let mut lifted = items.get(item_index)?.clone();
        if item_index + 1 < items.len() {
            content_mut(&mut lifted).push(with_content(&inner, items[item_index + 1..].to_vec()));
        }
        let mut before_kids = outer_kids.get(..inner_index)?.to_vec();
        if item_index > 0 {
            before_kids.push(with_content(&inner, items[..item_index].to_vec()));
        }
        let after_kids = outer_kids.get(inner_index + 1..)?.to_vec();
        // A trailing item has to start with a paragraph (`paragraph block*`).
        if after_kids
            .first()
            .is_some_and(|first| kind(first) != "paragraph")
        {
            return None;
        }
        let mut replacement = vec![with_content(&outer_item, before_kids), lifted];
        if !after_kids.is_empty() {
            replacement.push(with_content(&outer_item, after_kids));
        }
        let outer_list = node_at_mut(&mut self.root, outer_list_path)?;
        content_mut(outer_list).splice(outer_index..=outer_index, replacement);
        self.reindex();
        Some(Caret { block, offset: 0 })
    }

    /// The innermost `listItem` / `taskItem` holding the textblock, as its
    /// node path.
    fn innermost_list_item(&self, block: usize) -> Option<Vec<usize>> {
        let path = self.textblocks.get(block)?;
        (1..path.len()).rev().find_map(|depth| {
            let item_path = &path[..depth];
            let kind = node_at(&self.root, item_path)?
                .get("type")
                .and_then(Value::as_str)?;
            (kind == "listItem" || kind == "taskItem").then(|| item_path.to_vec())
        })
    }

    /// The keymap's `moveListItem`: Alt-Up / Alt-Down swap the item holding
    /// the caret with its neighbour, or at the list's edge lift it into the
    /// enclosing list (of a compatible kind) before or after the outer item.
    /// Returns the caret's textblock after the move.
    pub fn move_list_item(&mut self, block: usize, up: bool) -> Option<usize> {
        let path = self.textblocks.get(block)?.clone();
        let item_path = self.innermost_list_item(block)?;
        let relative = path[item_path.len()..].to_vec();
        let (list_path, item_index) = item_path.split_at(item_path.len() - 1);
        let item_index = item_index[0];
        let sibling_count = children(node_at(&self.root, list_path)?).len();
        let at_boundary = if up {
            item_index == 0
        } else {
            item_index + 1 >= sibling_count
        };

        if !at_boundary {
            let target = if up { item_index - 1 } else { item_index + 1 };
            content_mut(node_at_mut(&mut self.root, list_path)?).swap(item_index, target);
            self.reindex();
            let mut new_path = list_path.to_vec();
            new_path.push(target);
            new_path.extend(relative);
            return self.textblock_index_of(&new_path);
        }

        // The outer item: the next listItem / taskItem up the path.
        let outer_item_path = (1..list_path.len()).rev().find_map(|depth| {
            let candidate = &list_path[..depth];
            let kind = node_at(&self.root, candidate)?
                .get("type")
                .and_then(Value::as_str)?;
            (kind == "listItem" || kind == "taskItem").then(|| candidate.to_vec())
        })?;
        let (outer_list_path, outer_index) = outer_item_path.split_at(outer_item_path.len() - 1);
        let outer_index = outer_index[0];
        let outer_list_kind = node_at(&self.root, outer_list_path)?
            .get("type")
            .and_then(Value::as_str)?
            .to_string();
        let item_kind = node_at(&self.root, &item_path)?
            .get("type")
            .and_then(Value::as_str)?
            .to_string();
        let compatible = match item_kind.as_str() {
            "listItem" => matches!(outer_list_kind.as_str(), "bulletList" | "orderedList"),
            "taskItem" => outer_list_kind == "taskList",
            _ => false,
        };
        if !compatible {
            return None;
        }

        // Take the item, or the whole nested list when it was the only child.
        let item = content_mut(node_at_mut(&mut self.root, list_path)?).remove(item_index);
        if sibling_count == 1 {
            let (list_parent_path, list_index) = list_path.split_at(list_path.len() - 1);
            content_mut(node_at_mut(&mut self.root, list_parent_path)?).remove(list_index[0]);
        }
        let insert_at = if up { outer_index } else { outer_index + 1 };
        content_mut(node_at_mut(&mut self.root, outer_list_path)?).insert(insert_at, item);
        self.reindex();
        let mut new_path = outer_list_path.to_vec();
        new_path.push(insert_at);
        new_path.extend(relative);
        self.textblock_index_of(&new_path)
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

/// Whether `kind`'s content expression takes a block node at `index`: `doc`
/// and `blockquote` are `block+`, list items `paragraph block*`, and lists
/// hold items only.
fn takes_block(kind: &str, index: usize) -> bool {
    match kind {
        "doc" | "blockquote" => true,
        "listItem" | "taskItem" => index >= 1,
        _ => false,
    }
}

/// What `deleteBarrier` did at a cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Barrier {
    /// The empty node before the cut was deleted.
    DeletedBefore,
    /// The node after the cut joined the one before; the offset where two
    /// textblocks met.
    Joined(Option<usize>),
    /// The node after the cut moved into the one before, wrapped as needed.
    Wrapped,
    /// The first textblock after the cut lifted up to the cut's parent.
    Lifted,
    /// The textblocks on either side of the barrier joined at the offset.
    TextJoined(usize),
}

const BLOCK_KINDS: [&str; 11] = [
    "paragraph",
    "heading",
    "codeBlock",
    "blockquote",
    "bulletList",
    "orderedList",
    "taskList",
    "image",
    "horizontalRule",
    "clip",
    "fileAttachment",
];

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or("")
}

fn is_textblock_kind(kind: &str) -> bool {
    TEXTBLOCKS.contains(&kind)
}

/// Nodes without content of their own (atoms, text).
fn is_leaf_kind(kind: &str) -> bool {
    !matches!(
        kind,
        "doc"
            | "blockquote"
            | "paragraph"
            | "heading"
            | "codeBlock"
            | "bulletList"
            | "orderedList"
            | "taskList"
            | "listItem"
            | "taskItem"
    )
}

/// The note schema's content expressions: whether `parent` admits a `child`
/// node anywhere in its content.
fn admits(parent: &str, child: &str) -> bool {
    match parent {
        "doc" | "blockquote" | "listItem" | "taskItem" => BLOCK_KINDS.contains(&child),
        "paragraph" | "heading" => {
            child == "text" || child == "hardBreak" || child.starts_with("mention-")
        }
        "codeBlock" => child == "text",
        "bulletList" | "orderedList" => child == "listItem",
        "taskList" => child == "taskItem",
        _ => false,
    }
}

/// `NodeType.compatibleContent`: the same type, or content expressions that
/// share a node type.
fn compatible_content(a: &str, b: &str) -> bool {
    a == b
        || BLOCK_KINDS
            .iter()
            .chain(["text", "hardBreak", "mention-human", "listItem", "taskItem"].iter())
            .any(|probe| admits(a, probe) && admits(b, probe))
}

/// `ContentMatch.findWrapping`: the wrappers that let `container` hold a
/// `child` — none, a list item, or no way at all.
fn find_wrapping(container: &str, child: &str) -> Option<Vec<&'static str>> {
    if admits(container, child) {
        return Some(Vec::new());
    }
    ["listItem", "taskItem"]
        .into_iter()
        .find(|wrapper| admits(container, wrapper) && admits(wrapper, child))
        .map(|wrapper| vec![wrapper])
}

/// `wrapper.create(null, content)`: a task item takes the schema defaults
/// with fresh ids, like the identity sweep would give it.
fn wrap_in(wrapper: &str, content: Value) -> Value {
    if wrapper == "taskItem" {
        let attrs =
            super::tasks::item_attrs("todo", &super::tasks::new_id(), &super::tasks::new_id());
        return json!({ "type": "taskItem", "attrs": attrs, "content": [content] });
    }
    json!({ "type": wrapper, "content": [content] })
}

/// `parent.canReplace(index, index + 1)` with nothing: the content stays
/// valid without that child (`block+` and `listItem+` need one left, a list
/// item its leading paragraph).
fn can_remove_child(parent: &Value, index: usize) -> bool {
    let kids = children(parent);
    match kind(parent) {
        "doc" | "blockquote" | "bulletList" | "orderedList" | "taskList" => kids.len() > 1,
        "listItem" | "taskItem" => {
            kids.len() > 1
                && (index != 0 || kids.get(1).is_some_and(|next| kind(next) == "paragraph"))
        }
        _ => false,
    }
}

/// `clearIncompatible` for inline content joined into `into`: a code block
/// keeps text only, without marks.
fn clear_incompatible(inline: Vec<Value>, into: &str) -> Vec<Value> {
    if into != "codeBlock" {
        return inline;
    }
    inline
        .into_iter()
        .filter(|child| kind(child) == "text")
        .map(|child| json!({ "type": "text", "text": child["text"] }))
        .collect()
}

fn textblock_count_in(node: &Value) -> usize {
    if is_textblock_kind(kind(node)) {
        return 1;
    }
    children(node).iter().map(textblock_count_in).sum()
}

fn first_textblock_path(node: &Value) -> Option<Vec<usize>> {
    if is_textblock_kind(kind(node)) {
        return Some(Vec::new());
    }
    children(node)
        .iter()
        .enumerate()
        .find_map(|(index, child)| {
            let mut path = vec![index];
            path.extend(first_textblock_path(child)?);
            Some(path)
        })
}

fn last_textblock_path(node: &Value) -> Option<Vec<usize>> {
    if is_textblock_kind(kind(node)) {
        return Some(Vec::new());
    }
    children(node)
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, child)| {
            let mut path = vec![index];
            path.extend(last_textblock_path(child)?);
            Some(path)
        })
}

/// The node is a chain of single children ending in a textblock.
fn after_is_single_chain(node: &Value) -> bool {
    if is_textblock_kind(kind(node)) {
        return true;
    }
    children(node).len() == 1 && after_is_single_chain(&children(node)[0])
}

/// Splits `node` around the descendant at `rel`: the copies of `node` (and
/// of each wrapper down the path) holding what comes before and after that
/// descendant. `Some(None)` for an empty half; `None` when a half would be
/// invalid (a list item not starting with a paragraph).
fn split_around(node: &Value, rel: &[usize]) -> Option<(Option<Value>, Option<Value>)> {
    let index = rel[0];
    let kids = children(node);
    let (inner_before, inner_after) = if rel.len() == 1 {
        (None, None)
    } else {
        split_around(kids.get(index)?, &rel[1..])?
    };
    let mut before_kids: Vec<Value> = kids.get(..index)?.to_vec();
    before_kids.extend(inner_before);
    let mut after_kids: Vec<Value> = Vec::new();
    after_kids.extend(inner_after);
    after_kids.extend(kids.get(index + 1..)?.iter().cloned());
    let half = |half_kids: Vec<Value>| -> Option<Option<Value>> {
        if half_kids.is_empty() {
            return Some(None);
        }
        if matches!(kind(node), "listItem" | "taskItem") && kind(&half_kids[0]) != "paragraph" {
            return None;
        }
        let mut copy = node.clone();
        copy["content"] = Value::Array(half_kids);
        Some(Some(copy))
    };
    Some((half(before_kids)?, half(after_kids)?))
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
            crate::mention::display_text(label)
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

    const CLIP: &str = r#"{"type":"clip","attrs":{"src":"https://www.youtube.com/embed/x"}}"#;

    fn clip() -> Value {
        serde_json::from_str(CLIP).unwrap()
    }

    #[test]
    fn block_atom_lands_beside_a_paragraph_at_its_edges() {
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"abc"}]}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(
            doc.insert_block_atom(caret(0, 0), caret(0, 0), clip()),
            caret(0, 0)
        );
        assert_eq!(
            doc.to_json(),
            format!(
                r#"{{"type":"doc","content":[{CLIP},{{"type":"paragraph","content":[{{"type":"text","text":"abc"}}]}}]}}"#
            )
        );
        let mut doc = Doc::parse(body);
        assert_eq!(
            doc.insert_block_atom(caret(0, 3), caret(0, 3), clip()),
            caret(0, 3)
        );
        assert_eq!(
            doc.to_json(),
            format!(
                r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"abc"}}]}},{CLIP}]}}"#
            )
        );
    }

    #[test]
    fn block_atom_replaces_an_empty_paragraph_and_splits_a_textblock() {
        let mut doc = Doc::parse(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#);
        doc.insert_block_atom(caret(0, 0), caret(0, 0), clip());
        assert_eq!(
            doc.to_json(),
            format!(r#"{{"type":"doc","content":[{CLIP}]}}"#)
        );

        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Open bugs"}]}]}"#,
        );
        assert_eq!(
            doc.insert_block_atom(caret(0, 4), caret(0, 4), clip()),
            caret(1, 0)
        );
        assert_eq!(
            doc.to_json(),
            format!(
                r#"{{"type":"doc","content":[{{"type":"heading","attrs":{{"level":2}},"content":[{{"type":"text","text":"Open"}}]}},{CLIP},{{"type":"heading","attrs":{{"level":2}},"content":[{{"type":"text","text":" bugs"}}]}}]}}"#
            )
        );
    }

    #[test]
    fn block_atom_in_list_items_follows_insert_point() {
        let list = |items: &str| {
            format!(r#"{{"type":"doc","content":[{{"type":"bulletList","content":[{items}]}}]}}"#)
        };
        let item = |content: &str| format!(r#"{{"type":"listItem","content":[{content}]}}"#);
        let para = |text: &str| {
            format!(r#"{{"type":"paragraph","content":[{{"type":"text","text":"{text}"}}]}}"#)
        };

        // The middle of an item's paragraph splits it inside the item.
        let mut doc = Doc::parse(&list(&item(&para("Open bugs"))));
        doc.insert_block_atom(caret(0, 4), caret(0, 4), clip());
        assert_eq!(
            doc.to_json(),
            list(&item(&format!("{},{CLIP},{}", para("Open"), para(" bugs"))))
        );
        // The end of the paragraph puts it after, still inside the item.
        let mut doc = Doc::parse(&list(&item(&para("a"))));
        doc.insert_block_atom(caret(0, 1), caret(0, 1), clip());
        assert_eq!(doc.to_json(), list(&item(&format!("{},{CLIP}", para("a")))));
        // The start of the first item climbs to the document: before the list.
        let mut doc = Doc::parse(&list(&item(&para("a"))));
        doc.insert_block_atom(caret(0, 0), caret(0, 0), clip());
        assert_eq!(
            doc.to_json(),
            format!(
                r#"{{"type":"doc","content":[{CLIP},{{"type":"bulletList","content":[{}]}}]}}"#,
                item(&para("a"))
            )
        );
        // The start of a later item has no insert point: the fit leaves an
        // empty paragraph ahead of the node.
        let mut doc = Doc::parse(&list(&format!("{},{}", item(&para("a")), item(&para("b")))));
        assert_eq!(
            doc.insert_block_atom(caret(1, 0), caret(1, 0), clip()),
            caret(2, 0)
        );
        assert_eq!(
            doc.to_json(),
            list(&format!(
                "{},{}",
                item(&para("a")),
                item(&format!(r#"{{"type":"paragraph"}},{CLIP},{}"#, para("b")))
            ))
        );
    }

    #[test]
    fn block_atom_over_a_selection_deletes_it_first() {
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"abcdef"}]}]}"#,
        );
        assert_eq!(
            doc.insert_block_atom(caret(0, 2), caret(0, 4), clip()),
            caret(1, 0)
        );
        assert_eq!(
            doc.to_json(),
            format!(
                r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"ab"}}]}},{CLIP},{{"type":"paragraph","content":[{{"type":"text","text":"ef"}}]}}]}}"#
            )
        );
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"abc"}]}]}"#,
        );
        doc.insert_block_atom(caret(0, 3), caret(0, 0), clip());
        assert_eq!(
            doc.to_json(),
            format!(r#"{{"type":"doc","content":[{CLIP}]}}"#)
        );
    }

    #[test]
    fn title_heading_is_enforced_like_the_plugin() {
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Sync"}]}]}"#,
        );
        assert!(!doc.enforce_title_heading());
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]}]}]}"#,
        );
        assert!(doc.enforce_title_heading());
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
        assert_eq!(doc.textblock_count(), 2);
        let mut doc =
            Doc::parse(r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}}]}"#);
        assert!(!doc.enforce_title_heading());
        assert_eq!(doc.textblock_count(), 2);
        assert_eq!(doc.block_type(1).as_deref(), Some("paragraph"));
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"T"}]},{"type":"paragraph"}]}"#,
        );
        let before = doc.to_json();
        assert!(!doc.enforce_title_heading());
        assert_eq!(doc.to_json(), before);
        // An empty document (a summary stored as "") gets the title heading
        // and a paragraph, like `normalizeTitleHeadingDoc`.
        let mut doc = Doc::parse(r#"{"type":"doc","content":[]}"#);
        assert!(doc.enforce_title_heading());
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
        assert_eq!(doc.block_type(1).as_deref(), Some("paragraph"));
        assert_eq!(doc.textblock_count(), 2);
        // A lone empty paragraph becomes the title with a paragraph after it.
        let mut doc = Doc::parse(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#);
        assert!(!doc.enforce_title_heading());
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
        assert_eq!(doc.textblock_count(), 2);
    }

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
        // Start of a non-empty heading: the empty half above becomes a
        // paragraph (`setNodeMarkup(first, deflt)`); an empty heading splits
        // into itself plus a paragraph.
        let c = doc.split_block(caret(1, 0));
        assert_eq!(c, caret(2, 0));
        assert_eq!(doc.root()["content"][1], json!({ "type": "paragraph" }));
        assert_eq!(
            doc.root()["content"][2],
            json!({ "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "tle" }] })
        );
        let mut doc =
            Doc::parse(r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}}]}"#);
        doc.split_block(caret(0, 0));
        assert_eq!(
            doc.root()["content"],
            json!([{ "type": "heading", "attrs": { "level": 1 } }, { "type": "paragraph" }])
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
        // Backspace at the start of the new item joins the items
        // (`joinMaybeClear` on two list items), then the paragraphs.
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(1, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]},{"type":"paragraph"}]}]}]}"#
        );
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(0, 5));
        assert_eq!(doc.to_json(), body);
    }

    #[test]
    fn backspace_at_block_start_lifts_out_of_a_list_then_joins() {
        // `deleteBarrier` between a paragraph and a list lifts the first
        // item's paragraph out (the list going with its only item); the next
        // Backspace joins the paragraphs.
        let body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(1, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}"#
        );
        let c = doc.join_backward(1).unwrap();
        assert_eq!(c, caret(0, 1));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"ab"}]}]}"#
        );
        assert!(doc.join_backward(0).is_none());
    }

    #[test]
    fn delete_barrier_follows_the_recorded_sequences() {
        // L3: Delete at the end of a paragraph before a list lifts the first
        // item's paragraph out; Delete again joins the text.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#,
        );
        assert_eq!(doc.join_forward(0), Some(caret(0, 3)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#
        );
        assert_eq!(doc.join_forward(0), Some(caret(0, 3)));
        assert_eq!(doc.text(0), "onea");

        // L4: Backspace at a paragraph after a list wraps it into a new item;
        // again, and the items join.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}"#,
        );
        assert_eq!(doc.join_backward(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#
        );
        assert_eq!(doc.join_backward(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#
        );

        // L5: Delete at the end of the last item wraps the paragraph after
        // the list into a new item.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}"#,
        );
        assert_eq!(doc.join_forward(0), Some(caret(0, 1)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}]}]}"#
        );

        // L7b: after an item ending in a nested list, Backspace wraps the
        // following paragraph into that nested list.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"d"}]}]}]}]}"#,
        );
        assert_eq!(doc.join_backward(2), Some(caret(2, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"d"}]}]}]}]}]}]}"#
        );

        // L8: a paragraph after a blockquote moves into it.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q"}]}]},{"type":"paragraph","content":[{"type":"text","text":"p"}]}]}"#,
        );
        assert_eq!(doc.join_backward(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q"}]},{"type":"paragraph","content":[{"type":"text","text":"p"}]}]}]}"#
        );

        // H2: an empty heading before the cut is deleted, not joined into.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}},{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}"#,
        );
        assert_eq!(doc.join_backward(1), Some(caret(0, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}"#
        );

        // C1: joined into a code block, text loses its marks.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"codeBlock","content":[{"type":"text","text":"code"}]},{"type":"paragraph","content":[{"type":"text","text":"bo"},{"type":"text","marks":[{"type":"bold"}],"text":"ld"}]}]}"#,
        );
        assert_eq!(doc.join_forward(0), Some(caret(0, 4)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"codeBlock","content":[{"type":"text","text":"codebold"}]}]}"#
        );

        // Delete at the end of an empty paragraph deletes it; the caret moves
        // to the start of what followed.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph"},{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"h"}]}]}"#,
        );
        assert_eq!(doc.join_forward(0), Some(caret(0, 0)));
        assert_eq!(doc.textblock_count(), 1);
        assert_eq!(doc.block_type(0).as_deref(), Some("heading"));
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
    fn moving_a_list_item_swaps_neighbours_and_lifts_at_the_edge() {
        // `- 1` / `- 2` with `- nested` under `2`.
        let body = r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"1"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"2"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"nested"}]}]}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        // Alt-Up on `2` swaps it (with its sublist) above `1`; the caret's
        // textblock moves with it.
        assert_eq!(doc.move_list_item(1, true), Some(0));
        assert_eq!(doc.text(0), "2");
        assert_eq!(doc.text(1), "nested");
        assert_eq!(doc.text(2), "1");
        // Alt-Up at the top of the outer list goes nowhere.
        assert_eq!(doc.move_list_item(0, true), None);
        // Alt-Up on the nested item (the only child) lifts it before `2` and
        // drops the emptied sublist.
        assert_eq!(doc.move_list_item(1, true), Some(0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"nested"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"2"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"1"}]}]}]}]}"#
        );
        // Alt-Down on `2` swaps it below `1`.
        assert_eq!(doc.move_list_item(1, false), Some(2));
        assert_eq!(doc.text(1), "1");
        assert_eq!(doc.text(2), "2");
        // A task item does not lift into a bullet list.
        let body = r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]},{"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":false},"content":[{"type":"paragraph","content":[{"type":"text","text":"t"}]}]}]}]}]}]}"#;
        let mut doc = Doc::parse(body);
        assert_eq!(doc.move_list_item(1, true), None);
        assert_eq!(doc.to_json(), body);
        // A paragraph outside any list is left alone.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"p"}]}]}"#,
        );
        assert_eq!(doc.move_list_item(0, false), None);
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

    #[test]
    fn trailing_empty_line_rule_follows_text_content() {
        // A list at the end: a paragraph is appended.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]}]}]}"#,
        );
        assert!(!doc.ends_in_blank_paragraph());
        doc.append_paragraph();
        assert!(doc.ends_in_blank_paragraph());
        assert_eq!(doc.textblock_count(), 2);
        assert_eq!(doc.text(1), "");
        // Whitespace-only text is blank; a mention alone contributes no
        // `textContent`; a filled paragraph is not blank.
        assert!(Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"  "}]}]}"#
        )
        .ends_in_blank_paragraph());
        assert!(Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"mention-human","attrs":{"id":"h1","label":"Ada"}}]}]}"#
        )
        .ends_in_blank_paragraph());
        assert!(!Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Z"}]}]}"#
        )
        .ends_in_blank_paragraph());
    }

    #[test]
    fn blockquote_blocks_lift_and_split_like_the_commands() {
        // `joinBackward` at the quote's first paragraph lifts it out, the
        // rest keeping the quote (P5 recorded in the Tauri app).
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q1"}]},{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}]}"#,
        );
        assert_eq!(doc.lift_out_of_blockquote(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"paragraph","content":[{"type":"text","text":"q1"}]},{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}]}"#
        );
        // A single-paragraph quote disappears with its paragraph.
        assert_eq!(doc.lift_out_of_blockquote(2), Some(caret(2, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"paragraph","content":[{"type":"text","text":"q1"}]},{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}"#
        );
        assert_eq!(doc.lift_out_of_blockquote(0), None);

        // `liftEmptyBlock`: Enter on the empty middle paragraph splits the
        // quote, Enter again lifts the paragraph out between the halves.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q1"}]},{"type":"paragraph"},{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}]}"#,
        );
        assert_eq!(doc.lift_empty_block(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q1"}]}]},{"type":"blockquote","content":[{"type":"paragraph"},{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}]}"#
        );
        assert_eq!(doc.lift_empty_block(1), Some(caret(1, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q1"}]}]},{"type":"paragraph"},{"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"q2"}]}]}]}"#
        );
        assert_eq!(doc.lift_empty_block(1), None);
    }

    #[test]
    fn lifting_a_nested_item_outdents_it_like_lift_to_outer_list() {
        // Recorded in the Tauri app: Shift-Tab on `c2`.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c1"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c2"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c3"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"d"}]}]}]}]}"#,
        );
        assert_eq!(doc.lift_list_item(3), Some(caret(3, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c1"}]}]}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c2"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c3"}]}]}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"d"}]}]}]}]}"#
        );
        // Enter on an empty nested item (the only one): a sibling of the
        // outer item, the inner list gone.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]},{"type":"listItem","content":[{"type":"paragraph"}]}]}]}]}]}"#,
        );
        assert_eq!(doc.lift_list_item(2), Some(caret(2, 0)));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}]}]},{"type":"listItem","content":[{"type":"paragraph"}]}]}]}"#
        );
    }

    #[test]
    fn deleting_from_a_block_start_into_a_list_keeps_the_list() {
        // S5 recorded in the Tauri app: `deleteRange` removes the paragraph
        // the range starts at and closes the list around what is left.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#,
        );
        assert_eq!(doc.delete_between(caret(0, 0), caret(2, 0)), caret(0, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#
        );
        // S2: a whole heading and paragraph selected from the heading's
        // start leave the empty heading.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Head"}]},{"type":"paragraph","content":[{"type":"text","text":"para"}]}]}"#,
        );
        assert_eq!(doc.delete_between(caret(0, 0), caret(1, 4)), caret(0, 0));
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1}}]}"#
        );
    }

    #[test]
    fn typing_over_a_selection_into_a_list_fits_like_replace_range_with() {
        // S6 and S8 recorded in the Tauri app.
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#,
        );
        assert_eq!(
            doc.replace_between_with_text(caret(0, 0), caret(2, 0), "Z", &[]),
            Some(caret(0, 1))
        );
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Zb"}]}]}"#
        );
        let mut doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"two"}]}]}"#,
        );
        assert_eq!(
            doc.replace_between_with_text(caret(1, 0), caret(2, 0), "Z", &[]),
            Some(caret(1, 1))
        );
        assert_eq!(
            doc.to_json(),
            r#"{"type":"doc","content":[{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ztwo"}]}]}]}]}"#
        );
        // S7: the deletion of that first range leaves no split point.
        let doc = Doc::parse(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}]}"#,
        );
        assert_eq!(
            doc.deletion_leaves_split_point(caret(0, 0), caret(1, 0)),
            Some(false)
        );
        assert_eq!(
            doc.deletion_leaves_split_point(caret(0, 1), caret(1, 0)),
            Some(true)
        );
    }
}
