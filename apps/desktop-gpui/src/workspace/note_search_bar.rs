//! Find in note (`note-input/search/context.tsx`, `search/bar.tsx`): the
//! Cmd/Ctrl+F bar under the note header searching the memo / enhanced note
//! textblocks or the transcript words, with match case, whole word, the
//! `n/m` count, previous / next, close, and the Cmd/Ctrl+H replace row for
//! the editor.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, SharedString, Window,
    div, prelude::*, px,
};

use super::{Note, NoteTab, Workspace};
use crate::db::NotePreview;
use crate::editor::SearchSpec;
use crate::note_search::{SearchOptions, highlight_ranges, prepare_query, transcript_matches};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// One `MatchResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NoteMatch {
    /// A transcript word (`id` is `data-word-id`, the `activeMatchId`).
    Word {
        segment_id: String,
        word_index: usize,
        word_id: Option<String>,
    },
    /// An editor block; `getEditorMatches` counts every occurrence.
    Block(usize),
}

pub(super) struct NoteSearch {
    pub session_id: String,
    pub tab: NoteTab,
    pub query: Entity<TextInput>,
    pub replace: Entity<TextInput>,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub show_replace: bool,
    /// `currentMatchIndex`
    pub current: usize,
    pub matches: Vec<NoteMatch>,
    /// The match to scroll into view on the next frame.
    pub scroll_to: Option<usize>,
    /// Hover state of the small buttons, by id.
    pub hovered: Option<&'static str>,
}

impl NoteSearch {
    pub fn options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
        }
    }
}

impl Workspace {
    /// `isSearchableTab`: the memo, an enhanced note, or the transcript.
    fn searchable_tab(tab: &NoteTab) -> bool {
        matches!(
            tab,
            NoteTab::Memo | NoteTab::Enhanced(_) | NoteTab::Transcript
        )
    }

    /// `allowReplace` = `isEditableTab`: the shell edits the memo.
    fn replace_allowed(&self, tab: &NoteTab) -> bool {
        matches!(tab, NoteTab::Memo)
    }

    /// The open bar for the shown note and tab, dropping one left over from
    /// another note or tab (`search?.close()` on tab change / remount).
    pub(super) fn note_search_for(
        &mut self,
        preview: &NotePreview,
        tab: &NoteTab,
        cx: &mut Context<Self>,
    ) -> bool {
        let stale = self
            .note_search
            .as_ref()
            .is_some_and(|search| search.session_id != preview.session.id || search.tab != *tab);
        if stale {
            self.close_note_search(Some(cx));
        }
        self.note_search.is_some()
    }

    /// `mod+f` → `toggle_visible`; `mod+h` → `open_visible` + `toggle_replace`.
    pub(super) fn toggle_note_search(
        &mut self,
        with_replace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Note::Ready { preview, tab } = &self.note else {
            return false;
        };
        if !Self::searchable_tab(tab) {
            return false;
        }
        let (session_id, tab) = (preview.session.id.clone(), tab.clone());
        let replace_allowed = self.replace_allowed(&tab);
        if let Some(search) = self.note_search.as_mut() {
            if !with_replace {
                self.close_note_search(Some(cx));
                return true;
            }
            if replace_allowed {
                search.show_replace = !search.show_replace;
                let focus = if search.show_replace {
                    search.replace.clone()
                } else {
                    search.query.clone()
                };
                focus.read(cx).focus_handle(cx).focus(window);
            }
            cx.notify();
            return true;
        }
        let style = TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        };
        let query = cx.new(|cx| TextInput::new("Search...", style, window, cx).enter_keeps_focus());
        let replace =
            cx.new(|cx| TextInput::new("Replace with...", style, window, cx).enter_keeps_focus());
        cx.subscribe_in(
            &query,
            window,
            |this, _, event: &TextInputEvent, _, cx| match event {
                TextInputEvent::Changed => this.note_search_changed(true, cx),
                TextInputEvent::Enter => this.step_note_search(1, cx),
                TextInputEvent::ShiftEnter => this.step_note_search(-1, cx),
                TextInputEvent::Escape => {
                    this.close_note_search(Some(cx));
                }
                _ => {}
            },
        )
        .detach();
        cx.subscribe_in(
            &replace,
            window,
            |this, _, event: &TextInputEvent, _, cx| match event {
                TextInputEvent::Enter => this.replace_note_search(false, cx),
                TextInputEvent::ModEnter => this.replace_note_search(true, cx),
                TextInputEvent::Escape => {
                    this.close_note_search(Some(cx));
                }
                _ => {}
            },
        )
        .detach();
        let show_replace = with_replace && replace_allowed;
        let focus = if show_replace {
            replace.clone()
        } else {
            query.clone()
        };
        self.note_search = Some(NoteSearch {
            session_id,
            tab,
            query,
            replace,
            case_sensitive: false,
            whole_word: false,
            show_replace,
            current: 0,
            matches: Vec::new(),
            scroll_to: None,
            hovered: None,
        });
        focus.read(cx).focus_handle(cx).focus(window);
        cx.notify();
        true
    }

    /// `close`: back to the initial state; the editor drops its decorations.
    pub(super) fn close_note_search(&mut self, cx: Option<&mut Context<Self>>) -> bool {
        if self.note_search.take().is_none() {
            return false;
        }
        if let Some(cx) = cx {
            if let Some(editor) = &self.editor {
                editor.update(cx, |editor, cx| editor.set_search(None, cx));
            }
            cx.notify();
        }
        true
    }

    /// The query or an option changed: `runSearch` recomputes the matches,
    /// resets to the first and scrolls it into view.
    fn note_search_changed(&mut self, reset: bool, cx: &mut Context<Self>) {
        self.refresh_note_search(cx);
        if let Some(search) = self.note_search.as_mut() {
            if reset {
                search.current = 0;
            }
            search.scroll_to = (!search.matches.is_empty()).then_some(search.current);
        }
        cx.notify();
    }

    /// `getMatchingElements` over the shown tab, and the editor's
    /// `setSearch`.
    pub(super) fn refresh_note_search(&mut self, cx: &mut Context<Self>) {
        let Note::Ready { preview, tab } = &self.note else {
            return;
        };
        let Some(search) = self.note_search.as_ref() else {
            return;
        };
        let raw_query = search.query.read(cx).text().to_string();
        let options = search.options();
        let prepared = prepare_query(&raw_query, options.case_sensitive);
        let matches: Vec<NoteMatch> = if prepared.is_empty() {
            Vec::new()
        } else {
            match tab {
                NoteTab::Transcript => {
                    let mut words: Vec<(&str, String, usize, Option<String>)> = Vec::new();
                    for transcript in &preview.transcripts {
                        for segment in &transcript.segments {
                            for (index, word) in segment.words.iter().enumerate() {
                                words.push((
                                    word.text.as_str(),
                                    segment.id.clone(),
                                    index,
                                    word.id.clone(),
                                ));
                            }
                        }
                    }
                    let texts: Vec<&str> = words.iter().map(|(text, ..)| *text).collect();
                    transcript_matches(&texts, &prepared, options)
                        .into_iter()
                        .map(|index| {
                            let (_, segment_id, word_index, word_id) = &words[index];
                            NoteMatch::Word {
                                segment_id: segment_id.clone(),
                                word_index: *word_index,
                                word_id: word_id.clone(),
                            }
                        })
                        .collect()
                }
                NoteTab::Memo | NoteTab::Enhanced(_) => {
                    self.editor_block_matches(&prepared, options, cx)
                }
            }
        };
        let spec = (!prepared.is_empty() && matches!(tab, NoteTab::Memo)).then_some(SearchSpec {
            query: raw_query,
            case_sensitive: options.case_sensitive,
            whole_word: options.whole_word,
        });
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_search(spec, cx));
        }
        if let Some(search) = self.note_search.as_mut() {
            search.matches = matches;
            if search.current >= search.matches.len() {
                search.current = 0;
            }
        }
    }

    /// `getEditorMatches`: one result per occurrence in each `p, h1-h6, li,
    /// blockquote, td, th` block. A list item's or blockquote's own
    /// `textContent` repeats its paragraph, so those occurrences count twice
    /// like the DOM query does.
    fn editor_block_matches(
        &self,
        prepared: &str,
        options: SearchOptions,
        cx: &gpui::App,
    ) -> Vec<NoteMatch> {
        let Some(editor) = &self.editor else {
            return Vec::new();
        };
        let editor = editor.read(cx);
        let doc = editor.doc();
        let mut matches = Vec::new();
        for block in 0..doc.textblock_count() {
            let text = crate::note_search::prepare_text(&doc.text(block), options.case_sensitive);
            let count =
                crate::note_search::find_occurrences(&text, prepared, options.whole_word).len();
            let repeats = if matches!(
                doc.parent_type(block).as_deref(),
                Some("listItem" | "taskItem" | "blockquote")
            ) {
                2
            } else {
                1
            };
            for _ in 0..count * repeats {
                matches.push(NoteMatch::Block(block));
            }
        }
        matches
    }

    /// `onNext` / `onPrev`
    fn step_note_search(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(search) = self.note_search.as_mut() else {
            return;
        };
        let count = search.matches.len();
        if count == 0 {
            return;
        }
        search.current = (search.current as i32 + delta).rem_euclid(count as i32) as usize;
        search.scroll_to = Some(search.current);
        cx.notify();
    }

    /// `replaceCurrent` / `replaceAll` through the editor.
    fn replace_note_search(&mut self, all: bool, cx: &mut Context<Self>) {
        let Some(search) = self.note_search.as_ref() else {
            return;
        };
        if search.query.read(cx).text().trim().is_empty() || (!all && search.matches.is_empty()) {
            return;
        }
        let replacement = search.replace.read(cx).text().to_string();
        let current = search.current;
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| {
                editor.replace_search(&replacement, all, current, cx)
            });
        }
        self.note_search_changed(false, cx);
    }

    /// The word the search highlights as active (`isActiveMatch`).
    pub(super) fn note_search_active_word(&self) -> Option<(&str, usize)> {
        let search = self.note_search.as_ref()?;
        match search.matches.get(search.current)? {
            NoteMatch::Word {
                segment_id,
                word_index,
                ..
            } => Some((segment_id.as_str(), *word_index)),
            NoteMatch::Block(_) => None,
        }
    }

    /// `createHighlightSegments` for a transcript word, with the colour:
    /// `bg-yellow-200/50`, or `bg-yellow-500` on the active match's word.
    pub(super) fn note_search_word_highlights(
        &self,
        segment_id: &str,
        word_index: usize,
        word_text: &str,
        word_range: std::ops::Range<usize>,
        cx: &gpui::App,
    ) -> Vec<crate::prose_text::Highlight> {
        let Some(search) = self.note_search.as_ref() else {
            return Vec::new();
        };
        if search.tab != NoteTab::Transcript {
            return Vec::new();
        }
        let query = search.query.read(cx).text().trim().to_string();
        if query.is_empty() {
            return Vec::new();
        }
        let active = self.note_search_active_word() == Some((segment_id, word_index));
        let color = if active {
            gpui::rgb(0xeab308)
        } else {
            alpha(gpui::rgb(0xfef08a), 0.5)
        };
        highlight_ranges(word_text, &query, search.options())
            .into_iter()
            .map(|range| crate::prose_text::Highlight {
                range: word_range.start + range.start..word_range.start + range.end,
                color,
                inset_x: px(0.0),
                radius: px(0.0),
            })
            .collect()
    }

    /// The pending `scrollIntoView({ block: "center" })` of a match: the
    /// transcript word's line, or the editor block.
    pub(super) fn apply_note_search_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(search) = self.note_search.as_mut() else {
            return;
        };
        let Some(index) = search.scroll_to else {
            return;
        };
        let Some(target) = search.matches.get(index).cloned() else {
            search.scroll_to = None;
            return;
        };
        match target {
            NoteMatch::Word {
                segment_id,
                word_index,
                ..
            } => {
                let Note::Ready { preview, .. } = &self.note else {
                    return;
                };
                let range = preview
                    .transcripts
                    .iter()
                    .flat_map(|t| t.segments.iter())
                    .find(|segment| segment.id == segment_id)
                    .and_then(|segment| segment.words.get(word_index))
                    .map(|word| word.range.clone());
                let spans = range
                    .and_then(|range| {
                        self.transcript_view
                            .layouts
                            .get(&segment_id)
                            .map(|layout| layout.line_spans(range))
                    })
                    .unwrap_or_default();
                let (Some(first), Some(last)) = (spans.first(), spans.last()) else {
                    // Not laid out yet: retry next frame.
                    return;
                };
                let viewport = self.transcript_view.scroll.bounds();
                if viewport.size.height <= px(0.0) {
                    return;
                }
                if let Some(search) = self.note_search.as_mut() {
                    search.scroll_to = None;
                }
                let center = (first.top() + last.bottom()) / 2.0;
                let viewport_center = viewport.top() + viewport.size.height / 2.0;
                let scroll_top = -self.transcript_view.scroll.offset().y;
                self.smooth_scroll_transcript(scroll_top + (center - viewport_center), cx);
            }
            NoteMatch::Block(block) => {
                let Some(bounds) = self
                    .editor
                    .as_ref()
                    .and_then(|editor| editor.read(cx).block_bounds(block))
                else {
                    return;
                };
                if let Some(search) = self.note_search.as_mut() {
                    search.scroll_to = None;
                }
                let viewport = self.note_scroll.bounds();
                if viewport.size.height <= px(0.0) {
                    return;
                }
                let center = bounds.top() + bounds.size.height / 2.0;
                let viewport_center = viewport.top() + viewport.size.height / 2.0;
                let mut offset = self.note_scroll.offset();
                let max = self.note_scroll.max_offset().height;
                offset.y = (offset.y - (center - viewport_center))
                    .min(px(0.0))
                    .max(-max);
                self.note_scroll.set_offset(offset);
            }
        }
        cx.notify();
    }

    /// `SearchBar` in its `px-3 pt-1` slot.
    pub(super) fn render_note_search_bar(
        &mut self,
        tab: &NoteTab,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.refresh_note_search(cx);
        self.apply_note_search_scroll(cx);
        let search = self.note_search.as_ref()?;
        let theme = self.theme;
        let allow_replace = self.replace_allowed(tab);
        let total = search.matches.len();
        let count = if total > 0 {
            format!("{}/{}", search.current + 1, total)
        } else {
            "0/0".to_string()
        };
        let hovered = search.hovered;
        let can_step = total > 0;
        // `ToggleButton` / `IconButton`: `rounded-sm p-0.5` around a 14px
        // glyph, `bg-accent` while active or hovered, `text-muted-foreground`
        // (`/70` and no hover when disabled).
        let small_button = |id: &'static str, glyph: &'static str, active: bool, disabled: bool| {
            let color = if disabled {
                alpha(theme.muted_foreground, 0.7)
            } else {
                theme.muted_foreground
            };
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .p(px(2.0))
                .rounded(px(4.0))
                .when(active || (!disabled && hovered == Some(id)), |button| {
                    button.bg(theme.accent)
                })
                .when(!disabled, |button| {
                    button.cursor_pointer().on_hover(cx.listener(
                        move |this, hovered: &bool, _, cx| {
                            if let Some(search) = this.note_search.as_mut() {
                                let next = hovered.then_some(id);
                                if search.hovered != next
                                    && (*hovered || search.hovered == Some(id))
                                {
                                    search.hovered = next;
                                    cx.notify();
                                }
                            }
                        },
                    ))
                })
                .when(disabled, |button| button.cursor_not_allowed())
                .child(icon(glyph, px(14.0), color))
        };

        let field = |input: Entity<TextInput>| {
            div()
                .h_full()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .tw_text_xs()
                .child(input)
        };
        let row = || {
            div()
                .relative()
                .flex()
                .h(px(28.0))
                .items_center()
                .gap(px(6.0))
                .rounded(px(crate::squircle::CONTROL_RADIUS))
                .px_2()
                .child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(theme.muted),
                    None,
                ))
        };

        let search_row = row()
            .child(field(search.query.clone()))
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        small_button("note-search-case", "text-aa", search.case_sensitive, false)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(search) = this.note_search.as_mut() {
                                    search.case_sensitive = !search.case_sensitive;
                                }
                                this.note_search_changed(true, cx);
                            })),
                    )
                    .child(
                        small_button("note-search-word", "textbox", search.whole_word, false)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(search) = this.note_search.as_mut() {
                                    search.whole_word = !search.whole_word;
                                }
                                this.note_search_changed(true, cx);
                            })),
                    )
                    .when(allow_replace, |buttons| {
                        buttons.child(
                            small_button("note-search-replace", "swap", search.show_replace, false)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    if let Some(search) = this.note_search.as_mut() {
                                        search.show_replace = !search.show_replace;
                                        if search.show_replace {
                                            search.replace.read(cx).focus_handle(cx).focus(window);
                                        }
                                        cx.notify();
                                    }
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .relative()
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .text_color(theme.muted_foreground)
                    .whitespace_nowrap()
                    .child(SharedString::from(count)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .child(
                        small_button("note-search-prev", "caret-up", false, !can_step).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.step_note_search(-1, cx)
                            }),
                        ),
                    )
                    .child(
                        small_button("note-search-next", "caret-down", false, !can_step).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.step_note_search(1, cx)),
                        ),
                    ),
            )
            .child(div().relative().child(
                small_button("note-search-close", "x", false, false).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.close_note_search(Some(cx));
                    },
                )),
            ));

        let replace_row = (allow_replace && search.show_replace).then(|| {
            row().child(field(search.replace.clone())).child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        small_button("note-search-replace-one", "swap", false, false).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.replace_note_search(false, cx)
                            }),
                        ),
                    )
                    .child(
                        small_button("note-search-replace-all", "repeat", false, false).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.replace_note_search(true, cx)
                            }),
                        ),
                    ),
            )
        });

        Some(
            div()
                .px_3()
                .pt_1()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(search_row)
                        .children(replace_row),
                )
                .into_any_element(),
        )
    }
}
