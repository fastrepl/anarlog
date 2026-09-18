//! The memo editor: caret editing of the note body on top of the TipTap JSON
//! model, with the same persistence cadence as `packages/editor` (changes
//! flushed 500ms after the last keystroke, at most 10s apart).

pub mod clip;
pub mod links;
pub mod mention_picker;
pub mod model;
pub mod paste;
pub mod pm;
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
        SelectHome,
        SelectEnd,
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
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        DeleteWordBackward,
        DeleteWordForward,
        DocumentStart,
        DocumentEnd,
        SelectDocumentStart,
        SelectDocumentEnd,
        MoveListItemUp,
        MoveListItemDown,
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
    // Word movement modifier.
    let w = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-left", Home, ctx),
            KeyBinding::new("cmd-right", End, ctx),
            KeyBinding::new("cmd-shift-left", SelectHome, ctx),
            KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
            KeyBinding::new("cmd-up", DocumentStart, ctx),
            KeyBinding::new("cmd-down", DocumentEnd, ctx),
            KeyBinding::new("cmd-shift-up", SelectDocumentStart, ctx),
            KeyBinding::new("cmd-shift-down", SelectDocumentEnd, ctx),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("ctrl-home", DocumentStart, ctx),
            KeyBinding::new("ctrl-end", DocumentEnd, ctx),
            KeyBinding::new("ctrl-shift-home", SelectDocumentStart, ctx),
            KeyBinding::new("ctrl-shift-end", SelectDocumentEnd, ctx),
        ]);
    }
    cx.bind_keys([
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new(&format!("{m}-a"), SelectAll, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        // `Shift-Backspace` is bound to the same command; a shifted Delete
        // reaches WebKit's own deletion.
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("shift-delete", Delete, ctx),
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
        // The webview's own word and document movement: Alt on macOS, Ctrl
        // elsewhere, with Cmd-Left/Right as Home/End and Cmd-Up/Down as the
        // document's ends on macOS.
        KeyBinding::new(&format!("{w}-left"), WordLeft, ctx),
        KeyBinding::new(&format!("{w}-right"), WordRight, ctx),
        KeyBinding::new(&format!("{w}-shift-left"), SelectWordLeft, ctx),
        KeyBinding::new(&format!("{w}-shift-right"), SelectWordRight, ctx),
        KeyBinding::new(&format!("{w}-backspace"), DeleteWordBackward, ctx),
        KeyBinding::new(&format!("{w}-delete"), DeleteWordForward, ctx),
        KeyBinding::new(&format!("{w}-shift-backspace"), DeleteWordBackward, ctx),
        KeyBinding::new(&format!("{w}-shift-delete"), DeleteWordForward, ctx),
        // `Alt-ArrowUp` / `Alt-ArrowDown`: `moveListItem`.
        KeyBinding::new("alt-up", MoveListItemUp, ctx),
        KeyBinding::new("alt-down", MoveListItemDown, ctx),
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
    /// Files were pasted (`fileHandlerPlugin.handlePaste`): the owner saves
    /// them as attachments and inserts the nodes with `insert_attachment`.
    Files(Vec<PastedFile>),
    /// Paths were dropped on the editor (`handleDrop`), the caret already
    /// placed at the drop point.
    Dropped(Vec<std::path::PathBuf>),
}

/// A file from the clipboard or a drop, before it is saved as an attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastedFile {
    pub name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

/// `ResizableImageView`'s drag: the image's document-order index, which
/// handle, and the widths the pointer delta applies to.
#[derive(Debug, Clone, Copy)]
struct ImageResize {
    nth: usize,
    left: bool,
    start_x: Pixels,
    start_width: Pixels,
    max_width: Pixels,
    current: Pixels,
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
    /// The selection is `selectAll`'s `AllSelection` rather than a text
    /// selection over the same range: typing or deleting over it leaves one
    /// empty paragraph (its `$from` is the document, so the replaced content
    /// takes the schema's default block), where a text selection keeps the
    /// first block's type.
    all_selected: bool,
    is_selecting: bool,
    pasting: bool,
    marked_range: Option<Range<usize>>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<(Instant, EditKind)>,
    /// Whether the editor had focus at the last frame.
    focused: bool,
    /// An external body that arrived while the editor had focus, applied once
    /// focus leaves (`syncContent`'s `blur` listener).
    pending_external: Option<String>,
    /// Text layouts captured while painting, one per textblock.
    layouts: Vec<Option<(ProseLayout, Bounds<Pixels>)>>,
    dirty_since: Option<Instant>,
    /// `enforceTitleHeading` (the enhanced editor): keep an h1 first.
    enforce_title_heading: bool,
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
    /// Runs the network lookups (`resolveYouTubeClipUrl`).
    runtime: Option<tokio::runtime::Handle>,
    /// Image frames measured while painting, by document order.
    image_bounds: Vec<Option<Bounds<Pixels>>>,
    /// The editor root's width (`.ProseMirror`), the resize maximum.
    root_width: Option<Pixels>,
    image_resize: Option<ImageResize>,
    /// The caret this position was placed with upstream affinity: at the end
    /// of a soft-wrapped line (End, a click past the line's end), after its
    /// collapsed trailing space but drawn on that line, like WebKit.
    upstream_at: Option<Caret>,
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
            focus_handle: cx.focus_handle().tab_stop(true),
            session_id,
            layouts: vec![None; doc.textblock_count()],
            doc,
            caret: None,
            anchor: None,
            stored_marks: None,
            all_selected: false,
            is_selecting: false,
            pasting: false,
            marked_range: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
            focused: false,
            pending_external: None,
            dirty_since: None,
            enforce_title_heading: false,
            last_input: None,
            flush_scheduled: false,
            mention: None,
            mention_dismissed: None,
            mention_search: None,
            search: None,
            runtime: None,
            image_bounds: Vec::new(),
            root_width: None,
            image_resize: None,
            upstream_at: None,
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
    pub fn set_runtime(&mut self, runtime: tokio::runtime::Handle) {
        self.runtime = Some(runtime);
    }

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

    pub fn selection(&self) -> Option<(Caret, Caret)> {
        let (anchor, caret) = (self.anchor?, self.caret?);
        (anchor != caret).then(|| model::order(anchor, caret))
    }

    /// Collapses the selection into the caret, deleting its content.
    fn delete_selection(&mut self) -> bool {
        let Some((from, to)) = self.selection() else {
            return false;
        };
        if self.all_selected {
            // `deleteRange(0, doc.content.size)`: `block+` is refilled with
            // the default block.
            self.doc.replace_root(
                serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph" }] }),
            );
            self.caret = Some(Caret {
                block: 0,
                offset: 0,
            });
        } else {
            self.caret = Some(self.doc.delete_between(from, to));
        }
        self.anchor = None;
        self.all_selected = false;
        true
    }

    fn set_head(&mut self, head: Caret, extend: bool, cx: &mut Context<Self>) {
        if extend {
            self.anchor.get_or_insert(self.caret.unwrap_or(head));
        } else {
            self.anchor = None;
        }
        self.all_selected = false;
        self.caret = Some(head);
        self.upstream_at = None;
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
            crate::opener::open_url(&href);
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
    /// Like the web editor's `externalContentSync` transaction (#7462), the
    /// sync is one closed history step: Undo brings the previous content
    /// back, the next typing starts its own group, and nothing is persisted.
    /// While the editor has focus the body waits for the blur
    /// (`shouldReplaceEditorContent` without `syncContentWhenFocused`).
    pub fn replace_body(&mut self, body: &str, cx: &mut Context<Self>) {
        if self.is_dirty() {
            return;
        }
        if self.focused {
            self.pending_external = Some(body.to_string());
            return;
        }
        self.apply_external_body(body, cx);
    }

    /// `editor.commands.replaceContent(content)` from a generation job: the
    /// document is replaced whatever the focus or dirty state; the first
    /// replacement of a job is one undo step, later partials extend it.
    pub fn replace_body_generated(&mut self, body: &str, first: bool, cx: &mut Context<Self>) {
        let mut doc = Doc::parse(body);
        if self.enforce_title_heading {
            doc.enforce_title_heading();
        }
        if doc.to_json() == self.doc.to_json() {
            return;
        }
        if first {
            self.undo_stack.push(Snapshot {
                json: self.doc.to_json(),
                caret: self.caret,
            });
            if self.undo_stack.len() > 200 {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
        self.last_edit = None;
        self.pending_external = None;
        self.doc = doc;
        self.layouts = vec![None; self.doc.textblock_count()];
        self.clamp_caret();
        // The replaced content is the editor's own change: `flush` persists it.
        self.dirty_since.get_or_insert_with(std::time::Instant::now);
        cx.notify();
    }

    /// The workspace reports the focus state every frame: a body parked while
    /// focused is applied once focus leaves (`syncContent` on `blur`).
    pub fn sync_focus(&mut self, focused: bool, cx: &mut Context<Self>) {
        self.focused = focused;
        if !focused && let Some(body) = self.pending_external.take() {
            self.apply_external_body(&body, cx);
        }
    }

    fn apply_external_body(&mut self, body: &str, cx: &mut Context<Self>) {
        if self.is_dirty() {
            return;
        }
        let mut doc = Doc::parse(body);
        // `normalizeTitleHeadingDoc` on the incoming document, so a store body
        // the title rule already reshaped compares equal.
        if self.enforce_title_heading {
            doc.enforce_title_heading();
        }
        if doc.to_json() == self.doc.to_json() {
            return;
        }
        self.undo_stack.push(Snapshot {
            json: self.doc.to_json(),
            caret: self.caret,
        });
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.last_edit = None;
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

    /// `createSelectionVirtualElement`: `coordsAtPos(from)` and
    /// `coordsAtPos(to)` joined into one rectangle, in window coordinates.
    pub fn selection_rect(&self) -> Option<Bounds<Pixels>> {
        let (from, to) = self.selection()?;
        let coords = |caret: Caret| {
            let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
            let position = layout.position_for_index(caret.offset)?;
            Some((position, layout.line_height()))
        };
        let (start, _) = coords(from)?;
        let (end, end_line) = coords(to)?;
        let left = start.x.min(end.x);
        Some(Bounds::new(
            Point::new(left, start.y),
            gpui::size((end.x - start.x).abs(), end.y + end_line - start.y),
        ))
    }

    /// `isMarkActive` for a non-empty selection: `rangeHasMark`.
    pub fn selection_has_mark(&self, mark: &str) -> bool {
        self.selection()
            .is_some_and(|(from, to)| self.doc.range_has_mark(from, to, mark))
    }

    /// `selectionTouchesTitleHeading`: with the title heading enforced, a
    /// selection overlapping the first block is not formatted.
    pub fn selection_touches_title(&self) -> bool {
        self.enforce_title_heading
            && self.selection().is_some_and(|(from, _)| from.block == 0)
            && self.doc.block_type(0).as_deref() == Some("heading")
    }

    /// Caret position in window coordinates plus the line height, when the
    /// block has been laid out.
    pub fn caret_position(&self) -> Option<(Point<Pixels>, Pixels)> {
        let caret = self.caret?;
        let (layout, _) = self.layouts.get(caret.block)?.as_ref()?;
        let position = layout.position_for_index_biased(caret.offset, self.caret_upstream())?;
        Some((position, layout.line_height()))
    }

    /// The caret sits on a soft-wrap boundary with upstream affinity.
    pub fn caret_upstream(&self) -> bool {
        self.caret.is_some() && self.upstream_at == self.caret
    }

    /// The caret a click at `position` places, and whether it takes
    /// upstream affinity (past the end of a soft-wrapped line).
    fn caret_for_position(&self, block: usize, position: Point<Pixels>) -> (Caret, bool) {
        let (offset, upstream) = self
            .layouts
            .get(block)
            .and_then(Option::as_ref)
            .map(|(layout, _)| layout.caret_for_point(position))
            .unwrap_or((0, false));
        let block = block.min(self.doc.textblock_count().saturating_sub(1));
        let text = self.doc.text(block);
        let snapped = snap(&text, offset.min(text.len()));
        let offset = self.doc.snap_out_of_atoms(block, snapped, 0);
        (Caret { block, offset }, upstream && offset == snapped)
    }

    /// Mouse down in a textblock: place the caret (shift extends) and start a
    /// drag selection; a double-click selects the word under the pointer and
    /// a triple-click the textblock, like the browser's selection.
    pub fn place_caret_at(
        &mut self,
        block: usize,
        position: Point<Pixels>,
        extend: bool,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        let (head, upstream) = self.caret_for_position(block, position);
        match click_count {
            2 => {
                let text = self.doc.text(block);
                let range = crate::text_input::word_range_at(&text, head.offset);
                self.set_head(
                    Caret {
                        block,
                        offset: range.start,
                    },
                    false,
                    cx,
                );
                self.set_head(
                    Caret {
                        block,
                        offset: range.end,
                    },
                    true,
                    cx,
                );
            }
            count if count >= 3 => {
                let end = self.doc.text(block).len();
                self.set_head(Caret { block, offset: 0 }, false, cx);
                self.set_head(Caret { block, offset: end }, true, cx);
            }
            _ => {
                self.set_head(head, extend, cx);
                self.upstream_at = upstream.then_some(head);
            }
        }
        self.is_selecting = true;
        self.focus_handle.focus(window);
    }

    /// Mouse moved over a textblock while dragging: extend to that point.
    pub fn drag_to(&mut self, block: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let (head, upstream) = self.caret_for_position(block, position);
        if self.caret != Some(head) {
            self.set_head(head, true, cx);
            self.upstream_at = upstream.then_some(head);
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

    /// `focusTrailingEmptyLine` (a press in the note area outside the
    /// editor's blocks): the caret goes to the end of a document ending in a
    /// blank paragraph; any other ending gets a fresh paragraph appended for
    /// the caret, as one history step.
    pub fn focus_trailing_empty_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.doc.ensure_textblock();
        if !self.doc.ends_in_blank_paragraph() {
            self.record_edit(EditKind::Structural);
            self.doc.append_paragraph();
            let block = self.doc.textblock_count() - 1;
            self.set_head(Caret { block, offset: 0 }, false, cx);
            self.focus_handle.focus(window);
            self.changed(cx);
            return;
        }
        self.place_caret_at_end(window, cx);
    }

    /// `Selection.atEnd(doc)` plus focus.
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
    /// within `newGroupDelay` share one history entry. Like a transaction's
    /// `addStep`, an edit drops the stored marks: a pending `Ctrl+B` does
    /// not survive Enter or Backspace, only the text typed next.
    fn record_edit(&mut self, kind: EditKind) {
        self.stored_marks = None;
        // A local edit supersedes a body parked while focused: the store will
        // come back with this edit's own document.
        self.pending_external = None;
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

    /// `enforceTitleHeading`: the document keeps an h1 as its first block.
    pub fn set_enforce_title_heading(&mut self, enforce: bool) {
        self.enforce_title_heading = enforce;
        if enforce {
            // `normalizeTitleHeadingDoc` on the initial state.
            self.normalize_title_heading();
        }
    }

    /// `normalizeTitleHeadingDoc`: the document starts with an h1, with a
    /// paragraph after a lone empty one; the layouts and caret follow.
    fn normalize_title_heading(&mut self) {
        if !self.enforce_title_heading {
            return;
        }
        let inserted = self.doc.enforce_title_heading();
        self.layouts = vec![None; self.doc.textblock_count()];
        if inserted {
            if let Some(caret) = self.caret.as_mut() {
                caret.block += 1;
            }
            if let Some(anchor) = self.anchor.as_mut() {
                anchor.block += 1;
            }
        }
        self.clamp_caret();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        // `taskIdentityPlugin`: ids stay unique after splits and pastes.
        self.doc.ensure_task_identity();
        // `imageTrailingParagraphPlugin`: a paragraph follows every top-level
        // image. The caret's block index shifts when one lands above it.
        let caret_path = self
            .caret
            .and_then(|caret| self.doc.textblock_path(caret.block));
        let inserted_after = self.doc.ensure_image_trailing_paragraphs();
        if !inserted_after.is_empty()
            && let Some(caret) = self.caret
            && let Some(mut path) = caret_path
        {
            path[0] += inserted_after
                .iter()
                .filter(|image| **image < path[0])
                .count();
            if let Some(block) = self.doc.textblock_index_of(&path) {
                self.caret = Some(model::Caret {
                    block,
                    offset: caret.offset,
                });
            }
        }
        if self.enforce_title_heading && self.doc.enforce_title_heading() {
            // A heading was inserted above: the caret's block shifted down.
            if let Some(caret) = self.caret.as_mut() {
                caret.block += 1;
            }
            if let Some(anchor) = self.anchor.as_mut() {
                anchor.block += 1;
            }
        }
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
            .map(|(layout, _)| layout.caret_for_point(Point::new(position.x, target_y)));
        let (next, upstream) = match within {
            Some((index, upstream)) => (
                Caret {
                    block: caret.block,
                    offset: index,
                },
                upstream,
            ),
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
                let (index, upstream) = layout.caret_for_point(Point::new(position.x, y));
                (
                    Caret {
                        block,
                        offset: index,
                    },
                    upstream,
                )
            }
        };
        let text = self.doc.text(next.block);
        let snapped = snap(&text, next.offset.min(text.len()));
        let upstream = upstream && snapped == next.offset;
        let next = Caret {
            block: next.block,
            offset: snapped,
        };
        self.set_head(next, extend, cx);
        self.upstream_at = upstream.then_some(next);
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
        self.all_selected = true;
        self.stored_marks = None;
        cx.notify();
    }

    fn on_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_line_edge(false, false, cx);
    }

    fn on_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_line_edge(true, false, cx);
    }

    fn on_select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_line_edge(false, true, cx);
    }

    fn on_select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_line_edge(true, true, cx);
    }

    /// Ctrl/Alt-Left/Right: WebKit's word movement, crossing into the
    /// neighbouring textblock from an edge like a plain arrow does.
    fn move_by_word(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        if !extend && let Some((from, to)) = self.selection() {
            self.set_head(if forward { to } else { from }, false, cx);
            return;
        }
        let Some(next) = self.word_target(caret, forward) else {
            return;
        };
        self.set_head(next, extend, cx);
    }

    /// Where a word step from `caret` lands, `None` at the document's edge.
    fn word_target(&self, caret: Caret, forward: bool) -> Option<Caret> {
        let text = self.doc.text(caret.block);
        if forward {
            if caret.offset >= text.len() {
                return (caret.block + 1 < self.doc.textblock_count()).then(|| Caret {
                    block: caret.block + 1,
                    offset: 0,
                });
            }
            let offset = crate::text_input::next_word_end(&text, caret.offset);
            Some(Caret {
                block: caret.block,
                offset: self.doc.snap_out_of_atoms(caret.block, offset, 1),
            })
        } else {
            if caret.offset == 0 {
                return (caret.block > 0).then(|| Caret {
                    block: caret.block - 1,
                    offset: self.doc.text(caret.block - 1).len(),
                });
            }
            let offset = crate::text_input::previous_word_start(&text, caret.offset);
            Some(Caret {
                block: caret.block,
                offset: self.doc.snap_out_of_atoms(caret.block, offset, -1),
            })
        }
    }

    fn on_word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_word(false, false, cx);
    }

    fn on_word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_word(true, false, cx);
    }

    fn on_select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_word(false, true, cx);
    }

    fn on_select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_by_word(true, true, cx);
    }

    /// Ctrl/Alt-Backspace and -Delete: the browser's `deleteWordBackward` /
    /// `deleteWordForward` within the textblock (a selection is deleted, and
    /// at a textblock edge the plain key's join applies).
    fn delete_by_word(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
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
        if (forward && caret.offset >= text.len()) || (!forward && caret.offset == 0) {
            if forward {
                self.on_delete(&Delete, window, cx);
            } else {
                self.on_backspace(&Backspace, window, cx);
            }
            return;
        }
        let Some(target) = self.word_target(caret, forward) else {
            return;
        };
        let range = if forward {
            caret.offset..target.offset
        } else {
            target.offset..caret.offset
        };
        self.record_edit(EditKind::Deleting);
        self.doc.delete_range(caret.block, range.clone());
        self.caret = Some(Caret {
            block: caret.block,
            offset: range.start,
        });
        self.anchor = None;
        self.changed(cx);
    }

    fn on_delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_by_word(false, window, cx);
    }

    fn on_delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_by_word(true, window, cx);
    }

    /// Ctrl-Home / Ctrl-End (Cmd-Up / Cmd-Down): the document's ends.
    fn move_to_document_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let count = self.doc.textblock_count();
        if count == 0 {
            return;
        }
        let target = if end {
            Caret {
                block: count - 1,
                offset: self.doc.text(count - 1).len(),
            }
        } else {
            Caret {
                block: 0,
                offset: 0,
            }
        };
        self.set_head(target, extend, cx);
    }

    fn on_document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_document_edge(false, false, cx);
    }

    fn on_document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to_document_edge(true, false, cx);
    }

    fn on_select_document_start(
        &mut self,
        _: &SelectDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to_document_edge(false, true, cx);
    }

    fn on_select_document_end(
        &mut self,
        _: &SelectDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to_document_edge(true, true, cx);
    }

    /// Alt-Up / Alt-Down: `moveListItem`, the caret staying in its text.
    fn move_list_item(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        // Recorded before the move so a no-op (not in a list, incompatible
        // outer list) leaves the history alone.
        let before = self.doc.clone();
        if let Some(block) = self.doc.move_list_item(caret.block, up) {
            let after = std::mem::replace(&mut self.doc, before);
            self.record_edit(EditKind::Structural);
            self.doc = after;
            self.caret = Some(Caret {
                block,
                offset: caret.offset,
            });
            self.anchor = None;
            self.changed(cx);
        }
    }

    fn on_move_list_item_up(&mut self, _: &MoveListItemUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_list_item(true, cx);
    }

    fn on_move_list_item_down(
        &mut self,
        _: &MoveListItemDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_list_item(false, cx);
    }

    /// Home / End (with Shift extending) to the textblock's edge.
    /// Home / End: WebKit's `startOfLine` / `endOfLine` — the visual line the
    /// caret sits on (a wrapped line or one ended by a hard break), the
    /// whole textblock before it has been laid out.
    fn move_to_line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        if let Some(caret) = self.caret {
            let edges = self
                .layouts
                .get(caret.block)
                .and_then(|layout| layout.as_ref())
                .and_then(|(layout, _)| {
                    layout.line_edges_for_index(caret.offset, self.caret_upstream())
                });
            let (offset, upstream) = match (edges, end) {
                (Some(edges), true) => (edges.end, edges.soft_wrap),
                (Some(edges), false) => (edges.start, false),
                (None, true) => (self.doc.text(caret.block).len(), false),
                (None, false) => (0, false),
            };
            let head = Caret {
                block: caret.block,
                offset,
            };
            self.set_head(head, extend, cx);
            self.upstream_at = upstream.then_some(head);
        }
    }

    fn on_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.selection() {
            self.copy_selection(from, to, cx);
        }
    }

    fn on_cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((from, to)) = self.selection() {
            self.copy_selection(from, to, cx);
            self.record_edit(EditKind::Structural);
            self.delete_selection();
            self.changed(cx);
        }
    }

    /// `serializeForClipboard(view, selection.content())`: the text from
    /// `clipboardTextSerializer` and the HTML with its `data-pm-slice`
    /// context go on the clipboard together; the text alone where the
    /// platform clipboard takes one string.
    fn copy_selection(&self, from: Caret, to: Caret, cx: &mut Context<Self>) {
        let text = self.doc.clipboard_text_between(from, to);
        let html = self.selection_html(from, to);
        if html
            .as_deref()
            .is_none_or(|html| !paste::write_clipboard(&text, html))
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn selection_html(&self, from: Caret, to: Caret) -> Option<String> {
        let schema = pm::schema::schema();
        let doc = pm::node::Node::from_json(schema, self.doc.root())?;
        let (from, to) = (paste::position(&doc, from)?, paste::position(&doc, to)?);
        Some(pm::serialize::serialize_for_clipboard(
            schema,
            &doc.slice_at(from, to, true),
        ))
    }

    fn on_paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // `clipboardData.files`: images on the clipboard become attachments.
        let files: Vec<PastedFile> = item
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                gpui::ClipboardEntry::Image(image) => Some(PastedFile {
                    name: format!("image.{}", image_extension(image.format)),
                    mime_type: image.format.mime_type().to_string(),
                    bytes: image.bytes.clone(),
                }),
                gpui::ClipboardEntry::String(_) => None,
            })
            .collect();
        if !files.is_empty() {
            cx.emit(EditorEvent::Files(files));
            return;
        }
        let text = item.text();
        // `clipPastePlugin.handlePaste` reads the plain text first.
        if let Some(text) = text.as_deref()
            && self.paste_clip(text, cx)
        {
            return;
        }
        // `parseFromClipboard`: the HTML is used unless the caret sits in a
        // code block (`inCode`) or the clipboard carries none.
        let in_code = self
            .caret
            .is_some_and(|caret| self.doc.block_type(caret.block).as_deref() == Some("codeBlock"));
        if !in_code
            && let Some(html) = paste::clipboard_html()
            && self.paste_html(&html, cx)
        {
            return;
        }
        if let Some(text) = text {
            self.record_edit(EditKind::Structural);
            self.pasting = true;
            self.replace_text_in_range(None, &text, window, cx);
            self.pasting = false;
        }
    }

    /// `doPaste` with the clipboard's HTML: the slice `parseFromClipboard`
    /// builds replaces the selection like `replaceSelection`, the caret
    /// lands at the insertion end, and the autolink plugins run over the
    /// textblocks the paste touched. `false` when the HTML yields nothing
    /// (the text is pasted instead), `true` once handled.
    fn paste_html(&mut self, html: &str, cx: &mut Context<Self>) -> bool {
        self.doc.ensure_textblock();
        let schema = pm::schema::schema();
        let Some(doc) = pm::node::Node::from_json(schema, self.doc.root()) else {
            return false;
        };
        let caret = self.caret.unwrap_or(Caret {
            block: 0,
            offset: 0,
        });
        let anchor = self.anchor.unwrap_or(caret);
        let (from_caret, to_caret) = model::order(anchor, caret);
        // An `AllSelection` spans the document itself, not the first and
        // last textblocks' insides.
        let range = if self.all_selected && anchor != caret {
            (Some(0), Some(doc.content.size))
        } else {
            (
                paste::position(&doc, from_caret),
                paste::position(&doc, to_caret),
            )
        };
        let (Some(from), Some(to)) = range else {
            return false;
        };
        let context = doc.resolve(from);
        let Some(slice) = pm::clipboard::parse_from_clipboard(schema, html, &context) else {
            return false;
        };
        let Some(pasted) = pm::clipboard::paste(schema, &doc, from, to, &slice) else {
            return true;
        };
        let end = match pasted.selection {
            pm::clipboard::SelectionEnd::Text(pos) => Some(pos),
            pm::clipboard::SelectionEnd::Node { to, .. } => {
                pm::clipboard::near_text(schema, &pasted.doc, to, 1)
            }
        };
        let new_caret = end.and_then(|pos| paste::caret(&pasted.doc, pos));
        self.record_edit(EditKind::Structural);
        self.doc.replace_root(pasted.doc.to_json(schema));
        let new_caret = new_caret.unwrap_or(Caret {
            block: from_caret
                .block
                .min(self.doc.textblock_count().saturating_sub(1)),
            offset: 0,
        });
        self.caret = Some(new_caret);
        self.anchor = None;
        self.all_selected = false;
        self.stored_marks = None;
        // `autolinkPlugin` / `linkBoundaryGuardPlugin` over the changed
        // textblocks: the pasted range of the first and last, all of the rest.
        for block in from_caret.block
            ..=new_caret
                .block
                .min(self.doc.textblock_count().saturating_sub(1))
        {
            let len = self.doc.text(block).len();
            let start = if block == from_caret.block {
                from_caret.offset.min(len)
            } else {
                0
            };
            let end = if block == new_caret.block {
                new_caret.offset.min(len)
            } else {
                len
            };
            self.doc.maintain_links(block, start..end.max(start));
        }
        self.layouts = vec![None; self.doc.textblock_count()];
        self.changed(cx);
        true
    }

    /// `handleDrop`: the caret moves to `posAtCoords` of the drop, then the
    /// owner sorts the paths into an audio import and attachments.
    pub fn drop_paths(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        self.doc.ensure_textblock();
        // The textblock under the pointer, else the nearest one above it
        // (or the first).
        let block = self
            .layouts
            .iter()
            .enumerate()
            .filter_map(|(index, layout)| layout.as_ref().map(|(_, bounds)| (index, *bounds)))
            .fold(None::<(usize, Bounds<Pixels>)>, |best, (index, bounds)| {
                if bounds.top() <= position.y {
                    Some((index, bounds))
                } else {
                    best
                }
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let (head, _) = self.caret_for_position(block, position);
        self.set_head(head, false, cx);
        cx.emit(EditorEvent::Dropped(paths));
    }

    /// `sessionMentionDropPlugin.handleDrop`: the mention node plus a space
    /// at `posAtCoords`, the caret after them, the editor focused.
    pub fn drop_mention(
        &mut self,
        item: mention_picker::MentionItem,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.doc.ensure_textblock();
        let block = self
            .layouts
            .iter()
            .enumerate()
            .filter_map(|(index, layout)| layout.as_ref().map(|(_, bounds)| (index, *bounds)))
            .fold(None::<(usize, Bounds<Pixels>)>, |best, (index, bounds)| {
                if bounds.top() <= position.y {
                    Some((index, bounds))
                } else {
                    best
                }
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let (head, _) = self.caret_for_position(block, position);
        self.record_edit(EditKind::Structural);
        let caret = self
            .doc
            .insert_mention(head.block, head.offset..head.offset, &item);
        self.caret = Some(caret);
        self.anchor = None;
        self.changed(cx);
        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window);
        }
    }

    /// `insertImage` / `insertFileAttachment` once an upload finished:
    /// `tr.replaceSelectionWith(node)` at the selection of that moment.
    pub fn insert_attachment(&mut self, node: serde_json::Value, cx: &mut Context<Self>) {
        let Some(caret) = self.caret.or_else(|| {
            (self.doc.textblock_count() > 0).then_some(model::Caret {
                block: 0,
                offset: 0,
            })
        }) else {
            return;
        };
        let anchor = self.anchor.unwrap_or(caret);
        self.record_edit(EditKind::Structural);
        let caret = self.doc.insert_block_atom(anchor, caret, node);
        self.caret = Some(caret);
        self.anchor = None;
        self.stored_marks = None;
        self.changed(cx);
    }

    /// `FileAttachmentView.handleRemove`: delete the `nth` node of `kind`.
    pub fn remove_block_atom(&mut self, kind: &str, nth: usize, cx: &mut Context<Self>) {
        let Some(path) = self.doc.nth_block_path(kind, nth) else {
            return;
        };
        self.record_edit(EditKind::Structural);
        let caret = self.doc.remove_block(&path);
        self.caret = Some(caret);
        self.anchor = None;
        self.changed(cx);
    }

    pub fn set_image_bounds(&mut self, nth: usize, bounds: Bounds<Pixels>) {
        if self.image_bounds.len() <= nth {
            self.image_bounds.resize(nth + 1, None);
        }
        self.image_bounds[nth] = Some(bounds);
    }

    pub fn set_root_width(&mut self, width: Pixels) {
        self.root_width = Some(width);
    }

    /// The image being resized and its draft pixel width.
    pub fn image_resize_draft(&self) -> Option<(usize, Pixels)> {
        self.image_resize.map(|resize| (resize.nth, resize.current))
    }

    /// `handleResizeStart`: the drag starts from the image's painted width
    /// against the editor's width.
    pub fn begin_image_resize(
        &mut self,
        nth: usize,
        left: bool,
        x: Pixels,
        cx: &mut Context<Self>,
    ) {
        let Some(start_width) = self
            .image_bounds
            .get(nth)
            .copied()
            .flatten()
            .map(|bounds| bounds.size.width)
        else {
            return;
        };
        let max_width = self.root_width.unwrap_or(start_width);
        self.image_resize = Some(ImageResize {
            nth,
            left,
            start_x: x,
            start_width,
            max_width,
            current: start_width,
        });
        cx.notify();
    }

    /// `handlePointerMove`: `min(maxWidth, max(120, startWidth + deltaX))`.
    fn update_image_resize(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(resize) = self.image_resize.as_mut() else {
            return;
        };
        let delta = if resize.left {
            resize.start_x - x
        } else {
            x - resize.start_x
        };
        let next = (resize.start_width + delta)
            .max(gpui::px(120.0))
            .min(resize.max_width);
        if next != resize.current {
            resize.current = next;
            cx.notify();
        }
    }

    /// `onCommit`: `editorWidth = clampImageWidth(currentWidth / maxWidth * 100)`.
    fn commit_image_resize(&mut self, cx: &mut Context<Self>) {
        let Some(resize) = self.image_resize.take() else {
            return;
        };
        let percent = f32::from(resize.current) / f32::from(resize.max_width).max(1.0) * 100.0;
        let width = crate::document::clamp_image_width(Some(percent as f64));
        if let Some(path) = self.doc.nth_block_path("image", resize.nth) {
            self.record_edit(EditKind::Structural);
            self.doc
                .set_block_attr(&path, "editorWidth", serde_json::json!(width));
            self.changed(cx);
        } else {
            cx.notify();
        }
    }

    /// `clipPastePlugin.handlePaste`: an embed snippet or a YouTube link
    /// becomes a `clip` node; a clip link is looked up first and the paste is
    /// consumed either way.
    fn paste_clip(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        if let Some(embed) = clip::parse_youtube_embed_snippet(text) {
            self.insert_clip(&embed, cx);
            return true;
        }
        if text.is_empty() {
            return false;
        }
        if let Some(clip_id) = clip::parse_youtube_clip_id(text) {
            if let Some(runtime) = self.runtime.clone() {
                let lookup = runtime.spawn(clip::resolve_youtube_clip_url(clip_id));
                cx.spawn(async move |this, cx| {
                    if let Ok(Some(embed)) = lookup.await {
                        this.update(cx, |this, cx| this.insert_clip(&embed, cx))
                            .ok();
                    }
                })
                .detach();
            }
            return true;
        }
        match clip::parse_youtube_url(text) {
            Some(embed) => {
                self.insert_clip(&embed, cx);
                true
            }
            None => false,
        }
    }

    /// `tr.replaceSelectionWith(clip)`.
    fn insert_clip(&mut self, embed_url: &str, cx: &mut Context<Self>) {
        let Some(caret) = self.caret else {
            return;
        };
        let anchor = self.anchor.unwrap_or(caret);
        self.record_edit(EditKind::Structural);
        let caret = self
            .doc
            .insert_block_atom(anchor, caret, clip::clip_node(embed_url));
        self.caret = Some(caret);
        self.anchor = None;
        self.stored_marks = None;
        self.changed(cx);
    }

    /// `toggleMark`: with a selection, adds or removes the mark across it;
    /// with a caret only, toggles it in the stored marks for the next input.
    pub fn toggle_mark(&mut self, mark: &'static str, cx: &mut Context<Self>) {
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
            // `revertBlockToParagraph`, `joinTaskItemBackward`, `joinBackward`.
            if matches!(
                self.doc.block_type(caret.block).as_deref(),
                Some("heading" | "codeBlock")
            ) {
                self.record_edit(EditKind::Structural);
                self.doc.set_block_type(caret.block, "paragraph", None);
                self.changed(cx);
                return;
            }
            if self.doc.parent_type(caret.block).as_deref() == Some("taskItem")
                && self.doc.is_first_child(caret.block)
            {
                if self.doc.is_first_list_item(caret.block) {
                    // The first task keeps its text by lifting; an empty one
                    // falls through to `joinBackward`.
                    if !self.doc.text(caret.block).is_empty() {
                        self.record_edit(EditKind::Structural);
                        if let Some(next) = self.doc.lift_list_item(caret.block) {
                            self.caret = Some(next);
                        }
                        self.changed(cx);
                        return;
                    }
                } else {
                    // Later tasks merge their paragraph into the previous task.
                    self.record_edit(EditKind::Structural);
                    if let Some(next) = self.doc.join_task_item_backward(caret.block) {
                        self.caret = Some(next);
                    }
                    self.changed(cx);
                    return;
                }
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
            // `joinForward`.
            self.record_edit(EditKind::Structural);
            if let Some(next) = self.doc.join_forward(caret.block) {
                self.caret = Some(next);
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
        // Over an `AllSelection` every command in the Enter chain declines
        // (`$from` has no depth to split at) and WebKit's own handling only
        // deletes the selection.
        let all_selected = self.all_selected;
        // `splitBlock` deletes the selection and splits at the mapped
        // start; when `deleteRange` leaves that position between blocks the
        // split is impossible, the chain declines, and WebKit's own handling
        // joins the ends into the start block without a break.
        if !all_selected
            && let Some((from, to)) = self.selection()
            && from.block != to.block
            && self.doc.deletion_leaves_split_point(from, to) == Some(false)
        {
            self.caret = Some(self.doc.join_delete_between(from, to));
            self.anchor = None;
            self.all_selected = false;
            self.changed(cx);
            return;
        }
        self.delete_selection();
        if all_selected {
            self.changed(cx);
            return;
        }
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
        // `liftEmptyBlock`: an empty paragraph in a blockquote splits the
        // quote or leaves it.
        if text.is_empty()
            && self.doc.parent_type(caret.block).as_deref() == Some("blockquote")
            && let Some(next) = self.doc.lift_empty_block(caret.block)
        {
            self.caret = Some(next);
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
        // The typed text takes the stored marks (`insertText` inherits
        // `storedMarks`), which the edit itself then clears.
        let stored_marks = self.stored_marks.take();
        self.record_edit(EditKind::Typing);
        // Input rules see typed text only (`handleTextInput`), never pastes.
        if !self.pasting {
            let in_code = self.doc.block_type(caret.block).as_deref() == Some("codeBlock")
                || stored_marks
                    .clone()
                    .unwrap_or_else(|| self.doc.marks_at(caret))
                    .contains(&"code");
            if let Some(outcome) = rules::apply(&mut self.doc, caret, text, in_code) {
                self.caret = Some(outcome.caret);
                if let Some(mark) = outcome.clear_stored_mark {
                    let mut marks =
                        stored_marks.unwrap_or_else(|| self.doc.marks_at(outcome.caret));
                    marks.retain(|m| *m != mark);
                    self.stored_marks = Some(marks);
                }
                self.layouts = vec![None; self.doc.textblock_count()];
                self.changed(cx);
                return;
            }
        }
        let end = self.doc.insert_text(caret, text);
        if let Some(marks) = stored_marks {
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
            .on_action(cx.listener(Self::on_select_home))
            .on_action(cx.listener(Self::on_select_end))
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
            .on_action(cx.listener(Self::on_word_left))
            .on_action(cx.listener(Self::on_word_right))
            .on_action(cx.listener(Self::on_select_word_left))
            .on_action(cx.listener(Self::on_select_word_right))
            .on_action(cx.listener(Self::on_delete_word_backward))
            .on_action(cx.listener(Self::on_delete_word_forward))
            .on_action(cx.listener(Self::on_document_start))
            .on_action(cx.listener(Self::on_document_end))
            .on_action(cx.listener(Self::on_select_document_start))
            .on_action(cx.listener(Self::on_select_document_end))
            .on_action(cx.listener(Self::on_move_list_item_up))
            .on_action(cx.listener(Self::on_move_list_item_down))
            // The title bar's Edit menu (`runEditCommand`) targets the editor
            // that had focus with the app-level actions.
            .on_action(cx.listener(|this, _: &crate::actions::Undo, window, cx| {
                this.on_undo(&Undo, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Redo, window, cx| {
                this.on_redo(&Redo, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Cut, window, cx| {
                this.on_cut(&Cut, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Copy, window, cx| {
                this.on_copy(&Copy, window, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::actions::Paste, window, cx| {
                this.on_paste(&Paste, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::actions::SelectAll, window, cx| {
                    this.on_select_all(&SelectAll, window, cx)
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.update_image_resize(event.position.x, cx)
            }))
            // A right-click in a contenteditable focuses it and opens the
            // editing context menu.
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle);
                    crate::edit_menu::request(cx, event.position, this.focus_handle.clone());
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.end_mouse_selection();
                    this.commit_image_resize(cx);
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.end_mouse_selection();
                    this.commit_image_resize(cx);
                }),
            )
    }
}

/// The extension `File.name` carries for a pasted image (`image.png`).
fn image_extension(format: gpui::ImageFormat) -> &'static str {
    match format {
        gpui::ImageFormat::Png => "png",
        gpui::ImageFormat::Jpeg => "jpeg",
        gpui::ImageFormat::Webp => "webp",
        gpui::ImageFormat::Gif => "gif",
        gpui::ImageFormat::Svg => "svg",
        gpui::ImageFormat::Bmp => "bmp",
        gpui::ImageFormat::Tiff => "tiff",
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
        // Typed text over a selection spanning blocks is
        // `insertText(text, from, to)`: one `replaceRangeWith`, not a
        // deletion and an insertion.
        if range_utf16.is_none()
            && self.marked_range.is_none()
            && !self.pasting
            && !self.all_selected
            && let Some((from, to)) = self.selection()
            && from.block != to.block
        {
            let marks = self
                .stored_marks
                .clone()
                .unwrap_or_else(|| self.doc.marks_at(from));
            let snapshot = self.doc.to_json();
            self.record_edit(EditKind::Structural);
            if let Some(caret) = self
                .doc
                .replace_between_with_text(from, to, new_text, &marks)
            {
                self.caret = Some(caret);
                self.anchor = None;
                self.marked_range = None;
                self.layouts = vec![None; self.doc.textblock_count()];
                self.changed(cx);
                return;
            }
            debug_assert_eq!(self.doc.to_json(), snapshot);
        }
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
