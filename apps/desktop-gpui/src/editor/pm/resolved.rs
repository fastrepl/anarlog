//! `ResolvedPos` (`prosemirror-model/src/resolvedpos.ts`): a position with
//! the chain of nodes around it.

use super::node::Node;

#[derive(Clone)]
pub struct ResolvedPos<'a> {
    pub pos: usize,
    /// `(node, index into node, position where the node's content starts
    /// minus one)`, from the document down to the parent.
    path: Vec<(&'a Node, usize, usize)>,
    pub parent_offset: usize,
}

impl<'a> ResolvedPos<'a> {
    pub fn resolve(doc: &'a Node, pos: usize) -> ResolvedPos<'a> {
        assert!(pos <= doc.content.size, "position {pos} out of range");
        let mut path = Vec::new();
        let mut start = 0;
        let mut parent_offset = pos;
        let mut node = doc;
        loop {
            let (index, offset) = node.content.find_index(parent_offset);
            let rem = parent_offset - offset;
            path.push((node, index, start + offset));
            if rem == 0 {
                break;
            }
            node = node.child(index);
            if node.is_text() {
                break;
            }
            parent_offset = rem - 1;
            start += offset + 1;
        }
        ResolvedPos {
            pos,
            path,
            parent_offset,
        }
    }

    pub fn depth(&self) -> usize {
        self.path.len() - 1
    }

    pub fn doc(&self) -> &'a Node {
        self.path[0].0
    }

    pub fn parent(&self) -> &'a Node {
        self.path[self.depth()].0
    }

    pub fn node(&self, depth: usize) -> &'a Node {
        self.path[depth].0
    }

    pub fn index(&self, depth: usize) -> usize {
        self.path[depth].1
    }

    pub fn index_after(&self, depth: usize) -> usize {
        self.index(depth)
            + if depth == self.depth() && self.text_offset() == 0 {
                0
            } else {
                1
            }
    }

    pub fn start(&self, depth: usize) -> usize {
        if depth == 0 {
            0
        } else {
            self.path[depth - 1].2 + 1
        }
    }

    pub fn end(&self, depth: usize) -> usize {
        self.start(depth) + self.node(depth).content.size
    }

    /// `before(depth)`: `depth == this.depth + 1` addresses the position itself.
    pub fn before(&self, depth: usize) -> usize {
        assert!(depth > 0, "there is no position before the top-level node");
        if depth == self.depth() + 1 {
            self.pos
        } else {
            self.path[depth - 1].2
        }
    }

    pub fn after(&self, depth: usize) -> usize {
        assert!(depth > 0, "there is no position after the top-level node");
        if depth == self.depth() + 1 {
            self.pos
        } else {
            self.path[depth - 1].2 + self.path[depth].0.node_size()
        }
    }

    pub fn text_offset(&self) -> usize {
        self.pos - self.path[self.path.len() - 1].2
    }

    pub fn node_after(&self) -> Option<Node> {
        let parent = self.parent();
        let index = self.index(self.depth());
        if index == parent.child_count() {
            return None;
        }
        let d_off = self.text_offset();
        let child = parent.child(index);
        Some(if d_off > 0 {
            child.cut(d_off, None)
        } else {
            child.clone()
        })
    }

    pub fn node_before(&self) -> Option<Node> {
        let index = self.index(self.depth());
        let d_off = self.text_offset();
        if d_off > 0 {
            return Some(self.parent().child(index).cut(0, Some(d_off)));
        }
        if index == 0 {
            None
        } else {
            Some(self.parent().child(index - 1).clone())
        }
    }

    pub fn shared_depth(&self, pos: usize) -> usize {
        for depth in (1..=self.depth()).rev() {
            if self.start(depth) <= pos && self.end(depth) >= pos {
                return depth;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::node::Fragment;
    use super::super::schema::schema;
    use super::*;

    #[test]
    fn resolves_like_prosemirror() {
        let s = schema();
        let p = s.node("paragraph").unwrap();
        let li = s.node("listItem").unwrap();
        let ul = s.node("bulletList").unwrap();
        let para = |text: &str| {
            Node::new(
                s,
                p,
                None,
                Fragment::from(vec![Node::text(s, text, Vec::new())]),
                Vec::new(),
            )
        };
        // doc(paragraph("Hello"), bulletList(listItem(paragraph("item"))))
        let doc = Node::new(
            s,
            0,
            None,
            Fragment::from(vec![
                para("Hello"),
                Node::new(
                    s,
                    ul,
                    None,
                    Fragment::from(vec![Node::new(
                        s,
                        li,
                        None,
                        Fragment::from(vec![para("item")]),
                        Vec::new(),
                    )]),
                    Vec::new(),
                ),
            ]),
            Vec::new(),
        );
        let mid = ResolvedPos::resolve(&doc, 3);
        assert_eq!(mid.depth(), 1);
        assert_eq!(mid.parent_offset, 2);
        assert_eq!(mid.text_offset(), 2);
        assert_eq!(mid.start(1), 1);
        assert_eq!(mid.end(1), 6);
        assert_eq!(mid.before(1), 0);
        assert_eq!(mid.after(1), 7);
        assert_eq!(mid.before(2), 3);
        assert_eq!(mid.index(0), 0);
        assert_eq!(mid.index_after(0), 1);
        assert_eq!(mid.node_before().unwrap().text_content(), "He");
        assert_eq!(mid.node_after().unwrap().text_content(), "llo");
        // Inside the list item's paragraph: positions 10..14.
        let inner = ResolvedPos::resolve(&doc, 12);
        assert_eq!(inner.depth(), 3);
        assert_eq!(inner.node(1).type_id, ul);
        assert_eq!(inner.node(2).type_id, li);
        assert_eq!(inner.start(3), 10);
        assert_eq!(inner.end(3), 14);
        assert_eq!(inner.before(2), 8);
        assert_eq!(inner.after(2), 16);
        assert_eq!(inner.shared_depth(13), 3);
        assert_eq!(inner.shared_depth(5), 0);
        // The end of the document.
        let end = ResolvedPos::resolve(&doc, doc.content.size);
        assert_eq!(end.depth(), 0);
        assert_eq!(end.index(0), 2);
    }
}
