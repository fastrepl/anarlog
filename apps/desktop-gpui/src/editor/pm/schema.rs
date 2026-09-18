//! `packages/editor/src/note/schema.ts`: the node and mark types with the
//! fields the parser and the replace algorithms read (content expressions,
//! groups, attribute defaults, `defining` / `isolating` / `code`, the
//! allowed mark set) in the schema's definition order, which sets the mark
//! ranks, the group member order and the parse-rule order.

use std::sync::OnceLock;

use serde_json::{Value, json};

use super::content::Dfa;
use super::node::Attrs;

pub type TypeId = usize;
pub type MarkId = usize;

pub struct NodeType {
    pub id: TypeId,
    pub name: &'static str,
    pub inline: bool,
    /// Name and default; `None` is a required attribute.
    pub attrs: Vec<(&'static str, Option<Value>)>,
    pub defining: bool,
    pub isolating: bool,
    pub code: bool,
    pub atom: bool,
    /// `None` allows every mark (the default for inline content).
    pub mark_set: Option<Vec<MarkId>>,
    pub(super) dfa: Dfa,
}

pub struct MarkType {
    pub name: &'static str,
    pub attrs: Vec<(&'static str, Option<Value>)>,
    /// `excluded`: the marks this one cannot coexist with (itself by default).
    pub excluded: Vec<MarkId>,
}

pub struct Schema {
    pub nodes: Vec<NodeType>,
    pub marks: Vec<MarkType>,
}

impl NodeType {
    pub fn is_text(&self) -> bool {
        self.name == "text"
    }

    pub fn is_leaf(&self) -> bool {
        self.dfa.is_empty()
    }

    pub fn is_block(&self) -> bool {
        !self.inline
    }

    /// `inlineContent`: the first edge of the content match is inline.
    pub fn inline_content(&self, schema: &Schema) -> bool {
        self.dfa
            .first_type(0)
            .is_some_and(|id| schema.nodes[id].inline)
    }

    pub fn is_textblock(&self, schema: &Schema) -> bool {
        self.is_block() && self.inline_content(schema)
    }

    pub fn is_atom(&self) -> bool {
        self.is_leaf() || self.atom
    }

    pub fn has_required_attrs(&self) -> bool {
        self.attrs.iter().any(|(_, default)| default.is_none())
    }

    /// `defaultAttrs`: every attribute's default, `None` when one is required.
    pub fn default_attrs(&self) -> Option<Attrs> {
        let mut attrs = Attrs::new();
        for (name, default) in &self.attrs {
            attrs.insert((*name).to_string(), default.clone()?);
        }
        Some(attrs)
    }

    /// `computeAttrs`: the given values over the defaults, in spec order.
    pub fn compute_attrs(&self, given: Option<&Attrs>) -> Attrs {
        let mut attrs = Attrs::new();
        for (name, default) in &self.attrs {
            // A given `null` stays; only a missing key takes the default.
            let value = given
                .and_then(|given| given.get(*name))
                .cloned()
                .or_else(|| default.clone())
                .unwrap_or(Value::Null);
            attrs.insert((*name).to_string(), value);
        }
        attrs
    }

    pub fn allows_mark_type(&self, mark: MarkId) -> bool {
        self.mark_set.as_ref().is_none_or(|set| set.contains(&mark))
    }

    pub fn whitespace_pre(&self) -> bool {
        self.code
    }
}

impl MarkType {
    pub fn excludes(&self, other: MarkId) -> bool {
        self.excluded.contains(&other)
    }

    #[cfg(test)]
    pub fn has_required_attrs(&self) -> bool {
        self.attrs.iter().any(|(_, default)| default.is_none())
    }

    pub fn compute_attrs(&self, given: Option<&Attrs>) -> Attrs {
        let mut attrs = Attrs::new();
        for (name, default) in &self.attrs {
            let value = given
                .and_then(|given| given.get(*name))
                .cloned()
                .or_else(|| default.clone())
                .unwrap_or(Value::Null);
            attrs.insert((*name).to_string(), value);
        }
        attrs
    }
}

impl Schema {
    pub fn node(&self, name: &str) -> Option<TypeId> {
        self.nodes.iter().position(|node| node.name == name)
    }

    pub fn mark(&self, name: &str) -> Option<MarkId> {
        self.marks.iter().position(|mark| mark.name == name)
    }

    pub fn text_type(&self) -> TypeId {
        self.node("text").expect("the schema has a text node")
    }

    pub fn top_type(&self) -> TypeId {
        0
    }

    /// `compatibleContent`: the same type or content matches sharing an edge.
    pub fn compatible_content(&self, a: TypeId, b: TypeId) -> bool {
        a == b || self.nodes[a].dfa.compatible(0, &self.nodes[b].dfa, 0)
    }
}

struct NodeDef {
    name: &'static str,
    content: &'static str,
    groups: &'static [&'static str],
    inline: bool,
    attrs: Vec<(&'static str, Option<Value>)>,
    defining: bool,
    isolating: bool,
    code: bool,
    atom: bool,
    /// `marks: ""` when `Some(&[])`; `None` leaves the schema default.
    marks: Option<&'static [&'static str]>,
}

fn def(name: &'static str, content: &'static str, groups: &'static [&'static str]) -> NodeDef {
    NodeDef {
        name,
        content,
        groups,
        inline: false,
        attrs: Vec::new(),
        defining: false,
        isolating: false,
        code: false,
        atom: false,
        marks: None,
    }
}

fn nullable(names: &[&'static str]) -> Vec<(&'static str, Option<Value>)> {
    names
        .iter()
        .map(|name| (*name, Some(Value::Null)))
        .collect()
}

fn table_cell_attrs() -> Vec<(&'static str, Option<Value>)> {
    vec![
        ("colspan", Some(json!(1))),
        ("rowspan", Some(json!(1))),
        ("colwidth", Some(Value::Null)),
    ]
}

fn note_nodes() -> Vec<NodeDef> {
    const BLOCK: &[&str] = &["block"];
    const INLINE: &[&str] = &["inline"];
    vec![
        def("doc", "block+", &[]),
        def("paragraph", "inline*", BLOCK),
        NodeDef {
            inline: true,
            ..def("text", "", INLINE)
        },
        NodeDef {
            attrs: vec![("level", Some(json!(1)))],
            defining: true,
            ..def("heading", "inline*", BLOCK)
        },
        NodeDef {
            defining: true,
            ..def("blockquote", "block+", BLOCK)
        },
        NodeDef {
            code: true,
            defining: true,
            marks: Some(&[]),
            ..def("codeBlock", "text*", BLOCK)
        },
        def("horizontalRule", "", BLOCK),
        NodeDef {
            inline: true,
            ..def("hardBreak", "", INLINE)
        },
        def("bulletList", "listItem+", BLOCK),
        NodeDef {
            attrs: vec![("start", Some(json!(1)))],
            ..def("orderedList", "listItem+", BLOCK)
        },
        NodeDef {
            defining: true,
            ..def("listItem", "paragraph block*", &[])
        },
        NodeDef {
            isolating: true,
            ..def("table", "tableRow+", BLOCK)
        },
        def("tableRow", "(tableCell | tableHeader)*", &[]),
        NodeDef {
            attrs: table_cell_attrs(),
            isolating: true,
            ..def("tableCell", "block+", &[])
        },
        NodeDef {
            attrs: table_cell_attrs(),
            isolating: true,
            ..def("tableHeader", "block+", &[])
        },
        def("taskList", "taskItem+", BLOCK),
        NodeDef {
            defining: true,
            attrs: vec![
                ("status", Some(json!("todo"))),
                ("checked", Some(json!(false))),
                ("taskId", Some(Value::Null)),
                ("taskItemId", Some(Value::Null)),
            ],
            ..def("taskItem", "paragraph block*", &[])
        },
        NodeDef {
            attrs: vec![
                ("src", Some(Value::Null)),
                ("alt", Some(Value::Null)),
                ("title", Some(Value::Null)),
                ("attachmentId", Some(Value::Null)),
                ("sharedAttachmentId", Some(Value::Null)),
                (
                    "editorWidth",
                    Some(json!(crate::document::DEFAULT_IMAGE_WIDTH)),
                ),
            ],
            ..def("image", "", BLOCK)
        },
        NodeDef {
            atom: true,
            attrs: vec![
                ("attachmentId", Some(Value::Null)),
                ("sharedAttachmentId", Some(Value::Null)),
                ("name", Some(json!(""))),
                ("mimeType", Some(json!(""))),
                ("src", Some(Value::Null)),
                ("path", Some(Value::Null)),
                ("size", Some(Value::Null)),
            ],
            ..def("fileAttachment", "", BLOCK)
        },
        NodeDef {
            inline: true,
            atom: true,
            attrs: {
                let mut attrs = vec![("provider", Some(json!("github")))];
                attrs.extend(nullable(&[
                    "kind",
                    "url",
                    "owner",
                    "repo",
                    "number",
                    "subId",
                    "workspace",
                    "channelId",
                    "messageTs",
                    "threadTs",
                    "guildId",
                    "messageId",
                    "inviteCode",
                    "resourceId",
                    "resourceTitle",
                ]));
                attrs
            },
            ..def("appLink", "", INLINE)
        },
        NodeDef {
            inline: true,
            atom: true,
            attrs: nullable(&["id", "type", "label"]),
            ..def("mention-@", "", INLINE)
        },
        NodeDef {
            defining: true,
            isolating: true,
            marks: Some(&[]),
            attrs: nullable(&["sessionId", "status", "checked"]),
            ..def("session", "paragraph", BLOCK)
        },
        NodeDef {
            atom: true,
            attrs: nullable(&["src"]),
            ..def("clip", "", BLOCK)
        },
    ]
}

fn build() -> Schema {
    let defs = note_nodes();
    // Name, attributes, `excludes: "_"`.
    type MarkDef = (&'static str, Vec<(&'static str, Option<Value>)>, bool);
    let mark_defs: [MarkDef; 7] = [
        ("bold", vec![], false),
        ("italic", vec![], false),
        ("underline", vec![], false),
        ("strike", vec![], false),
        ("code", vec![], true),
        (
            "link",
            vec![("href", None), ("target", Some(Value::Null))],
            false,
        ),
        ("highlight", vec![], false),
    ];
    let marks: Vec<MarkType> = mark_defs
        .iter()
        .enumerate()
        .map(|(id, (name, attrs, excludes_all))| MarkType {
            name,
            attrs: attrs.clone(),
            excluded: if *excludes_all {
                (0..mark_defs.len()).collect()
            } else {
                vec![id]
            },
        })
        .collect();
    let names: Vec<&'static str> = defs.iter().map(|def| def.name).collect();
    let groups: Vec<&'static [&'static str]> = defs.iter().map(|def| def.groups).collect();
    let inline: Vec<bool> = defs.iter().map(|def| def.inline).collect();
    let resolve = |name: &str| -> Vec<TypeId> {
        if let Some(id) = names.iter().position(|n| *n == name) {
            return vec![id];
        }
        (0..names.len())
            .filter(|id| groups[*id].contains(&name))
            .collect()
    };
    let nodes = defs
        .into_iter()
        .enumerate()
        .map(|(id, def)| {
            let dfa = Dfa::parse(def.content, &resolve, &inline);
            let inline_content = dfa.first_type(0).is_some_and(|first| inline[first]);
            let mark_set = match def.marks {
                Some(list) => Some(
                    list.iter()
                        .map(|name| marks.iter().position(|m| m.name == *name).unwrap())
                        .collect(),
                ),
                None if inline_content => None,
                None => Some(Vec::new()),
            };
            NodeType {
                id,
                name: def.name,
                inline: def.inline,
                attrs: def.attrs,
                defining: def.defining,
                isolating: def.isolating,
                code: def.code,
                atom: def.atom,
                mark_set,
                dfa,
            }
        })
        .collect();
    Schema { nodes, marks }
}

/// The note schema, built once.
pub fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_flags_follow_the_specs() {
        let s = schema();
        let paragraph = &s.nodes[s.node("paragraph").unwrap()];
        assert!(paragraph.is_textblock(s));
        assert!(paragraph.mark_set.is_none());
        let code = &s.nodes[s.node("codeBlock").unwrap()];
        assert!(code.is_textblock(s));
        assert_eq!(code.mark_set, Some(vec![]));
        let list = &s.nodes[s.node("bulletList").unwrap()];
        assert!(!list.is_textblock(s));
        assert_eq!(list.mark_set, Some(vec![]));
        assert!(s.nodes[s.node("image").unwrap()].is_leaf());
        assert!(s.nodes[s.node("hardBreak").unwrap()].inline);
        assert!(s.nodes[s.node("listItem").unwrap()].defining);
        assert!(!s.nodes[s.node("listItem").unwrap()].has_required_attrs());
        assert!(s.marks[s.mark("link").unwrap()].has_required_attrs());
        assert!(s.marks[s.mark("code").unwrap()].excludes(s.mark("bold").unwrap()));
        assert!(!s.marks[s.mark("bold").unwrap()].excludes(s.mark("italic").unwrap()));
        assert!(s.compatible_content(s.node("paragraph").unwrap(), s.node("heading").unwrap()));
        assert!(!s.compatible_content(s.node("paragraph").unwrap(), s.node("bulletList").unwrap()));
    }
}
