use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::{Map, Value, json};

use super::sequence::{Measured, Sequence};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const CHUNK_BYTES: usize = 1024;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_NODES: usize = 250_000;

pub type NodeId = u64;
pub type NodeRef = Arc<Node>;
pub type EditResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub text: Arc<str>,
    units: usize,
}

impl Measured for Chunk {
    fn units(&self) -> usize {
        self.units
    }
}

#[derive(Clone, Debug, Default)]
pub struct Text(Sequence<Chunk>);

impl Text {
    pub fn new(text: &str) -> Self {
        let mut pieces = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let mut end = (start + CHUNK_BYTES).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let piece = &text[start..end];
            pieces.push(Chunk {
                text: piece.into(),
                units: piece.encode_utf16().count(),
            });
            start = end;
        }
        Self(Sequence::from_items(pieces))
    }

    pub fn units(&self) -> usize {
        self.0.units()
    }

    pub fn concat(&self, other: &Self) -> Self {
        Self(self.0.concat(&other.0))
    }

    pub fn split(&self, at: usize) -> EditResult<(Self, Self)> {
        if at == self.units() {
            return Ok((self.clone(), Self::default()));
        }
        let (index, offset, chunk) = self
            .0
            .locate(at)
            .ok_or("Text offset is outside the document")?;
        let byte = utf8(&chunk.text, offset)?;
        let (left, rest) = self.0.split(index);
        let (_, right) = rest.split(1);
        Ok((
            Self(left.concat(&Self::new(&chunk.text[..byte]).0)),
            Self(Self::new(&chunk.text[byte..]).0.concat(&right)),
        ))
    }

    pub fn slice(&self, range: Range<usize>) -> EditResult<Self> {
        let (left, _) = self.split(range.end)?;
        Ok(left.split(range.start)?.1)
    }

    pub fn as_string(&self) -> String {
        let mut output = String::new();
        self.0.visit(&mut |chunk| output.push_str(&chunk.text));
        output
    }
}

pub fn utf8(text: &str, position: usize) -> EditResult<usize> {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units == position {
            return Ok(byte);
        }
        units += character.len_utf16();
        if units > position {
            return Err("Position splits a UTF-16 surrogate pair".into());
        }
    }
    if units == position {
        Ok(text.len())
    } else {
        Err("Text position is out of range".into())
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub fields: Arc<Map<String, Value>>,
    pub children: Sequence<NodeRef>,
    pub text: Option<Text>,
    had_content: bool,
}

impl Measured for NodeRef {
    fn render_blocks(&self) -> usize {
        if self.projects_children() {
            self.children.render_blocks()
        } else {
            1
        }
    }

    fn units(&self) -> usize {
        if let Some(text) = &self.text {
            text.units()
        } else if self.kind() == "doc" {
            self.children.units()
        } else if self.is_atom() {
            1
        } else {
            self.children.units() + 2
        }
    }
}

impl Node {
    pub fn projects_children(&self) -> bool {
        self.known() && !self.is_textblock() && self.kind() != "table" && self.children.len() > 0
    }

    pub fn kind(&self) -> &str {
        self.fields
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub fn attrs(&self) -> Option<&Map<String, Value>> {
        self.fields.get("attrs").and_then(Value::as_object)
    }

    pub fn attr(&self, key: &str) -> Option<&Value> {
        self.attrs()?.get(key)
    }

    pub fn task_done(&self) -> bool {
        if let Some(done) = self.attr("status").and_then(Value::as_bool) {
            return done;
        }
        match self.attr("status").and_then(Value::as_str) {
            Some("done") => true,
            Some("todo" | "in_progress") => false,
            _ => self
                .attr("checked")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    pub fn marks(&self) -> &[Value] {
        self.fields
            .get("marks")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn is_textblock(&self) -> bool {
        matches!(self.kind(), "paragraph" | "heading" | "codeBlock")
    }

    pub fn is_atom(&self) -> bool {
        matches!(
            self.kind(),
            "hardBreak"
                | "horizontalRule"
                | "image"
                | "fileAttachment"
                | "appLink"
                | "mention-@"
                | "session"
                | "clip"
        ) || (!self.known() && !self.had_content)
    }

    pub fn known(&self) -> bool {
        matches!(
            self.kind(),
            "doc"
                | "text"
                | "paragraph"
                | "heading"
                | "codeBlock"
                | "blockquote"
                | "bulletList"
                | "orderedList"
                | "listItem"
                | "taskList"
                | "taskItem"
                | "table"
                | "tableRow"
                | "tableCell"
                | "tableHeader"
                | "hardBreak"
                | "horizontalRule"
                | "image"
                | "fileAttachment"
                | "appLink"
                | "mention-@"
                | "session"
                | "clip"
        )
    }

    pub fn with_children(&self, children: Sequence<NodeRef>) -> NodeRef {
        Arc::new(Self {
            children,
            ..self.clone()
        })
    }

    pub fn with_fields(&self, fields: Map<String, Value>) -> NodeRef {
        Arc::new(Self {
            fields: Arc::new(fields),
            ..self.clone()
        })
    }

    pub fn fresh(kind: &str, children: Sequence<NodeRef>) -> NodeRef {
        Arc::new(Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fields: Arc::new(Map::from_iter([("type".into(), json!(kind))])),
            children,
            text: None,
            had_content: true,
        })
    }

    pub fn text_node(text: &str, fields: Arc<Map<String, Value>>) -> NodeRef {
        Arc::new(Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fields,
            children: Sequence::default(),
            text: Some(Text::new(text)),
            had_content: false,
        })
    }

    pub fn value(&self) -> Value {
        let mut fields = (*self.fields).clone();
        if let Some(text) = &self.text {
            fields.insert("text".into(), json!(text.as_string()));
        }
        if self.had_content || self.children.len() > 0 {
            let mut children = Vec::with_capacity(self.children.len());
            self.children
                .visit(&mut |child| children.push(child.value()));
            fields.insert("content".into(), children.into());
        }
        Value::Object(fields)
    }

    fn parse(value: Value, depth: usize, nodes: &mut usize) -> EditResult<NodeRef> {
        *nodes += 1;
        if depth > 96 || *nodes > MAX_NODES {
            return Err("Document exceeds safe nesting or node limits; original retained".into());
        }
        let Value::Object(mut fields) = value else {
            return Err("Node must be a JSON object".into());
        };
        let kind = fields
            .get("type")
            .and_then(Value::as_str)
            .ok_or("Node type is missing")?
            .to_owned();
        let text = if kind == "text" {
            let Value::String(text) = fields.remove("text").ok_or("Text is missing")? else {
                return Err("Text must be a string".into());
            };
            Some(Text::new(&text))
        } else {
            None
        };
        let content = fields.remove("content");
        let had_content = content.is_some();
        let children = match content {
            None => Sequence::default(),
            Some(Value::Array(values)) => Sequence::from_items(
                values
                    .into_iter()
                    .map(|value| Self::parse(value, depth + 1, nodes))
                    .collect::<EditResult<Vec<_>>>()?,
            ),
            _ => return Err("Node content must be an array".into()),
        };
        if fields
            .get("attrs")
            .is_some_and(|attrs| !attrs.is_object() && !attrs.is_null())
        {
            return Err("Node attributes must be an object; original retained".into());
        }
        if let Some(marks) = fields.get("marks") {
            let marks = marks
                .as_array()
                .ok_or("Node marks must be an array; original retained")?;
            if marks
                .iter()
                .any(|mark| mark.get("type").and_then(Value::as_str).is_none())
            {
                return Err("Mark type is missing; original retained".into());
            }
        }
        let mut invalid = false;
        children.visit(&mut |child| {
            invalid |= match kind.as_str() {
                "text" | "hardBreak" | "horizontalRule" | "image" | "fileAttachment"
                | "mention-@" | "appLink" | "session" | "clip" => true,
                "codeBlock" => child.kind() != "text" || !child.marks().is_empty(),
                "paragraph" | "heading" => {
                    child.known()
                        && !matches!(child.kind(), "text" | "hardBreak" | "mention-@" | "appLink")
                }
                "bulletList" | "orderedList" => child.known() && child.kind() != "listItem",
                "taskList" => child.known() && child.kind() != "taskItem",
                "table" => child.known() && child.kind() != "tableRow",
                "tableRow" => child.known() && !matches!(child.kind(), "tableCell" | "tableHeader"),
                "doc" | "blockquote" | "listItem" | "taskItem" | "tableCell" | "tableHeader" => {
                    child.known()
                        && matches!(child.kind(), "text" | "hardBreak" | "mention-@" | "appLink")
                }
                _ => false,
            };
        });
        if invalid {
            return Err(format!(
                "Invalid child in {kind}; original retained read-only"
            ));
        }
        Ok(Arc::new(Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fields: Arc::new(fields),
            children,
            text,
            had_content,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub root: NodeRef,
    pub original: Arc<str>,
    original_root: NodeRef,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub path: Vec<usize>,
    pub node: NodeRef,
    pub start: usize,
    pub offset: usize,
}

impl Document {
    pub fn serialize_for_save(&self) -> EditResult<Arc<str>> {
        fn normalize(node: &mut Value) -> EditResult<()> {
            if matches!(node["type"].as_str(), Some("image" | "fileAttachment"))
                && let Some(attrs) = node.get_mut("attrs").and_then(Value::as_object_mut)
            {
                let local_src = attrs.get("src").and_then(Value::as_str).is_some_and(|src| {
                    [
                        "asset:",
                        "file:",
                        "http://asset.localhost/",
                        "https://asset.localhost/",
                    ]
                    .iter()
                    .any(|prefix| src.starts_with(prefix))
                });
                if local_src || attrs.contains_key("path") {
                    if attrs
                        .get("attachmentId")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    {
                        return Err("A local attachment has no catalogued attachmentId. Draft retained; catalog the attachment before saving.".into());
                    }
                    if local_src {
                        attrs.remove("src");
                    }
                    attrs.remove("path");
                }
            }
            if let Some(children) = node.get_mut("content").and_then(Value::as_array_mut) {
                for child in children {
                    normalize(child)?;
                }
            }
            Ok(())
        }
        let mut value = self.root.value();
        normalize(&mut value)?;
        serde_json::to_string(&value)
            .map(Arc::from)
            .map_err(|error| error.to_string())
    }

    pub fn parse(body: Arc<str>) -> EditResult<Self> {
        if body.len() > MAX_BYTES {
            return Err("Document exceeds the 16 MiB limit; original retained".into());
        }
        let value: Value = serde_json::from_str(&body)
            .map_err(|e| format!("Invalid JSON: {e}; original retained"))?;
        let root = Node::parse(value, 0, &mut 0)?;
        if root.kind() != "doc" || root.children.len() == 0 {
            return Err("Expected a nonempty ProseMirror document; original retained".into());
        }
        Ok(Self {
            original_root: root.clone(),
            root,
            original: body,
        })
    }

    pub fn serialize(&self) -> EditResult<Arc<str>> {
        if Arc::ptr_eq(&self.root, &self.original_root) {
            return Ok(self.original.clone());
        }
        let body = serde_json::to_string(&self.root.value()).map_err(|e| e.to_string())?;
        if body.len() > MAX_BYTES {
            return Err("Edited document exceeds the 16 MiB save limit".into());
        }
        Ok(body.into())
    }

    pub fn units(&self) -> usize {
        self.root.children.units()
    }

    pub fn resolve(&self, position: usize) -> EditResult<Resolved> {
        fn descend(
            node: &NodeRef,
            position: usize,
            start: usize,
            path: Vec<usize>,
        ) -> EditResult<Resolved> {
            if node.is_textblock() && position >= start && position <= start + node.children.units()
            {
                return Ok(Resolved {
                    path,
                    node: node.clone(),
                    start,
                    offset: position - start,
                });
            }
            if !node.known() || node.is_atom() {
                return Err("This node is preserved but cannot be edited".into());
            }
            let relative = position
                .checked_sub(start)
                .ok_or("Position is on a structural boundary")?;
            let (index, _, child) = node
                .children
                .locate(relative)
                .ok_or("No editable text at this position")?;
            let child_start = start + node.children.prefix(index) + 1;
            let mut path = path;
            path.push(index);
            descend(child, position, child_start, path)
        }
        descend(&self.root, position, 0, Vec::new())
    }

    pub fn node(&self, path: &[usize]) -> Option<NodeRef> {
        let mut node = self.root.clone();
        for &index in path {
            node = node.children.get(index)?.clone();
        }
        Some(node)
    }

    pub fn position_at_path(&self, path: &[usize]) -> Option<usize> {
        let mut node = self.root.clone();
        let mut start = 0;
        for &index in path {
            start += usize::from(node.kind() != "doc") + node.children.prefix(index);
            node = node.children.get(index)?.clone();
        }
        Some(start)
    }

    pub fn replace_node(&mut self, path: &[usize], replacement: NodeRef) {
        fn replace(node: &NodeRef, path: &[usize], replacement: NodeRef) -> NodeRef {
            let Some((&index, rest)) = path.split_first() else {
                return replacement;
            };
            let child = replace(
                node.children.get(index).expect("validated path"),
                rest,
                replacement,
            );
            node.with_children(
                node.children
                    .splice(index..index + 1, &Sequence::one(child)),
            )
        }
        self.root = replace(&self.root, path, replacement);
    }

    pub fn first_caret(&self) -> usize {
        self.edge_caret(false)
    }

    pub fn edge_caret(&self, end: bool) -> usize {
        fn edge(node: &NodeRef, start: usize, end: bool) -> Option<usize> {
            if node.is_textblock() {
                return Some(start + if end { node.children.units() } else { 0 });
            }
            if !node.known() || node.is_atom() {
                return None;
            }
            for offset in 0..node.children.len() {
                let index = if end {
                    node.children.len() - 1 - offset
                } else {
                    offset
                };
                if let Some(position) = edge(
                    node.children.get(index)?,
                    start + node.children.prefix(index) + 1,
                    end,
                ) {
                    return Some(position);
                }
            }
            None
        }
        edge(&self.root, 0, end).unwrap_or(0)
    }
}

pub fn split_inline(
    children: &Sequence<NodeRef>,
    at: usize,
) -> EditResult<(Sequence<NodeRef>, Sequence<NodeRef>)> {
    if at == children.units() {
        return Ok((children.clone(), Sequence::default()));
    }
    let (index, offset, node) = children.locate(at).ok_or("Inline offset is out of range")?;
    if offset == 0 {
        return Ok(children.split(index));
    }
    let text = node.text.as_ref().ok_or("Cannot split an embedded node")?;
    let (a, b) = text.split(offset)?;
    let left_text = Arc::new(Node {
        text: Some(a),
        ..(**node).clone()
    });
    let right_text = Arc::new(Node {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        text: Some(b),
        ..(**node).clone()
    });
    let (left, tail) = children.split(index);
    let (_, right) = tail.split(1);
    Ok((
        left.concat(&Sequence::one(left_text)),
        Sequence::one(right_text).concat(&right),
    ))
}

pub fn inline_text(children: &Sequence<NodeRef>) -> String {
    let mut output = String::new();
    children.visit(&mut |node| {
        if let Some(text) = &node.text {
            output.push_str(&text.as_string());
        } else if node.kind() == "hardBreak" {
            output.push('\n');
        } else {
            output.push('\u{fffc}');
        }
    });
    output
}

pub fn concat_inline(left: &Sequence<NodeRef>, right: &Sequence<NodeRef>) -> Sequence<NodeRef> {
    if let (Some(a), Some(b)) = (left.get(left.len().saturating_sub(1)), right.get(0))
        && let (Some(a_text), Some(b_text)) = (&a.text, &b.text)
        && (Arc::ptr_eq(&a.fields, &b.fields) || a.fields == b.fields)
    {
        let merged = Arc::new(Node {
            text: Some(a_text.concat(b_text)),
            ..(**a).clone()
        });
        return left
            .split(left.len() - 1)
            .0
            .concat(&Sequence::one(merged))
            .concat(&right.split(1).1);
    }
    left.concat(right)
}
