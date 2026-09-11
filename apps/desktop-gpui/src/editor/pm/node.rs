//! `Node`, `Fragment`, `Mark` and `Slice` (`prosemirror-model`'s `node.ts`,
//! `fragment.ts`, `mark.ts`, `replace.ts`) with the operations the parser
//! and the replace algorithms use. Values are immutable: every operation
//! builds a new tree.

use serde_json::{Map, Value};

use super::content::Match;
use super::schema::{MarkId, Schema, TypeId};

pub type Attrs = Map<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub type_id: MarkId,
    pub attrs: Attrs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub type_id: TypeId,
    pub attrs: Attrs,
    pub content: Fragment,
    pub marks: Vec<Mark>,
    /// The text of a text node.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Fragment {
    pub children: Vec<Node>,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub content: Fragment,
    pub open_start: usize,
    pub open_end: usize,
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

fn char_slice(text: &str, from: usize, to: usize) -> String {
    text.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .collect()
}

impl Mark {
    pub fn new(schema: &Schema, type_id: MarkId, attrs: Option<&Attrs>) -> Mark {
        Mark {
            type_id,
            attrs: schema.marks[type_id].compute_attrs(attrs),
        }
    }

    /// `addToSet`: the marks stay ordered by rank and exclusion applies.
    pub fn add_to_set(&self, schema: &Schema, set: &[Mark]) -> Vec<Mark> {
        let mut copy: Option<Vec<Mark>> = None;
        let mut placed = false;
        for (i, other) in set.iter().enumerate() {
            if self == other {
                return set.to_vec();
            }
            if schema.marks[self.type_id].excludes(other.type_id) {
                if copy.is_none() {
                    copy = Some(set[..i].to_vec());
                }
            } else if schema.marks[other.type_id].excludes(self.type_id) {
                return set.to_vec();
            } else {
                if !placed && other.type_id > self.type_id {
                    let target = copy.get_or_insert_with(|| set[..i].to_vec());
                    target.push(self.clone());
                    placed = true;
                }
                if let Some(target) = copy.as_mut() {
                    target.push(other.clone());
                }
            }
        }
        let mut copy = copy.unwrap_or_else(|| set.to_vec());
        if !placed {
            copy.push(self.clone());
        }
        copy
    }

    pub fn to_json(&self, schema: &Schema) -> Value {
        let mut json = Map::new();
        json.insert(
            "type".into(),
            Value::String(schema.marks[self.type_id].name.into()),
        );
        if !self.attrs.is_empty() {
            json.insert("attrs".into(), Value::Object(self.attrs.clone()));
        }
        Value::Object(json)
    }

    pub fn from_json(schema: &Schema, json: &Value) -> Option<Mark> {
        let type_id = schema.mark(json.get("type")?.as_str()?)?;
        let attrs = json.get("attrs").and_then(Value::as_object);
        Some(Mark::new(schema, type_id, attrs))
    }
}

impl Fragment {
    /// `Fragment.fromArray`: adjacent text nodes with the same marks join.
    pub fn from(children: Vec<Node>) -> Fragment {
        let mut joined: Vec<Node> = Vec::with_capacity(children.len());
        for node in children {
            if let Some(last) = joined.last_mut()
                && node.is_text()
                && last.same_markup(&node)
            {
                let text = format!(
                    "{}{}",
                    last.text.as_deref().unwrap_or(""),
                    node.text.as_deref().unwrap_or("")
                );
                *last = last.with_text(text);
            } else {
                joined.push(node);
            }
        }
        let size = joined.iter().map(Node::node_size).sum();
        Fragment {
            children: joined,
            size,
        }
    }

    pub fn empty() -> Fragment {
        Fragment::default()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn child(&self, index: usize) -> &Node {
        &self.children[index]
    }

    pub fn maybe_child(&self, index: usize) -> Option<&Node> {
        self.children.get(index)
    }

    pub fn first_child(&self) -> Option<&Node> {
        self.children.first()
    }

    pub fn last_child(&self) -> Option<&Node> {
        self.children.last()
    }

    /// `append`: adjacent text nodes with the same marks join.
    pub fn append(&self, other: &Fragment) -> Fragment {
        if other.children.is_empty() {
            return self.clone();
        }
        if self.children.is_empty() {
            return other.clone();
        }
        let mut content = self.children.clone();
        let last = content.len() - 1;
        let mut rest = other.children.iter();
        if let Some(first) = other.children.first()
            && first.is_text()
            && content[last].is_text()
            && content[last].same_markup(first)
        {
            let joined = format!(
                "{}{}",
                content[last].text.as_deref().unwrap_or(""),
                first.text.as_deref().unwrap_or("")
            );
            content[last] = content[last].with_text(joined);
            rest.next();
        }
        content.extend(rest.cloned());
        Fragment::from(content)
    }

    /// `cut`: the content between two positions, cutting into nodes.
    pub fn cut(&self, from: usize, to: Option<usize>) -> Fragment {
        let to = to.unwrap_or(self.size);
        if from == 0 && to == self.size {
            return self.clone();
        }
        let mut result = Vec::new();
        let mut pos = 0;
        if to > from {
            for child in &self.children {
                if pos >= to {
                    break;
                }
                let end = pos + child.node_size();
                if end > from {
                    let mut child = child.clone();
                    if pos < from || end > to {
                        if child.is_text() {
                            child = child.cut(from.saturating_sub(pos), Some(to.min(end) - pos));
                        } else {
                            child = child.cut(
                                from.saturating_sub(pos + 1),
                                Some((to.min(end - 1)).saturating_sub(pos + 1)),
                            );
                        }
                    }
                    result.push(child);
                }
                pos = end;
            }
        }
        Fragment::from(result)
    }

    pub fn cut_by_index(&self, from: usize, to: usize) -> Fragment {
        if from == to {
            return Fragment::empty();
        }
        if from == 0 && to == self.children.len() {
            return self.clone();
        }
        Fragment::from(self.children[from..to].to_vec())
    }

    pub fn replace_child(&self, index: usize, node: Node) -> Fragment {
        if self.children[index] == node {
            return self.clone();
        }
        let mut children = self.children.clone();
        children[index] = node;
        Fragment::from(children)
    }

    /// `findIndex`: the child at `pos` and where it starts; `pos == size`
    /// is the end (index `childCount`).
    pub fn find_index(&self, pos: usize) -> (usize, usize) {
        if pos == 0 {
            return (0, pos);
        }
        if pos == self.size {
            return (self.children.len(), pos);
        }
        assert!(pos <= self.size, "position {pos} outside of fragment");
        let mut cur_pos = 0;
        for (i, child) in self.children.iter().enumerate() {
            let end = cur_pos + child.node_size();
            if end >= pos {
                if end == pos {
                    return (i + 1, end);
                }
                return (i, cur_pos);
            }
            cur_pos = end;
        }
        unreachable!()
    }

    pub fn to_json(&self, schema: &Schema) -> Value {
        Value::Array(
            self.children
                .iter()
                .map(|child| child.to_json(schema))
                .collect(),
        )
    }

    pub fn from_json(schema: &Schema, json: Option<&Value>) -> Option<Fragment> {
        let Some(array) = json.and_then(Value::as_array) else {
            return Some(Fragment::empty());
        };
        Some(Fragment::from(
            array
                .iter()
                .map(|child| Node::from_json(schema, child))
                .collect::<Option<Vec<_>>>()?,
        ))
    }
}

impl Node {
    /// `type.create(attrs, content, marks)`.
    pub fn new(
        schema: &Schema,
        type_id: TypeId,
        attrs: Option<&Attrs>,
        content: Fragment,
        marks: Vec<Mark>,
    ) -> Node {
        Node {
            type_id,
            attrs: schema.nodes[type_id].compute_attrs(attrs),
            content,
            marks,
            text: None,
        }
    }

    pub fn text(schema: &Schema, text: impl Into<String>, marks: Vec<Mark>) -> Node {
        Node {
            type_id: schema.text_type(),
            attrs: Attrs::new(),
            content: Fragment::empty(),
            marks,
            text: Some(text.into()),
        }
    }

    /// `type.createAndFill`: the content padded to match the expression,
    /// `None` when it cannot be.
    pub fn create_and_fill(
        schema: &'static Schema,
        type_id: TypeId,
        attrs: Option<&Attrs>,
        content: Option<Fragment>,
        marks: Vec<Mark>,
    ) -> Option<Node> {
        let content = content.unwrap_or_default();
        let m = Match {
            dfa: &schema.nodes[type_id].dfa,
            state: 0,
        };
        let before = m.fill_before(&content, false, 0, schema)?;
        let content = before.append(&content);
        let after = m
            .match_fragment(&content, 0, content.child_count())?
            .fill_before(&Fragment::empty(), true, 0, schema)?;
        let content = content.append(&after);
        Some(Node::new(schema, type_id, attrs, content, marks))
    }

    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }

    pub fn node_size(&self) -> usize {
        match &self.text {
            Some(text) => chars(text),
            None if super::schema::schema().nodes[self.type_id].is_leaf() => 1,
            None => self.content.size + 2,
        }
    }

    pub fn child_count(&self) -> usize {
        self.content.child_count()
    }

    pub fn child(&self, index: usize) -> &Node {
        self.content.child(index)
    }

    pub fn first_child(&self) -> Option<&Node> {
        self.content.first_child()
    }

    pub fn last_child(&self) -> Option<&Node> {
        self.content.last_child()
    }

    #[cfg(test)]
    pub fn text_content(&self) -> String {
        match &self.text {
            Some(text) => text.clone(),
            None => self
                .content
                .children
                .iter()
                .map(Node::text_content)
                .collect(),
        }
    }

    pub fn copy(&self, content: Fragment) -> Node {
        Node {
            content,
            ..self.clone()
        }
    }

    pub fn mark(&self, marks: Vec<Mark>) -> Node {
        Node {
            marks,
            ..self.clone()
        }
    }

    pub fn with_text(&self, text: String) -> Node {
        Node {
            text: Some(text),
            ..self.clone()
        }
    }

    /// `cut`: a text node between two offsets, a node with its content cut.
    pub fn cut(&self, from: usize, to: Option<usize>) -> Node {
        match &self.text {
            Some(text) => {
                let to = to.unwrap_or_else(|| chars(text));
                if from == 0 && to == chars(text) {
                    self.clone()
                } else {
                    self.with_text(char_slice(text, from, to))
                }
            }
            None => {
                let to = to.unwrap_or(self.content.size);
                if from == 0 && to == self.content.size {
                    self.clone()
                } else {
                    self.copy(self.content.cut(from, Some(to)))
                }
            }
        }
    }

    pub fn same_markup(&self, other: &Node) -> bool {
        self.type_id == other.type_id && self.attrs == other.attrs && self.marks == other.marks
    }

    /// `contentMatchAt`: the match after `index` children.
    pub fn content_match_at(&self, schema: &'static Schema, index: usize) -> Match {
        Match {
            dfa: &schema.nodes[self.type_id].dfa,
            state: 0,
        }
        .match_fragment(&self.content, 0, index)
        .expect("called contentMatchAt on a node with invalid content")
    }

    /// `canReplace`: `replacement[start..end]` fits between children `from`
    /// and `to`, marks allowed.
    pub fn can_replace(
        &self,
        schema: &'static Schema,
        from: usize,
        to: usize,
        replacement: &Fragment,
        start: usize,
        end: usize,
    ) -> bool {
        let Some(one) = self
            .content_match_at(schema, from)
            .match_fragment(replacement, start, end)
        else {
            return false;
        };
        let Some(two) = one.match_fragment(&self.content, to, self.child_count()) else {
            return false;
        };
        if !two.valid_end() {
            return false;
        }
        replacement.children[start..end]
            .iter()
            .all(|child| self.allows_marks(schema, &child.marks))
    }

    pub fn can_replace_with(
        &self,
        schema: &'static Schema,
        from: usize,
        to: usize,
        type_id: TypeId,
        marks: &[Mark],
    ) -> bool {
        if !self.allows_marks(schema, marks) {
            return false;
        }
        let Some(start) = self.content_match_at(schema, from).match_type(type_id) else {
            return false;
        };
        start
            .match_fragment(&self.content, to, self.child_count())
            .is_some_and(|end| end.valid_end())
    }

    pub fn allows_marks(&self, schema: &Schema, marks: &[Mark]) -> bool {
        marks
            .iter()
            .all(|mark| schema.nodes[self.type_id].allows_mark_type(mark.type_id))
    }

    pub fn is_inline(&self, schema: &Schema) -> bool {
        schema.nodes[self.type_id].inline
    }

    pub fn is_textblock(&self, schema: &Schema) -> bool {
        schema.nodes[self.type_id].is_textblock(schema)
    }

    pub fn inline_content(&self, schema: &Schema) -> bool {
        schema.nodes[self.type_id].inline_content(schema)
    }

    pub fn is_atom(&self, schema: &Schema) -> bool {
        schema.nodes[self.type_id].is_atom()
    }

    pub fn to_json(&self, schema: &Schema) -> Value {
        let mut json = Map::new();
        json.insert(
            "type".into(),
            Value::String(schema.nodes[self.type_id].name.into()),
        );
        if !self.attrs.is_empty() {
            json.insert("attrs".into(), Value::Object(self.attrs.clone()));
        }
        if self.content.size > 0 {
            json.insert("content".into(), self.content.to_json(schema));
        }
        if !self.marks.is_empty() {
            json.insert(
                "marks".into(),
                Value::Array(self.marks.iter().map(|mark| mark.to_json(schema)).collect()),
            );
        }
        if let Some(text) = &self.text {
            json.insert("text".into(), Value::String(text.clone()));
        }
        Value::Object(json)
    }

    pub fn from_json(schema: &Schema, json: &Value) -> Option<Node> {
        let type_id = schema.node(json.get("type")?.as_str()?)?;
        let marks = json
            .get("marks")
            .and_then(Value::as_array)
            .map(|marks| {
                marks
                    .iter()
                    .map(|mark| Mark::from_json(schema, mark))
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or(Some(Vec::new()))?;
        if type_id == schema.text_type() {
            return Some(Node::text(
                schema,
                json.get("text").and_then(Value::as_str).unwrap_or(""),
                marks,
            ));
        }
        let content = Fragment::from_json(schema, json.get("content"))?;
        Some(Node::new(
            schema,
            type_id,
            json.get("attrs").and_then(Value::as_object),
            content,
            marks,
        ))
    }
}

impl Slice {
    pub fn new(content: Fragment, open_start: usize, open_end: usize) -> Slice {
        Slice {
            content,
            open_start,
            open_end,
        }
    }

    pub fn empty() -> Slice {
        Slice::new(Fragment::empty(), 0, 0)
    }

    pub fn size(&self) -> usize {
        self.content.size - self.open_start - self.open_end
    }

    /// `maxOpen`: open as deep as the first and last non-leaf children go.
    pub fn max_open(schema: &Schema, fragment: Fragment, open_isolating: bool) -> Slice {
        let mut open_start = 0;
        let mut node = fragment.first_child();
        while let Some(n) = node
            && !schema.nodes[n.type_id].is_leaf()
            && (open_isolating || !schema.nodes[n.type_id].isolating)
        {
            open_start += 1;
            node = n.first_child();
        }
        let mut open_end = 0;
        let mut node = fragment.last_child();
        while let Some(n) = node
            && !schema.nodes[n.type_id].is_leaf()
            && (open_isolating || !schema.nodes[n.type_id].isolating)
        {
            open_end += 1;
            node = n.last_child();
        }
        Slice::new(fragment, open_start, open_end)
    }

    /// `insertAt`: `fragment` inserted `pos` into the slice's content.
    pub fn insert_at(
        &self,
        schema: &'static Schema,
        pos: usize,
        fragment: &Fragment,
    ) -> Option<Slice> {
        let content = insert_into(schema, &self.content, pos + self.open_start, fragment, None)?;
        Some(Slice::new(content, self.open_start, self.open_end))
    }
}

fn insert_into(
    schema: &'static Schema,
    content: &Fragment,
    dist: usize,
    insert: &Fragment,
    parent: Option<&Node>,
) -> Option<Fragment> {
    let (index, offset) = content.find_index(dist);
    let child = content.maybe_child(index);
    if offset == dist || child.is_some_and(Node::is_text) {
        if let Some(parent) = parent
            && !parent.can_replace(schema, index, index, insert, 0, insert.child_count())
        {
            return None;
        }
        return Some(
            content
                .cut(0, Some(dist))
                .append(insert)
                .append(&content.cut(dist, None)),
        );
    }
    let child = child?;
    let inner = insert_into(
        schema,
        &child.content,
        dist - offset - 1,
        insert,
        Some(child),
    )?;
    Some(content.replace_child(index, child.copy(inner)))
}

#[cfg(test)]
mod tests {
    use super::super::schema::schema;
    use super::*;

    fn para(s: &Schema, text: &str) -> Node {
        Node::new(
            s,
            s.node("paragraph").unwrap(),
            None,
            Fragment::from(vec![Node::text(s, text, Vec::new())]),
            Vec::new(),
        )
    }

    #[test]
    fn sizes_and_cuts_follow_prosemirror() {
        let s = schema();
        let doc = Node::new(
            s,
            0,
            None,
            Fragment::from(vec![para(s, "Hello"), para(s, "world")]),
            Vec::new(),
        );
        assert_eq!(doc.node_size(), 7 + 7 + 2);
        assert_eq!(doc.content.size, 14);
        let cut = doc.content.cut(3, Some(10));
        assert_eq!(cut.children.len(), 2);
        assert_eq!(cut.children[0].text_content(), "llo");
        assert_eq!(cut.children[1].text_content(), "wo");
        assert_eq!(doc.content.find_index(7), (1, 7));
        assert_eq!(doc.content.find_index(3), (0, 0));
        assert_eq!(doc.content.find_index(14), (2, 14));
        let json = doc.to_json(s);
        assert_eq!(Node::from_json(s, &json).unwrap(), doc);
        assert_eq!(
            json.to_string(),
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hello"}]},{"type":"paragraph","content":[{"type":"text","text":"world"}]}]}"#
        );
    }

    #[test]
    fn marks_order_by_rank_and_exclude() {
        let s = schema();
        let bold = Mark::new(s, s.mark("bold").unwrap(), None);
        let italic = Mark::new(s, s.mark("italic").unwrap(), None);
        let code = Mark::new(s, s.mark("code").unwrap(), None);
        let set = italic.add_to_set(s, &[]);
        let set = bold.add_to_set(s, &set);
        assert_eq!(set, vec![bold.clone(), italic.clone()]);
        assert_eq!(bold.add_to_set(s, &set), set);
        // `code` excludes everything.
        assert_eq!(code.add_to_set(s, &set), vec![code.clone()]);
        assert_eq!(bold.add_to_set(s, std::slice::from_ref(&code)), vec![code]);
        // Text nodes with equal marks join on append.
        let a = Fragment::from(vec![Node::text(s, "a", vec![bold.clone()])]);
        let b = Fragment::from(vec![Node::text(s, "b", vec![bold.clone()])]);
        let c = Fragment::from(vec![Node::text(s, "c", vec![])]);
        assert_eq!(a.append(&b).children.len(), 1);
        assert_eq!(a.append(&c).children.len(), 2);
    }

    #[test]
    fn replacement_checks_follow_the_content_match() {
        let s = schema();
        let doc = Node::new(
            s,
            0,
            None,
            Fragment::from(vec![para(s, "Hello")]),
            Vec::new(),
        );
        let li = s.node("listItem").unwrap();
        let p = s.node("paragraph").unwrap();
        assert!(doc.can_replace_with(s, 0, 0, p, &[]));
        assert!(!doc.can_replace_with(s, 0, 0, li, &[]));
        assert!(!doc.can_replace(s, 0, 1, &Fragment::empty(), 0, 0));
        let filled = Node::create_and_fill(s, li, None, None, Vec::new()).unwrap();
        assert_eq!(filled.child_count(), 1);
        assert_eq!(filled.child(0).type_id, p);
        let table_cell =
            Node::create_and_fill(s, s.node("tableCell").unwrap(), None, None, Vec::new()).unwrap();
        assert_eq!(table_cell.attrs.get("colspan"), Some(&serde_json::json!(1)));
    }
}
