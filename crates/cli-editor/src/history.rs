use std::collections::VecDeque;

const MAX_HISTORY: usize = 100;

pub(crate) struct Snapshot {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionKind {
    CharInsert,
    #[allow(dead_code)]
    Other,
}

pub(crate) struct History {
    undo_stack: VecDeque<Snapshot>,
    redo_stack: VecDeque<Snapshot>,
    last_action: Option<ActionKind>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            last_action: None,
        }
    }

    pub fn push(&mut self, snapshot: Snapshot) {
        self.last_action = None;
        self.push_inner(snapshot);
    }

    pub fn push_coalesced(&mut self, snapshot: Snapshot, kind: ActionKind) {
        if kind == ActionKind::CharInsert && self.last_action == Some(ActionKind::CharInsert) {
            self.redo_stack.clear();
            return;
        }
        self.last_action = Some(kind);
        self.push_inner(snapshot);
    }

    fn push_inner(&mut self, snapshot: Snapshot) {
        if self.undo_stack.len() >= MAX_HISTORY {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(snapshot);
        self.redo_stack.clear();
    }

    pub fn break_coalescing(&mut self) {
        self.last_action = None;
    }

    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let prev = self.undo_stack.pop_back()?;
        self.redo_stack.push_back(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo_stack.pop_back()?;
        self.undo_stack.push_back(current);
        Some(next)
    }
}
