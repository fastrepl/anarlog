mod builder;
mod error;
mod loader;
mod manager;

pub use builder::ModelManagerBuilder;
pub use error::Error;
pub use loader::ModelLoader;
pub use manager::ModelManager;

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc, time::Duration};

    use super::*;

    struct MockModel;

    #[derive(Debug, thiserror::Error)]
    #[error("mock error")]
    struct MockError;

    impl ModelLoader for MockModel {
        type Error = MockError;

        fn load(_path: &Path) -> Result<Self, Self::Error> {
            Ok(MockModel)
        }
    }

    fn temp_model_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("model-manager-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"").unwrap();
        path
    }

    fn build_manager(
        timeout: Duration,
        check_interval: Duration,
        models: &[(&str, std::path::PathBuf)],
    ) -> ModelManager<MockModel> {
        let mut builder = ModelManager::<MockModel>::builder()
            .inactivity_timeout(timeout)
            .check_interval(check_interval);
        for (name, path) in models {
            builder = builder.register(*name, path.clone());
        }
        builder.build()
    }

    #[tokio::test(start_paused = true)]
    async fn idle_model_gets_evicted() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();
        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m2));

        tokio::time::advance(Duration::from_millis(120)).await;
        tokio::task::yield_now().await;

        let m3 = mgr.get(Some("a")).await.unwrap();
        assert!(!Arc::ptr_eq(&m1, &m3));
    }

    #[tokio::test(start_paused = true)]
    async fn activity_prevents_eviction() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        for _ in 0..5 {
            tokio::time::advance(Duration::from_millis(50)).await;
            tokio::task::yield_now().await;

            let m = mgr.get(Some("a")).await.unwrap();
            assert!(Arc::ptr_eq(&m1, &m));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn access_near_timeout_resets_timer() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_millis(10),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        tokio::time::advance(Duration::from_millis(90)).await;
        tokio::task::yield_now().await;

        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m2));

        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        let m3 = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m3));
    }

    struct GatedModel;

    static GATE: std::sync::OnceLock<(
        std::sync::Arc<std::sync::Barrier>,
        std::sync::Arc<std::sync::Barrier>,
    )> = std::sync::OnceLock::new();
    static GATED_LOADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn gate() -> &'static (
        std::sync::Arc<std::sync::Barrier>,
        std::sync::Arc<std::sync::Barrier>,
    ) {
        GATE.get_or_init(|| {
            (
                std::sync::Arc::new(std::sync::Barrier::new(2)),
                std::sync::Arc::new(std::sync::Barrier::new(2)),
            )
        })
    }

    impl ModelLoader for GatedModel {
        type Error = MockError;

        fn load(_path: &Path) -> Result<Self, Self::Error> {
            if GATED_LOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                let (enter, exit) = gate();
                enter.wait();
                exit.wait();
            }
            Ok(GatedModel)
        }
    }

    // Regression for the lock inversion between get() and the inactivity
    // monitor: get() held `active` then `last_activity` while the monitor
    // acquired them in the opposite order. This drives both paths into the
    // former deadlock interleaving with barriers instead of timing sleeps.
    #[tokio::test(start_paused = true)]
    async fn concurrent_get_and_monitor_eviction_do_not_deadlock() {
        let path = temp_model_path();
        let mgr = ModelManager::<GatedModel>::builder()
            .inactivity_timeout(Duration::from_millis(100))
            .check_interval(Duration::from_millis(100))
            .register("a", path)
            .build();

        let (enter, exit) = gate();

        let mgr1 = mgr.clone();
        let g1 = tokio::spawn(async move { mgr1.get(Some("a")).await });

        // Wait until the first load is running, i.e. get() holds its lock(s).
        let enter = std::sync::Arc::clone(enter);
        tokio::task::spawn_blocking(move || enter.wait())
            .await
            .unwrap();

        // Fire a monitor tick with the eviction condition satisfied so it
        // queues on the manager state while get() still holds it.
        tokio::time::advance(Duration::from_millis(150)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        let mgr2 = mgr.clone();
        let g2 = tokio::spawn(async move { mgr2.get(Some("a")).await });
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        let exit = std::sync::Arc::clone(exit);
        tokio::task::spawn_blocking(move || exit.wait())
            .await
            .unwrap();

        // Under the old two-lock layout this interleaving deadlocked; the
        // paused clock auto-advances into the timeout instead of hanging.
        let joined = tokio::time::timeout(Duration::from_secs(300), async {
            g1.await.unwrap().unwrap();
            g2.await.unwrap().unwrap();
        })
        .await;

        assert!(
            joined.is_ok(),
            "get() and the inactivity monitor deadlocked"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn access_after_timeout_before_monitor_tick_reloads() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_secs(60),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        tokio::time::advance(Duration::from_millis(120)).await;
        tokio::task::yield_now().await;

        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(!Arc::ptr_eq(&m1, &m2));
    }
}
