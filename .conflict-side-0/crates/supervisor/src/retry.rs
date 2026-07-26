use ractor::{ActorCell, SpawnErr};
use std::future::Future;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
        }
    }
}

/// Spawn an actor with exponential-backoff retries.
/// Returns `Some(cell)` on first success, `None` if all attempts fail.
pub async fn spawn_with_retry<F, Fut>(strategy: &RetryStrategy, spawn_fn: F) -> Option<ActorCell>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<ActorCell, SpawnErr>>,
{
    for attempt in 0..strategy.max_attempts {
        let delay = strategy.base_delay * 2u32.pow(attempt);
        tokio::time::sleep(delay).await;

        match spawn_fn().await {
            Ok(cell) => {
                tracing::info!(attempt, "spawn_retry_succeeded");
                return Some(cell);
            }
            Err(e) => {
                tracing::warn!(attempt, error = ?e, "spawn_retry_failed");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ractor::{Actor, ActorProcessingErr, ActorRef};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn unique_name(prefix: &str) -> String {
        format!(
            "{prefix}_{}_{}",
            std::process::id(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        )
    }

    struct DummyActor;

    #[ractor::async_trait]
    impl Actor for DummyActor {
        type Msg = ();
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }
    }

    async fn spawn_name_collision_err(name: &str) -> SpawnErr {
        Actor::spawn(Some(name.to_string()), DummyActor, ())
            .await
            .expect_err("expected name collision to produce SpawnErr")
    }

    #[tokio::test]
    async fn zero_attempts_never_calls_spawn() {
        let strategy = RetryStrategy {
            max_attempts: 0,
            base_delay: Duration::from_millis(5),
        };
        let calls = Arc::new(AtomicU64::new(0));
        let calls2 = calls.clone();

        let result = spawn_with_retry(&strategy, move || {
            calls2.fetch_add(1, Ordering::SeqCst);
            async {
                panic!("spawn closure must not be called when max_attempts is zero");
                #[allow(unreachable_code)]
                Err(spawn_name_collision_err("unused").await)
            }
        })
        .await;

        assert!(result.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn retries_until_success() {
        let keep_name = unique_name("retry_keep");
        let (keep_ref, keep_handle) = Actor::spawn(Some(keep_name.clone()), DummyActor, ())
            .await
            .expect("failed to spawn keeper actor");

        let success_name = unique_name("retry_success");
        let attempts = Arc::new(AtomicU64::new(0));
        let attempts2 = attempts.clone();

        let strategy = RetryStrategy {
            max_attempts: 4,
            base_delay: Duration::from_millis(10),
        };

        let start = Instant::now();
        let result = spawn_with_retry(&strategy, move || {
            let keep_name = keep_name.clone();
            let success_name = success_name.clone();
            let attempts = attempts2.clone();
            async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    Err(spawn_name_collision_err(&keep_name).await)
                } else {
                    let (ok_ref, _ok_handle) =
                        Actor::spawn(Some(success_name), DummyActor, ()).await?;
                    Ok(ok_ref.get_cell())
                }
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert!(result.is_some());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert!(
            elapsed >= Duration::from_millis(60),
            "expected backoff delays before success, got {elapsed:?}"
        );

        if let Some(cell) = result {
            cell.kill();
        }
        keep_ref.stop(None);
        let _ = keep_handle.await;
    }

    #[tokio::test]
    async fn returns_none_after_exhausting_attempts() {
        let keep_name = unique_name("retry_keep_fail");
        let (keep_ref, keep_handle) = Actor::spawn(Some(keep_name.clone()), DummyActor, ())
            .await
            .expect("failed to spawn keeper actor");

        let attempts = Arc::new(AtomicU64::new(0));
        let attempts2 = attempts.clone();
        let strategy = RetryStrategy {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
        };

        let result = spawn_with_retry(&strategy, move || {
            let keep_name = keep_name.clone();
            let attempts = attempts2.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(spawn_name_collision_err(&keep_name).await)
            }
        })
        .await;

        assert!(result.is_none());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);

        keep_ref.stop(None);
        let _ = keep_handle.await;
    }
}
