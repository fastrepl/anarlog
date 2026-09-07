//! The memo editor: caret editing of the note body on top of the TipTap JSON
//! model, with the same persistence cadence as `packages/editor` (changes
//! flushed 500ms after the last keystroke, at most 10s apart).

pub mod links;
pub mod mention_picker;
pub mod model;
pub mod rules;
pub mod tasks;

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    KeyBinding, Pixels, Point, UTF16Selection, Window, actions,
};

use model::{Caret, Doc};

use crate::prose_text::ProseLayout;

actions!(
    body_editor,
    [
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Backspace,
        Delete,
        Enter,
        MentionEscape,
        Copy,
        Cut,
        Paste,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleCode,
        Tab,
        ShiftTab,
        Undo,
        Redo,
    ]
);

pub const KEY_CONTEXT: &str = "BodyEditor";
const FLUSH_DEBOUNCE: Duration = Duration::from_millis(500);
const FLUSH_MAX_WAIT: Duration = Duration::from_secs(10);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    let m = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new(&format!("{m}-a"), SelectAll, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("enter", Enter, ctx),
        KeyBinding::new("escape", MentionEscape, ctx),
        KeyBinding::new(&format!("{m}-c"), Copy, ctx),
        KeyBinding::new(&format!("{m}-x"), Cut, ctx),
        KeyBinding::new(&format!("{m}-v"), Paste, ctx),
        // `packages/editor/src/note/keymap.ts`
        KeyBinding::new(&format!("{m}-b"), ToggleBold, ctx),
        KeyBinding::new(&format!("{m}-i"), ToggleItalic, ctx),
        KeyBinding::new(&format!("{m}-u"), ToggleUnderline, ctx),
        KeyBinding::new(&format!("{m}-`"), ToggleCode, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("shift-tab", ShiftTab, ctx),
        KeyBinding::new(&format!("{m}-z"), Undo, ctx),
        KeyBinding::new(&format!("{m}-shift-z"), Redo, ctx),
        KeyBinding::new(&format!("{m}-y"), Redo, ctx),
    ]);
}

/// prosemirror-history's `newGroupDelay`.
const HISTORY_GROUP_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone)]
struct Snapshot {
    json: String,
    caret: Option<Caret>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Structural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    /// The document changed; the payload is `doc.toJSON()`.
    Flush(String),
    /// A mention chip was clicked: navigate to `/app/<kind>/<id>`.
    OpenMention { kind: String, id: String },
}

pub struct BodyEditor {
    focus_handle: FocusHandle,
    pub session_id: String,
    doc: Doc,
    caret: Option<Caret>,
    /// The other end of the selection while one exists (`caret` is the head).
    anchor: Option<Caret>,
    /// ProseMirror `storedMarks`: the mark set for the next typed text after
    /// toggling a mark with an empty selection.
    stored_marks: Option<Vec<&'static str>>,
    is_selecting: bool,
    pasting: bool,
    marked_range: Option<Range<usize>>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<(Instant, EditKind)>,
    /// Text layouts captured while painting, one per textblock.
    layouts: Vec<Option<(ProseLayout, Bounds<Pixels>)>>,
    dirty_since: Option<Instant>,
    last_input: Option<Instant>,
    flush_scheduled: bool,
    /// `MentionSuggestion`: derived from the caret after every change.
    mention: Option<mention_picker::MentionState>,
    /// `dismissedFrom`: Escape (or an insertion) hides the popup for this
    /// trigger position until the caret leaves it.
    mention_dismissed: Option<(usize, usize)>,
    mention_search: Option<mention_picker::Search>,
    /// `prosemirror-search`'s `SearchQuery` set by the find bar: matches are
    /// decorated in every textblock.
    search: Option<SearchSpec>,
}

/// The find bar's query as `setSearch` / `replace` hand it to the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSpec {
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

impl EventEmitter<EditorEvent> for BodyEditor {}

impl BodyEditor {
    pub fn new(session_id: String, body: &str, cx: &mut Context<Self>) -> Self {
        let doc = Doc::parse(body);
        Self {
            focus_handle: cx.focus_handle(),
            session_id,
            layouts: vec![None; doc.textblock_count()],
            doc,
            caret: None,
            anchor: None,
            stored_marks: None,
            is_selecting: false,
            pasting: false,
            marked_range: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
            dirty_since: None,
            last_input: None,
            flush_scheduled: false,
            mention: None,
            mention_dismissed: None,
            mention_search: None,
            search: None,
        }
    }

    /// `setSearch(query, caseSensitive)`: decorate the matches of `spec`
    /// (`None` clears them).
    pub fn set_search(&mut self, spec: Option<SearchSpec>, cx: &mut Context<Self>) {
        let spec = spec.filter(|spec| !spec.query.trim().is_empty());
        if self.search != spec {
            self.search = spec;
            cx.notify();
        }
    }

    /// Every match of the current search as `(textblock, byte range)` in
    /// document order.
    pub fn search_matches(&self) -> Vec<(usize, Range<usize>)> {
        let Some(spec) = &self.search else {
            return Vec::new();
        };
        let query = crate::note_search::prepare_query(&spec.query, spec.case_sensitive);
        if query.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        for block in 0..self.doc.textblock_count() {
            let raw = self.doc.text(block);
            let text = crate::note_search::prepare_text(&raw, spec.case_sensitive);
            if text.len() != raw.len() {
                // Case folding changed byte lengths: match on the folded text
                // and map back through character counts.
                let starts = crate::note_search::find_occurrences(&text, &query, spec.whole_word);
                let boundaries: Vec<usize> = raw
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(raw.len()))
                    .collect();
                for start in starts {
                    let from = text[..start].chars().count();
                    let to = text[..start + query.len()].chars().count();
                    if let (Some(&from), Some(&to)) = (boundaries.get(from), boundaries.get(to)) {
                        matches.push((block, from..to));
                    }
                }
                continue;
            }
            for start in crate::note_search::find_occurrences(&text, &query, spec.whole_word) {
                matches.push((block, start..start + query.len()));
            }
        }
        matches
    }

    /// The `.ProseMirror-search-match` ranges of `block`, and whether each
    /// is the `.ProseMirror-active-search-match` (the selection covers it).
    pub fn search_ranges_in_block(&self, block: usize) -> Vec<(Range<usize>, bool)> {
        let selection = self.selection();
        self.search_matches()
            .into_iter()
            .filter(|(b, _)| *b == block)
            .map(|(_, range)| {
                let active = selection.is_some_and(|(from, to)| {
                    from.block == block
                        && to.block == block
                        && from.offset == range.start
                        && to.offset == range.end
                });
                (range, active)
            })
            .collect()
    }

    /// `commands.replace`: `replaceAll`, or `findNext` from the selection
    /// `match_index` times and `replaceCurrent` on that match.
    pub fn replace_search(
        &mut self,
        replacement: &str,
        all: bool,
        match_index: usize,
        cx: &mut Context<Self>,
    ) {
        let matches = self.search_matches();
        if matches.is_empty() {
            return;
        }
        self.record_edit(EditKind::Structural);
        if all {
            // Back to front so earlier offsets stay valid.
            for (block, range) in matches.iter().rev() {
                self.doc.delete_range(*block, range.clone());
                self.doc.insert_text(
                    Caret {
                        block: *block,
                        offset: range.start,
                    },
                    replacement,
                );
            }
            if let Some((block, range)) = matches.last() {
                self.caret = Some(Caret {
                    block: *block,
                    offset: range.start + replacement.len(),
                });
            }
        } else {
            // `findNext(state)` starts at the selection's end and wraps.
            let from = self
                .caret
                .map(|caret| (caret.block, caret.offset))
                .unwrap_or((0, 0));
            let first = matches
                .iter()
                .position(|(block, range)| (*block, range.start) >= from)
                .unwrap_or(0);
            let (block, range) = matches[(first + match_index) % matches.len()].clone();
            self.doc.delete_range(block, range.clone());
            let caret = self.doc.insert_text(
                Caret {
                    block,
                    offset: range.start,
                },
                replacement,
            );
            self.caret = Some(caret);
        }
        self.anchor = None;
        self.clamp_caret();
        self.changed(cx);
    }

    /// `mentionConfig`: how `@query` resolves to candidates.
    pub fn set_mention_search(&mut self, search: mention_picker::Search) {
        self.mention_search = Some(search);
    }

    pub fn mention(&self) -> Option<&mention_picker::MentionState> {
        self.mention
            .as_ref()
            .filter(|state| !state.items.is_empty())
    }

    /// Window position under the trigger character (`coordsAtPos(from)`),
    /// for the popup's `bottom-start` placement.
    pub fn mention_anchor(&self) -> Option<(Point<Pixels>, Pixels)> {
        let state = self.mention()?;
        let (layout, _) = self.layouts.get(state.block)?.as_ref()?;
        let position = layout.position_for_index(state.from)?;
        Some((position, layout.line_height()))
    }

    /// Re-derive the popup from the caret (`findMention` on the new state).
    fn refresh_mention(&mut self) {
        let Some(search) = self.mention_search.clone() else {
            self.mention = None;
            return;
        };
        let found = self
            .caret
            .filter(|_| self.selection().is_none())
            .and_then(|caret| {
                let text = self.doc.text(caret.block);
                let atoms = self.doc.atom_ranges(caret.block);
                mention_picker::find_mention(&text, &atoms, caret)
                    .map(|(from, to, query)| (caret.block, from, to, query))
            });
        let Some((block, from, to, query)) = found else {
            self.mention = None;
            self.mention_dismissed = None;
            return;
        };
        if self.mention_dismissed == Some((block, from)) {
            self.mention = None;
            return;
        }
        let unchanged = self.mention.as_ref().is_some_and(|state| {
            state.block == block && state.from == from && state.query == query
        });
        if unchanged {
            if let Some(state) = self.mention.as_mut() {
                state.to = to;
            }
            return;
        }
        let items = search(&query);
        self.mention = Some(mention_picker::MentionState {
            block,
            from,
            to,
            query,
            items,
            selected: 0,
        });
    }

    /// The candidate rows changed (`handleSearch` identity in the web app):
    /// re-run the open popup's query.
    pub fn rerun_mention_search(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.take() {
            let _ = state;
            self.refresh_mention();
            cx.notify();
        }
    }

    pub fn select_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut()
            && index < state.items.len()
        {
            state.selected = index;
            cx.notify();
        }
    }

    /// `insertMention`: the node plus a space replace `@query`; the popup
    /// stays dismissed for that trigger.
    pub fn insert_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(state) = self.mention.clone() else {
            return;
        };
        let Some(item) = state.items.get(index) else {
            return;
        };
        self.record_edit(EditKind::Structural);
        let caret = self
            .doc
            .insert_mention(state.block, state.from..state.to, item);
        self.caret = Some(caret);
        self.anchor = None;
        self.mention_dismissed = Some((state.block, state.from));
        self.changed(cx);
    }

    fn dismiss_mention(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.take() {
            self.mention_dismissed = Some((state.block, state.from));
            cx.notify();
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn caret(&self) -> Option<Caret> {
        self.caret
    }

    /// The selected byte range within `block`, if the selection covers it.
    pub fn selection_in_block(&self, block: usize) -> Option<Range<usize>> {
        let (from, to) = model::order(self.anchor?, self.caret?);
        if from == to || block < from.block || block > to.block {
            return None;
        }
        let start = if block == from.block { from.offset } else { 0 };
        let end = if block == to.block {
            to.offset
        } else {
            self.doc.text(block).len()
        };
        (start < end).then_some(start..end)
    }

    fn selection(&self) -> Option<(Caret, Caret)> {
        let (anchor, caret) = (self.anchor?, self.caret?);
        (anchor != caret).then(|| model::order(anchor, caret))
    }

    /// Collapses the selection into the caret, deleting its content.
    fn delete_selection(&mut self) -> bool {
        let Some((from, to)) = self.selection() else {
            return false;
        };
        self.caret = Some(self.doc.delete_between(from, to));
        self.anchor = None;
        true
    }

    fn set_head(&mut self, head: Caret, extend: bool, cx: &mut Context<Self>) {
        if extend {
            self.anchor.get_or_insert(self.caret.unwrap_or(head));
        } else {
            self.anchor = None;
        }
        self.caret = Some(head);
        self.stored_marks = None;
        self.refresh_mention();
        cx.notify();
    }

    pub fn end_mouse_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Mouse up in a textblock. A press that did not drag is a click, and
    /// `linkOpenPlugin` opens the http(s) link under the pointer.
    pub fn end_mouse_click(
        &mut self,
        block: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let was_selecting = std::mem::replace(&mut self.is_selecting, false);
        if !was_selecting || self.selection().is_some() {
            return;
        }
        let Some(Ok(index)) = self
            .layouts
            .get(block)
            .and_then(Option::as_ref)
            .map(|(layout, _)| layout.index_for_position(position))
        else {
            return;
        };
        if let Some(href) = self
            .doc
            .link_href_at(block, index)
            .and_then(|href| links::openable_href(&href))
        {
            tracing::info!(%href, "opening link from the memo");
            cx.open_url(&href);
        } else if let Some((kind, id)) = self.doc.mention_at(block, index) {
            // `MentionNodeView`'s click navigates to `/app/<type>/<id>`.
            cx.emit(EditorEvent::OpenMention { kind, id });
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    /// Replace the document from the store unless local edits are pending.
    pub fn replace_body(&mut self, body: &str, cx: &mut Context<Self>) {
        if self.is_dirty() {
            return;
        }
        let doc = Doc::parse(body);
        if doc.to_json() == self.doc.to_json() {
            return;
        }
        self.doc = doc;
        self.layouts = vec![None; self.doc.textblock_count()];
        self.clamp_caret();
        cx.notify();
    }

    /// The window bounds a textblock was last painted at.
    pub fn block_bounds(&self, block: usize) -> Option<Bounds<Pixels>> {
        self.layouts
            .get(block)
            .and_then(|slot| slot.as_ref())
            .map(|(_, bounds)| *bounds)
    }

    pub fn record_layout(&mut self, block: usize, layout: ProseLayout, bounds: Bounds<Pixels>) {
        if self.layouts.len() != self.doc.textblock_count() {
            self.layouts = vec![None; self.doc.textblock_count()];
        }
        if let Some(slot) = self.layouts.get_mut(block) {
            *slot = Some((layout, bounds));
        }
    }

    /// Caret position in window coordinates plus the line height, when the
    /// block has been laid out.
    pub fn caret_position(&self) -> Option<(Point<Pixels>, Pixels)> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let position = layout.position_for_index(caret.offset)?;
        Some((position, layout.line_height()))
    }

    fn caret_for_position(&self, block: usize, position: Point<Pixels>) -> Caret {
        let offset = self
            .layouts
            .get(block)
            .and_then(Option::as_ref)
            .map(|(layout, _)| match layout.index_for_position(position) {
                Ok(index) | Err(index) => index,
            })
            .unwrap_or(0);
        let block = block.min(self.doc.textblock_count().saturating_sub(1));
        let text = self.doc.text(block);
        let offset = snap(&text, offset.min(text.len()));
        Caret {
            block,
            offset: self.doc.snap_out_of_atoms(block, offset, 0),
        }
    }

    /// Mouse down in a textblock: place the caret (shift extends) and start a
    /// drag selection.
    pub fn place_caret_at(
        &mut self,
        block: usize,
        position: Point<Pixels>,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        let head = self.caret_for_position(block, position);
        self.set_head(head, extend, cx);
        self.is_selecting = true;
        self.focus_handle.focus(window);
    }

    /// Mouse moved over a textblock while dragging: extend to that point.
    pub fn drag_to(&mut self, block: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let head = self.caret_for_position(block, position);
        if self.caret != Some(head) {
            self.set_head(head, true, cx);
        }
    }

    /// `editor.commands.focus()` on a freshly opened note: the caret sits at
    /// the document start unless the editor already has one.
    pub fn focus_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        if self.caret.is_none() {
            self.set_head(
                Caret {
                    block: 0,
                    offset: 0,
                },
                false,
                cx,
            );
        }
        self.focus_handle.focus(window);
    }

    /// Clicking below the last block puts the caret at the end, like
    /// `trailing-empty-line-click.ts`.
    pub fn place_caret_at_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        let block = self.doc.textblock_count() - 1;
        let head = Caret {
            block,
            offset: self.doc.text(block).len(),
        };
        self.set_head(head, false, cx);
        self.focus_handle.focus(window);
    }

    fn clamp_caret(&mut self) {
        self.anchor = None;
        if let Some(caret) = &mut self.caret {
            if self.doc.textblock_count() == 0 {
                self.caret = None;
                return;
            }
            caret.block = caret.block.min(self.doc.textblock_count() - 1);
            let text = self.doc.text(caret.block);
            caret.offset = snap(&text, caret.offset.min(text.len()));
        }
    }

    /// Records the pre-edit state for undo; adjacent edits of the same kind
    /// within `newGroupDelay` share one history entry.
    fn record_edit(&mut self, kind: EditKind) {
        let now = Instant::now();
        let grouped = matches!(
            self.last_edit,
            Some((at, last_kind)) if last_kind == kind && kind != EditKind::Structural && now.duration_since(at) < HISTORY_GROUP_DELAY
        );
        if !grouped {
            self.undo_stack.push(Snapshot {
                json: self.doc.to_json(),
                caret: self.caret,
            });
            if self.undo_stack.len() > 200 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
        self.last_edit = Some((now, kind));
    }

    fn restore(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        self.doc = Doc::parse(&snapshot.json);
        self.layouts = vec![None; self.doc.textblock_count()];
        self.caret = snapshot.caret;
        self.clamp_caret();
        self.anchor = None;
        self.stored_marks = None;
        self.last_edit = None;
        self.changed(cx);
    }

    fn on_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(Snapshot {
            json: self.doc.to_json(),
            caret: self.caret,
        });
        self.restore(snapshot, cx);
    }

    fn on_redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(Snapshot {
            json: self.doc.to_json(),
            caret: self.caret,
        });
        self.restore(snapshot, cx);
    }

    /// `TaskItemView`'s checkbox: flip the item holding `block` between
    /// `done` and `todo`.
    pub fn toggle_task(&mut self, block: usize, cx: &mut Context<Self>) {
        self.record_edit(EditKind::Structural);
        if self.doc.toggle_task(block) {
            self.changed(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        // `taskIdentityPlugin`: ids stay unique after splits and pastes.
        self.doc.ensure_task_identity();
        // `appendTransaction` of the autolink and link-boundary-guard plugins:
        // the caret's block is the changed textblock; a structural edit (split,
        // join, lift) may also have reshaped the block before it.
        if let Some(caret) = self.caret {
            self.doc
                .maintain_links(caret.block, caret.offset..caret.offset);
            if matches!(self.last_edit, Some((_, EditKind::Structural))) && caret.block > 0 {
                let previous = caret.block - 1;
                let end = self.doc.text(previous).len();
                self.doc.maintain_links(previous, end..end);
            }
        }
        self.refresh_mention();
        let now = Instant::now();
        self.dirty_since.get_or_insert(now);
        self.last_input = Some(now);
        self.layouts.resize(self.doc.textblock_count(), None);
        self.schedule_flush(cx);
        cx.notify();
    }

    /// `useDebounceCallback(flush, 500, { maxWait: 10_000, trailing: true })`.
    fn schedule_flush(&mut self, cx: &mut Context<Self>) {
        if self.flush_scheduled {
            return;
        }
        self.flush_scheduled = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FLUSH_DEBOUNCE).await;
                let done = this
                    .update(cx, |this, cx| {
                        let now = Instant::now();
                        let quiet = this
                            .last_input
                            .is_none_or(|t| now.duration_since(t) >= FLUSH_DEBOUNCE);
                        let overdue = this
                            .dirty_since
                            .is_some_and(|t| now.duration_since(t) >= FLUSH_MAX_WAIT);
                        if quiet || overdue {
                            this.flush_scheduled = false;
                            this.flush(cx);
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    /// `flushPendingChanges`: emit the current JSON if anything changed.
    pub fn flush(&mut self, cx: &mut Context<Self>) {
        if let Some(json) = self.take_pending() {
            cx.emit(EditorEvent::Flush(json));
        }
    }

    /// The unsaved JSON, clearing the dirty state, for callers that write it
    /// themselves.
    pub fn take_pending(&mut self) -> Option<String> {
        self.dirty_since.take().map(|_| self.doc.to_json())
    }

    fn move_horizontally(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        // Left/right with a selection collapse it to the corresponding end.
        if !extend && let Some((from, to)) = self.selection() {
            self.set_head(if delta < 0 { from } else { to }, false, cx);
            return;
        }
        let text = self.doc.text(caret.block);
        let next = if delta < 0 {
            if caret.offset == 0 {
                if caret.block == 0 {
                    return;
                }
                let block = caret.block - 1;
                Caret {
                    block,
                    offset: self.doc.text(block).len(),
                }
            } else {
                Caret {
                    block: caret.block,
                    offset: self.doc.snap_out_of_atoms(
                        caret.block,
                        previous_boundary(&text, caret.offset),
                        -1,
                    ),
                }
            }
        } else if caret.offset >= text.len() {
            if caret.block + 1 >= self.doc.textblock_count() {
                return;
            }
            Caret {
                block: caret.block + 1,
                offset: 0,
            }
        } else {
            Caret {
                block: caret.block,
                offset: self.doc.snap_out_of_atoms(
                    caret.block,
                    next_boundary(&text, caret.offset),
                    1,
                ),
            }
        };
        self.set_head(next, extend, cx);
    }

    /// Up/down keep the x position, moving a line within the block when the
    /// layout wrapped, otherwise into the neighbouring block.
    fn move_vertically(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        let Some((position, line_height)) = self.caret_position() else {
            return;
        };
        let target_y = position.y + line_height * delta as f32 + line_height / 2.0;
        let within = self
            .layouts
            .get(caret.block)
            .and_then(Option::as_ref)
            .filter(|(_, bounds)| target_y >= bounds.top() && target_y < bounds.bottom())
            .map(|(layout, _)| layout.index_for_position(Point::new(position.x, target_y)));
        let next = match within {
            Some(Ok(index) | Err(index)) => Caret {
                block: caret.block,
                offset: index,
            },
            None => {
                let block = if delta < 0 {
                    if caret.block == 0 {
                        return;
                    }
                    caret.block - 1
                } else {
                    if caret.block + 1 >= self.doc.textblock_count() {
                        return;
                    }
                    caret.block + 1
                };
                let Some((layout, bounds)) = self.layouts.get(block).and_then(Option::as_ref)
                else {
                    return;
                };
                let y = if delta < 0 {
                    bounds.bottom() - line_height / 2.0
                } else {
                    bounds.top() + line_height / 2.0
                };
                let index = match layout.index_for_position(Point::new(position.x, y)) {
                    Ok(index) | Err(index) => index,
                };
                Caret {
                    block,
                    offset: index,
                }
            }
        };
        let text = self.doc.text(next.block);
        let next = Caret {
            block: next.block,
            offset: snap(&text, next.offset.min(text.len())),
        };
        self.set_head(next, extend, cx);
    }

    fn on_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(-1, false, cx);
    }

    fn on_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(1, false, cx);
    }

    fn on_up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut().filter(|s| !s.items.is_empty()) {
            state.selected = (state.selected + state.items.len() - 1) % state.items.len();
            cx.notify();
            return;
        }
        self.move_vertically(-1, false, cx);
    }

    fn on_down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention.as_mut().filter(|s| !s.items.is_empty()) {
            state.selected = (state.selected + 1) % state.items.len();
            cx.notify();
            return;
        }
        self.move_vertically(1, false, cx);
    }

    /// Escape only means something to the popup; otherwise the workspace's
    /// own Escape handling runs.
    fn on_escape(&mut self, _: &MentionEscape, _: &mut Window, cx: &mut Context<Self>) {
        if self.mention().is_some() {
            self.dismiss_mention(cx);
        } else {
            cx.propagate();
        }
    }

    fn on_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(-1, true, cx);
    }

    fn on_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontally(1, true, cx);
    }

    fn on_select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, cx);
    }

    fn on_select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, cx);
    }

    fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.textblock_count() == 0 {
            return;
        }
        let last = self.doc.textblock_count() - 1;
        self.anchor = Some(Caret {
            block: 0,
            offset: 0,
        });
        self.caret = Some(Caret {
            block: last,
            offset: self.doc.text(last).len(),
        });
        self.stored_marks = None;
        cx.notify();
    }

    fn on_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(caret) = self.caret {
            self.set_head(
                Caret {
                    block: caret.block,
                    offset: 0,
                },
                false,
                cx,
            );
        }
    }

    fn on_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(caret) = self.caret {
            let offset = self.doc.text(caret.block).len();
            self.set_head(
                Caret {
                    block: caret.block,
                    offset,
                },
                false,
                cx,
            );
        }
    }

    fn on_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.doc.clipboard_text_between(from, to),
            ));
        }
    }

    fn on_cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.doc.clipboard_text_between(from, to),
            ));
            self.record_edit(EditKind::Structural);
            self.delete_selection();
            self.changed(cx);
        }
    }

    fn on_paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.record_edit(EditKind::Structural);
            self.pasting = true;
            self.replace_text_in_range(None, &text, window, cx);
            self.pasting = false;
        }
    }

    /// `toggleMark`: with a selection, adds or removes the mark across it;
    /// with a caret only, toggles it in the stored marks for the next input.
    fn toggle_mark(&mut self, mark: &'static str, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.selection() {
            self.record_edit(EditKind::Structural);
            self.doc.toggle_mark(from, to, mark);
            self.changed(cx);
            return;
        }
        let Some(caret) = self.caret else {
            return;
        };
        let mut marks = self
            .stored_marks
            .clone()
            .unwrap_or_else(|| self.doc.marks_at(caret));
        if let Some(index) = marks.iter().position(|m| *m == mark) {
            marks.remove(index);
        } else {
            marks.push(mark);
        }
        self.stored_marks = Some(marks);
        cx.notify();
    }

    fn on_toggle_bold(&mut self, _: &ToggleBold, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mark("bold", cx);
    }

    fn on_toggle_italic(&mut self, _: &ToggleItalic, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mark("italic", cx);
    }

    fn on_toggle_underline(&mut self, _: &ToggleUnderline, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mark("underline", cx);
    }

    fn on_toggle_code(&mut self, _: &ToggleCode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mark("code", cx);
    }

    fn on_backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection().is_some() {
            self.record_edit(EditKind::Structural);
            self.delete_selection();
            self.changed(cx);
            return;
        }
        let Some(caret) = self.caret else {
            return;
        };
        if caret.offset == 0 {
            // `revertBlockToParagraph`, then `joinBackward` (which lifts the
            // first item out of a list instead of joining across it).
            if matches!(
                self.doc.block_type(caret.block).as_deref(),
                Some("heading" | "codeBlock")
            ) {
                self.record_edit(EditKind::Structural);
                self.doc.set_block_type(caret.block, "paragraph", None);
                self.changed(cx);
                return;
            }
            if self.doc.is_first_list_item(caret.block) {
                self.record_edit(EditKind::Structural);
                if let Some(next) = self.doc.lift_list_item(caret.block) {
                    self.caret = Some(next);
                }
                self.changed(cx);
                return;
            }
            self.record_edit(EditKind::Structural);
            if let Some(next) = self.doc.join_backward(caret.block) {
                self.caret = Some(next);
                self.changed(cx);
            }
            return;
        }
        self.record_edit(EditKind::Deleting);
        let text = self.doc.text(caret.block);
        let start = previous_boundary(&text, caret.offset);
        self.doc.delete_range(caret.block, start..caret.offset);
        self.caret = Some(Caret {
            block: caret.block,
            offset: start,
        });
        self.changed(cx);
    }

    /// Tab: `sinkListItem`.
    fn on_tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        if self.doc.in_list_item(caret.block) {
            self.record_edit(EditKind::Structural);
            if self.doc.sink_list_item(caret.block) {
                self.changed(cx);
            }
        }
    }

    /// Shift-Tab: `liftListItem`.
    fn on_shift_tab(&mut self, _: &ShiftTab, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        if self.doc.in_list_item(caret.block) {
            self.record_edit(EditKind::Structural);
            if let Some(next) = self.doc.lift_list_item(caret.block) {
                self.caret = Some(next);
                self.changed(cx);
            }
        }
    }

    fn on_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection().is_some() {
            self.record_edit(EditKind::Structural);
            self.delete_selection();
            self.changed(cx);
            return;
        }
        let Some(caret) = self.caret else {
            return;
        };
        let text = self.doc.text(caret.block);
        if caret.offset >= text.len() {
            if caret.block + 1 < self.doc.textblock_count() {
                self.record_edit(EditKind::Structural);
                self.doc.join_backward(caret.block + 1);
                self.changed(cx);
            }
            return;
        }
        self.record_edit(EditKind::Deleting);
        let end = next_boundary(&text, caret.offset);
        self.doc.delete_range(caret.block, caret.offset..end);
        self.changed(cx);
    }

    /// The `Enter` chain: exit a code block from an empty last line or insert
    /// a newline in it, lift an empty list item out of its list, otherwise
    /// `splitBlock` (which splits list items).
    fn on_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.mention() {
            let index = state.selected;
            self.insert_mention(index, cx);
            return;
        }
        self.record_edit(EditKind::Structural);
        self.delete_selection();
        let Some(caret) = self.caret else {
            return;
        };
        let text = self.doc.text(caret.block);
        if self.doc.block_type(caret.block).as_deref() == Some("codeBlock") {
            let at_end = caret.offset >= text.len();
            let empty_last_line = at_end && (text.is_empty() || text.ends_with('\n'));
            if empty_last_line {
                // `exitCodeBlockOnEmptyLine`: drop the trailing newline and
                // continue in a paragraph after the block.
                if !text.is_empty() {
                    self.doc
                        .delete_range(caret.block, text.len() - 1..text.len());
                }
                let end = Caret {
                    block: caret.block,
                    offset: self.doc.text(caret.block).len(),
                };
                self.caret = Some(self.doc.split_block(end));
            } else {
                self.caret = Some(self.doc.insert_text(caret, "\n"));
            }
            self.changed(cx);
            return;
        }
        if self.doc.in_list_item(caret.block) && text.is_empty() {
            if let Some(next) = self.doc.lift_list_item(caret.block) {
                self.caret = Some(next);
            }
            self.changed(cx);
            return;
        }
        self.caret = Some(self.doc.split_block(caret));
        self.changed(cx);
    }

    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        self.record_edit(EditKind::Typing);
        // Input rules see typed text only (`handleTextInput`), never pastes.
        if !self.pasting {
            let in_code = self.doc.block_type(caret.block).as_deref() == Some("codeBlock")
                || self
                    .stored_marks
                    .clone()
                    .unwrap_or_else(|| self.doc.marks_at(caret))
                    .contains(&"code");
            if let Some(outcome) = rules::apply(&mut self.doc, caret, text, in_code) {
                self.caret = Some(outcome.caret);
                if let Some(mark) = outcome.clear_stored_mark {
                    let mut marks = self
                        .stored_marks
                        .clone()
                        .unwrap_or_else(|| self.doc.marks_at(outcome.caret));
                    marks.retain(|m| *m != mark);
                    self.stored_marks = Some(marks);
                }
                self.layouts = vec![None; self.doc.textblock_count()];
                self.changed(cx);
                return;
            }
        }
        let end = self.doc.insert_text(caret, text);
        if let Some(marks) = self.stored_marks.take() {
            self.doc.set_marks(caret, end, &marks);
        }
        self.caret = Some(end);
        self.changed(cx);
    }

    fn utf16_to_offset(text: &str, utf16: usize) -> usize {
        let mut count = 0;
        for (index, ch) in text.char_indices() {
            if count >= utf16 {
                return index;
            }
            count += ch.len_utf16();
        }
        text.len()
    }

    fn offset_to_utf16(text: &str, offset: usize) -> usize {
        text[..offset.min(text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    pub fn render_root(&self, cx: &mut Context<Self>) -> gpui::Div {
        use gpui::prelude::*;
        gpui::div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_left))
            .on_action(cx.listener(Self::on_right))
            .on_action(cx.listener(Self::on_up))
            .on_action(cx.listener(Self::on_down))
            .on_action(cx.listener(Self::on_home))
            .on_action(cx.listener(Self::on_end))
            .on_action(cx.listener(Self::on_select_left))
            .on_action(cx.listener(Self::on_select_right))
            .on_action(cx.listener(Self::on_select_up))
            .on_action(cx.listener(Self::on_select_down))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_backspace))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_enter))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_toggle_bold))
            .on_action(cx.listener(Self::on_toggle_italic))
            .on_action(cx.listener(Self::on_toggle_underline))
            .on_action(cx.listener(Self::on_toggle_code))
            .on_action(cx.listener(Self::on_tab))
            .on_action(cx.listener(Self::on_shift_tab))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, _| this.end_mouse_selection()),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, _| this.end_mouse_selection()),
            )
    }
}

impl Focusable for BodyEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for BodyEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        let start = Self::utf16_to_offset(&text, range_utf16.start);
        let end = Self::utf16_to_offset(&text, range_utf16.end);
        actual_range
            .replace(Self::offset_to_utf16(&text, start)..Self::offset_to_utf16(&text, end));
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        let offset = Self::offset_to_utf16(&text, caret.offset);
        Some(UTF16Selection {
            range: offset..offset,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let caret = self.caret?;
        let text = self.doc.text(caret.block);
        self.marked_range.as_ref().map(|range| {
            Self::offset_to_utf16(&text, range.start)..Self::offset_to_utf16(&text, range.end)
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        // Typing over a selection replaces it, carrying a link across the
        // range like `insertText(text, from, to)` does.
        let mut carried_link = None;
        if range_utf16.is_none()
            && self.marked_range.is_none()
            && let Some((from, to)) = self.selection()
        {
            carried_link = self.doc.link_across(from, to);
            self.record_edit(EditKind::Structural);
            self.delete_selection();
        }
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        let text = self.doc.text(caret.block);
        let range = range_utf16
            .map(|r| Self::utf16_to_offset(&text, r.start)..Self::utf16_to_offset(&text, r.end))
            .or_else(|| self.marked_range.clone())
            .unwrap_or(caret.offset..caret.offset);
        if !range.is_empty() {
            self.record_edit(EditKind::Typing);
            self.doc.delete_range(caret.block, range.clone());
        }
        self.caret = Some(Caret {
            block: caret.block,
            offset: range.start,
        });
        self.anchor = None;
        self.marked_range = None;
        // Newlines pasted into a textblock become new blocks, as in
        // ProseMirror's `clipboardTextParser`: `text.split(/(?:\r\n?|\n)+/)`,
        // so a run of line breaks makes one paragraph boundary.
        let mut first = true;
        for line in split_pasted_lines(new_text) {
            if !first && let Some(caret) = self.caret {
                self.caret = Some(self.doc.split_block(caret));
            }
            first = false;
            if !line.is_empty() {
                let start = self.caret;
                self.insert(line, cx);
                if let (Some(mark), Some(start), Some(end)) =
                    (carried_link.take(), start, self.caret)
                    && start.block == end.block
                {
                    self.doc
                        .set_link(start.block, start.offset, end.offset, mark);
                }
            }
        }
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        if range_utf16.is_none() && self.marked_range.is_none() && self.selection().is_some() {
            self.record_edit(EditKind::Structural);
            self.delete_selection();
        }
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        self.anchor = None;
        let text = self.doc.text(caret.block);
        let range = range_utf16
            .map(|r| Self::utf16_to_offset(&text, r.start)..Self::utf16_to_offset(&text, r.end))
            .or_else(|| self.marked_range.clone())
            .unwrap_or(caret.offset..caret.offset);
        if !range.is_empty() {
            self.doc.delete_range(caret.block, range.clone());
        }
        let start = Caret {
            block: caret.block,
            offset: range.start,
        };
        let after = self.doc.insert_text(start, new_text);
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let updated = self.doc.text(caret.block);
        self.caret = Some(match new_selected_range_utf16 {
            Some(selected) => Caret {
                block: caret.block,
                offset: (range.start + Self::utf16_to_offset(new_text, selected.start))
                    .min(updated.len()),
            },
            None => after,
        });
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let text = self.doc.text(caret.block);
        let start = layout.position_for_index(Self::utf16_to_offset(&text, range_utf16.start))?;
        let end = layout.position_for_index(Self::utf16_to_offset(&text, range_utf16.end))?;
        Some(Bounds::from_corners(
            start,
            Point::new(end.x, end.y + layout.line_height()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let text = self.doc.text(caret.block);
        let index = layout.index_for_position(point).ok()?;
        Some(Self::offset_to_utf16(&text, index))
    }
}

fn snap(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn previous_boundary(text: &str, offset: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

fn next_boundary(text: &str, offset: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(text.len())
}

/// ProseMirror's default `clipboardTextParser` split: each maximal run of
/// `\r\n`, `\r` or `\n` separates paragraphs (empty ends included).
fn split_pasted_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut in_break = false;
    for (index, character) in text.char_indices() {
        let is_break = character == '\n' || character == '\r';
        if is_break && !in_break {
            lines.push(&text[start..index]);
            in_break = true;
        } else if !is_break && in_break {
            start = index;
            in_break = false;
        }
    }
    if in_break {
        lines.push("");
    } else {
        lines.push(&text[start..]);
    }
    lines
}

#[cfg(test)]
mod paste_tests {
    use super::split_pasted_lines;

    #[test]
    fn pasted_lines_split_on_runs_of_line_breaks() {
        assert_eq!(split_pasted_lines("a\n\nb"), vec!["a", "b"]);
        assert_eq!(split_pasted_lines("a\r\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_pasted_lines("\na\n"), vec!["", "a", ""]);
        assert_eq!(split_pasted_lines("plain"), vec!["plain"]);
        assert_eq!(split_pasted_lines(""), vec![""]);
    }
}
