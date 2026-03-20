#![forbid(unsafe_code)]

pub mod explain;

use std::collections::{HashMap, HashSet};

pub use explain::extract_tables;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchId(u64);

pub struct TableDeps {
    next_id: u64,
    forward: HashMap<WatchId, HashSet<String>>,
    reverse: HashMap<String, HashSet<WatchId>>,
}

impl TableDeps {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            forward: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub fn register(&mut self, tables: HashSet<String>) -> WatchId {
        debug_assert!(
            !tables.is_empty(),
            "registering a watch with no table dependencies — the callback will never fire"
        );
        let id = WatchId(self.next_id);
        self.next_id += 1;

        for table in &tables {
            self.reverse.entry(table.clone()).or_default().insert(id);
        }
        self.forward.insert(id, tables);
        id
    }

    pub fn unregister(&mut self, id: WatchId) {
        if let Some(tables) = self.forward.remove(&id) {
            for table in &tables {
                if let Some(set) = self.reverse.get_mut(table) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.reverse.remove(table);
                    }
                }
            }
        }
    }

    pub fn affected(&self, changed_tables: &[&str]) -> HashSet<WatchId> {
        let mut result = HashSet::new();
        for table in changed_tables {
            if let Some(ids) = self.reverse.get(*table) {
                result.extend(ids);
            }
        }
        result
    }
}

impl Default for TableDeps {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_affected() {
        let mut deps = TableDeps::new();

        let w1 = deps.register(HashSet::from(["sessions".into(), "words".into()]));
        let w2 = deps.register(HashSet::from(["sessions".into(), "chat_messages".into()]));

        let affected = deps.affected(&["words"]);
        assert!(affected.contains(&w1));
        assert!(!affected.contains(&w2));

        let affected = deps.affected(&["sessions"]);
        assert!(affected.contains(&w1));
        assert!(affected.contains(&w2));
    }

    #[test]
    fn unregister_removes_from_index() {
        let mut deps = TableDeps::new();

        let w1 = deps.register(HashSet::from(["sessions".into()]));
        let w2 = deps.register(HashSet::from(["sessions".into()]));

        deps.unregister(w1);

        let affected = deps.affected(&["sessions"]);
        assert!(!affected.contains(&w1));
        assert!(affected.contains(&w2));
    }

    #[test]
    fn empty_changed_tables() {
        let mut deps = TableDeps::new();
        deps.register(HashSet::from(["sessions".into()]));

        let affected = deps.affected(&[]);
        assert!(affected.is_empty());
    }

    #[test]
    fn unregister_nonexistent_is_noop() {
        let mut deps = TableDeps::new();
        deps.unregister(WatchId(999));
    }
}
