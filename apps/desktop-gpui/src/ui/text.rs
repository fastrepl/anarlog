use std::{collections::VecDeque, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct TextBuffer {
    pub text: String,
    pub anchor: usize,
    pub cursor: usize,
    pub marked: Option<Range<usize>>,
    history: History,
    composition: Option<Composition>,
    history_disabled: bool,
}

const HISTORY_LIMIT: usize = 256;
const HISTORY_BYTES: usize = 1024 * 1024;

struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before: (usize, usize),
    after: (usize, usize),
}

impl Edit {
    fn bytes(&self) -> usize {
        self.removed.len() + self.inserted.len()
    }
}

#[derive(Default)]
struct History {
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    bytes: usize,
}

impl History {
    fn record(&mut self, edit: Edit) {
        if edit.removed == edit.inserted {
            return;
        }
        self.bytes -= self.redo.iter().map(Edit::bytes).sum::<usize>();
        self.redo.clear();
        self.bytes += edit.bytes();
        self.undo.push_back(edit);
        while self.undo.len() > HISTORY_LIMIT || self.bytes > HISTORY_BYTES {
            if let Some(edit) = self.undo.pop_front() {
                self.bytes -= edit.bytes();
            }
        }
    }
}

struct Composition {
    text: String,
    selection: (usize, usize),
}

impl TextBuffer {
    pub fn new(text: String) -> Self {
        Self {
            anchor: text.len(),
            cursor: text.len(),
            text,
            ..Self::default()
        }
    }

    pub fn disable_history(&mut self) {
        self.history_disabled = true;
        self.history = History::default();
        self.composition = None;
    }

    pub fn undo(&mut self) -> bool {
        if self.marked.is_some() {
            return false;
        }
        let Some(edit) = self.history.undo.pop_back() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.inserted.len(), &edit.removed);
        (self.anchor, self.cursor) = edit.before;
        self.history.redo.push(edit);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.marked.is_some() {
            return false;
        }
        let Some(edit) = self.history.redo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.removed.len(), &edit.inserted);
        (self.anchor, self.cursor) = edit.after;
        self.history.undo.push_back(edit);
        true
    }

    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    pub fn utf8(&self, offset: usize) -> usize {
        utf8(&self.text, offset)
    }

    pub fn utf16(&self, offset: usize) -> usize {
        self.text[..offset].encode_utf16().count()
    }

    pub fn previous(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i < self.cursor)
            .last()
            .unwrap_or(0)
    }

    pub fn next(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > self.cursor)
            .unwrap_or(self.text.len())
    }

    pub fn previous_word(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(start, _)| start)
            .take_while(|start| *start < self.cursor)
            .last()
            .unwrap_or(0)
    }

    pub fn next_word(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(start, word)| start + word.len())
            .find(|end| *end > self.cursor)
            .unwrap_or(self.text.len())
    }

    pub fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }

    pub fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i)
    }

    pub fn word_range(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .find(|(start, word)| {
                offset < start + word.len()
                    || offset == self.text.len() && start + word.len() == offset
            })
            .map_or(offset..offset, |(start, word)| start..start + word.len())
    }

    pub fn unmark(&mut self) {
        self.marked = None;
        if let Some(before) = self.composition.take() {
            let start = before
                .text
                .chars()
                .zip(self.text.chars())
                .take_while(|(left, right)| left == right)
                .map(|(ch, _)| ch.len_utf8())
                .sum::<usize>();
            let suffix = before.text[start..]
                .chars()
                .rev()
                .zip(self.text[start..].chars().rev())
                .take_while(|(left, right)| left == right)
                .map(|(ch, _)| ch.len_utf8())
                .sum::<usize>();
            self.history.record(Edit {
                start,
                removed: before.text[start..before.text.len() - suffix].into(),
                inserted: self.text[start..self.text.len() - suffix].into(),
                before: before.selection,
                after: (self.anchor, self.cursor),
            });
        }
    }

    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: Option<Range<usize>>,
    ) -> bool {
        self.replace_mode(range, text, marked, false)
    }

    pub fn replace_mode(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: Option<Range<usize>>,
        multiline: bool,
    ) -> bool {
        let range = range
            .map(|range| self.utf8(range.start)..self.utf8(range.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        if range.start > range.end {
            return false;
        }
        let text = if multiline {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.replace(['\n', '\r'], " ")
        };
        let limit = if multiline { 128 * 1024 } else { 4096 };
        if self.text.len() - range.len() + text.len() > limit {
            return false;
        }
        if marked.is_some() && self.composition.is_none() && !self.history_disabled {
            self.composition = Some(Composition {
                text: self.text.clone(),
                selection: (self.anchor, self.cursor),
            });
        }
        let edit = (!self.history_disabled && self.composition.is_none()).then(|| Edit {
            start: range.start,
            removed: self.text[range.clone()].into(),
            inserted: text.clone(),
            before: (self.anchor, self.cursor),
            after: (range.start + text.len(), range.start + text.len()),
        });
        self.text.replace_range(range.clone(), &text);
        if let Some(selection) = marked {
            self.marked = Some(range.start..range.start + text.len());
            self.anchor = range.start + utf8(&text, selection.start);
            self.cursor = range.start + utf8(&text, selection.end);
        } else {
            self.marked = None;
            self.cursor = range.start + text.len();
            self.anchor = self.cursor;
            self.unmark();
        }
        if let Some(edit) = edit {
            self.history.record(edit);
        }
        true
    }

    pub fn move_to(&mut self, offset: usize, extend: bool) {
        self.cursor = offset;
        if !extend {
            self.anchor = offset;
        }
    }
}

fn utf8(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (i, ch) in text.char_indices() {
        if units >= offset {
            return i;
        }
        units += ch.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_restores_unicode_text_reversed_selection_and_caret() {
        let mut buffer = TextBuffer::new("a😀éz".into());
        buffer.move_to(7, false);
        buffer.move_to(1, true);
        assert!(buffer.replace(None, "日本", None));
        assert_eq!((buffer.text.as_str(), buffer.cursor), ("a日本z", 7));
        assert!(buffer.undo());
        assert_eq!(buffer.text, "a😀éz");
        assert_eq!((buffer.anchor, buffer.cursor), (7, 1));
        assert!(buffer.redo());
        assert_eq!(buffer.text, "a日本z");
        assert_eq!((buffer.anchor, buffer.cursor), (7, 7));
        assert!(buffer.replace(Some(3..4), "", None));
        assert_eq!(buffer.text, "a日本");
        assert!(buffer.undo());
        assert_eq!((buffer.text.as_str(), buffer.cursor), ("a日本z", 7));
    }

    #[test]
    fn composition_is_one_undo_transaction_for_commit_and_unmark() {
        for explicit_commit in [false, true] {
            let mut buffer = TextBuffer::new("before 😀 after".into());
            buffer.move_to(7, false);
            buffer.move_to(11, true);
            assert!(buffer.replace(None, "に", Some(1..1)));
            assert!(buffer.replace(None, "日本", Some(2..2)));
            assert!(!buffer.undo());
            if explicit_commit {
                assert!(buffer.replace(None, "日本", None));
            } else {
                buffer.unmark();
            }
            assert_eq!(buffer.text, "before 日本 after");
            assert!(buffer.undo());
            assert_eq!(buffer.text, "before 😀 after");
            assert_eq!((buffer.anchor, buffer.cursor), (7, 11));
            assert!(!buffer.undo());
            assert!(buffer.redo());
            assert_eq!(buffer.text, "before 日本 after");
            assert_eq!(buffer.marked, None);
        }
    }

    #[test]
    fn rejected_and_cancelled_edits_preserve_redo_but_new_edits_clear_it() {
        let mut buffer = TextBuffer::new("original".into());
        assert!(buffer.replace(None, " draft", None));
        assert!(buffer.undo());
        assert!(!buffer.replace(None, &"x".repeat(4096), None));
        assert!(buffer.replace(None, "に", Some(1..1)));
        assert!(buffer.replace(None, "", None));
        assert!(buffer.redo());
        assert_eq!(buffer.text, "original draft");
        assert!(buffer.undo());
        assert!(buffer.replace(None, " new", None));
        assert!(!buffer.redo());
        assert_eq!(buffer.text, "original new");
    }

    #[test]
    fn history_bounds_both_edit_count_and_payload() {
        let mut buffer = TextBuffer::default();
        for _ in 0..HISTORY_LIMIT + 10 {
            assert!(buffer.replace(None, "x", None));
        }
        assert_eq!(buffer.history.undo.len(), HISTORY_LIMIT);
        assert_eq!(buffer.history.bytes, HISTORY_LIMIT);
        for _ in 0..HISTORY_LIMIT {
            assert!(buffer.undo());
        }
        assert!(!buffer.undo());
        assert_eq!(buffer.text, "xxxxxxxxxx");
        assert!(buffer.replace(None, "y", None));
        assert_eq!(buffer.history.bytes, 1);
        assert!(!buffer.redo());

        let mut buffer = TextBuffer::default();
        for ch in ['a', 'b'].into_iter().cycle().take(20) {
            buffer.anchor = 0;
            assert!(buffer.replace_mode(None, &ch.to_string().repeat(128 * 1024), None, true));
            assert!(buffer.history.bytes <= HISTORY_BYTES);
        }
        assert!(buffer.history.undo.len() < 20);
        while buffer.undo() {}
        while buffer.redo() {}
        assert_eq!(buffer.text, "b".repeat(128 * 1024));
    }

    #[test]
    fn multiline_history_preserves_normalization_and_disabled_history_stays_empty() {
        let mut buffer = TextBuffer::new("first".into());
        assert!(buffer.replace_mode(None, "\r\n第二\rthird", None, true));
        assert_eq!(buffer.text, "first\n第二\nthird");
        assert!(buffer.undo());
        assert_eq!(buffer.text, "first");
        assert!(buffer.redo());
        assert_eq!(buffer.text, "first\n第二\nthird");
        buffer.disable_history();
        assert!(!buffer.undo());
        assert!(buffer.replace(None, "secret", None));
        assert!(buffer.replace(None, "に", Some(1..1)));
        assert!(buffer.replace(None, "日", None));
        assert_eq!(buffer.history.bytes, 0);
        assert!(buffer.composition.is_none());
        assert!(!buffer.undo());
        assert!(!buffer.redo());
    }

    #[test]
    fn word_navigation_selection_and_deletion_preserve_graphemes() {
        let mut buffer = TextBuffer::new("cafe\u{301}, 😀 東京\nsecond".into());
        buffer.move_to(0, false);
        assert_eq!(buffer.next_word(), "cafe\u{301}".len());
        assert_eq!(buffer.word_range(3), 0.."cafe\u{301}".len());
        buffer.move_to(buffer.next_word(), false);
        assert_eq!(buffer.previous_word(), 0);
        let target = buffer.next_word();
        assert!(buffer.text.is_char_boundary(target));
        assert!(target > "cafe\u{301}, 😀 ".len());
        buffer.move_to(buffer.text.len(), false);
        assert_eq!(buffer.line_start(), "cafe\u{301}, 😀 東京\n".len());
        assert_eq!(buffer.previous_word(), buffer.line_start());
        assert_eq!(
            buffer.word_range(buffer.cursor),
            buffer.line_start()..buffer.text.len()
        );
        let original = buffer.text.clone();
        assert!(buffer.replace(
            Some(buffer.utf16(buffer.previous_word())..buffer.utf16(buffer.cursor)),
            "",
            None
        ));
        assert!(buffer.text.ends_with('\n'));
        assert!(buffer.undo());
        assert_eq!(buffer.text, original);
        assert_eq!(buffer.cursor, original.len());
    }

    #[test]
    fn multiline_preserves_unicode_newlines_and_composition() {
        let mut buffer = TextBuffer::default();
        assert!(buffer.replace_mode(None, "日本😀\r\nsecond\n", None, true));
        assert_eq!(buffer.text, "日本😀\nsecond\n");
        assert!(buffer.replace_mode(Some(0..4), "試\n験", Some(1..2), true));
        assert_eq!(buffer.marked, Some(0..7));
        assert_eq!(buffer.selection(), 3..4);
        assert!(buffer.replace_mode(None, "試験", None, true));
        assert_eq!(buffer.text, "試験\nsecond\n");
        let before = buffer.text.clone();
        assert!(!buffer.replace_mode(Some(Range { start: 6, end: 2 }), "x", None, true));
        assert_eq!(buffer.text, before);
        assert!(buffer.replace_mode(None, &"x".repeat(8000), None, true));
        assert!(!buffer.replace_mode(None, &"x".repeat(128 * 1024), None, true));
    }

    #[test]
    fn composition_ranges_are_relative_to_inserted_text() {
        let mut buffer = TextBuffer {
            text: "ab😀cd".into(),
            anchor: 6,
            cursor: 6,
            marked: None,
            ..TextBuffer::default()
        };
        buffer.replace(None, "日本😀", Some(2..4));
        assert_eq!(buffer.selection(), 12..16);
        assert_eq!(buffer.marked, Some(6..16));
        buffer.replace(None, "日本語", None);
        assert_eq!(buffer.text, "ab😀日本語cd");
        assert_eq!(buffer.cursor, 15);
    }

    #[test]
    fn grapheme_delete_preserves_combining_sequences_and_utf16_offsets() {
        let mut buffer = TextBuffer {
            text: "e\u{301}😀".into(),
            anchor: 7,
            cursor: 7,
            marked: None,
            ..TextBuffer::default()
        };
        assert_eq!(buffer.previous(), 3);
        assert_eq!(buffer.utf16(7), 4);
        buffer.anchor = buffer.previous();
        buffer.replace(None, "", None);
        assert_eq!(buffer.text, "e\u{301}");
        assert_eq!(buffer.previous(), 0);
    }
}
