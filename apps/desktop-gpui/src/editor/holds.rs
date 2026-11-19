//! Notes whose editors take no edits for now. A deletion holds its notes while their drafts
//! save, so no newer edit can miss the tombstone, which rejects later saves. Every editor of
//! a held note is read-only, including one opened during the hold.

use std::collections::HashMap;

use desktop_runtime::SessionId;
use gpui::{App, Global};

/// Each note's active holds; a note stays held until all of them are released.
#[derive(Default)]
pub struct NoteHolds(HashMap<SessionId, usize>);

impl Global for NoteHolds {}

impl NoteHolds {
    pub fn held(cx: &App, id: &SessionId) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|holds| holds.0.contains_key(id))
    }

    /// Holds `ids` until the returned hold is released; overlapping holds don't end each other.
    pub fn hold(ids: Vec<SessionId>, cx: &mut App) -> NoteHold {
        let holds = &mut cx.default_global::<Self>().0;
        for id in &ids {
            *holds.entry(id.clone()).or_default() += 1;
        }
        NoteHold(ids)
    }
}

#[must_use = "a dropped hold keeps its notes read-only"]
pub struct NoteHold(Vec<SessionId>);

impl NoteHold {
    pub fn release(self, cx: &mut App) {
        let holds = &mut cx.default_global::<NoteHolds>().0;
        for id in self.0 {
            if let Some(count) = holds.get_mut(&id) {
                *count -= 1;
                if *count == 0 {
                    holds.remove(&id);
                }
            }
        }
    }
}
