//! `Node.replace` and `Node.slice` (`prosemirror-model/src/replace.ts`):
//! splicing a slice between two resolved positions.

use super::node::{Fragment, Node, Slice};
use super::resolved::ResolvedPos;
use super::schema::Schema;

impl Node {
    pub fn resolve(&self, pos: usize) -> ResolvedPos<'_> {
        ResolvedPos::resolve(self, pos)
    }

    /// `slice(from, to)`: the content between the positions, open at the
    /// depths of each side.
    pub fn slice(&self, from: usize, to: usize) -> Slice {
        self.slice_at(from, to, false)
    }

    /// `slice(from, to, includeParents)`: with `include_parents` the slice
    /// keeps every enclosing node open down from the document, which is
    /// what `selection.content()` copies.
    pub fn slice_at(&self, from: usize, to: usize, include_parents: bool) -> Slice {
        if from == to {
            return Slice::empty();
        }
        let from_pos = self.resolve(from);
        let to_pos = self.resolve(to);
        let depth = if include_parents {
            0
        } else {
            from_pos.shared_depth(to)
        };
        let start = from_pos.start(depth);
        let node = from_pos.node(depth);
        let content = node
            .content
            .cut(from_pos.pos - start, Some(to_pos.pos - start));
        Slice::new(content, from_pos.depth() - depth, to_pos.depth() - depth)
    }

    /// `replace(from, to, slice)`: `None` when the slice cannot be fitted
    /// (the transform algorithms only produce fitting slices).
    pub fn replace(
        &self,
        schema: &'static Schema,
        from: usize,
        to: usize,
        slice: &Slice,
    ) -> Option<Node> {
        let from_pos = self.resolve(from);
        let to_pos = self.resolve(to);
        replace(schema, &from_pos, &to_pos, slice)
    }
}

pub fn replace(
    schema: &'static Schema,
    from: &ResolvedPos,
    to: &ResolvedPos,
    slice: &Slice,
) -> Option<Node> {
    if slice.open_start > from.depth() {
        return None;
    }
    if from.depth() - slice.open_start != to.depth() - slice.open_end {
        return None;
    }
    replace_outer(schema, from, to, slice, 0)
}

fn replace_outer(
    schema: &'static Schema,
    from: &ResolvedPos,
    to: &ResolvedPos,
    slice: &Slice,
    depth: usize,
) -> Option<Node> {
    let index = from.index(depth);
    let node = from.node(depth);
    if index == to.index(depth) && depth < from.depth() - slice.open_start {
        let inner = replace_outer(schema, from, to, slice, depth + 1)?;
        Some(node.copy(node.content.replace_child(index, inner)))
    } else if slice.content.size == 0 {
        Some(close(
            schema,
            node,
            replace_two_way(schema, from, to, depth)?,
        )?)
    } else if slice.open_start == 0
        && slice.open_end == 0
        && from.depth() == depth
        && to.depth() == depth
    {
        let parent = from.parent();
        let content = &parent.content;
        close(
            schema,
            parent,
            content
                .cut(0, Some(from.parent_offset))
                .append(&slice.content)
                .append(&content.cut(to.parent_offset, None)),
        )
    } else {
        let prepared = prepare_slice_for_replace(slice, from);
        let start = prepared.resolve(slice.open_start + from.depth() - slice.open_start);
        let end = prepared
            .resolve(prepared.content.size - slice.open_end - (from.depth() - slice.open_start));
        close(
            schema,
            node,
            replace_three_way(schema, from, &start, &end, to, depth)?,
        )
    }
}

fn joinable<'a>(
    schema: &Schema,
    before: &ResolvedPos<'a>,
    after: &ResolvedPos,
    depth: usize,
) -> Option<&'a Node> {
    let node = before.node(depth);
    if !schema.compatible_content(node.type_id, after.node(depth).type_id) {
        return None;
    }
    Some(node)
}

fn add_node(child: Node, target: &mut Vec<Node>) {
    if let Some(last) = target.last_mut()
        && child.is_text()
        && child.same_markup(last)
    {
        let joined = format!(
            "{}{}",
            last.text.as_deref().unwrap_or(""),
            child.text.as_deref().unwrap_or("")
        );
        *last = last.with_text(joined);
    } else {
        target.push(child);
    }
}

fn add_range(
    start: Option<&ResolvedPos>,
    end: Option<&ResolvedPos>,
    depth: usize,
    target: &mut Vec<Node>,
) {
    let node = end.or(start).expect("a range end").node(depth);
    let mut start_index = 0;
    let end_index = end.map_or(node.child_count(), |end| end.index(depth));
    if let Some(start) = start {
        start_index = start.index(depth);
        if start.depth() > depth {
            start_index += 1;
        } else if start.text_offset() > 0 {
            add_node(start.node_after().expect("text after"), target);
            start_index += 1;
        }
    }
    for i in start_index..end_index {
        add_node(node.child(i).clone(), target);
    }
    if let Some(end) = end
        && end.depth() == depth
        && end.text_offset() > 0
    {
        add_node(end.node_before().expect("text before"), target);
    }
}

/// `close`: `checkContent` then copy; invalid content is a `None`.
fn close(schema: &'static Schema, node: &Node, content: Fragment) -> Option<Node> {
    let m = super::content::Match {
        dfa: &schema.nodes[node.type_id].dfa,
        state: 0,
    }
    .match_fragment(&content, 0, content.child_count())?;
    if !m.valid_end() {
        return None;
    }
    if !node.allows_marks_of(schema, &content) {
        return None;
    }
    Some(node.copy(content))
}

impl Node {
    fn allows_marks_of(&self, schema: &Schema, content: &Fragment) -> bool {
        content
            .children
            .iter()
            .all(|child| self.allows_marks(schema, &child.marks))
    }
}

fn replace_three_way(
    schema: &'static Schema,
    from: &ResolvedPos,
    start: &ResolvedPos,
    end: &ResolvedPos,
    to: &ResolvedPos,
    depth: usize,
) -> Option<Fragment> {
    let open_start = if from.depth() > depth {
        Some(joinable(schema, from, start, depth + 1)?)
    } else {
        None
    };
    let open_end = if to.depth() > depth {
        Some(joinable(schema, end, to, depth + 1)?)
    } else {
        None
    };
    let mut content = Vec::new();
    add_range(None, Some(from), depth, &mut content);
    match (open_start, open_end) {
        (Some(open_start), Some(open_end)) if start.index(depth) == end.index(depth) => {
            if !schema.compatible_content(open_start.type_id, open_end.type_id) {
                return None;
            }
            add_node(
                close(
                    schema,
                    open_start,
                    replace_three_way(schema, from, start, end, to, depth + 1)?,
                )?,
                &mut content,
            );
        }
        _ => {
            if let Some(open_start) = open_start {
                add_node(
                    close(
                        schema,
                        open_start,
                        replace_two_way(schema, from, start, depth + 1)?,
                    )?,
                    &mut content,
                );
            }
            add_range(Some(start), Some(end), depth, &mut content);
            if let Some(open_end) = open_end {
                add_node(
                    close(
                        schema,
                        open_end,
                        replace_two_way(schema, end, to, depth + 1)?,
                    )?,
                    &mut content,
                );
            }
        }
    }
    add_range(Some(to), None, depth, &mut content);
    Some(Fragment::from(content))
}

fn replace_two_way(
    schema: &'static Schema,
    from: &ResolvedPos,
    to: &ResolvedPos,
    depth: usize,
) -> Option<Fragment> {
    let mut content = Vec::new();
    add_range(None, Some(from), depth, &mut content);
    if from.depth() > depth {
        let ty = joinable(schema, from, to, depth + 1)?;
        add_node(
            close(schema, ty, replace_two_way(schema, from, to, depth + 1)?)?,
            &mut content,
        );
    }
    add_range(Some(to), None, depth, &mut content);
    Some(Fragment::from(content))
}

/// `prepareSliceForReplace`: the slice's content wrapped in copies of the
/// nodes around `along`, so its ends resolve at the same depths.
fn prepare_slice_for_replace(slice: &Slice, along: &ResolvedPos) -> Node {
    let extra = along.depth() - slice.open_start;
    let parent = along.node(extra);
    let mut node = parent.copy(slice.content.clone());
    for i in (0..extra).rev() {
        node = along.node(i).copy(Fragment::from(vec![node]));
    }
    node
}

#[cfg(test)]
mod tests {
    use super::super::node::Node;
    use super::super::schema::schema;
    use super::*;

    fn doc(json: &str) -> Node {
        Node::from_json(schema(), &serde_json::from_str(json).unwrap()).unwrap()
    }

    #[test]
    fn slices_and_replaces_like_prosemirror() {
        let s = schema();
        let d = doc(
            r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hello"}]},{"type":"paragraph","content":[{"type":"text","text":"world"}]}]}"#,
        );
        // "llo" .. "wo" across the two paragraphs: open on both sides.
        let slice = d.slice(3, 10);
        assert_eq!(slice.open_start, 1);
        assert_eq!(slice.open_end, 1);
        assert_eq!(slice.content.child_count(), 2);
        assert_eq!(slice.size(), 7);
        // Deleting the range joins the paragraphs.
        let joined = d.replace(s, 3, 10, &Slice::empty()).unwrap();
        assert_eq!(joined.content.child_count(), 1);
        assert_eq!(joined.child(0).text_content(), "Herld");
        // Inserting a paragraph's inline content splits and joins.
        let inserted = d.replace(s, 3, 3, &slice).unwrap();
        assert_eq!(inserted.content.child_count(), 3);
        assert_eq!(inserted.child(0).text_content(), "Hello");
        assert_eq!(inserted.child(1).text_content(), "wollo");
        assert_eq!(inserted.child(2).text_content(), "world");
        // Flat text insertion.
        let text = Slice::new(Fragment::from(vec![Node::text(s, "XY", Vec::new())]), 0, 0);
        let flat = d.replace(s, 3, 3, &text).unwrap();
        assert_eq!(flat.child(0).text_content(), "HeXYllo");
        assert_eq!(flat.child(0).child_count(), 1);
    }
}
