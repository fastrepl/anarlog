use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::OwnedMutexGuard;

/// Serializes filesystem mutations of one session's audio across webview
/// commands and Rust-owned background work.
#[derive(Default)]
pub struct SessionAudioLocks(Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>);

impl SessionAudioLocks {
    pub async fn lock(&self, session_id: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            locks.entry(session_id.to_string()).or_default().clone()
        };
        lock.lock_owned().await
    }

    pub async fn lock_pair(
        &self,
        first: &str,
        second: &str,
    ) -> (OwnedMutexGuard<()>, OwnedMutexGuard<()>) {
        if first <= second {
            let a = self.lock(first).await;
            (a, self.lock(second).await)
        } else {
            let b = self.lock(second).await;
            (self.lock(first).await, b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn session_audio_lock_blocks_the_same_session_only() {
        let locks = Arc::new(SessionAudioLocks::default());
        let held = locks.lock("a").await;

        let other = tokio::time::timeout(Duration::from_millis(50), locks.lock("b")).await;
        assert!(other.is_ok());

        let waiter = tokio::spawn({
            let locks = locks.clone();
            async move { locks.lock("a").await }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());

        drop(held);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
    }
}
