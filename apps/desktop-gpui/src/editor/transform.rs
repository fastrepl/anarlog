use std::ops::Range;

use super::{
    document::{Document, EditResult, Node, NodeRef, Resolved, split_inline},
    sequence::{Measured, Sequence},
};

pub(super) fn removable(node: &NodeRef) -> EditResult<()> {
    if !node.known() {
        return Err("This selection contains a future node; original content retained".into());
    }
    let mut result = Ok(());
    node.children.visit(&mut |child| {
        if result.is_ok() {
            result = removable(child);
        }
    });
    result
}

pub(super) fn removable_inline(
    children: &Sequence<NodeRef>,
    range: Range<usize>,
) -> EditResult<()> {
    if range.is_empty() {
        return Ok(());
    }
    let through = split_inline(children, range.end)?.0;
    let selected = split_inline(&through, range.start)?.1;
    let mut result = Ok(());
    selected.visit(&mut |node| {
        if result.is_ok() {
            result = removable(node);
        }
    });
    result
}

pub(super) fn adjacent_block(document: &Document, path: &[usize], forward: bool) -> Option<usize> {
    fn edge(node: &NodeRef, path: &mut Vec<usize>, forward: bool) -> Option<(Vec<usize>, usize)> {
        if node.is_textblock() {
            return Some((
                path.clone(),
                if forward { 0 } else { node.children.units() },
            ));
        }
        if !node.known() {
            return None;
        }
        for offset in 0..node.children.len() {
            let index = if forward {
                offset
            } else {
                node.children.len() - 1 - offset
            };
            path.push(index);
            let result = edge(node.children.get(index)?, path, forward);
            path.pop();
            if result.is_some() {
                return result;
            }
        }
        None
    }
    let mut cursor = path.to_vec();
    while let Some(index) = cursor.pop() {
        let parent = document.node(&cursor)?;
        let mut next = if forward {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        while let Some(index) = next.filter(|index| *index < parent.children.len()) {
            cursor.push(index);
            let result = edge(parent.children.get(index)?, &mut cursor, forward);
            cursor.pop();
            if let Some((path, offset)) = result {
                return document
                    .position_at_path(&path)
                    .map(|start| start + 1 + offset);
            }
            next = if forward {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            };
        }
    }
    None
}

pub(super) fn replace_across(
    document: &Document,
    from: &Resolved,
    to: &Resolved,
    children: Sequence<NodeRef>,
) -> EditResult<NodeRef> {
    fn visit(
        node: &NodeRef,
        start: usize,
        range: &Range<usize>,
        from: &Resolved,
        to: &Resolved,
        children: &Sequence<NodeRef>,
    ) -> EditResult<Option<NodeRef>> {
        if node.id == from.node.id {
            return Ok(Some(node.with_children(children.clone())));
        }
        if node.id == to.node.id {
            return Ok(None);
        }
        let end = start + node.units();
        if end <= range.start || start >= range.end {
            return Ok(Some(node.clone()));
        }
        if !node.known() {
            return Err("This selection contains a future node; original content retained".into());
        }
        if node.kind() != "doc" && start >= range.start && end <= range.end {
            removable(node)?;
            return Ok(None);
        }
        let content_start = start + usize::from(node.kind() != "doc");
        let Some((first, _, _)) = node
            .children
            .locate(range.start.saturating_sub(content_start))
        else {
            return Ok(Some(node.clone()));
        };
        let mut last = first;
        let mut changed = Vec::new();
        while let Some(child) = node.children.get(last) {
            let child_start = content_start + node.children.prefix(last);
            if child_start >= range.end {
                break;
            }
            if let Some(child) = visit(child, child_start, range, from, to, children)? {
                changed.push(child);
            }
            last += 1;
        }
        let mut content = node
            .children
            .splice(first..last, &Sequence::from_items(changed));
        if content.len() == 0 {
            return Ok(None);
        }
        if matches!(node.kind(), "listItem" | "taskItem")
            && content
                .get(0)
                .is_some_and(|child| child.kind() != "paragraph")
        {
            content = Sequence::one(Node::fresh("paragraph", Sequence::default())).concat(&content);
        }
        Ok(Some(node.with_children(content)))
    }
    let range = from.start - 1..to.start + to.node.children.units() + 1;
    visit(&document.root, 0, &range, from, to, &children)?
        .ok_or_else(|| "Replacement would remove the document root".into())
}

pub(super) fn selected_blocks(
    document: &Document,
    range: Range<usize>,
) -> EditResult<Vec<Resolved>> {
    fn visit(
        node: &NodeRef,
        start: usize,
        path: &mut Vec<usize>,
        range: &Range<usize>,
        output: &mut Vec<Resolved>,
    ) -> EditResult<()> {
        if start + node.units() <= range.start || start > range.end {
            return Ok(());
        }
        if !node.known() {
            return Err("This selection contains a future node; original content retained".into());
        }
        if node.is_textblock() {
            output.push(Resolved {
                path: path.clone(),
                node: node.clone(),
                start: start + 1,
                offset: range
                    .start
                    .saturating_sub(start + 1)
                    .min(node.children.units()),
            });
            return Ok(());
        }
        let content_start = start + usize::from(node.kind() != "doc");
        if let Some((mut index, _, _)) = node
            .children
            .locate(range.start.saturating_sub(content_start))
        {
            while let Some(child) = node.children.get(index) {
                let child_start = content_start + node.children.prefix(index);
                if child_start > range.end {
                    break;
                }
                path.push(index);
                visit(child, child_start, path, range, output)?;
                path.pop();
                index += 1;
            }
        }
        Ok(())
    }
    let mut blocks = Vec::new();
    visit(&document.root, 0, &mut Vec::new(), &range, &mut blocks)?;
    Ok(blocks)
}

pub(super) fn copy_range(
    document: &Document,
    range: Range<usize>,
) -> EditResult<Sequence<NodeRef>> {
    fn slice(node: &NodeRef, start: usize, range: &Range<usize>) -> EditResult<Option<NodeRef>> {
        if start + node.units() <= range.start || start >= range.end {
            return Ok(None);
        }
        if range.start <= start && range.end >= start + node.units() {
            return Ok(Some(node.clone()));
        }
        if node.is_textblock() {
            let a = range.start.saturating_sub(start + 1);
            let b = range
                .end
                .saturating_sub(start + 1)
                .min(node.children.units());
            let through = split_inline(&node.children, b)?.0;
            return Ok(Some(node.with_children(split_inline(&through, a)?.1)));
        }
        if !node.known() || node.is_atom() {
            return Err("Cannot partially copy an opaque node".into());
        }
        let mut content = Vec::new();
        for index in 0..node.children.len() {
            let child = node.children.get(index).expect("child");
            let child_start =
                start + usize::from(node.kind() != "doc") + node.children.prefix(index);
            if child_start >= range.end {
                break;
            }
            if let Some(child) = slice(child, child_start, range)? {
                content.push(child);
            }
        }
        Ok(Some(node.with_children(Sequence::from_items(content))))
    }
    Ok(slice(&document.root, 0, &range)?
        .ok_or("Empty selection")?
        .children
        .clone())
}
