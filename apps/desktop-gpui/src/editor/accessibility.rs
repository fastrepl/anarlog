use super::{document::Document, model::Selection, sequence::Measured};

#[derive(Clone, Debug)]
pub struct AccessibleBlock {
    pub id: u64,
    pub role: String,
    pub start_utf16: usize,
    pub length_utf16: usize,
    pub label: Option<String>,
    pub children: Vec<AccessibleBlock>,
}

pub struct EditorAccessibility {
    pub role: &'static str,
    pub read_only: bool,
    pub selection: Selection,
    pub blocks: Vec<AccessibleBlock>,
}

impl EditorAccessibility {
    /// Build on a worker before passing this snapshot to the platform accessibility adapter.
    pub fn snapshot(document: &Document, selection: Selection, read_only: bool) -> Self {
        fn blocks(node: &super::document::NodeRef, start: usize) -> Vec<AccessibleBlock> {
            let mut result = Vec::new();
            for index in 0..node.children.len() {
                let child = node.children.get(index).expect("child");
                let offset = start + node.children.prefix(index);
                result.push(AccessibleBlock {
                    id: child.id,
                    role: child.kind().into(),
                    start_utf16: offset,
                    length_utf16: child.units(),
                    label: child
                        .text
                        .as_ref()
                        .map(|text| text.as_string())
                        .or_else(|| {
                            child
                                .attr("label")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        }),
                    children: blocks(child, offset + 1),
                });
            }
            result
        }
        Self {
            role: "textbox",
            read_only,
            selection,
            blocks: blocks(&document.root, 0),
        }
    }
}
