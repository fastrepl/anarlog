//! Locked notes open to meeting work. Shipping lets capture, transcript, chat and participant
//! changes through once a locked note is revealed, so the app lock (`application::lock`) opens
//! revealed notes here; the runtime's SQL keeps refusing every other locked note.

use std::{
    collections::BTreeSet,
    sync::{Mutex, PoisonError},
};

use serde_json::Value;

static OPEN: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

pub fn open(session: &str) {
    OPEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(session.to_owned());
}

pub fn close(session: &str) {
    OPEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(session);
}

/// The open notes, bound to a `locked = 0 OR id IN (SELECT value FROM json_each(?))` predicate.
pub fn param() -> Value {
    let open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    Value::String(serde_json::to_string(&*open).unwrap_or_else(|_| "[]".into()))
}
