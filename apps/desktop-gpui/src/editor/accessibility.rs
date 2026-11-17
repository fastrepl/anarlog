use std::ops::Range;

use super::{
    document::{Document, NodeRef, utf8},
    model::Selection,
    sequence::Measured,
};

struct Segment {
    position: usize,
    units: usize,
    bytes: Range<usize>,
    text: bool,
}

/// The note as a screen reader reads it: each text block on its own line, mentions and
/// chips by their label. Returns that text and the selection as byte offsets into it, or no
/// selection when it is not inside text (an image or a table cell boundary).
pub fn plain_text(document: &Document, selection: Selection) -> (String, Option<(usize, usize)>) {
    fn walk(node: &NodeRef, start: usize, out: &mut String, segments: &mut Vec<Segment>) {
        if node.is_textblock() {
            if !out.is_empty() {
                out.push('\n');
            }
            segments.push(Segment {
                position: start,
                units: 0,
                bytes: out.len()..out.len(),
                text: true,
            });
            let mut position = start;
            for index in 0..node.children.len() {
                let child = node.children.get(index).expect("child");
                let byte = out.len();
                let units = child.units();
                let text = if let Some(text) = &child.text {
                    out.push_str(&text.as_string());
                    true
                } else {
                    out.push_str(match child.kind() {
                        "hardBreak" => "\n",
                        _ => child
                            .attr("label")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("\u{fffc}"),
                    });
                    false
                };
                segments.push(Segment {
                    position,
                    units,
                    bytes: byte..out.len(),
                    text,
                });
                position += units;
            }
            return;
        }
        if !node.known() || node.is_atom() {
            return;
        }
        for index in 0..node.children.len() {
            let child = node.children.get(index).expect("child");
            walk(
                child,
                start + node.children.prefix(index) + 1,
                out,
                segments,
            );
        }
    }
    let mut out = String::new();
    let mut segments = Vec::new();
    walk(&document.root, 0, &mut out, &mut segments);
    let byte = |position: usize| {
        let segment = segments.iter().find(|segment| {
            segment.position <= position && position <= segment.position + segment.units
        })?;
        Some(if position == segment.position {
            segment.bytes.start
        } else if segment.text {
            segment.bytes.start
                + utf8(&out[segment.bytes.clone()], position - segment.position).ok()?
        } else {
            segment.bytes.end
        })
    };
    let selection = byte(selection.anchor).zip(byte(selection.head));
    (out, selection)
}
