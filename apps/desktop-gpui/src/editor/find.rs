//! Shipping's note find & replace (`session/components/note-input/search`): ⌘F searches the
//! shown memo, summary or transcript; ⌘H adds the replace row on editable tabs.

use std::ops::Range;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Hsla, KeyDownEvent, MouseButton, Subscription,
    WeakEntity, Window, div, prelude::*, px, rgb, rgba, svg,
};

use super::{
    EditorPane,
    document::{Document, NodeRef, inline_text},
    transcript::TranscriptView,
};
use crate::ui::a11y::Accessible;
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::{Theme, theme},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// Folds `text` for matching and maps each folded byte back to its source byte.
fn fold(text: &str, case_sensitive: bool) -> (String, Vec<usize>) {
    let mut folded = String::with_capacity(text.len());
    let mut sources = Vec::with_capacity(text.len() + 1);
    for (byte, character) in text.char_indices() {
        let start = folded.len();
        if case_sensitive {
            folded.push(character);
        } else {
            folded.extend(character.to_lowercase());
        }
        sources.extend(std::iter::repeat_n(byte, folded.len() - start));
    }
    sources.push(text.len());
    (folded, sources)
}

/// Shipping's `isWordBoundary`: JavaScript's `\w` is ASCII letters, digits and `_`.
fn boundary(character: Option<char>) -> bool {
    !character.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Byte ranges of `query` (already folded) in `folded`, honoring whole-word matching.
fn occurrences(folded: &str, query: &str, whole_word: bool) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = folded[from..].find(query) {
        let start = from + offset;
        let end = start + query.len();
        if !whole_word
            || (boundary(folded[..start].chars().next_back())
                && boundary(folded[end..].chars().next()))
        {
            found.push(start..end);
            from = end;
        } else {
            from = start + folded[start..].chars().next().map_or(1, char::len_utf8);
        }
    }
    found
}

/// Shipping's `prepareQuery` + `findOccurrences` over one text: byte ranges in `text`.
pub fn text_matches(text: &str, query: &str, options: FindOptions) -> Vec<Range<usize>> {
    let query = fold(query.trim(), options.case_sensitive).0;
    if query.is_empty() {
        return Vec::new();
    }
    let (folded, sources) = fold(text, options.case_sensitive);
    occurrences(&folded, &query, options.whole_word)
        .into_iter()
        .map(|range| sources[range.start]..sources[range.end])
        .filter(|range| !range.is_empty())
        .collect()
}

/// Every match in the document's text blocks, in document order, as editor positions.
pub fn document_matches(
    document: &Document,
    query: &str,
    options: FindOptions,
) -> Vec<Range<usize>> {
    fn visit(
        node: &NodeRef,
        start: usize,
        query: &str,
        options: FindOptions,
        found: &mut Vec<Range<usize>>,
    ) {
        for index in 0..node.children.len() {
            let child = node.children.get(index).expect("child");
            let child_start = start + node.children.prefix(index);
            if child.is_textblock() {
                let text = inline_text(&child.children);
                let mut units = 0;
                let mut byte = 0;
                for range in text_matches(&text, query, options) {
                    units += text[byte..range.start].encode_utf16().count();
                    let len = text[range.clone()].encode_utf16().count();
                    found.push(child_start + 1 + units..child_start + 1 + units + len);
                    units += len;
                    byte = range.end;
                }
            } else if child.known() && !child.is_atom() {
                visit(child, child_start + 1, query, options, found);
            }
        }
    }
    let mut found = Vec::new();
    visit(&document.root, 0, query, options, &mut found);
    found
}

/// Shipping's `getTranscriptSearchIndexMatches`: words joined by spaces; each match belongs to
/// the word it starts in (or the next one when it starts on a joining space).
pub fn transcript_matches(words: &[&str], query: &str, options: FindOptions) -> Vec<usize> {
    let mut text = String::new();
    let mut ends = Vec::with_capacity(words.len());
    for word in words {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word);
        ends.push(text.len());
    }
    text_matches(&text, query, options)
        .into_iter()
        .filter_map(|range| {
            let word = ends.partition_point(|end| *end <= range.start);
            (word < words.len()).then_some(word)
        })
        .collect()
}

/// Shipping's `createHighlightSegments`: every whitespace-separated query token found in a
/// transcript word, merged into byte ranges.
pub fn highlight_ranges(text: &str, query: &str, options: FindOptions) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = query
        .split_whitespace()
        .flat_map(|token| text_matches(text, token, options))
        .collect();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Shipping's `.ProseMirror-search-match`. prosemirror-search only marks the match the
/// selection sits on as active, and next/previous never move the selection, so shipping's
/// note shows every match alike.
pub fn editor_match_color(dark: bool) -> Hsla {
    if dark {
        rgba(0x713f1273).into()
    } else {
        rgb(0xfef08a).into()
    }
}

/// The transcript's `bg-yellow-500` current word and `bg-yellow-200/50` other matches.
pub fn transcript_match_color(active: bool) -> Hsla {
    if active {
        rgb(0xeab308).into()
    } else {
        rgba(0xfef08a80).into()
    }
}

/// What the bar searches: the shown memo or summary editor, or the transcript.
#[derive(Clone)]
pub enum FindTarget {
    Editor(WeakEntity<EditorPane>),
    Transcript(WeakEntity<TranscriptView>),
}

impl FindTarget {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Editor(a), Self::Editor(b)) => a.entity_id() == b.entity_id(),
            (Self::Transcript(a), Self::Transcript(b)) => a.entity_id() == b.entity_id(),
            _ => false,
        }
    }

    fn search(&self, query: &str, options: FindOptions, cx: &mut App) {
        match self {
            Self::Editor(editor) => {
                let _ = editor.update(cx, |editor, cx| editor.find(query, options, cx));
            }
            Self::Transcript(transcript) => {
                let _ = transcript.update(cx, |view, cx| view.find(query, options, cx));
            }
        }
    }

    /// The current match and the match count.
    fn status(&self, cx: &mut App) -> (usize, usize) {
        match self {
            Self::Editor(editor) => editor
                .update(cx, |editor, _| editor.find_status())
                .unwrap_or_default(),
            Self::Transcript(transcript) => transcript
                .upgrade()
                .map(|view| view.read(cx).find_status(cx))
                .unwrap_or_default(),
        }
    }

    fn step(&self, forward: bool, cx: &mut App) {
        match self {
            Self::Editor(editor) => {
                let _ = editor.update(cx, |editor, cx| editor.find_step(forward, cx));
            }
            Self::Transcript(transcript) => {
                let _ = transcript.update(cx, |view, cx| view.find_step(forward, cx));
            }
        }
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = match self {
            Self::Editor(editor) => editor.upgrade().map(|editor| editor.focus_handle(cx)),
            Self::Transcript(transcript) => transcript.upgrade().map(|view| view.focus_handle(cx)),
        };
        if let Some(handle) = handle {
            handle.focus(window, cx);
        }
    }
}

/// Shipping's `SearchBar` plus its `SearchProvider` state.
pub struct FindBar {
    query: Entity<TextInput>,
    replacement: Entity<TextInput>,
    options: FindOptions,
    visible: bool,
    show_replace: bool,
    /// Replace is offered on the memo and summary tabs, not the transcript.
    allow_replace: bool,
    target: Option<FindTarget>,
    focus: FocusHandle,
    _target_changes: Option<Subscription>,
    _subscriptions: [Subscription; 1],
}

impl FindBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            TextInput::new("Search...", cx)
                .inline()
                .text_size(px(12.))
                .label("Find")
        });
        let replacement = cx.new(|cx| {
            TextInput::new("Replace with...", cx)
                .inline()
                .text_size(px(12.))
                .label("Replace")
        });
        let subscription = cx.subscribe(&query, |this, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                this.search(cx);
            }
        });
        Self {
            query,
            replacement,
            options: FindOptions::default(),
            visible: false,
            show_replace: false,
            allow_replace: false,
            target: None,
            focus: cx.focus_handle(),
            _target_changes: None,
            _subscriptions: [subscription],
        }
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Points the bar at the shown tab's content; a different target is searched afresh.
    pub fn set_target(
        &mut self,
        target: Option<FindTarget>,
        allow_replace: bool,
        cx: &mut Context<Self>,
    ) {
        self.allow_replace = allow_replace;
        if match (&self.target, &target) {
            (Some(current), Some(next)) => current.same(next),
            (None, None) => true,
            _ => false,
        } {
            return;
        }
        if let Some(previous) = self.target.take() {
            previous.search("", self.options, cx);
        }
        self._target_changes = match &target {
            Some(FindTarget::Editor(editor)) => editor
                .upgrade()
                .map(|editor| cx.observe(&editor, |_, _, cx| cx.notify())),
            Some(FindTarget::Transcript(view)) => view
                .upgrade()
                .map(|view| cx.observe(&view, |_, _, cx| cx.notify())),
            None => None,
        };
        self.target = target;
        if self.visible {
            self.search(cx);
        }
    }

    /// ⌘F: opens the bar, or closes and resets it when it is already open.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible {
            self.dismiss(window, cx);
        } else {
            self.visible = true;
            self.query.focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    /// ⌘H: opens the bar and toggles its replace row.
    pub fn toggle_replace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = true;
        self.show_replace = !self.show_replace;
        let input = if self.show_replace && self.allow_replace {
            &self.replacement
        } else {
            &self.query
        };
        input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Tab and note changes: hides the bar, clears highlights and resets every field.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.show_replace = false;
        self.options = FindOptions::default();
        self.query
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.replacement
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        if let Some(target) = &self.target {
            target.search("", self.options, cx);
        }
        cx.notify();
    }

    /// Esc, ⌘F and the close button: closes, keeping the keyboard in the searched content
    /// so the note's shortcuts still reach it.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
        if let Some(target) = &self.target {
            target.focus(window, cx);
        }
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).buffer.text.clone();
        if let Some(target) = &self.target {
            target.search(&query, self.options, cx);
        }
        cx.notify();
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(target) = &self.target {
            target.step(forward, cx);
        }
        cx.notify();
    }

    fn replace(&mut self, all: bool, cx: &mut Context<Self>) {
        let Some(FindTarget::Editor(editor)) = &self.target else {
            return;
        };
        let replacement = self.replacement.read(cx).buffer.text.clone();
        let _ = editor.update(cx, |editor, cx| editor.replace_found(&replacement, all, cx));
        cx.notify();
    }

    fn toggle_option(&mut self, whole_word: bool, cx: &mut Context<Self>) {
        if whole_word {
            self.options.whole_word = !self.options.whole_word;
        } else {
            self.options.case_sensitive = !self.options.case_sensitive;
        }
        self.search(cx);
    }

    fn button(
        id: &'static str,
        icon: &'static str,
        active: bool,
        disabled: bool,
        colors: Theme,
    ) -> gpui::Stateful<gpui::Div> {
        let color = if disabled {
            colors.muted_foreground.opacity(0.7)
        } else {
            colors.muted_foreground
        };
        let label = match id {
            "find-match-case" => "Match case",
            "find-whole-word" => "Whole word",
            "find-toggle-replace" => "Toggle replace",
            "find-previous" => "Previous match",
            "find-next" => "Next match",
            "find-close" => "Close",
            "find-replace" => "Replace",
            _ => "Replace all",
        };
        div()
            .id(id)
            .a11y_button(label)
            .when(
                matches!(
                    id,
                    "find-match-case" | "find-whole-word" | "find-toggle-replace"
                ),
                |view| view.aria_toggled(crate::ui::a11y::toggled(active)),
            )
            .a11y_disabled(disabled)
            .p(px(2.))
            .flex_shrink_0()
            .when(active, |view| view.bg(colors.accent))
            .when(!disabled, |view| {
                view.cursor_pointer().hover(|view| view.bg(colors.accent))
            })
            .child(svg().path(icon).size(px(14.)).text_color(color))
    }
}

impl Focusable for FindBar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FindBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let (active, total) = self
            .target
            .as_ref()
            .map(|target| target.status(cx))
            .unwrap_or_default();
        let count = if total > 0 {
            format!("{}/{}", active + 1, total)
        } else {
            "0/0".into()
        };
        let row = || {
            div()
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(6.))
                .px_2()
                .bg(colors.muted)
                .text_xs()
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .track_focus(&self.focus)
            .child(
                row()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key == "enter" {
                                    this.step(!event.keystroke.modifiers.shift, cx);
                                    cx.stop_propagation();
                                }
                            }))
                            .child(self.query.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(
                                Self::button(
                                    "find-match-case",
                                    "editor/TextFontIcon.svg",
                                    self.options.case_sensitive,
                                    false,
                                    colors,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.toggle_option(false, cx)),
                                ),
                            )
                            .child(
                                Self::button(
                                    "find-whole-word",
                                    "editor/TextSquareIcon.svg",
                                    self.options.whole_word,
                                    false,
                                    colors,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.toggle_option(true, cx)),
                                ),
                            )
                            .when(self.allow_replace, |view| {
                                view.child(
                                    Self::button(
                                        "find-toggle-replace",
                                        "editor/Exchange01Icon.svg",
                                        self.show_replace,
                                        false,
                                        colors,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.show_replace = !this.show_replace;
                                            if this.show_replace {
                                                this.replacement.focus_handle(cx).focus(window, cx);
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(10.))
                            .text_color(colors.muted_foreground)
                            .child(count),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                Self::button(
                                    "find-previous",
                                    "editor/ArrowUp01Icon.svg",
                                    false,
                                    total == 0,
                                    colors,
                                )
                                .when(total > 0, |view| {
                                    view.on_click(
                                        cx.listener(|this, _, _, cx| this.step(false, cx)),
                                    )
                                }),
                            )
                            .child(
                                Self::button(
                                    "find-next",
                                    "editor/ArrowDown01Icon.svg",
                                    false,
                                    total == 0,
                                    colors,
                                )
                                .when(total > 0, |view| {
                                    view.on_click(cx.listener(|this, _, _, cx| this.step(true, cx)))
                                }),
                            ),
                    )
                    .child(
                        Self::button("find-close", "editor/XIcon.svg", false, false, colors)
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss(window, cx))),
                    ),
            )
            .when(self.allow_replace && self.show_replace, |view| {
                view.child(
                    row()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .capture_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, _, cx| {
                                        if event.keystroke.key == "enter" {
                                            this.replace(
                                                event.keystroke.modifiers.secondary(cx),
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }
                                    },
                                ))
                                .child(self.replacement.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.))
                                .child(
                                    Self::button(
                                        "find-replace",
                                        "editor/Exchange01Icon.svg",
                                        false,
                                        false,
                                        colors,
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.replace(false, cx)),
                                    ),
                                )
                                .child(
                                    Self::button(
                                        "find-replace-all",
                                        "editor/RepeatIcon.svg",
                                        false,
                                        false,
                                        colors,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| this.replace(true, cx))),
                                ),
                        ),
                )
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::editor::model::EditorModel;

    fn options(case_sensitive: bool, whole_word: bool) -> FindOptions {
        FindOptions {
            case_sensitive,
            whole_word,
        }
    }

    #[test]
    fn matching_folds_case_trims_the_query_and_respects_word_boundaries() {
        let text = "Plan the plan; PLANNED planning_x plan";
        let starts = |query: &str, options: FindOptions| -> Vec<usize> {
            text_matches(text, query, options)
                .into_iter()
                .map(|range| range.start)
                .collect()
        };
        assert_eq!(starts("  plan ", options(false, false)), [0, 9, 15, 23, 34]);
        assert_eq!(starts("plan", options(true, false)), [9, 23, 34]);
        assert_eq!(starts("plan", options(false, true)), [0, 9, 34]);
        assert_eq!(starts("plan", options(true, true)), [9, 34]);
        assert!(starts("   ", options(false, false)).is_empty());
        // Folding that changes byte lengths still maps back onto the source text.
        let ranges = text_matches("İstanbul ß", "i̇stanbul", options(false, false));
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 0..9);
    }

    #[test]
    fn document_matches_are_utf16_positions_across_nested_blocks() {
        let document = Document::parse(
            json!({"type":"doc","content":[
                {"type":"paragraph","content":[{"type":"text","text":"😀 Alpha"},
                    {"type":"mention-@","attrs":{"id":"h","type":"human","label":"Alpha"}},
                    {"type":"text","text":" alpha"}]},
                {"type":"bulletList","content":[{"type":"listItem","content":[
                    {"type":"paragraph","content":[{"type":"text","text":"ALPHA"}]}]}]}
            ]})
            .to_string()
            .into(),
        )
        .unwrap();
        let matches = document_matches(&document, "alpha", FindOptions::default());
        // The emoji is two UTF-16 units; the mention is one atom unit and never matches.
        assert_eq!(matches, [4..9, 11..16, 20..25]);
        let mut model = EditorModel::new(document);
        model.replace(matches[2].clone(), "beta").unwrap();
        assert!(
            model
                .document
                .serialize()
                .unwrap()
                .contains("\"text\":\"beta\"")
        );
    }

    #[test]
    fn transcript_matches_belong_to_the_word_they_start_in() {
        let words = ["Hello", "world.", "Hello", "again"];
        assert_eq!(
            transcript_matches(&words, "hello", FindOptions::default()),
            [0, 2]
        );
        // A phrase spanning words belongs to the word it starts in.
        assert_eq!(
            transcript_matches(&words, "world. hello", FindOptions::default()),
            [1]
        );
        assert_eq!(
            highlight_ranges("Hello, hello", "hello lo", FindOptions::default()),
            [0..5, 7..12]
        );
    }
}
