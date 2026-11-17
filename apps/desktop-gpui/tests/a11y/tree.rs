use std::collections::HashMap;

use gpui::accesskit::{Node as AkNode, NodeId, Role, Toggled, TreeUpdate};

pub struct Tree {
    nodes: HashMap<NodeId, AkNode>,
    root: NodeId,
    pub focus: NodeId,
}

#[derive(Clone, Copy)]
pub struct Node<'a> {
    tree: &'a Tree,
    pub id: NodeId,
}

impl Tree {
    pub fn new(update: &TreeUpdate) -> Self {
        Self {
            nodes: update.nodes.iter().cloned().collect(),
            root: update.tree.as_ref().map_or(NodeId(0), |tree| tree.root),
            focus: update.focus,
        }
    }

    pub fn root(&self) -> Node<'_> {
        Node {
            tree: self,
            id: self.root,
        }
    }

    /// Every node in reading (tree) order.
    pub fn all(&self) -> Vec<Node<'_>> {
        self.root().descendants()
    }

    /// The first node with this role whose name is exactly `label`.
    pub fn find(&self, role: Role, label: &str) -> Node<'_> {
        self.root().find(role, label)
    }

    pub fn try_find(&self, role: Role, label: &str) -> Option<Node<'_>> {
        self.root().try_find(role, label)
    }

    pub fn focused(&self) -> Node<'_> {
        Node {
            tree: self,
            id: self.focus,
        }
    }

    /// An indented outline, one node per line, for failure messages and snapshots.
    pub fn outline(&self) -> String {
        self.root().outline()
    }
}

impl<'a> Node<'a> {
    fn node(&self) -> &'a AkNode {
        &self.tree.nodes[&self.id]
    }

    pub fn role(&self) -> Role {
        self.node().role()
    }

    pub fn label(&self) -> &'a str {
        self.node().label().unwrap_or("")
    }

    pub fn value(&self) -> Option<&'a str> {
        self.node().value()
    }

    pub fn selected(&self) -> Option<bool> {
        self.node().is_selected()
    }

    pub fn expanded(&self) -> Option<bool> {
        self.node().is_expanded()
    }

    pub fn toggled(&self) -> Option<Toggled> {
        self.node().toggled()
    }

    pub fn disabled(&self) -> bool {
        self.node().is_disabled()
    }

    pub fn raw(&self) -> &'a AkNode {
        self.node()
    }

    pub fn clickable(&self) -> bool {
        self.node().supports_action(gpui::accesskit::Action::Click)
    }

    pub fn children(&self) -> Vec<Node<'a>> {
        self.node()
            .children()
            .iter()
            .filter(|id| self.tree.nodes.contains_key(id))
            .map(|id| Node {
                tree: self.tree,
                id: *id,
            })
            .collect()
    }

    pub fn descendants(&self) -> Vec<Node<'a>> {
        let mut result = Vec::new();
        for child in self.children() {
            result.push(child);
            result.extend(child.descendants());
        }
        result
    }

    pub fn try_find(&self, role: Role, label: &str) -> Option<Node<'a>> {
        self.descendants()
            .into_iter()
            .find(|node| node.role() == role && node.label() == label)
    }

    pub fn find(&self, role: Role, label: &str) -> Node<'a> {
        self.try_find(role, label).unwrap_or_else(|| {
            panic!(
                "no {role:?} named {label:?} under {:?} {:?}:\n{}",
                self.role(),
                self.label(),
                self.outline()
            )
        })
    }

    /// Children with this role, as a screen reader counts a list's items.
    pub fn items(&self, role: Role) -> Vec<Node<'a>> {
        self.descendants()
            .into_iter()
            .filter(|node| node.role() == role)
            .collect()
    }

    pub fn line(&self) -> String {
        let node = self.node();
        let mut line = format!("{:?}", node.role());
        if let Some(label) = node.label() {
            line += &format!(" {label:?}");
        }
        if let Some(value) = node.value() {
            line += &format!(" value={value:?}");
        }
        if let Some(placeholder) = node.placeholder() {
            line += &format!(" placeholder={placeholder:?}");
        }
        if let Some(description) = node.description() {
            line += &format!(" description={description:?}");
        }
        if let Some(toggled) = node.toggled() {
            line += &format!(" {toggled:?}");
        }
        if node.is_selected() == Some(true) {
            line += " selected";
        }
        if let Some(expanded) = node.is_expanded() {
            line += if expanded { " expanded" } else { " collapsed" };
        }
        if node.is_disabled() {
            line += " disabled";
        }
        if self.id == self.tree.focus {
            line += " focused";
        }
        line
    }

    pub fn outline(&self) -> String {
        fn walk(node: Node, depth: usize, out: &mut String) {
            out.push_str(&"  ".repeat(depth));
            out.push_str(&node.line());
            out.push('\n');
            for child in node.children() {
                walk(child, depth + 1, out);
            }
        }
        let mut out = String::new();
        walk(*self, 0, &mut out);
        out
    }
}
