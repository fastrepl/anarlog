//! `prosemirror-transform`'s `replaceStep` (the `Fitter`), `replaceRange`,
//! `replaceRangeWith` and `insertPoint`, applied straight to a document:
//! each returns the new document and the end of the inserted content the
//! selection moves to (`selectionToInsertionEnd`).

use super::content::Match;
use super::node::{Fragment, Mark, Node, Slice};
use super::resolved::ResolvedPos;
use super::schema::{Schema, TypeId};

pub struct Applied {
    pub doc: Node,
    /// `newTo` of the step map's first range.
    pub end: usize,
}

enum Step {
    Replace {
        from: usize,
        to: usize,
        slice: Slice,
    },
    ReplaceAround {
        from: usize,
        to: usize,
        gap_from: usize,
        gap_to: usize,
        slice: Slice,
        insert: usize,
    },
}

impl Step {
    fn apply(&self, schema: &'static Schema, doc: &Node) -> Option<Applied> {
        match self {
            Step::Replace { from, to, slice } => Some(Applied {
                doc: doc.replace(schema, *from, *to, slice)?,
                end: from + slice.size(),
            }),
            Step::ReplaceAround {
                from,
                to,
                gap_from,
                gap_to,
                slice,
                insert,
            } => {
                let gap = doc.slice(*gap_from, *gap_to);
                if gap.open_start > 0 || gap.open_end > 0 {
                    return None;
                }
                let inserted = slice.insert_at(schema, *insert, &gap.content)?;
                Some(Applied {
                    doc: doc.replace(schema, *from, *to, &inserted)?,
                    end: from + insert,
                })
            }
        }
    }
}

fn fits_trivially(
    schema: &'static Schema,
    from: &ResolvedPos,
    to: &ResolvedPos,
    slice: &Slice,
) -> bool {
    slice.open_start == 0
        && slice.open_end == 0
        && from.start(from.depth()) == to.start(to.depth())
        && from.parent().can_replace(
            schema,
            from.index(from.depth()),
            to.index(to.depth()),
            &slice.content,
            0,
            slice.content.child_count(),
        )
}

/// `replaceStep`: `None` for a no-op.
fn replace_step(
    schema: &'static Schema,
    doc: &Node,
    from: usize,
    to: usize,
    slice: &Slice,
) -> Option<Step> {
    if from == to && slice.size() == 0 {
        return None;
    }
    let from_pos = doc.resolve(from);
    let to_pos = doc.resolve(to);
    if fits_trivially(schema, &from_pos, &to_pos, slice) {
        return Some(Step::Replace {
            from,
            to,
            slice: slice.clone(),
        });
    }
    Fitter::new(schema, &from_pos, &to_pos, slice.clone()).fit()
}

/// `tr.replace(from, to, slice)`: `None` when nothing changes.
pub fn replace(
    schema: &'static Schema,
    doc: &Node,
    from: usize,
    to: usize,
    slice: &Slice,
) -> Option<Applied> {
    replace_step(schema, doc, from, to, slice)?.apply(schema, doc)
}

struct Fittable {
    slice_depth: usize,
    frontier_depth: usize,
    parent: Option<Node>,
    inject: Option<Fragment>,
    wrap: Option<Vec<TypeId>>,
}

struct Fitter<'a> {
    schema: &'static Schema,
    from: &'a ResolvedPos<'a>,
    to: &'a ResolvedPos<'a>,
    unplaced: Slice,
    frontier: Vec<(TypeId, Match)>,
    placed: Fragment,
}

impl<'a> Fitter<'a> {
    fn new(
        schema: &'static Schema,
        from: &'a ResolvedPos<'a>,
        to: &'a ResolvedPos<'a>,
        unplaced: Slice,
    ) -> Fitter<'a> {
        let mut frontier = Vec::new();
        for i in 0..=from.depth() {
            let node = from.node(i);
            frontier.push((
                node.type_id,
                node.content_match_at(schema, from.index_after(i)),
            ));
        }
        let mut placed = Fragment::empty();
        for i in (1..=from.depth()).rev() {
            placed = Fragment::from(vec![from.node(i).copy(placed)]);
        }
        Fitter {
            schema,
            from,
            to,
            unplaced,
            frontier,
            placed,
        }
    }

    fn depth(&self) -> usize {
        self.frontier.len() - 1
    }

    fn fit(mut self) -> Option<Step> {
        while self.unplaced.size() > 0 {
            match self.find_fittable() {
                Some(fit) => self.place_nodes(fit),
                None => {
                    if !self.open_more() {
                        self.drop_node();
                    }
                }
            }
        }
        let move_inline = self.must_move_inline();
        let placed_size = self.placed.size - self.depth() - self.from.depth();
        let from = self.from;
        let doc = from.doc();
        let moved;
        let to_target: &ResolvedPos = match move_inline {
            Some(pos) => {
                moved = doc.resolve(pos);
                &moved
            }
            None => self.to,
        };
        let to = self.close(to_target)?;
        let mut content = self.placed.clone();
        let mut open_start = from.depth();
        let mut open_end = to.depth();
        while open_start > 0 && open_end > 0 && content.child_count() == 1 {
            content = content.first_child().unwrap().content.clone();
            open_start -= 1;
            open_end -= 1;
        }
        let slice = Slice::new(content, open_start, open_end);
        if let Some(move_inline) = move_inline {
            return Some(Step::ReplaceAround {
                from: from.pos,
                to: move_inline,
                gap_from: self.to.pos,
                gap_to: self.to.end(self.to.depth()),
                slice,
                insert: placed_size,
            });
        }
        if slice.size() > 0 || from.pos != self.to.pos {
            return Some(Step::Replace {
                from: from.pos,
                to: to.pos,
                slice,
            });
        }
        None
    }

    fn find_fittable(&self) -> Option<Fittable> {
        let schema = self.schema;
        let mut start_depth = self.unplaced.open_start;
        {
            let mut cur = &self.unplaced.content;
            let mut open_end = self.unplaced.open_end;
            for d in 0..start_depth {
                let node = cur.first_child().unwrap();
                if cur.child_count() > 1 {
                    open_end = 0;
                }
                if schema.nodes[node.type_id].isolating && open_end <= d {
                    start_depth = d;
                    break;
                }
                cur = &node.content;
            }
        }
        for pass in 1..=2 {
            let top = if pass == 1 {
                start_depth
            } else {
                self.unplaced.open_start
            };
            for slice_depth in (0..=top).rev() {
                let (fragment, parent): (&Fragment, Option<&Node>) = if slice_depth > 0 {
                    let parent = content_at(&self.unplaced.content, slice_depth - 1)
                        .first_child()
                        .unwrap();
                    (&parent.content, Some(parent))
                } else {
                    (&self.unplaced.content, None)
                };
                let first = fragment.first_child();
                for frontier_depth in (0..=self.depth()).rev() {
                    let (ty, m) = self.frontier[frontier_depth];
                    if pass == 1 {
                        let mut inject = None;
                        let fits = match first {
                            Some(first) => {
                                m.match_type(first.type_id).is_some() || {
                                    inject = m.fill_before(
                                        &Fragment::from(vec![first.clone()]),
                                        false,
                                        0,
                                        schema,
                                    );
                                    inject.is_some()
                                }
                            }
                            None => parent.is_some_and(|parent| {
                                schema.compatible_content(ty, parent.type_id)
                            }),
                        };
                        if fits {
                            return Some(Fittable {
                                slice_depth,
                                frontier_depth,
                                parent: parent.cloned(),
                                inject,
                                wrap: None,
                            });
                        }
                    } else if let Some(first) = first
                        && let Some(wrap) = m.find_wrapping(first.type_id, schema)
                    {
                        return Some(Fittable {
                            slice_depth,
                            frontier_depth,
                            parent: parent.cloned(),
                            inject: None,
                            wrap: Some(wrap),
                        });
                    }
                    if let Some(parent) = parent
                        && m.match_type(parent.type_id).is_some()
                    {
                        break;
                    }
                }
            }
        }
        None
    }

    fn open_more(&mut self) -> bool {
        let Slice {
            content,
            open_start,
            open_end,
        } = &self.unplaced;
        let inner = content_at(content, *open_start);
        let Some(first) = inner.first_child() else {
            return false;
        };
        if self.schema.nodes[first.type_id].is_leaf() {
            return false;
        }
        let new_open_end = (*open_end).max(if inner.size + open_start >= content.size - open_end {
            open_start + 1
        } else {
            0
        });
        self.unplaced = Slice::new(content.clone(), open_start + 1, new_open_end);
        true
    }

    fn drop_node(&mut self) {
        let Slice {
            content,
            open_start,
            open_end,
        } = &self.unplaced;
        let inner = content_at(content, *open_start);
        if inner.child_count() <= 1 && *open_start > 0 {
            let open_at_end = content.size - open_start <= open_start + inner.size;
            self.unplaced = Slice::new(
                drop_from_fragment(content, open_start - 1, 1),
                open_start - 1,
                if open_at_end {
                    open_start - 1
                } else {
                    *open_end
                },
            );
        } else {
            self.unplaced = Slice::new(
                drop_from_fragment(content, *open_start, 1),
                *open_start,
                *open_end,
            );
        }
    }

    fn place_nodes(&mut self, fit: Fittable) {
        let schema = self.schema;
        let Fittable {
            slice_depth,
            frontier_depth,
            parent,
            inject,
            wrap,
        } = fit;
        while self.depth() > frontier_depth {
            self.close_frontier_node();
        }
        if let Some(wrap) = wrap {
            for ty in wrap {
                self.open_frontier_node(ty, None, None);
            }
        }
        let slice = self.unplaced.clone();
        let fragment = match &parent {
            Some(parent) => parent.content.clone(),
            None => slice.content.clone(),
        };
        let open_start = slice.open_start - slice_depth;
        let mut taken = 0;
        let mut add: Vec<Node> = Vec::new();
        let (ty, mut m) = self.frontier[frontier_depth];
        if let Some(inject) = &inject {
            add.extend(inject.children.iter().cloned());
            m = m
                .match_fragment(inject, 0, inject.child_count())
                .expect("the injected content matches");
        }
        let mut open_end_count =
            (fragment.size + slice_depth) as isize - (slice.content.size - slice.open_end) as isize;
        while taken < fragment.child_count() {
            let next = fragment.child(taken);
            let Some(matches) = m.match_type(next.type_id) else {
                break;
            };
            taken += 1;
            if taken > 1 || open_start == 0 || next.content.size > 0 {
                m = matches;
                let marks = schema.nodes[ty].allowed_marks_of(&next.marks);
                add.push(close_node_start(
                    schema,
                    &next.mark(marks),
                    if taken == 1 { open_start } else { 0 },
                    if taken == fragment.child_count() {
                        open_end_count
                    } else {
                        -1
                    },
                ));
            }
        }
        let to_end = taken == fragment.child_count();
        if !to_end {
            open_end_count = -1;
        }
        self.placed = add_to_fragment(&self.placed, frontier_depth, &Fragment::from(add));
        self.frontier[frontier_depth].1 = m;
        if to_end
            && open_end_count < 0
            && let Some(parent) = &parent
            && parent.type_id == self.frontier[self.depth()].0
            && self.frontier.len() > 1
        {
            self.close_frontier_node();
        }
        let mut cur = fragment;
        for _ in 0..open_end_count.max(0) {
            let node = cur.last_child().unwrap().clone();
            self.frontier.push((
                node.type_id,
                node.content_match_at(schema, node.child_count()),
            ));
            cur = node.content;
        }
        self.unplaced = if !to_end {
            Slice::new(
                drop_from_fragment(&slice.content, slice_depth, taken),
                slice.open_start,
                slice.open_end,
            )
        } else if slice_depth == 0 {
            Slice::empty()
        } else {
            Slice::new(
                drop_from_fragment(&slice.content, slice_depth - 1, 1),
                slice_depth - 1,
                if open_end_count < 0 {
                    slice.open_end
                } else {
                    slice_depth - 1
                },
            )
        };
    }

    fn must_move_inline(&self) -> Option<usize> {
        let schema = self.schema;
        if !self.to.parent().is_textblock(schema) {
            return None;
        }
        let (top_ty, top_m) = self.frontier[self.depth()];
        if !schema.nodes[top_ty].is_textblock(schema)
            || content_after_fits(schema, self.to, self.to.depth(), top_ty, top_m, false).is_none()
            || (self.to.depth() == self.depth()
                && self
                    .find_close_level(self.to)
                    .is_some_and(|level| level.depth == self.depth()))
        {
            return None;
        }
        let mut depth = self.to.depth();
        let mut after = self.to.after(depth);
        while depth > 1 {
            depth -= 1;
            if after == self.to.end(depth) {
                after += 1;
            } else {
                break;
            }
        }
        Some(after)
    }

    fn find_close_level(&self, to: &ResolvedPos) -> Option<CloseLevel> {
        let schema = self.schema;
        'scan: for i in (0..=self.depth().min(to.depth())).rev() {
            let (ty, m) = self.frontier[i];
            let drop_inner = i < to.depth() && to.end(i + 1) == to.pos + (to.depth() - (i + 1));
            let Some(fit) = content_after_fits(schema, to, i, ty, m, drop_inner) else {
                continue;
            };
            for d in (0..i).rev() {
                let (ty, m) = self.frontier[d];
                match content_after_fits(schema, to, d, ty, m, true) {
                    Some(matches) if matches.child_count() == 0 => {}
                    _ => continue 'scan,
                }
            }
            return Some(CloseLevel {
                depth: i,
                fit,
                move_to: if drop_inner {
                    Some(to.after(i + 1))
                } else {
                    None
                },
            });
        }
        None
    }

    fn close(&mut self, to: &ResolvedPos<'a>) -> Option<ResolvedPos<'a>> {
        let schema = self.schema;
        let close = self.find_close_level(to)?;
        while self.depth() > close.depth {
            self.close_frontier_node();
        }
        if close.fit.child_count() > 0 {
            self.placed = add_to_fragment(&self.placed, close.depth, &close.fit);
        }
        let to = match close.move_to {
            Some(pos) => to.doc().resolve(pos),
            None => to.clone(),
        };
        for d in close.depth + 1..=to.depth() {
            let node = to.node(d);
            let add = Match {
                dfa: &schema.nodes[node.type_id].dfa,
                state: 0,
            }
            .fill_before(&node.content, true, to.index(d), schema)
            .expect("the content after fits");
            self.open_frontier_node(node.type_id, Some(node), Some(add));
        }
        Some(to)
    }

    fn open_frontier_node(&mut self, ty: TypeId, like: Option<&Node>, content: Option<Fragment>) {
        let schema = self.schema;
        let depth = self.depth();
        let (_, m) = self.frontier[depth];
        self.frontier[depth].1 = m.match_type(ty).expect("the frontier accepts the node");
        let node = Node::new(
            schema,
            ty,
            like.map(|node| &node.attrs),
            content.unwrap_or_default(),
            Vec::new(),
        );
        self.placed = add_to_fragment(&self.placed, depth, &Fragment::from(vec![node]));
        self.frontier.push((
            ty,
            Match {
                dfa: &schema.nodes[ty].dfa,
                state: 0,
            },
        ));
    }

    fn close_frontier_node(&mut self) {
        let (_, m) = self.frontier.pop().expect("an open frontier node");
        let add = m
            .fill_before(&Fragment::empty(), true, 0, self.schema)
            .expect("an open node can be closed");
        if add.child_count() > 0 {
            self.placed = add_to_fragment(&self.placed, self.frontier.len(), &add);
        }
    }
}

struct CloseLevel {
    depth: usize,
    fit: Fragment,
    move_to: Option<usize>,
}

impl super::schema::NodeType {
    /// `allowedMarks`
    fn allowed_marks_of(&self, marks: &[Mark]) -> Vec<Mark> {
        marks
            .iter()
            .filter(|mark| self.allows_mark_type(mark.type_id))
            .cloned()
            .collect()
    }
}

fn drop_from_fragment(fragment: &Fragment, depth: usize, count: usize) -> Fragment {
    if depth == 0 {
        return fragment.cut_by_index(count, fragment.child_count());
    }
    let first = fragment.first_child().unwrap();
    fragment.replace_child(
        0,
        first.copy(drop_from_fragment(&first.content, depth - 1, count)),
    )
}

fn add_to_fragment(fragment: &Fragment, depth: usize, content: &Fragment) -> Fragment {
    if depth == 0 {
        return fragment.append(content);
    }
    let last = fragment.last_child().unwrap();
    fragment.replace_child(
        fragment.child_count() - 1,
        last.copy(add_to_fragment(&last.content, depth - 1, content)),
    )
}

fn content_at(fragment: &Fragment, depth: usize) -> &Fragment {
    let mut fragment = fragment;
    for _ in 0..depth {
        fragment = &fragment.first_child().unwrap().content;
    }
    fragment
}

fn close_node_start(
    schema: &'static Schema,
    node: &Node,
    open_start: usize,
    open_end: isize,
) -> Node {
    if open_start == 0 {
        return node.clone();
    }
    let mut frag = node.content.clone();
    if open_start > 1 {
        let first = frag.first_child().unwrap();
        frag = frag.replace_child(
            0,
            close_node_start(
                schema,
                first,
                open_start - 1,
                if frag.child_count() == 1 {
                    open_end - 1
                } else {
                    0
                },
            ),
        );
    }
    let start = Match {
        dfa: &schema.nodes[node.type_id].dfa,
        state: 0,
    };
    frag = start
        .fill_before(&frag, false, 0, schema)
        .expect("the start can be closed")
        .append(&frag);
    if open_end <= 0 {
        let after = start
            .match_fragment(&frag, 0, frag.child_count())
            .expect("the closed content matches")
            .fill_before(&Fragment::empty(), true, 0, schema)
            .expect("the end can be closed");
        frag = frag.append(&after);
    }
    node.copy(frag)
}

fn content_after_fits(
    schema: &'static Schema,
    to: &ResolvedPos,
    depth: usize,
    ty: TypeId,
    m: Match,
    open: bool,
) -> Option<Fragment> {
    let node = to.node(depth);
    let index = if open {
        to.index_after(depth)
    } else {
        to.index(depth)
    };
    if index == node.child_count() && !schema.compatible_content(ty, node.type_id) {
        return None;
    }
    let fit = m.fill_before(&node.content, true, index, schema)?;
    if invalid_marks(schema, ty, &node.content, index) {
        return None;
    }
    Some(fit)
}

fn invalid_marks(schema: &Schema, ty: TypeId, fragment: &Fragment, start: usize) -> bool {
    fragment.children[start..].iter().any(|child| {
        !child
            .marks
            .iter()
            .all(|mark| schema.nodes[ty].allows_mark_type(mark.type_id))
    })
}

fn defines_content(schema: &Schema, ty: TypeId) -> bool {
    schema.nodes[ty].defining
}

/// `replaceRange`: `None` when nothing changes.
pub fn replace_range(
    schema: &'static Schema,
    doc: &Node,
    from: usize,
    to: usize,
    slice: &Slice,
) -> Option<Applied> {
    if slice.size() == 0 {
        return delete_range(schema, doc, from, to);
    }
    let from_pos = doc.resolve(from);
    let to_pos = doc.resolve(to);
    if fits_trivially(schema, &from_pos, &to_pos, slice) {
        return Step::Replace {
            from,
            to,
            slice: slice.clone(),
        }
        .apply(schema, doc);
    }
    let mut target_depths: Vec<isize> = covered_depths(schema, &from_pos, &to_pos)
        .into_iter()
        .map(|d| d as isize)
        .collect();
    if target_depths.last() == Some(&0) {
        target_depths.pop();
    }
    let mut preferred_target = -(from_pos.depth() as isize + 1);
    target_depths.insert(0, preferred_target);
    {
        let mut pos = from_pos.pos as isize - 1;
        for d in (1..=from_pos.depth()).rev() {
            let spec = &schema.nodes[from_pos.node(d).type_id];
            if spec.defining || spec.isolating {
                break;
            }
            if target_depths.contains(&(d as isize)) {
                preferred_target = d as isize;
            } else if from_pos.before(d) as isize == pos {
                target_depths.insert(1, -(d as isize));
            }
            pos -= 1;
        }
    }
    let preferred_target_index = target_depths
        .iter()
        .position(|d| *d == preferred_target)
        .unwrap();
    // The open chain's first nodes; the innermost may be missing when the
    // first open node is empty.
    let mut left_nodes: Vec<Option<&Node>> = Vec::new();
    let mut preferred_depth = slice.open_start;
    {
        let mut content = &slice.content;
        let mut i = 0;
        loop {
            let node = content.first_child();
            left_nodes.push(node);
            if i == slice.open_start {
                break;
            }
            content = &node.expect("an open node has a first child").content;
            i += 1;
        }
    }
    for d in (0..preferred_depth).rev() {
        let Some(left_node) = left_nodes[d] else {
            break;
        };
        let def = defines_content(schema, left_node.type_id);
        let against = from_pos.node(preferred_target.unsigned_abs() - 1);
        if def && !left_node.same_markup(against) {
            preferred_depth = d;
        } else if def || !schema.nodes[left_node.type_id].is_textblock(schema) {
            break;
        }
    }
    for j in (0..=slice.open_start).rev() {
        let open_depth = (j + preferred_depth + 1) % (slice.open_start + 1);
        let Some(insert) = left_nodes.get(open_depth).copied().flatten() else {
            continue;
        };
        for i in 0..target_depths.len() {
            let mut target_depth =
                target_depths[(i + preferred_target_index) % target_depths.len()];
            let mut expand = true;
            if target_depth < 0 {
                expand = false;
                target_depth = -target_depth;
            }
            let target_depth = target_depth as usize;
            let parent = from_pos.node(target_depth - 1);
            let index = from_pos.index(target_depth - 1);
            if parent.can_replace_with(schema, index, index, insert.type_id, &insert.marks) {
                let closed = close_fragment(
                    schema,
                    &slice.content,
                    0,
                    slice.open_start,
                    open_depth,
                    None,
                );
                return replace(
                    schema,
                    doc,
                    from_pos.before(target_depth),
                    if expand {
                        to_pos.after(target_depth)
                    } else {
                        to
                    },
                    &Slice::new(closed, open_depth, slice.open_end),
                );
            }
        }
    }
    let mut from = from;
    let mut to = to;
    for i in (0..target_depths.len()).rev() {
        if let Some(applied) = replace(schema, doc, from, to, slice) {
            return Some(applied);
        }
        let depth = target_depths[i];
        if depth < 0 {
            continue;
        }
        from = from_pos.before(depth as usize);
        to = to_pos.after(depth as usize);
    }
    None
}

fn close_fragment(
    schema: &'static Schema,
    fragment: &Fragment,
    depth: usize,
    old_open: usize,
    new_open: usize,
    parent: Option<&Node>,
) -> Fragment {
    let mut fragment = fragment.clone();
    if depth < old_open {
        let first = fragment.first_child().unwrap().clone();
        fragment = fragment.replace_child(
            0,
            first.copy(close_fragment(
                schema,
                &first.content,
                depth + 1,
                old_open,
                new_open,
                Some(&first),
            )),
        );
    }
    if depth > new_open {
        let m = parent.unwrap().content_match_at(schema, 0);
        let start = m
            .fill_before(&fragment, false, 0, schema)
            .expect("the fragment can be closed")
            .append(&fragment);
        let end = m
            .match_fragment(&start, 0, start.child_count())
            .expect("the closed start matches")
            .fill_before(&Fragment::empty(), true, 0, schema)
            .expect("the end can be closed");
        fragment = start.append(&end);
    }
    fragment
}

/// `replaceRangeWith`: a block node at an empty selection moves to the
/// nearest `insertPoint`.
pub fn replace_range_with(
    schema: &'static Schema,
    doc: &Node,
    from: usize,
    to: usize,
    node: Node,
) -> Option<Applied> {
    let mut from = from;
    let mut to = to;
    if !node.is_inline(schema)
        && from == to
        && doc.resolve(from).parent().content.size > 0
        && let Some(point) = insert_point(schema, doc, from, node.type_id)
    {
        from = point;
        to = point;
    }
    replace_range(
        schema,
        doc,
        from,
        to,
        &Slice::new(Fragment::from(vec![node]), 0, 0),
    )
}

/// `insertPoint`: where a node of `ty` can go at or around `pos`.
pub fn insert_point(schema: &'static Schema, doc: &Node, pos: usize, ty: TypeId) -> Option<usize> {
    let p = doc.resolve(pos);
    let depth = p.depth();
    if p.parent()
        .can_replace_with(schema, p.index(depth), p.index(depth), ty, &[])
    {
        return Some(pos);
    }
    if p.parent_offset == 0 {
        for d in (0..depth).rev() {
            let index = p.index(d);
            if p.node(d).can_replace_with(schema, index, index, ty, &[]) {
                return Some(p.before(d + 1));
            }
            if index > 0 {
                return None;
            }
        }
    }
    if p.parent_offset == p.parent().content.size {
        for d in (0..depth).rev() {
            let index = p.index_after(d);
            if p.node(d).can_replace_with(schema, index, index, ty, &[]) {
                return Some(p.after(d + 1));
            }
            if index < p.node(d).child_count() {
                return None;
            }
        }
    }
    None
}

/// `deleteRange`, for a paste of nothing over a selection.
fn delete_range(schema: &'static Schema, doc: &Node, from: usize, to: usize) -> Option<Applied> {
    let mut from = from;
    let mut to = to;
    let mut from_pos = doc.resolve(from);
    let mut to_pos = doc.resolve(to);
    if from_pos.parent().is_textblock(schema)
        && to_pos.parent().is_textblock(schema)
        && from_pos.start(from_pos.depth()) != to_pos.start(to_pos.depth())
        && from_pos.parent_offset == 0
        && to_pos.parent_offset == 0
    {
        let shared = from_pos.shared_depth(to);
        let isolated = (shared + 1..=from_pos.depth())
            .any(|d| schema.nodes[from_pos.node(d).type_id].isolating)
            || (shared + 1..=to_pos.depth())
                .any(|d| schema.nodes[to_pos.node(d).type_id].isolating);
        if !isolated {
            let mut d = from_pos.depth();
            while d > 0 && from == from_pos.start(d) {
                from = from_pos.before(d);
                d -= 1;
            }
            let mut d = to_pos.depth();
            while d > 0 && to == to_pos.start(d) {
                to = to_pos.before(d);
                d -= 1;
            }
            from_pos = doc.resolve(from);
            to_pos = doc.resolve(to);
        }
    }
    let covered = covered_depths(schema, &from_pos, &to_pos);
    for (i, depth) in covered.iter().enumerate() {
        let depth = *depth;
        let last = i == covered.len() - 1;
        let content_valid_end = schema.nodes[from_pos.node(depth).type_id].dfa.valid_end(0);
        if (last && depth == 0) || content_valid_end {
            return replace(
                schema,
                doc,
                from_pos.start(depth),
                to_pos.end(depth),
                &Slice::empty(),
            );
        }
        if depth > 0
            && (last
                || from_pos.node(depth - 1).can_replace(
                    schema,
                    from_pos.index(depth - 1),
                    to_pos.index_after(depth - 1),
                    &Fragment::empty(),
                    0,
                    0,
                ))
        {
            return replace(
                schema,
                doc,
                from_pos.before(depth),
                to_pos.after(depth),
                &Slice::empty(),
            );
        }
    }
    for d in 1..=from_pos.depth().min(to_pos.depth()) {
        if from - from_pos.start(d) == from_pos.depth() - d
            && to > from_pos.end(d)
            && to_pos.end(d) - to != to_pos.depth() - d
            && from_pos.start(d - 1) == to_pos.start(d - 1)
            && from_pos.node(d - 1).can_replace(
                schema,
                from_pos.index(d - 1),
                to_pos.index(d - 1),
                &Fragment::empty(),
                0,
                0,
            )
        {
            return replace(schema, doc, from_pos.before(d), to, &Slice::empty());
        }
    }
    replace(schema, doc, from, to, &Slice::empty())
}

fn covered_depths(schema: &Schema, from: &ResolvedPos, to: &ResolvedPos) -> Vec<usize> {
    let mut result = Vec::new();
    let min_depth = from.depth().min(to.depth());
    for d in (0..=min_depth).rev() {
        let start = from.start(d);
        if start < from.pos - (from.depth() - d)
            || to.end(d) > to.pos + (to.depth() - d)
            || schema.nodes[from.node(d).type_id].isolating
            || schema.nodes[to.node(d).type_id].isolating
        {
            break;
        }
        if start == to.start(d)
            || (d == from.depth()
                && d == to.depth()
                && from.parent().inline_content(schema)
                && to.parent().inline_content(schema)
                && d > 0
                && to.start(d - 1) == start - 1)
        {
            result.push(d);
        }
    }
    result
}
