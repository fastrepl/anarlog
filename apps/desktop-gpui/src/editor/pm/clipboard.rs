//! `parseFromClipboard` and `doPaste` (`prosemirror-view/src/clipboard.ts`,
//! `input.ts`) for HTML on the clipboard, plus `Selection.near` for where
//! the caret lands (`selectionToInsertionEnd`).

use super::dom::{self, ParseOptions, Parser, PreserveWs};
use super::node::{Fragment, Node, Slice};
use super::resolved::ResolvedPos;
use super::schema::{Schema, TypeId};
use super::transform;

/// Where the selection goes after a paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionEnd {
    Text(usize),
    /// A `NodeSelection` of the atom spanning `from..to`.
    Node {
        from: usize,
        to: usize,
    },
}

pub struct Pasted {
    pub doc: Node,
    pub selection: SelectionEnd,
}

/// `parseFromClipboard(view, text, html, false, $context)` when the HTML
/// is used: `None` for nothing to paste.
pub fn parse_from_clipboard(
    schema: &'static Schema,
    html: &str,
    context: &ResolvedPos,
) -> Option<Slice> {
    if html.is_empty() {
        return None;
    }
    let mut dom = dom::read_html(html);
    // `browser.webkit`: the web view is WebKit.
    dom::restore_replaced_spaces(&dom);
    let slice_data = dom.select_first("[data-pm-slice]").ok().and_then(|node| {
        node.attributes
            .borrow()
            .get("data-pm-slice")
            .and_then(parse_slice_data)
    });
    if let Some(data) = &slice_data {
        for _ in 0..data.wrappers {
            let mut child = dom.first_child();
            while let Some(node) = &child
                && node.as_element().is_none()
            {
                child = node.next_sibling();
            }
            match child {
                Some(child) => dom = child,
                None => break,
            }
        }
    }
    let parser = Parser::for_schema();
    let slice = parser.parse_slice(
        &dom,
        &ParseOptions {
            preserve_whitespace: Some(if slice_data.is_some() {
                PreserveWs::Preserve
            } else {
                PreserveWs::Collapse
            }),
            context: Some(context),
            clipboard_br_rule: true,
        },
    );
    let slice = match slice_data {
        Some(data) => add_context(
            schema,
            close_slice(schema, slice, data.open_start, data.open_end),
            &data.context,
        ),
        None => {
            // HTML not made by ProseMirror: coherent top-level siblings.
            let mut slice = Slice::max_open(
                schema,
                normalize_siblings(schema, slice.content, context),
                true,
            );
            if slice.open_start > 0 || slice.open_end > 0 {
                let mut open_start = 0;
                let mut node = slice.content.first_child();
                while open_start < slice.open_start
                    && let Some(n) = node
                    && !schema.nodes[n.type_id].isolating
                {
                    open_start += 1;
                    node = n.first_child();
                }
                let mut open_end = 0;
                let mut node = slice.content.last_child();
                while open_end < slice.open_end
                    && let Some(n) = node
                    && !schema.nodes[n.type_id].isolating
                {
                    open_end += 1;
                    node = n.last_child();
                }
                slice = close_slice(schema, slice, open_start, open_end);
            }
            slice
        }
    };
    Some(slice)
}

struct SliceData {
    open_start: usize,
    open_end: usize,
    wrappers: usize,
    context: String,
}

/// `/^(\d+) (\d+)(?: -(\d+))? (.*)/`
fn parse_slice_data(value: &str) -> Option<SliceData> {
    let mut rest = value;
    let number = |rest: &mut &str| -> Option<usize> {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        *rest = &rest[digits.len()..];
        digits.parse().ok()
    };
    let open_start = number(&mut rest)?;
    rest = rest.strip_prefix(' ')?;
    let open_end = number(&mut rest)?;
    let mut wrappers = 0;
    if let Some(after) = rest.strip_prefix(" -") {
        let mut probe = after;
        if let Some(count) = number(&mut probe) {
            wrappers = count;
            rest = probe;
        }
    }
    rest = rest.strip_prefix(' ')?;
    let context = rest.lines().next().unwrap_or("").to_string();
    Some(SliceData {
        open_start,
        open_end,
        wrappers,
        context,
    })
}

/// `addContext`: the slice's copied ancestors put back around it.
fn add_context(schema: &'static Schema, slice: Slice, context: &str) -> Slice {
    if slice.size() == 0 {
        return slice;
    }
    let Ok(serde_json::Value::Array(array)) = serde_json::from_str::<serde_json::Value>(context)
    else {
        return slice;
    };
    let Slice {
        mut content,
        mut open_start,
        mut open_end,
    } = slice;
    let mut i = array.len() as isize - 2;
    while i >= 0 {
        let index = i as usize;
        let Some(ty) = array[index].as_str().and_then(|name| schema.node(name)) else {
            break;
        };
        if schema.nodes[ty].has_required_attrs() {
            break;
        }
        let attrs = array.get(index + 1).and_then(|attrs| attrs.as_object());
        content = Fragment::from(vec![Node::new(schema, ty, attrs, content, Vec::new())]);
        open_start += 1;
        open_end += 1;
        i -= 2;
    }
    Slice::new(content, open_start, open_end)
}

/// `normalizeSiblings`: top-level nodes that cannot be siblings anywhere in
/// the context are wrapped so that they can.
fn normalize_siblings(
    schema: &'static Schema,
    fragment: Fragment,
    context: &ResolvedPos,
) -> Fragment {
    if fragment.child_count() < 2 {
        return fragment;
    }
    for d in (0..=context.depth()).rev() {
        let parent = context.node(d);
        let mut m = parent.content_match_at(schema, context.index(d));
        let mut last_wrap: Vec<TypeId> = Vec::new();
        let mut result: Vec<Node> = Vec::new();
        let mut failed = false;
        for node in &fragment.children {
            let Some(wrap) = m.find_wrapping(node.type_id, schema) else {
                failed = true;
                break;
            };
            let in_last = if !result.is_empty() && !last_wrap.is_empty() {
                add_to_sibling(schema, &wrap, &last_wrap, node, result.last().unwrap(), 0)
            } else {
                None
            };
            if let Some(in_last) = in_last {
                *result.last_mut().unwrap() = in_last;
            } else {
                if let Some(last) = result.last_mut() {
                    *last = close_right(schema, last, last_wrap.len());
                }
                let wrapped = with_wrappers(schema, node.clone(), &wrap, 0);
                m = m
                    .match_type(wrapped.type_id)
                    .expect("the wrapped node matches");
                result.push(wrapped);
                last_wrap = wrap;
            }
        }
        if !failed {
            return Fragment::from(result);
        }
    }
    fragment
}

fn with_wrappers(schema: &'static Schema, mut node: Node, wrap: &[TypeId], from: usize) -> Node {
    for ty in wrap[from..].iter().rev() {
        node = Node::new(schema, *ty, None, Fragment::from(vec![node]), Vec::new());
    }
    node
}

fn add_to_sibling(
    schema: &'static Schema,
    wrap: &[TypeId],
    last_wrap: &[TypeId],
    node: &Node,
    sibling: &Node,
    depth: usize,
) -> Option<Node> {
    if depth < wrap.len() && depth < last_wrap.len() && wrap[depth] == last_wrap[depth] {
        if let Some(inner) = add_to_sibling(
            schema,
            wrap,
            last_wrap,
            node,
            sibling.last_child()?,
            depth + 1,
        ) {
            return Some(
                sibling.copy(
                    sibling
                        .content
                        .replace_child(sibling.child_count() - 1, inner),
                ),
            );
        }
        let m = sibling.content_match_at(schema, sibling.child_count());
        let next = if depth == wrap.len() - 1 {
            node.type_id
        } else {
            wrap[depth + 1]
        };
        if m.match_type(next).is_some() {
            return Some(sibling.copy(sibling.content.append(&Fragment::from(vec![
                with_wrappers(schema, node.clone(), wrap, depth + 1),
            ]))));
        }
    }
    None
}

fn close_right(schema: &'static Schema, node: &Node, depth: usize) -> Node {
    if depth == 0 {
        return node.clone();
    }
    let fragment = node.content.replace_child(
        node.child_count() - 1,
        close_right(schema, node.last_child().unwrap(), depth - 1),
    );
    let fill = node
        .content_match_at(schema, node.child_count())
        .fill_before(&Fragment::empty(), true, 0, schema)
        .unwrap_or_default();
    node.copy(fragment.append(&fill))
}

fn close_range(
    schema: &'static Schema,
    fragment: &Fragment,
    side: i8,
    from: usize,
    to: usize,
    depth: usize,
    open_end: usize,
) -> Fragment {
    let node = if side < 0 {
        fragment.first_child().unwrap()
    } else {
        fragment.last_child().unwrap()
    };
    let mut inner = node.content.clone();
    let open_end = if fragment.child_count() > 1 {
        0
    } else {
        open_end
    };
    if depth < to - 1 {
        inner = close_range(schema, &inner, side, from, to, depth + 1, open_end);
    }
    if depth >= from {
        inner = if side < 0 {
            node.content_match_at(schema, 0)
                .fill_before(&inner, open_end <= depth, 0, schema)
                .unwrap_or_default()
                .append(&inner)
        } else {
            inner.append(
                &node
                    .content_match_at(schema, node.child_count())
                    .fill_before(&Fragment::empty(), true, 0, schema)
                    .unwrap_or_default(),
            )
        };
    }
    let index = if side < 0 {
        0
    } else {
        fragment.child_count() - 1
    };
    fragment.replace_child(index, node.copy(inner))
}

fn close_slice(
    schema: &'static Schema,
    mut slice: Slice,
    open_start: usize,
    open_end: usize,
) -> Slice {
    if open_start < slice.open_start {
        slice = Slice::new(
            close_range(
                schema,
                &slice.content,
                -1,
                open_start,
                slice.open_start,
                0,
                slice.open_end,
            ),
            open_start,
            slice.open_end,
        );
    }
    if open_end < slice.open_end {
        slice = Slice::new(
            close_range(schema, &slice.content, 1, open_end, slice.open_end, 0, 0),
            slice.open_start,
            open_end,
        );
    }
    slice
}

/// `doPaste` after `handlePaste` declined: a single closed node goes
/// through `replaceSelectionWith(node, preferPlain = false)` — its own
/// marks kept — anything else through `replaceSelection`; the selection
/// moves to the insertion end. `None` when the document does not change.
pub fn paste(
    schema: &'static Schema,
    doc: &Node,
    from: usize,
    to: usize,
    slice: &Slice,
) -> Option<Pasted> {
    let single = (slice.open_start == 0 && slice.open_end == 0 && slice.content.child_count() == 1)
        .then(|| slice.content.first_child().unwrap().clone());
    let (applied, bias) = match single {
        Some(node) => {
            let inline = node.is_inline(schema);
            (
                transform::replace_range_with(schema, doc, from, to, node)?,
                if inline { -1 } else { 1 },
            )
        }
        None => {
            let mut last_node = slice.content.last_child();
            let mut last_parent = None;
            for _ in 0..slice.open_end {
                last_parent = last_node;
                last_node = last_node.and_then(Node::last_child);
            }
            let bias = match (last_node, last_parent) {
                (Some(node), _) => {
                    if node.is_inline(schema) {
                        -1
                    } else {
                        1
                    }
                }
                (None, Some(parent)) if parent.is_textblock(schema) => -1,
                _ => 1,
            };
            (
                transform::replace_range(schema, doc, from, to, slice)?,
                bias,
            )
        }
    };
    let selection = near(schema, &applied.doc, applied.end, bias);
    Some(Pasted {
        doc: applied.doc,
        selection,
    })
}

/// `Selection.near($pos, bias)`.
pub fn near(schema: &Schema, doc: &Node, pos: usize, bias: i8) -> SelectionEnd {
    let resolved = doc.resolve(pos);
    find_from(schema, &resolved, bias, false)
        .or_else(|| find_from(schema, &resolved, -bias, false))
        .unwrap_or(SelectionEnd::Text(pos))
}

/// `Selection.near` restricted to text selections: where a caret can go
/// nearest to `pos`.
pub fn near_text(schema: &Schema, doc: &Node, pos: usize, bias: i8) -> Option<usize> {
    let resolved = doc.resolve(pos);
    match find_from(schema, &resolved, bias, true)
        .or_else(|| find_from(schema, &resolved, -bias, true))?
    {
        SelectionEnd::Text(pos) => Some(pos),
        SelectionEnd::Node { .. } => None,
    }
}

fn find_from(schema: &Schema, pos: &ResolvedPos, dir: i8, text_only: bool) -> Option<SelectionEnd> {
    let inner = if pos.parent().inline_content(schema) {
        Some(SelectionEnd::Text(pos.pos))
    } else {
        find_selection_in(
            schema,
            pos.parent(),
            pos.pos,
            pos.index(pos.depth()),
            dir,
            text_only,
        )
    };
    if inner.is_some() {
        return inner;
    }
    for depth in (0..pos.depth()).rev() {
        let found = if dir < 0 {
            find_selection_in(
                schema,
                pos.node(depth),
                pos.before(depth + 1),
                pos.index(depth),
                dir,
                text_only,
            )
        } else {
            find_selection_in(
                schema,
                pos.node(depth),
                pos.after(depth + 1),
                pos.index(depth) + 1,
                dir,
                text_only,
            )
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn find_selection_in(
    schema: &Schema,
    node: &Node,
    pos: usize,
    index: usize,
    dir: i8,
    text_only: bool,
) -> Option<SelectionEnd> {
    if node.inline_content(schema) {
        return Some(SelectionEnd::Text(pos));
    }
    let mut pos = pos as isize;
    let mut i = index as isize - if dir > 0 { 0 } else { 1 };
    while if dir > 0 {
        i < node.child_count() as isize
    } else {
        i >= 0
    } {
        let child = node.child(i as usize);
        if !child.is_atom(schema) {
            let inner = find_selection_in(
                schema,
                child,
                (pos + dir as isize) as usize,
                if dir < 0 { child.child_count() } else { 0 },
                dir,
                text_only,
            );
            if inner.is_some() {
                return inner;
            }
        } else if !text_only && is_selectable(schema, child) {
            let from = if dir < 0 {
                pos as usize - child.node_size()
            } else {
                pos as usize
            };
            return Some(SelectionEnd::Node {
                from,
                to: from + child.node_size(),
            });
        }
        pos += child.node_size() as isize * dir as isize;
        i += dir as isize;
    }
    None
}

/// `NodeSelection.isSelectable`: not text, `selectable` unless the spec
/// turns it off (`hardBreak`, `session`).
fn is_selectable(schema: &Schema, node: &Node) -> bool {
    !node.is_text() && !matches!(schema.nodes[node.type_id].name, "hardBreak" | "session")
}

#[cfg(test)]
mod tests {
    use super::super::schema::schema;
    use super::*;

    /// Cases recorded from the real editor: `EditorView.pasteHTML(html)` on
    /// `packages/editor`'s note schema under jsdom (WebKit user agent), with
    /// the document, selection and HTML of each case and the resulting
    /// document and selection.
    const FIXTURES: &str = include_str!("fixtures/paste.gen.json");

    #[test]
    fn pastes_land_like_the_web_view() {
        let s = schema();
        let fixtures: serde_json::Value = serde_json::from_str(FIXTURES).unwrap();
        let docs = &fixtures["docs"];
        let htmls = &fixtures["htmls"];
        let mut failures = Vec::new();
        let mut count = 0;
        for case in fixtures["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let doc = Node::from_json(s, &docs[case["doc"].as_str().unwrap()]["doc"]).unwrap();
            let html = htmls[case["html"].as_str().unwrap()].as_str().unwrap();
            let from = case["from"].as_u64().unwrap() as usize;
            let to = case["to"].as_u64().unwrap() as usize;
            let expected = &case["result"];
            let expected_selection = (
                case["selection"]["anchor"].as_u64().unwrap() as usize,
                case["selection"]["head"].as_u64().unwrap() as usize,
            );
            count += 1;
            let context = doc.resolve(from);
            let slice = parse_from_clipboard(s, html, &context);
            let handled = case["handled"].as_bool().unwrap();
            let Some(slice) = slice else {
                if handled {
                    failures.push(format!("{name}: nothing parsed"));
                }
                continue;
            };
            let (result, selection) = match paste(s, &doc, from, to, &slice) {
                Some(pasted) => (pasted.doc, pasted.selection),
                None => (doc.clone(), SelectionEnd::Text(from)),
            };
            let result_json = result.to_json(s);
            if &result_json != expected {
                failures.push(format!(
                    "{name}: doc\n  got      {}\n  expected {}",
                    result_json, expected
                ));
                continue;
            }
            let got_selection = match selection {
                SelectionEnd::Text(pos) => (pos, pos),
                SelectionEnd::Node { from, to } => (from, to),
            };
            if got_selection != expected_selection {
                failures.push(format!(
                    "{name}: selection got {got_selection:?} expected {expected_selection:?}"
                ));
            }
        }
        assert!(count > 100, "the fixtures loaded");
        assert!(
            failures.is_empty(),
            "{} of {count} cases differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
