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

    #[cfg(test)]
    pub fn targets_for(&self, id: WatchId) -> Option<HashSet<DependencyTarget>> {
        self.forward.get(&id).cloned()
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
    fn register_and_affected() {
        let mut deps = DependencyWatchIndex::default();

        let w1 = deps.register(HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("words".into()),
        ]));
        let w2 = deps.register(HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("chat_messages".into()),
        ]));

        let affected = deps.affected(&HashSet::from([DependencyTarget::Table("words".into())]));
        assert!(affected.contains(&w1));
        assert!(!affected.contains(&w2));

        let affected = deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())]));
        assert!(affected.contains(&w1));
        assert!(affected.contains(&w2));
    }

    #[test]
    fn unregister_removes_from_index() {
        let mut deps = DependencyWatchIndex::default();

        let w1 = deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));
        let w2 = deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));

        deps.unregister(w1);

        let affected = deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())]));
        assert!(!affected.contains(&w1));
        assert!(affected.contains(&w2));
    }

    #[test]
    fn empty_changed_tables() {
        let mut deps = DependencyWatchIndex::default();
        deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));

        let affected = deps.affected(&HashSet::new());
        assert!(affected.is_empty());
    }

    #[test]
    fn unregister_nonexistent_is_noop() {
        let mut deps = DependencyWatchIndex::default();
        deps.unregister(WatchId(999));
    }

    #[test]
    fn register_empty_tables_never_matches() {
        let mut deps = DependencyWatchIndex::default();
        let watch = deps.register(HashSet::new());

        assert!(
            deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())]))
                .is_empty()
        );

        deps.unregister(watch);
        assert!(
            deps.affected(&HashSet::from([DependencyTarget::Table("sessions".into())]))
                .is_empty()
        );
    }

    #[test]
    fn duplicate_changed_tables_are_deduped() {
        let mut deps = DependencyWatchIndex::default();
        let watch = deps.register(HashSet::from([DependencyTarget::Table("sessions".into())]));

        let affected = deps.affected(&HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("sessions".into()),
        ]));
        assert_eq!(affected.len(), 1);
        assert!(affected.contains(&watch));
    }

    #[test]
    fn tables_for_returns_registered_tables() {
        let mut deps = DependencyWatchIndex::default();
        let watch = deps.register(HashSet::from([
            DependencyTarget::Table("sessions".into()),
            DependencyTarget::Table("words".into()),
        ]));

        let targets = deps.targets_for(watch).unwrap();
        assert_eq!(
            targets,
            HashSet::from([
                DependencyTarget::Table("sessions".into()),
                DependencyTarget::Table("words".into()),
            ])
        );
    }
}
