use std::collections::HashMap;

use tokio_util::sync::CancellationToken;

struct TimerEntry {
    generation: u64,
    token: CancellationToken,
}

#[derive(Default)]
pub(crate) struct TimerRegistry {
    timers: HashMap<String, TimerEntry>,
    next_gen: u64,
}

impl Drop for TimerRegistry {
    fn drop(&mut self) {
        for (_, entry) in self.timers.drain() {
            entry.token.cancel();
        }
    }
}

impl TimerRegistry {
    pub fn contains(&self, key: &str) -> bool {
        self.timers.contains_key(key)
    }

    pub fn start_replace(&mut self, key: String, token: CancellationToken) -> u64 {
        let generation = self.next_gen;
        self.next_gen += 1;

        if let Some(old) = self.timers.insert(key, TimerEntry { generation, token }) {
            old.token.cancel();
        }

        generation
    }

    pub fn start_if_absent(&mut self, key: String, token: CancellationToken) -> Option<u64> {
        if self.timers.contains_key(&key) {
            return None;
        }

        Some(self.start_replace(key, token))
    }

    pub fn cancel(&mut self, key: &str) -> bool {
        if let Some(entry) = self.timers.remove(key) {
            entry.token.cancel();
            true
        } else {
            false
        }
    }

    pub fn claim(&mut self, key: &str, generation: u64) -> bool {
        match self.timers.get(key) {
            Some(entry) if entry.generation == generation => {
                self.timers.remove(key);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_replace_cancels_old_token_and_increments_generation() {
        let mut registry = TimerRegistry::default();
        let token0 = CancellationToken::new();
        let token0_clone = token0.clone();

        let generation_0 = registry.start_replace("app.x".to_string(), token0);
        let generation_1 = registry.start_replace("app.x".to_string(), CancellationToken::new());

        assert!(token0_clone.is_cancelled());
        assert_eq!(generation_0, 0);
        assert_eq!(generation_1, 1);
    }

    #[test]
    fn start_if_absent_does_not_replace_existing_timer() {
        let mut registry = TimerRegistry::default();
        let token0 = CancellationToken::new();
        let token0_clone = token0.clone();

        let generation_0 = registry.start_if_absent("app.x".to_string(), token0);
        let generation_1 = registry.start_if_absent("app.x".to_string(), CancellationToken::new());

        assert_eq!(generation_0, Some(0));
        assert_eq!(generation_1, None);
        assert!(!token0_clone.is_cancelled());
    }

    #[test]
    fn cancel_removes_entry_and_cancels_token() {
        let mut registry = TimerRegistry::default();
        let token = CancellationToken::new();
        let token_clone = token.clone();

        registry.start_replace("app.x".to_string(), token);

        assert!(registry.cancel("app.x"));
        assert!(token_clone.is_cancelled());
        assert!(!registry.contains("app.x"));
    }

    #[test]
    fn claim_only_succeeds_for_matching_generation() {
        let mut registry = TimerRegistry::default();

        let generation_0 = registry.start_replace("app.x".to_string(), CancellationToken::new());
        let generation_1 = registry.start_replace("app.x".to_string(), CancellationToken::new());

        assert!(!registry.claim("app.x", generation_0));
        assert!(registry.claim("app.x", generation_1));
        assert!(!registry.contains("app.x"));
    }

    #[test]
    fn drop_cancels_all_tokens() {
        let token0 = CancellationToken::new();
        let token1 = CancellationToken::new();
        let token0_clone = token0.clone();
        let token1_clone = token1.clone();

        {
            let mut registry = TimerRegistry::default();
            registry.start_replace("app.a".to_string(), token0);
            registry.start_replace("app.b".to_string(), token1);
        }

        assert!(token0_clone.is_cancelled());
        assert!(token1_clone.is_cancelled());
    }
}
