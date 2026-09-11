//! HTML on the clipboard through the ProseMirror port: the editor's caret
//! (textblock index and byte offset) mapped to and from document positions.

use super::model::Caret;
use super::pm::node::Node;
use super::pm::schema::{Schema, schema};

/// `clipboardData.setData` on copy: the text and ProseMirror's HTML together,
/// where the platform lets the app own both. `false` when it does not and
/// gpui's text-only clipboard is the fallback.
pub fn write_clipboard(text: &str, html: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::x11::write_clipboard(text.to_string(), html.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (text, html);
        false
    }
}

/// `clipboardData.getData("text/html")`, where the platform exposes it.
pub fn clipboard_html() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        crate::x11::clipboard_html()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The editor's plain text of an inline node (`model::inline_text`).
fn inline_text(schema: &Schema, node: &Node) -> String {
    if let Some(text) = &node.text {
        return text.clone();
    }
    match schema.nodes[node.type_id].name {
        "hardBreak" => "\n".to_string(),
        name if name.starts_with("mention-") => crate::mention::display_text(
            node.attrs
                .get("label")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
        ),
        _ => String::new(),
    }
}

fn is_textblock(schema: &Schema, node: &Node) -> bool {
    matches!(
        schema.nodes[node.type_id].name,
        "paragraph" | "heading" | "codeBlock"
    )
}

/// Every textblock in document order with the position its content starts at.
fn textblocks(schema: &Schema, doc: &Node) -> Vec<(Vec<usize>, usize)> {
    fn walk(
        schema: &Schema,
        node: &Node,
        path: &mut Vec<usize>,
        pos: usize,
        out: &mut Vec<(Vec<usize>, usize)>,
    ) {
        if is_textblock(schema, node) {
            out.push((path.clone(), pos));
            return;
        }
        let mut child_pos = pos;
        for (index, child) in node.content.children.iter().enumerate() {
            path.push(index);
            walk(schema, child, path, child_pos + 1, out);
            path.pop();
            child_pos += child.node_size();
        }
    }
    let mut out = Vec::new();
    walk(schema, doc, &mut Vec::new(), 0, &mut out);
    out
}

/// The document position of `caret`.
pub fn position(doc: &Node, caret: Caret) -> Option<usize> {
    let s = schema();
    let (path, start) = textblocks(s, doc).into_iter().nth(caret.block)?;
    let mut block = doc;
    for index in path {
        block = block.content.maybe_child(index)?;
    }
    let mut pos = start;
    let mut bytes = 0;
    for child in &block.content.children {
        let text = inline_text(s, child);
        if child.is_text() {
            if caret.offset <= bytes + text.len() {
                let within = text.get(..caret.offset - bytes).unwrap_or(&text);
                return Some(pos + within.chars().count());
            }
            pos += text.chars().count();
        } else {
            // An atom is one position wide whatever its text shows.
            if caret.offset <= bytes {
                return Some(pos);
            }
            pos += 1;
        }
        bytes += text.len();
    }
    Some(pos)
}

/// The caret at a document position inside a textblock.
pub fn caret(doc: &Node, pos: usize) -> Option<Caret> {
    let s = schema();
    let blocks = textblocks(s, doc);
    let (block, (path, start)) = blocks
        .iter()
        .enumerate()
        .find(|(_, (path, start))| {
            let mut node = doc;
            for index in path {
                node = node.child(*index);
            }
            *start <= pos && pos <= start + node.content.size
        })
        .map(|(block, entry)| (block, entry.clone()))?;
    let mut node = doc;
    for index in path {
        node = node.child(index);
    }
    let mut remaining = pos - start;
    let mut offset = 0;
    for child in &node.content.children {
        if remaining == 0 {
            break;
        }
        let text = inline_text(s, child);
        if child.is_text() {
            let chars = text.chars().count();
            if remaining < chars {
                offset += text
                    .chars()
                    .take(remaining)
                    .map(char::len_utf8)
                    .sum::<usize>();
                remaining = 0;
                break;
            }
            remaining -= chars;
        } else {
            remaining -= 1;
        }
        offset += text.len();
    }
    let _ = remaining;
    Some(Caret { block, offset })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(json: &str) -> Node {
        Node::from_json(schema(), &serde_json::from_str(json).unwrap()).unwrap()
    }

    #[test]
    fn carets_map_to_positions_and_back() {
        let d = doc(r#"{"type":"doc","content":[
                {"type":"paragraph","content":[{"type":"text","text":"Héllo "},{"type":"mention-@","attrs":{"id":"1","type":"session","label":"Seek"}},{"type":"text","text":" x"}]},
                {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"},{"type":"hardBreak"},{"type":"text","text":"b"}]}]}]}
            ]}"#);
        // "Héllo " is 7 bytes, 6 chars; the mention shows "@Seek".
        let mention_text = crate::mention::display_text("Seek");
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 0,
                    offset: 0
                }
            ),
            Some(1)
        );
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 0,
                    offset: 7
                }
            ),
            Some(7)
        );
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 0,
                    offset: 7 + mention_text.len()
                }
            ),
            Some(8)
        );
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 0,
                    offset: 7 + mention_text.len() + 2
                }
            ),
            Some(10)
        );
        assert_eq!(
            caret(&d, 1),
            Some(Caret {
                block: 0,
                offset: 0
            })
        );
        assert_eq!(
            caret(&d, 3),
            Some(Caret {
                block: 0,
                offset: 3
            })
        );
        assert_eq!(
            caret(&d, 7),
            Some(Caret {
                block: 0,
                offset: 7
            })
        );
        assert_eq!(
            caret(&d, 8),
            Some(Caret {
                block: 0,
                offset: 7 + mention_text.len()
            })
        );
        // The list item's paragraph: content starts at 14.
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 1,
                    offset: 0
                }
            ),
            Some(14)
        );
        assert_eq!(
            position(
                &d,
                Caret {
                    block: 1,
                    offset: 2
                }
            ),
            Some(16)
        );
        assert_eq!(
            caret(&d, 16),
            Some(Caret {
                block: 1,
                offset: 2
            })
        );
        assert_eq!(
            caret(&d, 17),
            Some(Caret {
                block: 1,
                offset: 3
            })
        );
        assert_eq!(caret(&d, 12), None);
    }
}
