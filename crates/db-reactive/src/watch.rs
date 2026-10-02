use std::collections::{HashMap, HashSet};

use crate::DependencyTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchId(u64);

#[derive(Default)]
pub struct DependencyWatchIndex {
    next_id: u64,
    forward: HashMap<WatchId, HashSet<DependencyTarget>>,
    reverse: HashMap<DependencyTarget, HashSet<WatchId>>,
}

impl DependencyWatchIndex {
    pub fn register(&mut self, targets: HashSet<DependencyTarget>) -> WatchId {
        let id = WatchId(self.next_id);
        self.next_id += 1;

        for target in &targets {
            self.reverse.entry(target.clone()).or_default().insert(id);
        }
        self.forward.insert(id, targets);
        id
    }

    pub fn unregister(&mut self, id: WatchId) {
        if let Some(targets) = self.forward.remove(&id) {
            for target in &targets {
                if let Some(set) = self.reverse.get_mut(target) {
                    set.remove(&id);
                    if set.is_empty() {
                        self.reverse.remove(target);
                    }
                }
            }
        }
    }

    pub fn affected(&self, changed_targets: &HashSet<DependencyTarget>) -> HashSet<WatchId> {
        let mut result = HashSet::new();
        for target in changed_targets {
            if let Some(ids) = self.reverse.get(target) {
                result.extend(ids);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affected_returns_watches_depending_on_changed_targets() {
        let mut deps = DependencyWatchIndex::default();

        let w1 = deps.register(HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("words".into()),
        ]));
        let w2 = deps.register(HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("chat_messages".into()),
        ]));
        let empty_watch = deps.register(HashSet::new());

        let affected = deps.affected(&HashSet::from([
            DependencyTarget::Table("words".into()),
            DependencyTarget::Table("sessions".into()),
        ]));
        assert!(affected.contains(&w1));
        assert!(affected.contains(&w2));
        assert!(!affected.contains(&empty_watch));

        let affected = deps.affected(&HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("sessions".into()),
        ]));
        assert_eq!(affected.len(), 2);

        assert!(deps.affected(&HashSet::new()).is_empty());
    }

    #[test]
    fn unregister_removes_only_that_watch() {
        let mut deps = DependencyWatchIndex::default();
        let w1 = deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));
        let w2 = deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));

        deps.unregister(WatchId(999));
        assert_eq!(
            deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())])),
            HashSet::from([w1, w2])
        );

        deps.unregister(w1);
        let affected = deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())]));
        assert!(!affected.contains(&w1));
        assert!(affected.contains(&w2));
    }
}
