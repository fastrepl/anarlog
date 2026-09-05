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
            if let Some(attrs) = item.get("attrs") {
                let mut attrs = attrs.clone();
                if let Some(checked) = attrs.get_mut("checked") {
                    *checked = Value::Bool(false);
                }
                new_item.insert("attrs".into(), attrs);
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
}
