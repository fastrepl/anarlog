//! Writing a dictation into the captured field, and telling afterwards what the write did.
//!
//! Every write is followed by a read-back of the same field: `Applied` when the field now
//! holds the text where it was meant to go, `Unchanged` only when the field provably holds
//! what it held before (or the clipboard promise was revoked unread), `Uncertain` otherwise.
//! A write that may have landed is never attempted again. The ladder (direct write, then a
//! clipboard paste over the same selection) and the continuation/raw-restoration rules follow
//! Char's `ax-text-edit` and `voice-input.ts` (reference only); unlike Char, a direct write the
//! app reports as done is not trusted without a read-back.

use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
};

use dictation_core::{corrections::FieldText, fit_to_caret};

use super::{InsertMiss, InsertMode, InsertOutcome};

/// How much of the field on each side of the selection a read returns, in UTF-16 units.
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub const WINDOW_UTF16: usize = 4_000;
const SETTLE_READS: usize = 8;
const KEPT_EDITS: usize = 8;

/// A UTF-16 range of the field's whole value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub len: usize,
}

/// The field around its selection, as read now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRead {
    /// The whole value's length in UTF-16 units.
    pub length: usize,
    pub selection: Span,
    /// Up to `WINDOW_UTF16` units ending at the selection.
    pub before: String,
    pub selected: String,
    /// Up to `WINDOW_UTF16` units starting after the selection.
    pub after: String,
}

impl FieldRead {
    pub fn fingerprint(&self) -> String {
        let mut hasher = DefaultHasher::new();
        (
            self.length,
            self.selection.start,
            self.selection.len,
            &self.before,
            &self.selected,
            &self.after,
        )
            .hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    pub fn text(&self) -> FieldText {
        FieldText {
            before: self.before.clone(),
            selected: self.selected.clone(),
            after: self.after.clone(),
        }
    }

    /// The window as UTF-16 units, with the absolute offset of its first unit.
    fn view(&self) -> (usize, Vec<u16>) {
        let units: Vec<u16> = self
            .before
            .encode_utf16()
            .chain(self.selected.encode_utf16())
            .chain(self.after.encode_utf16())
            .map(normalize)
            .collect();
        (
            self.selection.start.saturating_sub(utf16(&self.before)),
            units,
        )
    }
}

// Chromium's editors turn a typed space into a no-break space next to other whitespace.
fn normalize(unit: u16) -> u16 {
    if unit == 0x00a0 { 0x20 } else { unit }
}

pub fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// The platform reports the write as done; the read-back decides.
    Accepted,
    /// The platform refused it.
    Rejected,
    /// Sent, but the app never answered; it may still land, so only the read-back can tell.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Unconfirmed,
    /// Only part of it went through (typed keys); it may have landed in part.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paste {
    /// The keystroke was sent and the app read the promised text.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Delivered,
    /// The app never read the promised text and the promise was revoked: it cannot land now.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Revoked,
    /// No keystroke was sent.
    NotSent,
}

/// One captured field. Implementations re-check the captured element itself, never whatever
/// happens to be focused now.
pub trait FieldPort {
    /// The captured field is still the focused, editable field of the frontmost app.
    fn focused(&self) -> bool;
    fn read(&self) -> Option<FieldRead>;
    fn select(&self, span: Span) -> bool;
    fn write(&self, text: &str) -> Write;
    fn paste(&self, text: &str) -> Paste;
    /// The read-back shows the app acted on the delivered paste, so it is done reading it.
    fn paste_consumed(&self) {}
    /// Waits a moment before the next read-back.
    fn settle(&self);
    /// Waits briefly for keys still held from the shortcut, which typed text would combine
    /// with. False when they stay held.
    fn keys_released(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
struct Edit {
    id: String,
    applied: bool,
    typed: String,
    /// What the write went over: a joined sentence's period, or nothing.
    replaced: String,
    /// The field around the caret just before the write, for learning.
    start: FieldText,
}

/// The writes made into one captured field, so an edit is never repeated and can be restored.
#[derive(Debug, Default)]
pub struct History {
    edits: VecDeque<Edit>,
}

impl History {
    fn find(&self, id: &str) -> Option<&Edit> {
        self.edits.iter().find(|edit| edit.id == id)
    }

    fn record(&mut self, edit: Edit) {
        self.edits.push_front(edit);
        self.edits.truncate(KEPT_EDITS);
    }

    /// The field as it was around the caret just before an applied edit.
    pub fn start_of(&self, id: &str) -> Option<&FieldText> {
        self.find(id)
            .filter(|edit| edit.applied)
            .map(|edit| &edit.start)
    }
}

/// Validates dictation text before anything is written.
pub fn validate(text: &str) -> Result<(), String> {
    if text.contains('\0') {
        return Err("Dictation text contains an unsupported NUL character".into());
    }
    if text.is_empty() || text.len() > 100_000 {
        return Err("Dictation text is empty or too long".into());
    }
    Ok(())
}

/// Writes `text` as `mode` asks. `fitting`: the text around the caret may be read on this device
/// to fit the dictation (spacing, casing, joining a sentence); otherwise it goes in as given.
///
/// `live` is asked right before every write and selection change. Once the dictation ended,
/// nothing more is written (`Retired`); a selection this call moved is put back, and a write
/// already sent is still read back and never repeated.
pub fn insert(
    port: &impl FieldPort,
    history: &mut History,
    edit_id: &str,
    text: &str,
    mode: &InsertMode,
    fitting: bool,
    live: &dyn Fn() -> bool,
) -> InsertOutcome {
    if history.find(edit_id).is_some() {
        return unchanged(InsertMiss::Consumed);
    }
    if !live() {
        return unchanged(InsertMiss::Retired);
    }
    if !port.focused() {
        return unchanged(InsertMiss::FocusMoved);
    }
    if !fitting {
        return insert_blind(port, history, edit_id, text, mode, live);
    }
    let Some(read) = port.read() else {
        return insert_blind(port, history, edit_id, text, mode, live);
    };
    let (span, mut typed, mut replaced) = match plan(history, &read, text, mode, fitting) {
        Ok(plan) => plan,
        Err(miss) => return unchanged(miss),
    };
    let mut outcome = write_over(port, &read, span, &typed, live);
    let refused = outcome
        == InsertOutcome::Unchanged {
            reason: InsertMiss::Refused,
        };
    // A join that could not go over the period is a sentence of its own after all.
    if refused
        && replaced == "."
        && let Some(now) = port.read().filter(|now| *now == read)
    {
        typed = fit_to_caret(text, fitted_before(&read, fitting), false).text;
        replaced = String::new();
        outcome = write_over(port, &now, now.selection, &typed, live);
    }
    record(history, edit_id, &outcome, &typed, &replaced, Some(&read));
    with_text(outcome, typed)
}

/// A field that can't be read takes plain dictation only.
fn insert_blind(
    port: &impl FieldPort,
    history: &mut History,
    edit_id: &str,
    text: &str,
    mode: &InsertMode,
    live: &dyn Fn() -> bool,
) -> InsertOutcome {
    let typed = match mode {
        InsertMode::Caret => text.to_owned(),
        InsertMode::Fit { .. } => fit_to_caret(text, None, false).text,
        InsertMode::ReplaceSelection { .. } | InsertMode::Restore { .. } => {
            return unchanged(InsertMiss::ContentChanged);
        }
    };
    let outcome = write_blind(port, &typed, live);
    record(history, edit_id, &outcome, &typed, "", None);
    with_text(outcome, typed)
}

fn record(
    history: &mut History,
    edit_id: &str,
    outcome: &InsertOutcome,
    typed: &str,
    replaced: &str,
    read: Option<&FieldRead>,
) {
    if matches!(outcome, InsertOutcome::Unchanged { .. }) {
        return;
    }
    // Learning compares against the field without this dictation, a joined period included.
    let start = read.map_or_else(FieldText::default, |read| FieldText {
        before: read.before[..read.before.len() - replaced.len().min(read.before.len())].into(),
        selected: String::new(),
        after: read.after.clone(),
    });
    history.record(Edit {
        id: edit_id.into(),
        applied: matches!(outcome, InsertOutcome::Applied { .. }),
        typed: typed.into(),
        replaced: replaced.into(),
        start,
    });
}

fn with_text(outcome: InsertOutcome, typed: String) -> InsertOutcome {
    match outcome {
        InsertOutcome::Applied { verified, .. } => InsertOutcome::Applied {
            verified,
            text: typed,
        },
        other => other,
    }
}

fn fitted_before(read: &FieldRead, fitting: bool) -> Option<&str> {
    fitting.then_some(read.before.as_str())
}

/// Where the text goes, what is typed there, and what it goes over (a joined period).
fn plan(
    history: &History,
    read: &FieldRead,
    text: &str,
    mode: &InsertMode,
    fitting: bool,
) -> Result<(Span, String, String), InsertMiss> {
    match mode {
        InsertMode::Caret => Ok((read.selection, text.into(), String::new())),
        InsertMode::Fit { can_join } => {
            let collapsed = read.selection.len == 0;
            let fit = fit_to_caret(text, fitted_before(read, fitting), *can_join && collapsed);
            if fit.join && read.before.ends_with('.') && read.selection.start > 0 {
                let period = Span {
                    start: read.selection.start - 1,
                    len: 1,
                };
                return Ok((period, fit.text, ".".into()));
            }
            let fit = if fit.join {
                fit_to_caret(text, fitted_before(read, fitting), false)
            } else {
                fit
            };
            Ok((read.selection, fit.text, String::new()))
        }
        InsertMode::ReplaceSelection { fingerprint } => {
            if read.selection.len == 0 || read.fingerprint() != *fingerprint {
                return Err(InsertMiss::ContentChanged);
            }
            Ok((read.selection, text.into(), String::new()))
        }
        InsertMode::Restore { edit_id } => {
            let edit = history
                .find(edit_id)
                .filter(|edit| edit.applied)
                .ok_or(InsertMiss::ContentChanged)?;
            if read.selection.len != 0
                || edit.typed.is_empty()
                || !read.before.ends_with(&edit.typed)
            {
                return Err(InsertMiss::ContentChanged);
            }
            // The raw words go back after what the edit typed over, a joined period included.
            let kept = &read.before[..read.before.len() - edit.typed.len()];
            let before = format!("{kept}{}", edit.replaced);
            let raw = fit_to_caret(text, fitting.then_some(before.as_str()), false);
            let typed = utf16(&edit.typed);
            let span = Span {
                start: read.selection.start - typed,
                len: typed,
            };
            Ok((
                span,
                format!("{}{}", edit.replaced, raw.text),
                String::new(),
            ))
        }
    }
}

fn unchanged(reason: InsertMiss) -> InsertOutcome {
    InsertOutcome::Unchanged { reason }
}

fn applied(verified: bool) -> InsertOutcome {
    InsertOutcome::Applied {
        verified,
        text: String::new(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Applied,
    Unchanged,
    Other,
    Unreadable,
}

/// What the field shows now, against `pre` (read just before writing `text` over `span`).
fn classify(pre: &FieldRead, span: Span, text: &str, now: Option<&FieldRead>) -> Seen {
    let Some(now) = now else {
        return Seen::Unreadable;
    };
    let inserted = utf16(text);
    let (now_start, now_units) = now.view();
    let (pre_start, pre_units) = pre.view();
    if now.length == pre.length - span.len + inserted {
        let mut expected: Vec<u16> = pre_units[..span.start - pre_start].to_vec();
        expected.extend(text.encode_utf16().map(normalize));
        expected.extend_from_slice(&pre_units[span.start - pre_start + span.len..]);
        if covers_equal(
            (pre_start, &expected),
            (now_start, &now_units),
            span.start,
            span.start + inserted,
        ) {
            return Seen::Applied;
        }
    }
    // A field that ignored a write may still have moved its caret; content decides.
    if now.length == pre.length
        && covers_equal(
            (pre_start, &pre_units),
            (now_start, &now_units),
            span.start,
            span.start + span.len,
        )
    {
        return Seen::Unchanged;
    }
    Seen::Other
}

/// Both views cover `[from, to)` and agree wherever they overlap.
fn covers_equal(a: (usize, &[u16]), b: (usize, &[u16]), from: usize, to: usize) -> bool {
    let start = a.0.max(b.0);
    let end = (a.0 + a.1.len()).min(b.0 + b.1.len());
    if start > from || end < to || start > end {
        return false;
    }
    a.1[start - a.0..end - a.0] == b.1[start - b.0..end - b.0]
}

fn settled(port: &impl FieldPort, pre: &FieldRead, span: Span, text: &str) -> Seen {
    let mut seen = Seen::Unreadable;
    for read in 0..SETTLE_READS {
        if read > 0 {
            port.settle();
        }
        seen = classify(pre, span, text, port.read().as_ref());
        if seen != Seen::Unchanged {
            break;
        }
    }
    seen
}

/// Selects `span`, provided the field still reads as `read`, and writes over it.
fn write_over(
    port: &impl FieldPort,
    read: &FieldRead,
    span: Span,
    text: &str,
    live: &dyn Fn() -> bool,
) -> InsertOutcome {
    if !live() {
        return unchanged(InsertMiss::Retired);
    }
    let pre = if span == read.selection {
        read.clone()
    } else {
        match select(port, read, span) {
            Some(pre) => pre,
            None => return nothing_written(port, read, InsertMiss::Refused),
        }
    };
    if !live() {
        return nothing_written(port, read, InsertMiss::Retired);
    }
    match port.write(text) {
        Write::Accepted => match settled(port, &pre, span, text) {
            Seen::Applied => return applied(true),
            Seen::Unreadable => return applied(false),
            Seen::Other => return InsertOutcome::Uncertain,
            // A successful write may appear after these reads; another paste could duplicate it.
            Seen::Unchanged => return InsertOutcome::Uncertain,
        },
        Write::Unconfirmed => match settled(port, &pre, span, text) {
            Seen::Applied => return applied(true),
            Seen::Unchanged | Seen::Other | Seen::Unreadable => return InsertOutcome::Uncertain,
        },
        Write::Partial => return InsertOutcome::Uncertain,
        Write::Rejected => match classify(&pre, span, text, port.read().as_ref()) {
            Seen::Applied => return applied(true),
            Seen::Other => return InsertOutcome::Uncertain,
            Seen::Unchanged | Seen::Unreadable => {}
        },
    }
    // Nothing landed, so the paste would be a second write: only while the dictation is live.
    if !live() {
        return nothing_written(port, read, InsertMiss::Retired);
    }
    // A refused write can still move the selection; paste only over `span`.
    if port.read().is_some_and(|now| now.selection != span) && select(port, &pre, span).is_none() {
        return nothing_written(port, read, InsertMiss::Refused);
    }
    if !port.focused() {
        return nothing_written(port, read, InsertMiss::FocusMoved);
    }
    if !live() {
        return nothing_written(port, read, InsertMiss::Retired);
    }
    match port.paste(text) {
        Paste::Delivered => {
            let seen = settled(port, &pre, span, text);
            // Until the field changes the app may still be reading the clipboard.
            if matches!(seen, Seen::Applied | Seen::Other) {
                port.paste_consumed();
            }
            match seen {
                Seen::Applied => applied(true),
                Seen::Unreadable => applied(false),
                Seen::Unchanged | Seen::Other => InsertOutcome::Uncertain,
            }
        }
        Paste::Revoked | Paste::NotSent => match classify(&pre, span, text, port.read().as_ref()) {
            Seen::Unchanged | Seen::Unreadable => nothing_written(port, read, InsertMiss::Refused),
            Seen::Applied => applied(true),
            Seen::Other => InsertOutcome::Uncertain,
        },
    }
}

/// Moves the selection to `span` and confirms the field otherwise reads as before.
fn select(port: &impl FieldPort, read: &FieldRead, span: Span) -> Option<FieldRead> {
    if !port.select(span) {
        return None;
    }
    let now = port.read()?;
    let (start, units) = read.view();
    let (now_start, now_units) = now.view();
    (now.selection == span
        && now.length == read.length
        && covers_equal(
            (start, &units),
            (now_start, &now_units),
            span.start,
            span.start + span.len,
        ))
    .then_some(now)
}

/// Only reached once none of the text can have landed: puts the caret back where the person
/// left it, if a step moved it.
fn nothing_written(port: &impl FieldPort, read: &FieldRead, reason: InsertMiss) -> InsertOutcome {
    if port
        .read()
        .is_some_and(|now| now.selection != read.selection && now.length == read.length)
    {
        port.select(read.selection);
    }
    unchanged(reason)
}

/// Without read-back, prefer an acknowledged paste: some apps accept direct writes but ignore
/// them. A revoked or unsent paste cannot land, so only then try a direct write.
fn write_blind(port: &impl FieldPort, text: &str, live: &dyn Fn() -> bool) -> InsertOutcome {
    if !live() {
        return unchanged(InsertMiss::Retired);
    }
    if port.paste(text) == Paste::Delivered {
        return applied(false);
    }
    // Typing may first wait for held keys, so the dictation is checked again right before it.
    if !port.keys_released() {
        return unchanged(InsertMiss::Refused);
    }
    if !live() {
        return unchanged(InsertMiss::Retired);
    }
    match port.write(text) {
        Write::Accepted => applied(false),
        Write::Rejected => unchanged(InsertMiss::Refused),
        Write::Unconfirmed | Write::Partial => InsertOutcome::Uncertain,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use std::{
        cell::{Cell, RefCell},
        sync::mpsc,
    };

    use super::*;

    impl<P: FieldPort> FieldPort for &P {
        fn focused(&self) -> bool {
            (**self).focused()
        }
        fn read(&self) -> Option<FieldRead> {
            (**self).read()
        }
        fn select(&self, span: Span) -> bool {
            (**self).select(span)
        }
        fn write(&self, text: &str) -> Write {
            (**self).write(text)
        }
        fn paste(&self, text: &str) -> Paste {
            (**self).paste(text)
        }
        fn paste_consumed(&self) {
            (**self).paste_consumed()
        }
        fn settle(&self) {}
        fn keys_released(&self) -> bool {
            (**self).keys_released()
        }
    }

    impl Span {
        fn caret(at: usize) -> Self {
            Self { start: at, len: 0 }
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Direct {
        Applies,
        /// Reports success, but applies the write after read-back has finished.
        Delays,
        Rejects,
        /// Reports success and types something else (autocorrect).
        Mangles,
        /// Times out unanswered, then applies the write after read-back has finished.
        Unanswered,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Paster {
        Applies,
        /// Reads the clipboard and drops it.
        Drops,
        NeverReads,
        /// Still reading when first read back; pastes nothing if the clipboard goes back first.
        Slow,
    }

    pub(in crate::insertion) struct Fake {
        value: RefCell<Vec<u16>>,
        selection: Cell<Span>,
        focused: Cell<bool>,
        readable: bool,
        deny_reads: bool,
        selectable: bool,
        direct: Direct,
        paster: Paster,
        writes: Cell<usize>,
        /// Direct writes and pastes sent, whatever they did.
        attempts: Cell<usize>,
        pending: RefCell<Option<String>>,
        /// A slow paste the app has not finished reading.
        reading: RefCell<Option<String>>,
        cancel_on_refocus: bool,
        cancelled: Cell<bool>,
        /// A direct write reports itself here, then waits until it is let through.
        gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    }

    impl Fake {
        fn new(text: &str, selection: Span) -> Self {
            Self {
                value: RefCell::new(text.encode_utf16().collect()),
                selection: Cell::new(selection),
                focused: Cell::new(true),
                readable: true,
                deny_reads: false,
                selectable: true,
                direct: Direct::Applies,
                paster: Paster::Applies,
                writes: Cell::new(0),
                attempts: Cell::new(0),
                pending: RefCell::new(None),
                reading: RefCell::new(None),
                cancel_on_refocus: false,
                cancelled: Cell::new(false),
                gate: None,
            }
        }

        pub(in crate::insertion) fn at_end(text: &str) -> Self {
            Self::new(text, Span::caret(utf16(text)))
        }

        pub(in crate::insertion) fn gated(
            text: &str,
            entered: mpsc::Sender<()>,
            released: mpsc::Receiver<()>,
        ) -> Self {
            Self {
                gate: Some((entered, released)),
                ..Self::at_end(text)
            }
        }

        pub(in crate::insertion) fn text(&self) -> String {
            String::from_utf16(&self.value.borrow()).unwrap()
        }

        fn type_over_selection(&self, text: &str) {
            let span = self.selection.get();
            let units: Vec<u16> = text.encode_utf16().collect();
            let len = units.len();
            self.value
                .borrow_mut()
                .splice(span.start..span.start + span.len, units);
            self.selection.set(Span::caret(span.start + len));
            self.writes.set(self.writes.get() + 1);
        }
    }

    impl FieldPort for Fake {
        fn focused(&self) -> bool {
            if self.cancel_on_refocus && self.attempts.get() > 0 {
                self.cancelled.set(true);
            }
            self.focused.get()
        }

        fn read(&self) -> Option<FieldRead> {
            assert!(!self.deny_reads, "privacy denied reading the field");
            if !self.readable {
                return None;
            }
            if let Some(text) = self.reading.borrow_mut().take() {
                self.type_over_selection(&text);
            }
            let value = self.value.borrow();
            let span = self.selection.get();
            let from = span.start.saturating_sub(WINDOW_UTF16);
            let to = (span.start + span.len + WINDOW_UTF16).min(value.len());
            Some(FieldRead {
                length: value.len(),
                selection: span,
                before: String::from_utf16(&value[from..span.start]).unwrap(),
                selected: String::from_utf16(&value[span.start..span.start + span.len]).unwrap(),
                after: String::from_utf16(&value[span.start + span.len..to]).unwrap(),
            })
        }

        fn select(&self, span: Span) -> bool {
            if self.selectable && span.start + span.len <= self.value.borrow().len() {
                self.selection.set(span);
            }
            self.selectable
        }

        fn write(&self, text: &str) -> Write {
            self.attempts.set(self.attempts.get() + 1);
            if let Some((entered, released)) = &self.gate {
                entered.send(()).unwrap();
                released.recv().unwrap();
            }
            match self.direct {
                Direct::Applies => {
                    self.type_over_selection(text);
                    Write::Accepted
                }
                Direct::Delays => {
                    *self.pending.borrow_mut() = Some(text.to_owned());
                    Write::Accepted
                }
                Direct::Rejects => Write::Rejected,
                Direct::Mangles => {
                    self.type_over_selection(&text.replace('\'', "’"));
                    Write::Accepted
                }
                Direct::Unanswered => {
                    *self.pending.borrow_mut() = Some(text.to_owned());
                    Write::Unconfirmed
                }
            }
        }

        fn paste(&self, text: &str) -> Paste {
            self.attempts.set(self.attempts.get() + 1);
            match self.paster {
                Paster::Applies => {
                    self.type_over_selection(text);
                    Paste::Delivered
                }
                Paster::Drops => Paste::Delivered,
                Paster::NeverReads => Paste::Revoked,
                Paster::Slow => {
                    *self.reading.borrow_mut() = Some(text.to_owned());
                    Paste::Delivered
                }
            }
        }

        fn paste_consumed(&self) {
            self.reading.borrow_mut().take();
        }

        fn settle(&self) {}
    }

    fn caret() -> InsertMode {
        InsertMode::Caret
    }

    fn fit(can_join: bool) -> InsertMode {
        InsertMode::Fit { can_join }
    }

    fn run(
        field: &Fake,
        history: &mut History,
        id: &str,
        text: &str,
        mode: InsertMode,
    ) -> InsertOutcome {
        insert(field, history, id, text, &mode, true, &|| true)
    }

    #[test]
    fn a_verified_direct_write_is_applied() {
        let field = Fake::at_end("Hello");
        let outcome = run(&field, &mut History::default(), "a", " world", caret());
        assert_eq!(
            outcome,
            InsertOutcome::Applied {
                verified: true,
                text: " world".into()
            }
        );
        assert_eq!(field.text(), "Hello world");
    }

    #[test]
    fn a_write_that_appears_late_is_never_pasted_or_repeated() {
        let field = Fake {
            direct: Direct::Delays,
            ..Fake::at_end("Hello")
        };
        let mut history = History::default();
        // Cancelled once the write was sent: it may still land, so it is not "unchanged".
        let outcome = insert(&field, &mut history, "a", " world", &caret(), true, &|| {
            field.attempts.get() == 0
        });
        let delayed = field.pending.borrow_mut().take().unwrap();
        field.type_over_selection(&delayed);
        assert_eq!(field.text(), "Hello world");
        assert_eq!(outcome, InsertOutcome::Uncertain);
        assert_eq!(
            run(&field, &mut history, "a", " world", caret()),
            unchanged(InsertMiss::Consumed)
        );
        assert_eq!(field.text(), "Hello world");
    }

    #[test]
    fn a_direct_write_the_app_never_answered_is_never_pasted_or_repeated() {
        let field = Fake {
            direct: Direct::Unanswered,
            ..Fake::at_end("Hello")
        };
        let mut history = History::default();
        assert_eq!(
            run(&field, &mut history, "a", " world", caret()),
            InsertOutcome::Uncertain
        );
        let late = field.pending.borrow_mut().take().unwrap();
        field.type_over_selection(&late);
        assert_eq!(
            run(&field, &mut history, "a", " world", caret()),
            unchanged(InsertMiss::Consumed)
        );
        assert_eq!(field.text(), "Hello world");
    }

    #[test]
    fn a_delivered_paste_keeps_the_clipboard_until_the_field_shows_it() {
        let field = Fake {
            direct: Direct::Rejects,
            paster: Paster::Slow,
            ..Fake::at_end("Hello")
        };
        assert_eq!(
            run(&field, &mut History::default(), "a", " world", caret()),
            InsertOutcome::Applied {
                verified: true,
                text: " world".into()
            }
        );
        assert_eq!(field.text(), "Hello world");
    }

    #[test]
    fn a_dictation_cancelled_while_its_direct_write_is_refused_is_never_pasted() {
        for cancel_on_refocus in [false, true] {
            let field = Fake {
                direct: Direct::Rejects,
                cancel_on_refocus,
                ..Fake::at_end("Hello")
            };
            let outcome = insert(
                &field,
                &mut History::default(),
                "a",
                " world",
                &caret(),
                true,
                &|| {
                    if cancel_on_refocus {
                        !field.cancelled.get()
                    } else {
                        field.attempts.get() == 0
                    }
                },
            );
            assert_eq!(outcome, unchanged(InsertMiss::Retired));
            assert_eq!(field.text(), "Hello");
        }
    }

    #[test]
    fn a_paste_the_app_read_but_did_not_show_is_uncertain_and_never_repeated() {
        let field = Fake {
            direct: Direct::Rejects,
            paster: Paster::Drops,
            ..Fake::at_end("Hello")
        };
        let mut history = History::default();
        assert_eq!(
            run(&field, &mut history, "a", " world", caret()),
            InsertOutcome::Uncertain
        );
        let mut retry = Fake {
            paster: Paster::Applies,
            ..Fake::at_end("Hello")
        };
        retry.direct = Direct::Applies;
        assert_eq!(
            run(&retry, &mut history, "a", " world", caret()),
            InsertOutcome::Unchanged {
                reason: InsertMiss::Consumed
            }
        );
        assert_eq!(retry.text(), "Hello");
    }

    #[test]
    fn a_revoked_paste_into_an_unchanged_field_is_definitely_unchanged() {
        let field = Fake {
            direct: Direct::Rejects,
            paster: Paster::NeverReads,
            ..Fake::at_end("Hello")
        };
        let mut history = History::default();
        assert_eq!(
            run(&field, &mut history, "a", " world", caret()),
            InsertOutcome::Unchanged {
                reason: InsertMiss::Refused
            }
        );
        // Provably nothing landed, so the same edit may be tried again.
        let field = Fake::at_end("Hello");
        assert!(matches!(
            run(&field, &mut history, "a", " world", caret()),
            InsertOutcome::Applied { .. }
        ));
    }

    #[test]
    fn a_write_that_typed_something_else_is_uncertain_not_retried() {
        let field = Fake {
            direct: Direct::Mangles,
            ..Fake::at_end("")
        };
        assert_eq!(
            run(&field, &mut History::default(), "a", "it's", caret()),
            InsertOutcome::Uncertain
        );
        assert_eq!(field.text(), "it’s");
        assert_eq!(field.writes.get(), 1);
    }

    #[test]
    fn nothing_is_written_once_the_captured_field_lost_focus() {
        let field = Fake::at_end("Hello");
        field.focused.set(false);
        assert_eq!(
            run(&field, &mut History::default(), "a", " world", caret()),
            InsertOutcome::Unchanged {
                reason: InsertMiss::FocusMoved
            }
        );
        assert_eq!(field.text(), "Hello");
    }

    #[test]
    fn a_voice_edit_needs_the_selection_it_was_made_for() {
        let field = Fake::new("Send the report now", Span { start: 9, len: 6 });
        let fingerprint = field.read().unwrap().fingerprint();
        field.selection.set(Span { start: 0, len: 4 });
        let mut history = History::default();
        let edit = |fingerprint: &str| InsertMode::ReplaceSelection {
            fingerprint: fingerprint.into(),
        };
        assert_eq!(
            run(&field, &mut history, "a", "summary", edit(&fingerprint)),
            InsertOutcome::Unchanged {
                reason: InsertMiss::ContentChanged
            }
        );
        field.selection.set(Span { start: 9, len: 6 });
        assert!(matches!(
            run(&field, &mut history, "b", "summary", edit(&fingerprint)),
            InsertOutcome::Applied { verified: true, .. }
        ));
        assert_eq!(field.text(), "Send the summary now");
    }

    #[test]
    fn fitting_spaces_after_a_word_and_joins_a_finished_sentence() {
        let field = Fake::at_end("It works.");
        let outcome = run(
            &field,
            &mut History::default(),
            "a",
            ", if you want",
            fit(true),
        );
        assert_eq!(
            outcome,
            InsertOutcome::Applied {
                verified: true,
                text: ", if you want".into()
            }
        );
        assert_eq!(field.text(), "It works, if you want");

        let field = Fake::at_end("Hello");
        run(&field, &mut History::default(), "a", "world", fit(false));
        assert_eq!(field.text(), "Hello world");
    }

    #[test]
    fn a_join_that_cannot_select_the_period_goes_in_as_its_own_sentence() {
        let field = Fake {
            selectable: false,
            ..Fake::at_end("It works.")
        };
        let outcome = run(
            &field,
            &mut History::default(),
            "a",
            ", if you want",
            fit(true),
        );
        assert_eq!(
            outcome,
            InsertOutcome::Applied {
                verified: true,
                text: " If you want".into()
            }
        );
        assert_eq!(field.text(), "It works. If you want");
    }

    #[test]
    fn fitting_reads_nothing_where_the_field_may_not_be_read() {
        let field = Fake {
            deny_reads: true,
            direct: Direct::Delays,
            ..Fake::at_end("Hello")
        };
        insert(
            &field,
            &mut History::default(),
            "a",
            "world",
            &fit(true),
            false,
            &|| true,
        );
        assert_eq!(field.text(), "Helloworld");
    }

    #[test]
    fn restoring_raw_text_replaces_exactly_what_was_typed_while_it_is_still_there() {
        let field = Fake::at_end("It works.");
        let mut history = History::default();
        run(&field, &mut history, "a", ", if you want it", fit(true));
        let restore = InsertMode::Restore {
            edit_id: "a".into(),
        };
        let outcome = run(
            &field,
            &mut history,
            "a:raw",
            ", um, if you want it",
            restore.clone(),
        );
        assert!(matches!(
            outcome,
            InsertOutcome::Applied { verified: true, .. }
        ));
        assert_eq!(field.text(), "It works. Um, if you want it");

        let typed_after = Fake::at_end("It works.");
        let mut history = History::default();
        run(&typed_after, &mut history, "a", " Next", caret());
        typed_after.type_over_selection("!");
        assert_eq!(
            run(&typed_after, &mut history, "a:raw", "next", restore),
            InsertOutcome::Unchanged {
                reason: InsertMiss::ContentChanged
            }
        );
        assert_eq!(typed_after.text(), "It works. Next!");
    }

    #[test]
    fn an_unreadable_field_trusts_only_the_platform_and_pastes_only_after_a_refusal() {
        let field = Fake {
            readable: false,
            ..Fake::at_end("Hello")
        };
        assert_eq!(
            run(&field, &mut History::default(), "a", "x", caret()),
            InsertOutcome::Applied {
                verified: false,
                text: "x".into()
            }
        );
        assert_eq!(field.writes.get(), 1);

        let refused = Fake {
            readable: false,
            direct: Direct::Rejects,
            paster: Paster::NeverReads,
            ..Fake::at_end("Hello")
        };
        assert_eq!(
            run(&refused, &mut History::default(), "a", "x", caret()),
            InsertOutcome::Unchanged {
                reason: InsertMiss::Refused
            }
        );
    }

    #[test]
    fn typed_text_waits_for_held_keys_then_needs_its_dictation_still_live() {
        // A field that only takes typed keys (Windows): no read-back and no paste.
        struct Keys<'a> {
            typed: RefCell<String>,
            released: &'a dyn Fn() -> bool,
            partial: bool,
        }
        impl FieldPort for Keys<'_> {
            fn focused(&self) -> bool {
                true
            }
            fn read(&self) -> Option<FieldRead> {
                None
            }
            fn select(&self, _span: Span) -> bool {
                false
            }
            fn write(&self, text: &str) -> Write {
                if self.partial {
                    self.typed.borrow_mut().push_str(&text[..1]);
                    return Write::Partial;
                }
                self.typed.borrow_mut().push_str(text);
                Write::Accepted
            }
            fn paste(&self, _text: &str) -> Paste {
                Paste::NotSent
            }
            fn settle(&self) {}
            fn keys_released(&self) -> bool {
                (self.released)()
            }
        }
        let type_into = |released: &dyn Fn() -> bool, partial: bool, live: &dyn Fn() -> bool| {
            let field = Keys {
                typed: RefCell::default(),
                released,
                partial,
            };
            let outcome = insert(
                &field,
                &mut History::default(),
                "a",
                "Hello there",
                &fit(true),
                false,
                live,
            );
            (outcome, field.typed.into_inner())
        };

        assert_eq!(
            type_into(&|| true, false, &|| true),
            (
                InsertOutcome::Applied {
                    verified: false,
                    text: "Hello there".into()
                },
                "Hello there".into()
            )
        );
        // Cancelled while the shortcut's keys were still held: nothing is typed.
        let live = Cell::new(true);
        let cancel_meanwhile = || {
            live.set(false);
            true
        };
        assert_eq!(
            type_into(&cancel_meanwhile, false, &|| live.get()),
            (unchanged(InsertMiss::Retired), String::new())
        );
        assert_eq!(
            type_into(&|| false, false, &|| true),
            (unchanged(InsertMiss::Refused), String::new())
        );
        // Part of the keys went through: never reported as applied, never typed again.
        assert_eq!(
            type_into(&|| true, true, &|| true),
            (InsertOutcome::Uncertain, "H".into())
        );
    }

    #[test]
    fn a_chromium_no_break_space_still_reads_as_applied() {
        struct Nbsp(Fake);
        impl FieldPort for Nbsp {
            fn focused(&self) -> bool {
                self.0.focused()
            }
            fn read(&self) -> Option<FieldRead> {
                self.0.read()
            }
            fn select(&self, span: Span) -> bool {
                self.0.select(span)
            }
            fn write(&self, text: &str) -> Write {
                self.0.type_over_selection(&text.replace(' ', "\u{a0}"));
                Write::Accepted
            }
            fn paste(&self, text: &str) -> Paste {
                self.0.paste(text)
            }
            fn settle(&self) {}
        }
        let field = Nbsp(Fake::at_end("Hello"));
        assert!(matches!(
            run_port(&field, " world"),
            InsertOutcome::Applied { verified: true, .. }
        ));
        assert_eq!(field.0.writes.get(), 1);
    }

    fn run_port(port: &impl FieldPort, text: &str) -> InsertOutcome {
        insert(
            port,
            &mut History::default(),
            "a",
            text,
            &InsertMode::Caret,
            true,
            &|| true,
        )
    }

    #[test]
    fn the_start_of_an_applied_edit_is_kept_for_learning() {
        let field = Fake::at_end("Dear Mina, ");
        let mut history = History::default();
        run(&field, &mut history, "a", "thanks", caret());
        assert_eq!(history.start_of("a").unwrap().before, "Dear Mina, ");
        assert!(history.start_of("missing").is_none());
    }

    #[test]
    fn rejects_empty_oversized_and_nul_text() {
        assert!(validate("Hello").is_ok());
        assert!(validate("").is_err());
        assert!(validate("a\0b").is_err());
        assert!(validate(&"a".repeat(100_001)).is_err());
    }
}
